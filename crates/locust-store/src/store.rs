//! [`SqliteStore`]: opening a state directory, and the [`Store`] operations,
//! each ordering its file and database steps so that a crash at any point
//! leaves all of the operation or none of it.

use std::path::Path;
use std::slice;

use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::store::{Blob, Commit, LocalRecord, Space, Store, StoreError};
use rusqlite::{Connection, Transaction};

use crate::error::{OpenError, sql};
use crate::files::{Files, Staged};
use crate::{connection, events, local, objects, schema};

/// The database file in the state directory.
const DATABASE: &str = "locust.db";
/// The directory of large content objects in the state directory.
const OBJECTS: &str = "blobs";

const BROKEN: &str = "an earlier commit failed while it was being made durable, so its outcome \
     is unknown; reopen the store to read it back";

/// The durable store of one state directory. Owned by one thread; see the
/// crate documentation.
#[derive(Debug)]
pub struct SqliteStore {
    conn: Connection,
    files: Files,
    /// Set when a transaction failed during its commit. Such a failure can
    /// come after the commit reached the disk (a failed sync, say), so what
    /// the database holds is unknown until it is read back from disk; every
    /// later call fails until the store is reopened.
    broken: bool,
}

impl SqliteStore {
    /// Opens the state directory `dir`, creating it owner-only if missing,
    /// with its database (created or migrated to this binary's schema) and
    /// its object directory, from which the leftovers of an interrupted
    /// commit are removed.
    ///
    /// Fails with [`OpenError::InUse`] while another store holds `dir`, and
    /// with [`OpenError::NewerSchema`] for a database written by a newer
    /// release.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, OpenError> {
        let dir = dir.as_ref();
        let files = Files::create(dir.join(OBJECTS))?;
        let mut conn = connection::open(&dir.join(DATABASE), dir)?;
        schema::migrate(&mut conn)?;
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
            self.files.discard_staged(hash)?;
            return Ok(false);
        };
        if !objects::is_held(&self.conn, hash)? {
            let tx = self.conn.transaction().map_err(sql)?;
            objects::insert(&tx, slice::from_ref(&blob))?;
            commit_durably(tx, &mut self.broken)?;
        }
        // A crash before this removal leaves a staged copy of a held object,
        // which the next open removes.
        self.files.discard_staged(hash)?;
        Ok(true)
    }

    /// Holds a large staged copy by renaming it into place, once its hash,
    /// computed in pieces, matches.
    fn promote_file(&mut self, hash: &BlobHash, staged: Staged) -> Result<bool, StoreError> {
        let len = staged.len;
        if staged.hash()? != *hash {
            self.files.discard_staged(hash)?;
            return Ok(false);
        }
        if objects::is_held(&self.conn, hash)? {
            self.files.discard_staged(hash)?;
            return Ok(true);
        }
        // A crash between the rename and the commit leaves an object file
        // that no row names: the next open removes it, and the transfer
        // starts over.
        self.files.promote_staged(hash)?;
        let tx = self.conn.transaction().map_err(sql)?;
        objects::insert_file(&tx, hash, len)?;
        commit_durably(tx, &mut self.broken)?;
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
        self.files.stage(hash, offset, bytes)
    }

    fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError> {
        self.usable()?;
        self.files.staged_len(hash)
    }

    /// A small copy is verified in memory and moved into the database; a
    /// large one is hashed in 64 KiB pieces and renamed into place, so its
    /// bytes pass through memory once and are never copied on disk.
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
