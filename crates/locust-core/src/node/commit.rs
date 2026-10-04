//! The single write path.
//!
//! A request is planned against the node as it is (`&self`), which yields a
//! [`Tx`]: everything the request wants made durable. [`Node::land`] commits
//! it as one store commit and only then updates memory, from the commit's own
//! writes. A failed commit returns the store's error and leaves memory as it
//! was.

use locust_proto::api::{ApiError, Caller, ErrorCode, Response};
use locust_proto::codec;
use locust_proto::crypto::content_hash;
use locust_proto::engine::Entropy;
use locust_proto::id::{GoalId, IdempotencyKey, PublicKey};
use locust_proto::store::{Commit, LocalWrite, Space, Store, StoreError};
use serde::{Deserialize, Serialize};

use super::{Node, records};

/// What one request or received frame wants made durable.
#[derive(Debug, Default)]
pub(super) struct Tx {
    pub commit: Commit,
    /// Goals whose revision rises with this commit: their events change, or
    /// a local record that feeds pending work does.
    pub touched: Vec<GoalId>,
    /// The owner asked the daemon to stop.
    pub stop: bool,
}

impl Tx {
    /// A transaction with nothing in it.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn local(&mut self, write: LocalWrite) -> &mut Self {
        self.commit.local.push(write);
        self
    }

    /// Marks `goal` as changed by this commit.
    pub fn touch(&mut self, goal: GoalId) -> &mut Self {
        if !self.touched.contains(&goal) {
            self.touched.push(goal);
        }
        self
    }

    fn is_empty(&self) -> bool {
        let commit = &self.commit;
        commit.events.is_empty()
            && commit.blobs.is_empty()
            && commit.local.is_empty()
            && commit.drop_blobs.is_empty()
    }
}

/// What the store keeps under an idempotency key: the digest of the request
/// that first used it and the encoded response it got.
#[derive(Serialize, Deserialize)]
struct IdempotencyRecord {
    digest: [u8; 32],
    #[serde(with = "codec::bytes")]
    response: Vec<u8>,
}

/// Keys are scoped to the caller: the owner, or one principal.
fn idempotency_key(caller: Caller, key: &IdempotencyKey) -> Vec<u8> {
    match caller {
        Caller::Owner => records::key(0, &[&key.0]),
        Caller::Agent(principal) | Caller::Viewer(principal) => {
            records::key(1, &[&principal.0, &key.0])
        }
    }
}

/// Identifies a request for idempotency: the plain digest of its encoding
/// together with the principal it was made on behalf of.
pub(super) fn request_digest<T: Serialize>(on_behalf: Option<PublicKey>, request: &T) -> [u8; 32] {
    let encoded = codec::encode(&(on_behalf, request)).expect("requests encode");
    content_hash(&encoded).0
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// The response first given under `key`, if the caller used it before.
    /// `IdempotencyMismatch` when it was used for a different request.
    pub(super) fn replayed(
        &self,
        caller: Caller,
        key: &IdempotencyKey,
        digest: &[u8; 32],
    ) -> Result<Option<Response>, ApiError> {
        let Some(stored) = self
            .store
            .get(Space::Idempotency, &idempotency_key(caller, key))?
        else {
            return Ok(None);
        };
        let record: IdempotencyRecord = records::read(&stored)?;
        if record.digest != *digest {
            return Err(ApiError::new(
                ErrorCode::IdempotencyMismatch,
                "the idempotency key was used for a different request",
            ));
        }
        Ok(Some(records::read(&record.response)?))
    }

    /// The write that makes a retry of this request return `response`.
    pub(super) fn remember(
        caller: Caller,
        key: &IdempotencyKey,
        digest: [u8; 32],
        response: &Response,
    ) -> LocalWrite {
        records::put(
            Space::Idempotency,
            idempotency_key(caller, key),
            &IdempotencyRecord {
                digest,
                response: codec::encode(response).expect("responses encode"),
            },
        )
    }

    /// Commits `tx` and then updates memory from it.
    pub(super) fn land(&mut self, tx: Tx) -> Result<(), StoreError> {
        if !tx.is_empty() {
            self.store.commit(&tx.commit)?;
            for write in &tx.commit.local {
                match write {
                    LocalWrite::Put { space, key, value } => {
                        self.absorb(*space, key, Some(value))?;
                    }
                    LocalWrite::Delete { space, key } => self.absorb(*space, key, None)?,
                }
            }
        }
        self.changed.extend(tx.touched);
        self.stop |= tx.stop;
        Ok(())
    }
}
