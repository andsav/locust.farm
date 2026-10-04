// macOS helper for verify-store.patch. Unlike the original reviewer's LRC_*
// helper, this accepts the verifier's PROBE_* variables and arm files.
// PROBE_TRACE=1 traces flush/rename/unlink calls.
// PROBE_{FAIL,KILL}_SUB selects a path substring; a leading $ selects a suffix.
// PROBE_{FAIL,KILL}_ARM names the file whose existence arms that fault.
// A failure returns EIO without flushing; a kill exits 77 without flushing.
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/param.h>
#include <unistd.h>

#define INTERPOSE(replacement, replacee)                                      \
    __attribute__((used)) static struct {                                     \
        const void *new_function;                                             \
        const void *old_function;                                             \
    } interpose_##replacee __attribute__((section("__DATA,__interpose"))) = {   \
        (const void *)(unsigned long)&replacement,                             \
        (const void *)(unsigned long)&replacee};

static int tracing(void) {
    const char *value = getenv("PROBE_TRACE");
    return value && strcmp(value, "1") == 0;
}

static int matches(const char *path, const char *pattern) {
    if (!pattern || !*pattern) return 0;
    if (*pattern != '$') return strstr(path, pattern) != NULL;
    ++pattern;
    size_t a = strlen(path), b = strlen(pattern);
    return b && a >= b && strcmp(path + a - b, pattern) == 0;
}

static int armed(const char *name) {
    const char *path = getenv(name);
    return path && access(path, F_OK) == 0;
}

static void path_of(int fd, char *path) {
    strcpy(path, "?");
    fcntl(fd, F_GETPATH, path);
}

static void mark_fired(const char *name) {
    const char *arm = getenv(name);
    char path[MAXPATHLEN];
    int size = snprintf(path, sizeof(path), "%s.fired", arm);
    if (size < 0 || (size_t)size >= sizeof(path)) _exit(78);
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (fd < 0 || write(fd, "1", 1) != 1 || close(fd) != 0) _exit(78);
}

static int inject(const char *operation, const char *path) {
    if (armed("PROBE_KILL_ARM") && matches(path, getenv("PROBE_KILL_SUB"))) {
        mark_fired("PROBE_KILL_ARM");
        fprintf(stderr, "[verify-store] injected exit 77 before %s %s\n", operation, path);
        _exit(77);
    }
    if (armed("PROBE_FAIL_ARM") && matches(path, getenv("PROBE_FAIL_SUB"))) {
        mark_fired("PROBE_FAIL_ARM");
        fprintf(stderr, "[verify-store] injected EIO before %s %s\n", operation, path);
        errno = EIO;
        return 1;
    }
    return 0;
}

static int verify_fcntl(int fd, int cmd, ...) {
    if (cmd == F_FULLFSYNC) {
        char path[MAXPATHLEN];
        path_of(fd, path);
        if (inject("F_FULLFSYNC", path)) return -1;
        int result = fcntl(fd, cmd);
        if (tracing()) fprintf(stderr, "[verify-store] F_FULLFSYNC %s -> %d\n", path, result);
        return result;
    }
    // These commands have no third argument. Do not consume absent varargs.
    if (cmd == F_GETFD || cmd == F_GETFL || cmd == F_GETOWN) return fcntl(fd, cmd);
    va_list ap;
    va_start(ap, cmd);
    int result;
    switch (cmd) {
        case F_DUPFD:
        case F_DUPFD_CLOEXEC:
        case F_SETFD:
        case F_SETFL:
        case F_SETOWN:
        case F_NOCACHE:
        case F_RDAHEAD:
        case F_SETNOSIGPIPE:
            result = fcntl(fd, cmd, va_arg(ap, int));
            break;
        default:
            // SQLite's lock operations and F_GETPATH take pointer arguments.
            result = fcntl(fd, cmd, va_arg(ap, void *));
            break;
    }
    va_end(ap);
    return result;
}
INTERPOSE(verify_fcntl, fcntl)

static int verify_fsync(int fd) {
    char path[MAXPATHLEN];
    path_of(fd, path);
    if (inject("fsync", path)) return -1;
    int result = fsync(fd);
    if (tracing()) fprintf(stderr, "[verify-store] fsync %s -> %d\n", path, result);
    return result;
}
INTERPOSE(verify_fsync, fsync)

static int verify_rename(const char *from, const char *to) {
    int result = rename(from, to);
    if (tracing()) fprintf(stderr, "[verify-store] rename %s -> %s = %d\n", from, to, result);
    return result;
}
INTERPOSE(verify_rename, rename)

static int verify_unlink(const char *path) {
    int result = unlink(path);
    if (tracing()) fprintf(stderr, "[verify-store] unlink %s = %d\n", path, result);
    return result;
}
INTERPOSE(verify_unlink, unlink)
