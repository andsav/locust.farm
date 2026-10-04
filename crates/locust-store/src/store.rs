//! [`SqliteStore`]: opening a state directory, and the [`Store`] operations,
//! ordering file and database steps for durable acknowledgements and
//! recoverable interrupted operations.

use std::path::Path;
use std::slice;

use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::local as conventions;
use locust_proto::store::{Blob, Commit, LocalRecord, Space, Store, StoreError};
use rusqlite::{Connection, Transaction};

use crate::error::{OpenError, sql};
use crate::files::{Files, Staged};
use crate::{connection, events, local, objects, schema};

const BROKEN: &str = "an earlier write failed while it was being made durable, so its outcome \
     is unknown; reopen the store to read it back";

/// The durable store of one state directory. Owned by one thread; see the
/// crate documentation.
#[derive(Debug)]
pub struct SqliteStore {
    conn: Connection,
    files: Files,
    /// Set when a durable mutation fails with an uncertain outcome (database
    /// commit, staging append, promotion or discard). Every later call fails
    /// until reopen repairs and reads back the persisted state.
    broken: bool,
}

impl SqliteStore {
    /// Opens the state directory `dir`, durably creating it and its missing
    /// ancestors owner-only if needed,
    /// with its database (initialized directly in this binary's current schema) and
    /// its object directory, from which the leftovers of an interrupted
    /// commit are removed.
    ///
    /// Fails with [`OpenError::InUse`] while another store holds `dir`, and
    /// with [`OpenError::UnsupportedSchema`] for any unsupported database format. Signed events of another protocol are refused with
    /// [`OpenError::UnsupportedProtocolVersion`] before initialization or garbage
    /// collection. Unsupported formats are checked before configuring WAL or collecting files.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, OpenError> {
        let dir = dir.as_ref();
        connection::preflight(&conventions::database_path(dir), dir)?;
        let files = Files::create(conventions::blobs_dir(dir))?;
        let mut conn = connection::open(&conventions::database_path(dir), dir)?;
        connection::check_protocol(&conn)?;
        schema::initialize(&mut conn)?;
        files.recover_staging()?;
        objects::collect_garbage(&conn, &files)?;
        Ok(Self {
            conn,
            files,
            broken: false,
        })
    }

    /// The connection, for tests that inject failures.
    #[cfg(test)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }

    fn usable(&self) -> Result<(), StoreError> {
        if self.broken {
            Err(StoreError::Failed(BROKEN.to_owned()))
        } else {
            Ok(())
        }
    }

    fn discard_staging(&mut self, hash: &BlobHash) -> Result<(), StoreError> {
        let result = self.files.discard_staged(hash);
        if result.is_err() {
            self.broken = true;
        }
        result
    }

    /// Holds a staged copy small enough to live in the database, or the empty
    /// object when nothing is staged.
    fn promote_inline(
        &mut self,
        hash: &BlobHash,
        staged: Option<Staged>,
    ) -> Result<bool, StoreError> {
        let bytes = match staged {
            Some(staged) => staged.read()?,
            None => Vec::new(),
        };
        let Some(blob) = Blob::verified(*hash, bytes) else {
            self.discard_staging(hash)?;
            return Ok(false);
        };
        if !objects::is_held(&self.conn, hash)? {
            let tx = self.conn.transaction().map_err(sql)?;
            objects::insert(&tx, slice::from_ref(&blob))?;
            commit_durably(tx, &mut self.broken)?;
        }
        // A crash before removal leaves an independent staged copy. Keep it
        // on reopen so callers can retry promotion or discard explicitly.
        self.discard_staging(hash)?;
        Ok(true)
    }

    /// Holds a large staged copy by copying it into place, once its hash,
    /// computed in pieces, matches.
    fn promote_file(&mut self, hash: &BlobHash, staged: Staged) -> Result<bool, StoreError> {
        let len = staged.len;
        if staged.hash()? != *hash {
            self.discard_staging(hash)?;
            return Ok(false);
        }
        if objects::is_held(&self.conn, hash)? {
            self.discard_staging(hash)?;
            return Ok(true);
        }
        // Keep independent staging until the row is durable. A failed insert
        // or interrupted commit must not consume acknowledged staged bytes.
        self.files.promote_staged(hash, staged).inspect_err(|_| {
            self.broken = true;
        })?;
        let tx = self.conn.transaction().map_err(sql)?;
        objects::insert_file(&tx, hash, len)?;
        commit_durably(tx, &mut self.broken)?;
        self.discard_staging(hash)?;
        Ok(true)
    }
}

