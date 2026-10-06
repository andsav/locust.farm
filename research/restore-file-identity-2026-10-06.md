# Restore file identity: how copies and restores look to the operating system

Measured results, 6 October 2026. This note is the gate for phase G1 of the
[host safety and ending plan](../docs/host-safety-and-ending-plan.md) and the
recorded note its exit criteria ask for (G1, "Exit criteria", last item).

G1's restore guard decides, at every start, whether the database file is the
file this installation last used. It answers with `FileId { ino, created_ms }`:
the file's inode number and creation time, the device number deliberately left
out because it can change between boots (G1, "Changes",
`crates/locust-store/src/marks.rs` item, `FileId::of`). The first table under
"What a start finds" then classifies the start: the database is *the file last
used* (ordinary start, rows 1 and 3) or *another file* (the data directory was
replaced by a copy, rows 2 and 4). Nobody had measured how real tools treat
those two values. This note does, for the file systems this Mac uses.

## Where and how it was measured

- macOS 26.4 (build 25E246), arm64. The scratch directory was a fresh
  `mktemp -d` under `/tmp`, on APFS (`/System/Volumes/Data`, disk3s5). Tools:
  `/usr/bin/sqlite3` 3.51.0, rsync 3.2.3, bsdtar 3.5.3, `/usr/bin/ditto`, and
  the stock `cp`, `mv`, `stat`. The cross-volume case used the one other
  writable volume mounted, an HFS+ disk image (`/Volumes/dmg.PyiMhm`,
  disk6s1).
- Linux, inside Docker (already installed and running): `ubuntu:22.04` on
  Docker's overlayfs (kernel 5.15.49-linuxkit-pr), plus a tmpfs probe. No bind
  mount: everything on the container's own filesystem. rsync 3.2.7, GNU tar
  1.34, coreutils stat 8.32.
- The fixture mirrors the plan's layout: a SQLite database in a `data`
  directory (WAL mode, as `crates/locust-store` runs it) and a small marker
  file beside that directory, standing in for the marks file the plan keeps
  next to the data directory precisely so a copy of the data does not carry it
  (G1, "Changes", first items; `MARKS_SUFFIX`). Each case records inode and
  creation time of the base directory, the data directory, the database and
  the marker, before and after. Restores were verified to actually roll the
  content back (row counts 5 → 3), so an unchanged identity means a genuinely
  invisible rollback, not a restore that never happened.
- The script and its full output:
  [evidence/restore-file-identity-2026-10-06/](evidence/restore-file-identity-2026-10-06/README.md).
  Nothing outside `/tmp` scratch (and one removed scratch directory on the
  HFS+ image) was touched; no product code changed.

## The table

The subject is the database file; the marker's behavior is described after it.
"G1 concludes" applies the first table: identical inode and creation time
means *the file last used*, any difference means *another file*.

