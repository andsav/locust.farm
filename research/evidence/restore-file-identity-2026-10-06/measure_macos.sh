#!/bin/bash
# measure_macos.sh — gate measurements for phase G1 of the Locust v2 plan.
#
# Records inode number and creation (birth) time of a SQLite database file in
# a directory and a marker file beside that directory, before and after
# ordinary use, copies and restores, on macOS. Works only in a fresh mktemp
# directory under /tmp (and one scratch directory on $OTHER_VOLUME for the
# cross-volume move, removed on exit). Never touches ~/.locust or any real
# Locust data.
#
# Usage: measure_macos.sh [OTHER_VOLUME]   (default: first writable volume
#        found under /Volumes that is not the system volume)
# Output: a report on stdout; redirect it to keep it.

set -euo pipefail

PY="$(command -v python3)"
SQLITE3="$(command -v sqlite3)"
SCRATCH="$(mktemp -d /tmp/locust-fileid.XXXXXX)"

OTHER_VOLUME="${1:-}"
OTHER_SCRATCH=""
if [ -z "$OTHER_VOLUME" ]; then
  for v in /Volumes/*; do
    [ -d "$v" ] || continue
    [ -L "$v" ] && continue
    if [ -w "$v" ] && ! df "$v" | tail -1 | grep -q ' /$'; then
      OTHER_VOLUME="$v"
      break
    fi
  done
fi
if [ -n "$OTHER_VOLUME" ] && [ -w "$OTHER_VOLUME" ]; then
  OTHER_SCRATCH="$OTHER_VOLUME/locust-fileid-scratch"
  mkdir -p "$OTHER_SCRATCH"
else
  OTHER_VOLUME=""
fi

cleanup() {
  [ -n "$OTHER_SCRATCH" ] && rm -rf "$OTHER_SCRATCH" 2>/dev/null || true
  rm -rf "$SCRATCH" 2>/dev/null || true
}
trap cleanup EXIT

# ident PATH -> "inode birthtime_ms mtime_ms device"
ident() {
  "$PY" - "$1" <<'PYEOF'
import os, sys
s = os.stat(sys.argv[1])
print(s.st_ino, round(s.st_birthtime * 1000), round(s.st_mtime * 1000), s.st_dev)
PYEOF
}

# rows DB -> row count of table t, or "-" if unreadable
rows() {
  "$SQLITE3" "$1" 'SELECT count(*) FROM t;' 2>/dev/null || echo "-"
}

# new_fixture BASE: BASE/data/db.sqlite (WAL, table t, 3 rows, checkpointed)
# and BASE/marker.txt beside the data directory.
new_fixture() {
  mkdir -p "$1/data"
  "$SQLITE3" "$1/data/db.sqlite" >/dev/null <<'SQL'
PRAGMA journal_mode=WAL;
CREATE TABLE t(x);
INSERT INTO t VALUES (1),(2),(3);
PRAGMA wal_checkpoint(TRUNCATE);
SQL
  printf 'marks-v1\n' > "$1/marker.txt"
}

# write_more BASE: two more rows and a truncating checkpoint, plus a rewrite
# of the marker in place, as a commit updates the marks file.
write_more() {
  "$SQLITE3" "$1/data/db.sqlite" >/dev/null <<'SQL'
INSERT INTO t VALUES (4),(5);
PRAGMA wal_checkpoint(TRUNCATE);
SQL
  printf 'marks-v2\n' >> "$1/marker.txt"
}

# snapshot BASE: prints ident lines for basedir, datadir, db, marker
snapshot() {
  local base="$1"
  echo "basedir $(ident "$base")"
  echo "datadir $(ident "$base/data")"
  echo "db $(ident "$base/data/db.sqlite") rows=$(rows "$base/data/db.sqlite")"
  echo "marker $(ident "$base/marker.txt")"
}

# fetch SNAPFILE OBJECT -> "inode birthtime_ms"
fetch() {
  grep "^$2 " "$1" | cut -d' ' -f2-3
}

# verdict CASE OBJECT BEFORE AFTER: one table line
verdict() {
  local before after bi ai bb ab iv bv
  before="$(fetch "$3" "$2")"
  after="$(fetch "$4" "$2")"
  bi="$(echo "$before" | cut -d' ' -f1)"; ai="$(echo "$after" | cut -d' ' -f1)"
  bb="$(echo "$before" | cut -d' ' -f2)"; ab="$(echo "$after" | cut -d' ' -f2)"
  [ "$bi" = "$ai" ] && iv="same" || iv="CHANGED"
  [ "$bb" = "$ab" ] && bv="same" || bv="CHANGED"
  printf '  %-8s inode:%-7s birth:%-7s | before %s %s | after %s %s\n' \
    "$2" "$iv" "$bv" "$bi" "$bb" "$ai" "$ab"
}

case_dir() {
  mkdir -p "$SCRATCH/$1"
  echo "$SCRATCH/$1"
}

echo "== environment =="
echo "date: $(date '+%Y-%m-%d %H:%M:%S %z')"
sw_vers
echo "kernel: $(uname -a)"
echo "scratch: $SCRATCH"
df "$SCRATCH" | tail -1
diskutil info / | grep 'File System Personality' || true
echo "python3: $PY ($("$PY" --version 2>&1))"
echo "sqlite3: $("$SQLITE3" --version | cut -d' ' -f1) ($SQLITE3)"
echo "rsync: $(command -v rsync) $(rsync --version | head -1)"
echo "tar: $(command -v tar) $(tar --version 2>&1 | head -1)"
echo "ditto: $(command -v ditto)"
if [ -n "$OTHER_VOLUME" ]; then
  echo "other volume: $OTHER_VOLUME ($(df "$OTHER_VOLUME" | tail -1 | awk '{print $1}'))"
  diskutil info "$OTHER_VOLUME" | grep 'File System Personality' || true
else
  echo "other volume: none writable found; cross-volume move not measured"
fi
echo "time machine: $(tmutil destinationinfo 2>&1 | head -1)"

echo
echo "== ordinary use (one fixture, sequential) =="
d="$(case_dir ordinary)"; b="$d/base"
new_fixture "$b"
snapshot "$b" > "$d/0-created"
"$SQLITE3" "$b/data/db.sqlite" 'SELECT count(*) FROM t;' >/dev/null   # process stop/start, read only
snapshot "$b" > "$d/1-after-restart-read"
write_more "$b"
snapshot "$b" > "$d/2-after-write-checkpoint"
"$SQLITE3" "$b/data/db.sqlite" 'VACUUM;' >/dev/null
snapshot "$b" > "$d/3-after-vacuum"
for pair in "stop+start, read only:0-created:1-after-restart-read" \
            "write + WAL checkpoint(TRUNCATE):1-after-restart-read:2-after-write-checkpoint" \
            "VACUUM:2-after-write-checkpoint:3-after-vacuum"; do
  IFS=: read -r label p0 p1 <<<"$pair"
  echo "case ordinary/$label"
  verdict "x" basedir "$d/$p0" "$d/$p1"
  verdict "x" datadir "$d/$p0" "$d/$p1"
  verdict "x" db      "$d/$p0" "$d/$p1"
  verdict "x" marker  "$d/$p0" "$d/$p1"
  echo "    rows: $(grep '^db ' "$d/$p0" | grep -o 'rows=[0-9-]*') -> $(grep '^db ' "$d/$p1" | grep -o 'rows=[0-9-]*')"
done

echo
echo "== copies (identity of the copy, compared with the source) =="
run_copy() { # name, copy command (executed with SRC DST)
  local name="$1"; shift
  local d b
  d="$(case_dir "copy-$name")"; b="$d/base"
  new_fixture "$b"
  write_more "$b"   # birth and mtime of db and marker now differ, as on a live installation
  sleep 1.1         # move the fixture out of its creation second
  snapshot "$b" > "$d/source"
  SRC="$b" DST="$d/copy" bash -c "$*"
  snapshot "$d/copy" > "$d/copy-id"
  echo "case copy/$name"
  verdict "x" basedir "$d/source" "$d/copy-id"
  verdict "x" datadir "$d/source" "$d/copy-id"
  verdict "x" db      "$d/source" "$d/copy-id"
  verdict "x" marker  "$d/source" "$d/copy-id"
  echo "    source mtime: db=$(grep '^db ' "$d/source" | cut -d' ' -f4) marker=$(grep '^marker ' "$d/source" | cut -d' ' -f4)"
  echo "    copy   mtime: db=$(grep '^db ' "$d/copy-id" | cut -d' ' -f4) marker=$(grep '^marker ' "$d/copy-id" | cut -d' ' -f4)"
}
run_copy "cp -R"          'cp -R "$SRC" "$DST"'
run_copy "cp -Rp"         'cp -Rp "$SRC" "$DST"'
run_copy "cp -c (clone)"  'cp -Rc "$SRC" "$DST"'
run_copy "ditto"          'ditto "$SRC" "$DST"'
run_copy "rsync -a"       'rsync -a "$SRC/" "$DST/"'
run_copy "tar pack+unpack" 'cd "$(dirname "$SRC")" && tar czf "$DST.tgz" "$(basename "$SRC")" && mkdir -p "$DST" && tar xzf "$DST.tgz" -C "$DST" && mv "$DST/$(basename "$SRC")"/* "$DST/" && rmdir "$DST/$(basename "$SRC")" && rm "$DST.tgz"'

# move within the volume
d="$(case_dir copy-mv-within)"; b="$d/base"
new_fixture "$b"
write_more "$b"
snapshot "$b" > "$d/source"
mv "$b" "$d/moved"
snapshot "$d/moved" > "$d/copy-id"
echo "case copy/mv within volume"
verdict "x" basedir "$d/source" "$d/copy-id"
verdict "x" datadir "$d/source" "$d/copy-id"
verdict "x" db      "$d/source" "$d/copy-id"
verdict "x" marker  "$d/source" "$d/copy-id"

# move to another volume and back
if [ -n "$OTHER_VOLUME" ]; then
  d="$(case_dir copy-mv-crossvol)"; b="$d/base"
  new_fixture "$b"
  write_more "$b"
  sleep 1.1
  snapshot "$b" > "$d/source"
  mv "$b" "$OTHER_SCRATCH/base"
  snapshot "$OTHER_SCRATCH/base" > "$d/there"
  mv "$OTHER_SCRATCH/base" "$d/base-back"
  snapshot "$d/base-back" > "$d/copy-id"
  echo "case copy/mv to other volume (there) [$(basename "$OTHER_VOLUME")]"
  verdict "x" basedir "$d/source" "$d/there"
  verdict "x" datadir "$d/source" "$d/there"
  verdict "x" db      "$d/source" "$d/there"
  verdict "x" marker  "$d/source" "$d/there"
  echo "case copy/mv to other volume and back"
  verdict "x" basedir "$d/source" "$d/copy-id"
  verdict "x" datadir "$d/source" "$d/copy-id"
  verdict "x" db      "$d/source" "$d/copy-id"
  verdict "x" marker  "$d/source" "$d/copy-id"
fi

echo
echo "== restores (identity at the live path after the restore, compared with what the installation last recorded) =="
# Each case: fixture created, identity recorded; an older snapshot taken;
# db written further (3 -> 5 rows); then the restore rolls the data back to
# 3 rows. Marker file beside the data directory is left untouched by the
# restore. The sleep keeps the live db's mtime outside the snapshot's second,
# because rsync's default quick check skips files equal in size and mtime.
restore_setup() {
  local d b
  d="$(case_dir "$1")"; b="$d/base"
  new_fixture "$b"
  cp "$b/data/db.sqlite" "$d/older.sqlite"
  cp -R "$b/data" "$d/older-data"
  sleep 1.1
  write_more "$b"
  snapshot "$b" > "$d/recorded"
  echo "$d"
}
report_restore() {
  for o in basedir datadir db marker; do verdict "x" "$o" "$1/recorded" "$1/after"; done
  echo "    rows: $(grep '^db ' "$1/recorded" | grep -o 'rows=[0-9-]*') -> $(grep '^db ' "$1/after" | grep -o 'rows=[0-9-]*')"
}

d="$(restore_setup restore-replace-dir)"
rm -rf "$d/base/data"
cp -R "$d/older-data" "$d/base/data"
snapshot "$d/base" > "$d/after"
echo "case restore/directory replaced by a copy (rm + cp -R), marker kept"
report_restore "$d"

d="$(restore_setup restore-cp-over)"
cp "$d/older.sqlite" "$d/base/data/db.sqlite"
snapshot "$d/base" > "$d/after"
echo "case restore/db overwritten in place by cp over the existing path, marker kept"
report_restore "$d"

d="$(restore_setup restore-rsync-inplace)"
rsync --inplace "$d/older.sqlite" "$d/base/data/db.sqlite"
snapshot "$d/base" > "$d/after"
echo "case restore/db overwritten in place by rsync --inplace, marker kept"
report_restore "$d"

d="$(restore_setup restore-rsync-default)"
rsync "$d/older.sqlite" "$d/base/data/db.sqlite"
snapshot "$d/base" > "$d/after"
echo "case restore/db overwritten by rsync default (temp file + rename), marker kept"
report_restore "$d"

d="$(restore_setup restore-rename)"
mv "$d/base/data" "$d/base/data.newer"
mv "$d/older-data" "$d/base/data"
snapshot "$d/base" > "$d/after"
echo "case restore/data dir renamed aside, older copy renamed into its place, marker kept"
report_restore "$d"

echo
echo "done. scratch removed on exit: $SCRATCH"
