#!/bin/bash
# measure_linux_container.sh — the Linux half of the G1 gate measurements.
# Runs INSIDE a Docker container, on the container's own filesystem (no bind
# mount), e.g.:
#   docker run --rm -i --tmpfs /tmpfs ubuntu:22.04 bash -s < measure_linux_container.sh
# Records inode number and what the filesystem reports for creation (birth)
# time, before and after the copy and restore cases of measure_macos.sh.
# Works only under /work inside the container.

set -euo pipefail

WORK=/work
mkdir -p "$WORK"
cd "$WORK"
rm -rf case-* 2>/dev/null || true

echo "== environment (inside container) =="
echo "date: $(date '+%Y-%m-%d %H:%M:%S %z')"
echo "kernel: $(uname -a)"
echo "/work filesystem: $(stat -f -c %T "$WORK")"
if [ -d /tmpfs ]; then
  echo "/tmpfs filesystem: $(stat -f -c %T /tmpfs)"
fi
echo "stat: $(stat --version | head -1)"
echo "tar: $(tar --version | head -1)"

RSYNC=0
if command -v rsync >/dev/null 2>&1; then
  RSYNC=1
else
  if apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq rsync >/dev/null 2>&1; then
    RSYNC=1
  fi
fi
if [ "$RSYNC" = 1 ]; then
  echo "rsync: $(rsync --version | head -1)"
else
  echo "rsync: NOT AVAILABLE (no network or no package); rsync cases skipped"
fi

# ident PATH -> "inode|dev|birthW_epoch|birth_w_human"; birth is "-" / 0 when
# the filesystem reports no creation time
ident() {
  stat -c '%i|%D|%W|%w' "$1"
}

new_fixture() {
  mkdir -p "$1/data"
  printf 'locust-g1-gate-db-placeholder-v1\n' > "$1/data/db.sqlite"
  dd if=/dev/urandom of="$1/data/db.sqlite" bs=1k count=8 conv=notrunc status=none 2>/dev/null || \
    printf '0123456789abcdef\n' >> "$1/data/db.sqlite"
  printf 'marks-v1\n' > "$1/marker.txt"
}

write_more() {
  printf 'more-rows-v2\n' >> "$1/data/db.sqlite"
  printf 'marks-v2\n' >> "$1/marker.txt"
}

snapshot() {
  echo "basedir $(ident "$1")"
  echo "datadir $(ident "$1/data")"
  echo "db $(ident "$1/data/db.sqlite")"
  echo "marker $(ident "$1/marker.txt")"
}

fetch() { grep "^$2 " "$1" | sed "s/^$2 //"; }

verdict() {
  local before after bi ai bb ab iv bv brest arrest
  before="$(fetch "$3" "$2")"; after="$(fetch "$4" "$2")"
  bi="${before%%|*}"; brest="${before#*|}"
  ai="${after%%|*}";  arrest="${after#*|}"
  bb="${brest#*|}"; bb="${bb%%|*}"        # birthW epoch field
  ab="${arrest#*|}"; ab="${ab%%|*}"
  local bh ah
  bh="${before##*|}"; ah="${after##*|}"   # human %w, "-" when unsupported
  [ "$bi" = "$ai" ] && iv="same" || iv="CHANGED"
  if [ "$bh" = "-" ] && [ "$ah" = "-" ]; then
    bv="n/a (fs reports no creation time)"
  elif [ "$bh" = "$ah" ]; then
    bv="same"
  else
    bv="CHANGED"
  fi
  printf '  %-8s inode:%-7s birth:%-7s\n    before %s\n    after  %s\n' \
    "$2" "$iv" "$bv" "$before" "$after"
}

echo
echo "== what creation time looks like here =="
f=/work/probe; echo x > "$f"; echo "fresh file on /work: $(ident "$f")"; rm "$f"
if [ -d /tmpfs ]; then
  f=/tmpfs/probe; echo x > "$f"; echo "fresh file on /tmpfs: $(ident "$f")"; rm "$f"
fi
echo "inode reuse probe, five create/delete cycles on /work:"
for i in 1 2 3 4 5; do
  f=/work/reuse; echo x > "$f"; echo "  cycle $i: $(ident "$f")"; rm "$f"
done
if [ -d /tmpfs ]; then
  echo "inode reuse probe, five create/delete cycles on /tmpfs:"
  for i in 1 2 3 4 5; do
    f=/tmpfs/reuse; echo x > "$f"; echo "  cycle $i: $(ident "$f")"; rm "$f"
  done
fi

echo
echo "== copies (identity of the copy, compared with the source) =="
run_copy() {
  local name="$1"; shift
  local d="case-copy-$name"
  mkdir -p "$d"
  new_fixture "$d/base"
  write_more "$d/base"
  sleep 1.1
  snapshot "$d/base" > "$d/source"
  SRC="$d/base" DST="$d/copy" bash -c "$*"
  snapshot "$d/copy" > "$d/copy-id"
  echo "case copy/$name"
  for o in basedir datadir db marker; do verdict "x" "$o" "$d/source" "$d/copy-id"; done
}
run_copy "cp -R"   'cp -R "$SRC" "$DST"'
run_copy "cp -Rp"  'cp -Rp "$SRC" "$DST"'
run_copy "tar pack+unpack" 'tar czf "$DST.tgz" -C "$(dirname "$SRC")" "$(basename "$SRC")" && mkdir -p "$DST" && tar xzf "$DST.tgz" -C "$DST" && mv "$DST/$(basename "$SRC")"/* "$DST/" && rmdir "$DST/$(basename "$SRC")" && rm "$DST.tgz"'
if [ "$RSYNC" = 1 ]; then
  run_copy "rsync -a" 'rsync -a "$SRC/" "$DST/"'
fi

d="case-copy-mv-within"; mkdir -p "$d"
new_fixture "$d/base"
write_more "$d/base"
snapshot "$d/base" > "$d/source"
mv "$d/base" "$d/moved"
snapshot "$d/moved" > "$d/copy-id"
echo "case copy/mv within volume"
for o in basedir datadir db marker; do verdict "x" "$o" "$d/source" "$d/copy-id"; done

echo
echo "== restores (identity at the live path after the restore, compared with what the installation last recorded) =="
# Same shape as the macOS cases. The sleep keeps the live db's mtime outside
# the snapshot's second: rsync's quick check skips files equal in size and
# mtime, and would otherwise report a restore it never made.
restore_setup() {
  local d="case-$1"
  mkdir -p "$d"
  new_fixture "$d/base"
  cp "$d/base/data/db.sqlite" "$d/older.sqlite"
  cp -R "$d/base/data" "$d/older-data"
  sleep 1.1
  write_more "$d/base"
  snapshot "$d/base" > "$d/recorded"
  echo "$d"
}
report_restore() {
  for o in basedir datadir db marker; do verdict "x" "$o" "$1/recorded" "$1/after"; done
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

if [ "$RSYNC" = 1 ]; then
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
fi

d="$(restore_setup restore-rename)"
mv "$d/base/data" "$d/base/data.newer"
mv "$d/older-data" "$d/base/data"
snapshot "$d/base" > "$d/after"
echo "case restore/data dir renamed aside, older copy renamed into its place, marker kept"
report_restore "$d"

echo
echo "done."
