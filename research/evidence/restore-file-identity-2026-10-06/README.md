# Evidence for restore file identity measurements

Raw results and the scripts behind
[restore file identity: how copies and restores look to the operating system](../../restore-file-identity-2026-10-06.md),
the gate note for phase G1 of the
[host safety and ending plan](../../../docs/host-safety-and-ending-plan.md),
collected on 6 October 2026.

- `measure_macos.sh`: the macOS half. Builds a fixture (a WAL-mode SQLite
  database in a `data` directory, a marker file beside it) in a fresh
  `mktemp -d` under `/tmp`, then records inode and creation time
  (millisecond resolution, via `os.stat().st_birthtime`) around ordinary use,
  the copy tools, and the restore shapes. Takes the cross-volume target as an
  optional first argument; otherwise it picks the first writable non-system
  volume under `/Volumes`, and skips the case if there is none. Removes all of
  its scratch on exit.
- `measure_linux_container.sh`: the Linux half, run inside a Docker container
  on the container's own filesystem (no bind mount), e.g.
  `docker run --rm -i --tmpfs /tmpfs ubuntu:22.04 bash -s < measure_linux_container.sh`.
  Records `stat -c '%i|%D|%W|%w'`, installs rsync with apt if it is missing
  and the container has network, and skips the rsync cases otherwise.
- `run_all.sh`: runs both halves and writes `results-macos.txt` and
  `results-linux.txt` next to itself. Skips the Linux half when Docker is not
  running.
- `results-macos.txt`, `results-linux.txt`: the output the note's table is
  built from, on macOS 26.4 (APFS; cross-volume case against an HFS+ disk
  image) and in `ubuntu:22.04` on overlayfs + tmpfs (kernel
  5.15.49-linuxkit-pr).

To reproduce: `run_all.sh [other-volume]` on any Mac with sqlite3, rsync and
Docker; compare its two results files with the committed ones. Inode numbers
and timestamps differ per run; the same/CHANGED verdicts are the result.

Fixture notes:

- The restore cases sleep 1.1 s between snapshotting the "older" copy and
  writing newer rows. rsync's default quick check skips a file whose size and
  mtime match, and without the sleep the fixture lands inside one second, so
  rsync would report an identity for a restore it never made.
- `write_more` also appends to the marker, so its creation time and mtime
  differ as they do on a live marks file; without that, `cp -Rp`, `ditto` and
  `rsync -a` appear to preserve its creation time (the APFS clamp of creation
  time to the back-dated mtime; see the note).
- Nothing touches `~/.locust` or any real Locust data; nothing needs
  administrator rights.