| Case | Where | Inode changed | Creation time changed | G1 concludes | Right? |
| --- | --- | --- | --- | --- | --- |
| Ordinary: process stops and starts (read only) | macOS APFS | no | no | the file last used | yes |
| Ordinary: written and checkpointed (`wal_checkpoint(TRUNCATE)`) | macOS APFS | no | no | the file last used | yes |
| Ordinary: `VACUUM` (sqlite 3.51) | macOS APFS | no | no | the file last used | yes |
| Copy: `cp -R` | macOS APFS | yes | yes (copy's own time) | another file | yes |
| Copy: `cp -Rp` | macOS APFS | yes | yes (source's *mtime*, not its creation time) | another file | yes |
| Copy: `cp -c` (APFS clonefile) | macOS APFS | yes | **no — preserved exactly** | another file, on the inode alone | yes |
| Copy: `ditto` | macOS APFS | yes | yes (source's *mtime*) | another file | yes |
| Copy: `rsync -a` | macOS APFS | yes | yes (source's *mtime*) | another file | yes |
| Copy: tar packed and unpacked | macOS APFS | yes | yes (restored *mtime*) | another file | yes |
| Move within the volume (`mv`, i.e. rename) | macOS APFS | no | no | the file last used | yes — a rename is not a copy |
| Move to another volume and back | APFS → HFS+ → APFS | yes | yes (move time; HFS+ keeps 1 s precision) | another file | yes — conservative; see below |
| Restore: data directory replaced by a copy (`rm` + `cp -R`), marker kept | macOS APFS | yes | yes | another file → row 2 | yes |
| Restore: db overwritten in place, `cp` over the existing path | macOS APFS | **no** | **no** | the file last used → row 1 | **no — missed**, unless a mark is ahead |
| Restore: db overwritten in place, `rsync --inplace` | macOS APFS | **no** | **no** | the file last used → row 1 | **no — missed**, same shape |
| Restore: db overwritten by `rsync` default (temp file + rename) | macOS APFS | yes | yes | another file → row 2 | yes |
| Restore: data dir renamed aside, older copy renamed into its place | macOS APFS | yes | yes | another file → row 2 | yes |
| Copies: `cp -R`, `cp -Rp`, tar, `rsync -a` | Linux overlayfs | yes | yes (copy's own time; birth time cannot be set on Linux) | another file | yes |
| Move within the volume | Linux overlayfs | no | no | the file last used | yes |
| Restore: data directory replaced by a copy | Linux overlayfs | **no — the freed inode numbers were reused at once** | yes | another file → row 2 | yes, **on the creation time alone** |
| Restore: `cp` over the existing path | Linux overlayfs | no | no | the file last used → row 1 | no — missed, same shape as macOS |
| Restore: `rsync --inplace` | Linux overlayfs | no | no | the file last used → row 1 | no — missed, same shape |
| Restore: `rsync` default (temp + rename) | Linux overlayfs | yes | yes | another file → row 2 | yes |
| Restore: rename aside + rename in | Linux overlayfs | yes | yes | another file → row 2 | yes |

The marker beside the data directory: in every restore case it was left
untouched — a restore of the data directory does not carry it — so the marks
read as *kept* and the start lands in row 1 or 2, exactly as the plan's layout
intends. In the whole-fixture copies and the cross-volume move, the marker and
its directory changed identity with everything else, so the marks file's
header check (`FileId` of its own directory, G1's `marks.rs` item) reads them
as *lost or a copy* and the start lands in row 4, a copy of unknown age.

How to read the creation-time column:

- Every copy tool measured, on both systems, gives the copy a new inode
  number. No exception. Detection of a copy never has to rest on the creation
  time.
- The exception in the other direction is `cp -c`: APFS clonefile preserves
  the creation time exactly (and the mtime), so the clone's `FileId` differs
  from the original's *only* in the inode. `FileId` still catches it, but this
  is the copy for which the creation time contributes nothing.
- `cp -Rp`, `ditto`, `rsync -a` and tar on macOS do not preserve the creation
  time either, but they do something subtler: the copy's creation time comes
  out equal to the source's *modification time*. The tool back-dates the
  copy's mtime to the source's, and APFS clamps the creation time down to it.
  A file whose creation time equals its mtime (never rewritten since
  creation) therefore *appears* to keep its creation time across these tools —
  an artifact, measured as such, not preservation.
- On Linux the birth time cannot be set at all: every new file's creation
  time is its own creation instant, always.
- On overlayfs, deleting a file and immediately recreating it reused the same
  inode number in five of five cycles — and in the replace-directory restore,
  the new data directory and database landed on the freed inode numbers. Only
  the changed creation time kept the restore visible. The same probe on tmpfs
  showed no reuse (inodes increment), but tmpfs reports no creation time at
  all (`stat` prints `-`), so `created_ms` would be `None` there.

## The two dangerous kinds

**A restore that G1 would take for the same file — the in-place overwrite.**
Measured on both systems: `cp older.sqlite data/db.sqlite` over the existing
path and `rsync --inplace` keep the inode *and* the creation time while the
content verifiably rolls back (5 → 3 rows). The start lands in row 1. What the
plan already does about it: row 1's own caveat — "If a mark is ahead of the
store all the same, the database was overwritten in place, and that goal is
treated as in the next row" (first table, G1 "Changes") — so with the marks
kept, every goal that signed anything since the copy is caught; this is the
test `a_database_overwritten_in_place_is_found_by_its_marks` (G1 "Tests",
`durable_tests.rs` item), and `restore_found` runs for such a goal (G1
"Changes", `Node::restore_found`). What it does not cover: the marks are the
whole defence. If they are lost at the same time, row 3 ("marks lost, or a
copy" + "the file last used") reads the start as ordinary and nothing is
caught — residual 3 in "Risks and notes": "A database overwritten in place
while the marks are lost: two faults at once." And a tool that writes *both*
the database and the marks in place — a disk image, a VM or volume snapshot —
is residual 2, invisible by construction; the plan's only answer is the
farm-service check `guard_attest` for public goals, and only as far back as
the last request the service saw (G1 "Changes", `Node::guard_attest`). For a
goal where no mark is ahead, an in-place overwrite is invisible by design:
nothing this daemon signed in that goal was lost, and records it merely
received return by exchange. One more measured limit: where the filesystem
reports no creation time (tmpfs, measured) `created_ms` is `None`, and the
measured inode reuse on overlayfs shows that an allocator can hand a replaced
file its old number back; a filesystem with both properties would make even a
replaced directory read as the same file. This is the plan's note "The first
table rests on file numbers" ("Risks and notes") seen from the other side.

**An ordinary start that G1 would take for another file — a false alarm on
every start.** Not reproduced on this Mac. Ordinary use keeps both values:
process stop/start, WAL writes with truncating checkpoints, and even
`VACUUM` — SQLite 3.51 rewrites the database in place and the identity
survives. Locust itself never vacuums: `crates/locust-store` sets
`journal_mode=WAL` and runs `wal_checkpoint(FULL)` (`connection.rs`), and no
code path replaces the database file. A rename — the one operation the plan's
exit criterion calls out — changes nothing either. So on APFS there is no
ordinary path to a false alarm. What the plan does about the shape anyway:
the same "Risks and notes" item accepts it as conservative for filesystems
that do not keep file numbers across a remount ("every start looks like a
copy: pending invitations are revoked at each start, and with the marks on
such a disk too every goal waits"), and row 2 bounds the cost — keys whose
marks are not ahead sign at once, so a false row-2 start costs the revoked
pending invitations of hosted goals (`Node::restore_found` → `revoke_pending`)
and a `restored` flag that lasts until the first ordinary start with nothing
held ("Risks and notes", local-settings item). What it does not do: there is
no mechanism to re-trust a file the daemon itself replaced, because none is
needed today — and because an operator who replaces the database by hand
(`VACUUM INTO`, `.restore`, a scripted swap) *is* indistinguishable from a
restore, which is the safe direction for the guard to err.

## What was not measured

- **Time Machine.** No backup exists on this Mac (`tmutil destinationinfo`:
  "No destinations configured"; `tmutil listbackups`: no machine directory,
  operation not permitted). Turning Time Machine on, creating snapshots, or
  changing any system setting was out of scope. The plan's exit criterion
  already lists a Time Machine restore as "added when a second machine is at
  hand."
- **Migration Assistant**, for the same reason (needs a second machine; the
  plan already defers it).
- **A reboot.** The exit criterion asks whether `FileId` stays equal across a
  reboot; rebooting this Mac was out of scope. The rename half of that
  criterion is measured above (stays equal).
- **Native ext4.** No bare-metal Linux was at hand; the Linux rows are
  Docker's overlayfs (which does report birth time) on an ext4-backed VM, plus
  the tmpfs probe. An ext4 filesystem with small inodes — the real-world case
  of "no creation time *and* inode reuse" — is unmeasured.
- Finder drag-copies, `sqlite3` `.restore` / `VACUUM INTO`, cloud-synced
  folders, and network or removable filesystems (the remount case the plan's
  note names). Windows, which the plan does not ask about for this note.
