// Interposer for probes: traces F_FULLFSYNC, fsync, rename and unlink with
// the path they act on, and injects a failure or a process exit on a flush.
//
// LRC_TRACE=1        print each traced call to stderr
// LRC_FAIL=<suffix>  F_FULLFSYNC on a path ending in <suffix> fails with EIO
//                    without flushing
// LRC_DIE=<suffix>   F_FULLFSYNC on a path ending in <suffix> exits the
//                    process (status 77) without flushing, as a SIGKILL
//                    between the write and the flush would
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/param.h>
#include <unistd.h>

#define DYLD_INTERPOSE(_replacement, _replacee)                               \
    __attribute__((used)) static struct {                                    \
        const void *replacement;                                             \
        const void *replacee;                                                \
    } _interpose_##_replacee __attribute__((section("__DATA,__interpose"))) = \
        {(const void *)(unsigned long)&_replacement,                         \
         (const void *)(unsigned long)&_replacee};

static int ends_with(const char *s, const char *suffix) {
    size_t a = strlen(s), b = strlen(suffix);
    return b > 0 && a >= b && strcmp(s + a - b, suffix) == 0;
}

static int tracing(void) {
    const char *t = getenv("LRC_TRACE");
    return t && t[0] == '1';
}

static void path_of(int fd, char *buf) {
    strcpy(buf, "?");
    fcntl(fd, F_GETPATH, buf);
}

int lrc_fcntl(int fd, int cmd, ...) {
    va_list ap;
    va_start(ap, cmd);
    void *arg = va_arg(ap, void *);
    va_end(ap);
    if (cmd != F_FULLFSYNC) {
        return fcntl(fd, cmd, arg);
    }
    char path[MAXPATHLEN];
    path_of(fd, path);
    const char *die = getenv("LRC_DIE");
    if (die && ends_with(path, die)) {
        fprintf(stderr, "[lrc] F_FULLFSYNC %s -> exit before flushing\n", path);
        _exit(77);
    }
    const char *fail = getenv("LRC_FAIL");
    if (fail && ends_with(path, fail)) {
        fprintf(stderr, "[lrc] F_FULLFSYNC %s -> injected EIO, not flushed\n", path);
        errno = EIO;
        return -1;
    }
    int rc = fcntl(fd, cmd, arg);
    if (tracing()) {
        fprintf(stderr, "[lrc] F_FULLFSYNC %s -> %d\n", path, rc);
    }
    return rc;
}
DYLD_INTERPOSE(lrc_fcntl, fcntl)

int lrc_fsync(int fd) {
    char path[MAXPATHLEN];
    path_of(fd, path);
    const char *fail = getenv("LRC_FAIL");
    if (fail && ends_with(path, fail)) {
        fprintf(stderr, "[lrc] fsync %s -> injected EIO, not flushed\n", path);
        errno = EIO;
        return -1;
    }
    int rc = fsync(fd);
    if (tracing()) {
        fprintf(stderr, "[lrc] fsync %s -> %d\n", path, rc);
    }
    return rc;
}
DYLD_INTERPOSE(lrc_fsync, fsync)

int lrc_rename(const char *from, const char *to) {
    int rc = rename(from, to);
    if (tracing()) {
        fprintf(stderr, "[lrc] rename %s -> %s = %d\n", from, to, rc);
    }
    return rc;
}
DYLD_INTERPOSE(lrc_rename, rename)

int lrc_unlink(const char *path) {
    int rc = unlink(path);
    if (tracing()) {
        fprintf(stderr, "[lrc] unlink %s = %d\n", path, rc);
    }
    return rc;
}
DYLD_INTERPOSE(lrc_unlink, unlink)