/// Commits `tx`, durably. A failure here leaves the outcome unknown (see
/// `SqliteStore::broken`), so it breaks the store.
fn commit_durably(tx: Transaction<'_>, broken: &mut bool) -> Result<(), StoreError> {
    tx.commit().map_err(|error| {
        *broken = true;
        sql(error)
    })
}

impl Store for SqliteStore {
    /// Installs the commit's new large objects as files first; then one
    /// transaction appends the events, adds the objects' rows (small objects
    /// inline), applies the local writes in order and deletes the dropped
    /// objects' rows; the files of dropped objects are removed after it
    /// commits.
    fn commit(&mut self, commit: &Commit) -> Result<(), StoreError> {
        self.usable()?;
        objects::install(&self.conn, &self.files, &commit.blobs)?;
        let tx = self.conn.transaction().map_err(sql)?;
        events::append(&tx, &commit.events)?;
        objects::insert(&tx, &commit.blobs)?;
        local::apply(&tx, &commit.local)?;
        let removed = objects::delete(&tx, &commit.drop_blobs)?;
        commit_durably(tx, &mut self.broken)?;
        for hash in &removed {
            self.files.remove(hash);
        }
        Ok(())
    }

    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError> {
        self.usable()?;
        events::event(&self.conn, id)
    }

    fn has_event(&self, id: &EventId) -> Result<bool, StoreError> {
        self.usable()?;
        events::has_event(&self.conn, id)
    }

    fn log(
        &self,
        goal: &GoalId,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Event)>, StoreError> {
        self.usable()?;
        events::log(&self.conn, goal, after, limit)
    }

    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        after: Option<AuthorPoint>,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError> {
        self.usable()?;
        events::author_log(&self.conn, goal, author, after, limit)
    }

    fn goals(&self) -> Result<Vec<GoalId>, StoreError> {
        self.usable()?;
        events::goals(&self.conn)
    }

    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError> {
        self.usable()?;
        objects::read(&self.conn, &self.files, hash)
    }

    fn blob_len(&self, hash: &BlobHash) -> Result<Option<u64>, StoreError> {
        self.usable()?;
        objects::len(&self.conn, hash)
    }

    fn blob_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.usable()?;
        objects::read_range(&self.conn, &self.files, hash, offset, len)
    }

    /// Appends to `blobs/<hash>.staged` and syncs it (and the directory when
    /// the file is new); one or two flushes per chunk. An object that
    /// arrives in a single chunk is cheaper to verify and commit directly.
    fn stage_blob(
        &mut self,
        hash: &BlobHash,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, StoreError> {
        self.usable()?;
        let result = self.files.stage(hash, offset, bytes);
        if result.is_err() {
            self.broken = true;
        }
        result
    }

    fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError> {
        self.usable()?;
        self.files.staged_len(hash)
    }

    fn staged_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.usable()?;
        self.files.staged_range(hash, offset, len)
    }

    fn discard_staged_blob(&mut self, hash: &BlobHash) -> Result<(), StoreError> {
        self.usable()?;
        self.discard_staging(hash)
    }

    /// A small copy is verified in memory and moved into the database; a
    /// large one is hashed in 64 KiB pieces and copied into place while its
    /// independent staged copy remains recoverable until the row commits.
    fn finish_blob(&mut self, hash: &BlobHash) -> Result<bool, StoreError> {
        self.usable()?;
        match self.files.open_staged(hash)? {
            Some(staged) if objects::is_file(staged.len) => self.promote_file(hash, staged),
            staged => self.promote_inline(hash, staged),
        }
    }

    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        self.usable()?;
        local::get(&self.conn, space, key)
    }

    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError> {
        self.usable()?;
        local::scan(&self.conn, space, prefix)
    }
}
