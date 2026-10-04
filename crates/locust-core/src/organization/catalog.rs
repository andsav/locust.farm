//! Pure local catalog operations for the node's serialized writer.
//!
//! Prepare and commit in the same writer turn, with no intervening catalog
//! mutations. The Store seam does not offer conditional writes: a prepared
//! commit must never be queued for later application. The node may append its
//! idempotency/feed writes before committing the complete operation atomically.
use super::{Diagnostic, inspect};
use locust_proto::id::PublicKey;
use locust_proto::organization::catalog::{Draft, Presentation, Publication, source_hash};
use locust_proto::store::{Commit, LocalWrite, Space, Store, StoreError};
use serde::{Serialize, de::DeserializeOwned};

const DRAFT: u8 = 0;
const PRESENTATION: u8 = 1;
const PUBLICATION: u8 = 2;

#[derive(Debug)]
pub enum Error {
    Store(StoreError),
    NotFound,
    Invalid(String),
    SourceConflict { current: Box<Draft> },
    PresentationConflict { current: Box<Presentation> },
    PublicationConflict { current: Box<Publication> },
    InvalidDocument { diagnostics: Vec<Diagnostic> },
}
impl From<StoreError> for Error {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
#[derive(Debug)]
pub struct Prepared<T> {
    pub value: T,
    pub commit: Commit,
}
fn prefix(actor: PublicKey, kind: u8) -> Vec<u8> {
    let mut key = actor.as_bytes().to_vec();
    key.push(kind);
    key
}
fn key(actor: PublicKey, kind: u8, id: &str) -> Result<Vec<u8>, Error> {
    if id.is_empty() {
        return Err(Error::Invalid("catalog identifier must be nonempty".into()));
    }
    let mut key = prefix(actor, kind);
    key.extend_from_slice(id.as_bytes());
    Ok(key)
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes)
        .map_err(|error| StoreError::Corrupted(format!("formation catalog record: {error}")).into())
}
trait Record: DeserializeOwned {
    fn intact(&self, actor: PublicKey, id: &str) -> bool;
}
impl Record for Draft {
    fn intact(&self, actor: PublicKey, id: &str) -> bool {
        self.owner == actor
            && self.id == id
            && self.revision > 0
            && self.source_hash == source_hash(&self.source)
    }
}
impl Record for Presentation {
    fn intact(&self, actor: PublicKey, id: &str) -> bool {
        self.owner == actor
            && self.draft_id == id
            && serde_json::from_str::<serde_json::Value>(&self.data_json).is_ok()
    }
}
impl Record for Publication {
    fn intact(&self, actor: PublicKey, id: &str) -> bool {
        let inspection = inspect(&self.source);
        self.owner == actor
            && self.id == id
            && !self.draft_id.is_empty()
            && self.draft_revision > 0
            && self.source_hash == source_hash(&self.source)
            && inspection.valid
            && inspection.semantic_hash.as_deref() == Some(&self.semantic_hash)
            && self.normalized().ok() == inspection.normalized
    }
}
fn decode_record<T: Record>(bytes: &[u8], actor: PublicKey, id: &str) -> Result<T, Error> {
    let value: T = decode(bytes)?;
    if !value.intact(actor, id) {
        return Err(StoreError::Corrupted(
            "formation catalog record identity or content mismatch".into(),
        )
        .into());
    }
    Ok(value)
}
fn read<T: Record>(store: &impl Store, actor: PublicKey, kind: u8, id: &str) -> Result<T, Error> {
    let bytes = store
        .get(Space::Formation, &key(actor, kind, id)?)?
        .ok_or(Error::NotFound)?;
    decode_record(&bytes, actor, id)
}
fn existing<T: Record>(
    store: &impl Store,
    actor: PublicKey,
    kind: u8,
    id: &str,
) -> Result<Option<T>, Error> {
    store
        .get(Space::Formation, &key(actor, kind, id)?)?
        .map(|bytes| decode_record(&bytes, actor, id))
        .transpose()
}
fn write<T: Serialize>(
    actor: PublicKey,
    kind: u8,
    id: &str,
    value: T,
) -> Result<Prepared<T>, Error> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|error| Error::Invalid(format!("catalog encoding: {error}")))?;
    Ok(Prepared {
        value,
        commit: Commit {
            local: vec![LocalWrite::Put {
                space: Space::Formation,
                key: key(actor, kind, id)?,
                value: bytes,
            }],
            ..Commit::default()
        },
    })
}
fn next(revision: u64) -> Result<u64, Error> {
    revision
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("catalog revision exhausted".into()))
}
pub fn draft(store: &impl Store, actor: PublicKey, id: &str) -> Result<Draft, Error> {
    read(store, actor, DRAFT, id)
}
pub fn presentation(store: &impl Store, actor: PublicKey, id: &str) -> Result<Presentation, Error> {
    read(store, actor, PRESENTATION, id)
}
pub fn publication(store: &impl Store, actor: PublicKey, id: &str) -> Result<Publication, Error> {
    read(store, actor, PUBLICATION, id)
}
fn list<T: Record>(store: &impl Store, actor: PublicKey, kind: u8) -> Result<Vec<T>, Error> {
    let prefix = prefix(actor, kind);
    store
        .scan(Space::Formation, &prefix)?
        .into_iter()
        .map(|(key, bytes)| {
            let id = std::str::from_utf8(&key[prefix.len()..]).map_err(|_| {
                StoreError::Corrupted("formation catalog identifier is not UTF-8".into())
            })?;
            decode_record(&bytes, actor, id)
        })
        .collect()
}
pub fn drafts(store: &impl Store, actor: PublicKey) -> Result<Vec<Draft>, Error> {
    list(store, actor, DRAFT)
}
pub fn publications(store: &impl Store, actor: PublicKey) -> Result<Vec<Publication>, Error> {
    list(store, actor, PUBLICATION)
}
/// Revision zero means the caller expects this fresh identifier to be absent.
/// Incomplete and invalid source remains editable; publication validates it.
pub fn prepare_create(
    store: &impl Store,
    actor: PublicKey,
    id: &str,
    expected_revision: u64,
    source: String,
) -> Result<Prepared<Draft>, Error> {
    if let Some(current) = existing(store, actor, DRAFT, id)? {
        return Err(Error::SourceConflict {
            current: Box::new(current),
        });
    }
    if expected_revision != 0 {
        return Err(Error::Invalid(
            "new draft requires expected revision 0".into(),
        ));
    }
    let value = Draft {
        id: id.into(),
        owner: actor,
        revision: 1,
        source_hash: source_hash(&source),
        source,
    };
    let mut prepared = write(actor, DRAFT, id, value)?;
    let layout = Presentation {
        draft_id: id.into(),
        owner: actor,
        revision: 0,
        data_json: "null".into(),
    };
    prepared
        .commit
        .local
        .extend(write(actor, PRESENTATION, id, layout)?.commit.local);
    Ok(prepared)
}
pub fn prepare_update(
    store: &impl Store,
    actor: PublicKey,
    id: &str,
    expected_revision: u64,
    source: String,
) -> Result<Prepared<Draft>, Error> {
    let current = draft(store, actor, id)?;
    if current.revision != expected_revision {
        return Err(Error::SourceConflict {
            current: Box::new(current),
        });
    }
    let value = Draft {
        id: id.into(),
        owner: actor,
        revision: next(current.revision)?,
        source_hash: source_hash(&source),
        source,
    };
    write(actor, DRAFT, id, value)
}
/// Layout JSON syntax is checked before persistence. It cannot
/// change source revision or any publication's normalized semantic identity.
pub fn prepare_presentation(
    store: &impl Store,
    actor: PublicKey,
    id: &str,
    expected_revision: u64,
    data_json: String,
) -> Result<Prepared<Presentation>, Error> {
    serde_json::from_str::<serde_json::Value>(&data_json)
        .map_err(|error| Error::Invalid(format!("presentation JSON: {error}")))?;
    draft(store, actor, id)?;
    let current = presentation(store, actor, id)?;
    if current.revision != expected_revision {
        return Err(Error::PresentationConflict {
            current: Box::new(current),
        });
    }
    write(
        actor,
        PRESENTATION,
        id,
        Presentation {
            draft_id: id.into(),
            owner: actor,
            revision: next(current.revision)?,
            data_json,
        },
    )
}
/// An immutable publication identifies the exact validated draft revision.
/// Repeating the same identity returns its existing record with no new writes,
/// even when later edits have advanced the draft. A reused publication ID with
/// different expected source identity is a recoverable conflict.
pub fn prepare_publish(
    store: &impl Store,
    actor: PublicKey,
    draft_id: &str,
    publication_id: &str,
    expected_revision: u64,
    expected_source_hash: &str,
) -> Result<Prepared<Publication>, Error> {
    if let Some(current) = existing::<Publication>(store, actor, PUBLICATION, publication_id)? {
        if current.draft_id == draft_id
            && current.draft_revision == expected_revision
            && current.source_hash == expected_source_hash
        {
            return Ok(Prepared {
                value: current,
                commit: Commit::default(),
            });
        }
        return Err(Error::PublicationConflict {
            current: Box::new(current),
        });
    }
    let current = draft(store, actor, draft_id)?;
    if current.revision != expected_revision || current.source_hash != expected_source_hash {
        return Err(Error::SourceConflict {
            current: Box::new(current),
        });
    }
    let inspection = inspect(&current.source);
    if !inspection.valid {
        return Err(Error::InvalidDocument {
            diagnostics: inspection.diagnostics,
        });
    }
    let normalized_json = serde_json::to_string(
        &inspection
            .normalized
            .expect("valid inspection is normalized"),
    )
    .map_err(|error| Error::Invalid(format!("normalized JSON: {error}")))?;
    let value = Publication {
        id: publication_id.into(),
        owner: actor,
        draft_id: draft_id.into(),
        draft_revision: current.revision,
        source: current.source,
        source_hash: current.source_hash,
        normalized_json,
        semantic_hash: inspection
            .semantic_hash
            .expect("valid inspection has semantic identity"),
    };
    write(actor, PUBLICATION, publication_id, value)
}

#[cfg(test)]
mod tests;
