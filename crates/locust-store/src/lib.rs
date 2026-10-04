//! Durable storage: the SQLite implementation of
//! [`locust_proto::store::Store`].
//!
//! [`SqliteStore::open`] takes the daemon's state directory and keeps two
//! things in it:
//!
//! - `locust.db` (with `locust.db-wal`): the event log and its indexes, the
//!   local records, and every content object of at most
//!   [`INLINE_MAX_BYTES`];
//! - `blobs/`: one file per larger object, named by its hash in hex, and the
//!   partial copies of objects being received (`<hash>.staged`).
//!
//! One thread owns the store, as the storage seam requires: one connection,
//! no pool, no lock of our own, no async. The type is `Send` so the daemon
//! can move it onto that thread.
//!
//! # One commit, one transaction
//!
//! [`Store::commit`](locust_proto::store::Store::commit) writes the files of
//! its new large objects first: each to a temporary name, synced, renamed to
//! its hash, then the directory synced once. Then one SQLite transaction
//! appends the events with their positions, adds the object rows (small
//! objects inline), applies the local writes in order and deletes the rows
//! of dropped objects. When that transaction commits, the whole commit is
//! durable; the files of dropped objects are removed afterwards. A crash
//! before the transaction commits leaves only object files that no row
//! names, which the next open removes. A failure while the transaction
//! commits (a failed sync) has an unknown outcome, so the store then refuses
//! every call until it is reopened and reads the outcome back.
//!
//! # Durability and what it costs
//!
//! - `locking_mode=EXCLUSIVE`: the connection takes SQLite's exclusive lock
//!   on the database file when it opens it and holds it until it is
//!   dropped. A second store on the same directory, in another daemon or in
//!   this process, fails with [`OpenError::InUse`] instead of interleaving
//!   writes. It also keeps the WAL index in process memory, so there is no
//!   `-shm` file and no lock traffic per transaction. Nothing else, the
//!   `sqlite3` shell included, can read the database while the store is open.
//! - `journal_mode=WAL`: a commit appends the pages it changed to the WAL and
//!   syncs that one file; a checkpoint copies them into `locust.db` later
//!   (about every 4 MiB of WAL, SQLite's default, and on close).
//! - `synchronous=FULL`: the WAL is synced at every commit. Under `NORMAL` a
//!   commit acknowledged just before a power loss could disappear, and the
//!   daemon publishes its own events once a commit returns.
//! - `fullfsync=ON`: on macOS `fsync` only hands data to the drive, which may
//!   keep it in a volatile cache; `F_FULLFSYNC` makes the drive flush it.
//!   Elsewhere the pragma does nothing and `fsync` already flushes. Object
//!   files and the directory are synced with `File::sync_all`, which uses
//!   `F_FULLFSYNC` on macOS as well.
//!
//! A commit therefore costs one full flush, plus, when it carries new large
//! objects, one per object file and one for the directory. On an Apple SSD
//! with APFS one flush takes about 4 ms.
//!
//! # Positions
//!
//! Positions are per goal, dense from 1 and assigned in commit order. The
//! `goals` table records each goal's last position, which a commit reads
//! once per goal it touches and writes back once; a unique
//! `(goal, position)` index makes any gap or repeat a failed commit rather
//! than a damaged log. Sequence numbers and positions are SQLite integers,
//! which the contract's cap of `i64::MAX` on sequence numbers allows.
//!
//! # Integrity
//!
//! Events are rebuilt with `Event::from_stored`, which refuses bytes that no
//! longer hash to the stored identifier; that, a row indexed under another
//! goal, author or position, and an object file that is missing or of the
//! wrong length are reported as `StoreError::Corrupted`. Tables are `STRICT`.
//! The schema version is `PRAGMA user_version`, migrated forward on open; a
//! newer version than this binary knows is refused with
//! [`OpenError::NewerSchema`].
//!
//! Unix only: object files use positional reads and owner-only modes.
//! Depends on `locust-proto` only. Owner: the storage stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]

mod columns;
mod connection;
mod error;
mod events;
mod files;
mod local;
mod objects;
mod schema;
mod store;
#[cfg(test)]
mod tests;

pub use error::OpenError;
pub use objects::INLINE_MAX_BYTES;
pub use store::SqliteStore;
