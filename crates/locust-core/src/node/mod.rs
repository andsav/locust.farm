//! The daemon's state machine: local requests and peer exchanges over one store.
//!
//! [`Node`] owns the store, the daemon's identity, the enrolled principals
//! and every goal, and is the only writer. One request or one received frame
//! produces at most one [`Commit`](locust_proto::store::Commit); what the
//! node remembers is then updated from that commit's own writes
//! (`Node::absorb`), so memory is always what a restart would load.
//!
//! Module map: `identity` (who the daemon is, its principals), `callers`
//! (connections, who a request acts as), `commit` (the single write path),
//! `requests` (one module per family of operations).

// Removed once every family of requests exists; until then some shared
// helpers have no caller yet.
#![allow(dead_code)]

mod callers;
mod commit;
mod identity;
mod peers;
mod records;
mod requests;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use locust_proto::API_VERSION;
use locust_proto::api::{ApiError, Caller, ClientHello, ErrorCode, RequestFrame, ServerHello};
use locust_proto::engine::{ConnId, Engine, Entropy, Parked, Step};
use locust_proto::id::GoalId;
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::store::{Commit, Space, Store, StoreError};

use callers::Conn;
use identity::{Identity, Principals};

/// Every space the node keeps records in, in the order they are loaded.
const SPACES: [Space; 2] = [Space::Identity, Space::Agent];

/// The daemon's state machine over a store `S` and a random source `E`.
pub struct Node<S, E> {
    store: S,
    /// Shared so that planning a request, which only reads the node, can
    /// still draw keys and secrets.
    entropy: RefCell<E>,
    daemon_version: String,
    identity: Identity,
    principals: Principals,
    conns: HashMap<ConnId, Conn>,
    /// Goals whose revision rose since `take_changed` was last called.
    changed: BTreeSet<GoalId>,
    stop: bool,
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Opens the node over `store`: creates the endpoint secret on first
    /// start, records the digest of the owner credential the shell created,
    /// and loads every local record and goal.
    pub fn open(
        mut store: S,
        mut entropy: E,
        owner_credential_digest: [u8; 32],
        daemon_version: String,
        _now_ms: u64,
    ) -> Result<Self, StoreError> {
        let mut first = Commit::default();
        let mut identity = Identity {
            endpoint_secret: [0; 32],
            owner: [0; 32],
            endpoint: None,
        };
        for (key, value) in store.scan(Space::Identity, &[])? {
            identity.absorb(&key, Some(&value))?;
        }
        if identity.endpoint_secret == [0; 32] {
            let mut secret = [0u8; 32];
            entropy.fill(&mut secret);
            first.local.push(Identity::secret_write(&secret));
        }
        if identity.owner != owner_credential_digest {
            first
                .local
                .push(Identity::owner_write(&owner_credential_digest));
        }
        if !first.local.is_empty() {
            store.commit(&first)?;
        }

        let mut node = Self {
            store,
            entropy: RefCell::new(entropy),
            daemon_version,
            identity,
            principals: Principals::default(),
            conns: HashMap::new(),
            changed: BTreeSet::new(),
            stop: false,
        };
        for space in SPACES {
            for (key, value) in node.store.scan(space, &[])? {
                node.absorb(space, &key, Some(&value))?;
            }
        }
        Ok(node)
    }

    /// Fills `bytes` from the shell's random source.
    fn random<const N: usize>(&self) -> [u8; N] {
        let mut bytes = [0u8; N];
        self.entropy.borrow_mut().fill(&mut bytes);
        bytes
    }

    /// Updates memory from one committed local write. The same path loads
    /// the records at start, so memory cannot disagree with the store.
    fn absorb(&mut self, space: Space, key: &[u8], value: Option<&[u8]>) -> Result<(), StoreError> {
        match space {
            Space::Identity => self.identity.absorb(key, value),
            Space::Agent => self.principals.absorb(key, value),
            _ => Ok(()),
        }
    }

    /// One entry per goal and local principal in it; only `principal`'s own
    /// when one is named.
    fn goal_summaries(
        &self,
        _principal: Option<locust_proto::id::PublicKey>,
    ) -> Vec<locust_proto::api::GoalSummary> {
        Vec::new()
    }

    fn refuse(&self, code: ErrorCode, message: &'static str) -> ServerHello {
        ServerHello::Refused {
            error: ApiError::new(code, message),
            api_version: API_VERSION,
            daemon_version: self.daemon_version.clone(),
        }
    }
}

impl<S: Store, E: Entropy> Engine for Node<S, E> {
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, _now_ms: u64) -> ServerHello {
        if hello.api_version != API_VERSION {
            return self.refuse(
                ErrorCode::UnsupportedVersion,
                "the client speaks another API version",
            );
        }
        let digest = hello.credential.digest();
        let caller = if digest == self.identity.owner {
            Caller::Owner
        } else {
            match self.principals.credential(&digest) {
                Some(caller @ (Caller::Agent(key) | Caller::Viewer(key)))
                    if self.principals.active(&key).is_some() =>
                {
                    caller
                }
                _ => return self.refuse(ErrorCode::Denied, "the credential is unknown or revoked"),
            }
        };
        if matches!(caller, Caller::Viewer(_)) && hello.session.is_some() {
            return self.refuse(ErrorCode::Invalid, "a viewer is never an execution session");
        }
        self.conns.insert(
            conn,
            Conn {
                caller,
                session: hello.session.map(|secret| secret.instance()),
                parked: None,
            },
        );
        ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: self.daemon_version.clone(),
            caller,
            max_blob_bytes: MAX_BLOB_BYTES as u64,
        }
    }

    fn request(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        self.handle(conn, frame, now_ms)
    }

    fn resume(&mut self, conn: ConnId, parked: &Parked, timed_out: bool, now_ms: u64) -> Step {
        self.resume_wait(conn, parked, timed_out, now_ms)
    }

    fn take_changed(&mut self) -> Vec<GoalId> {
        std::mem::take(&mut self.changed).into_iter().collect()
    }

    fn disconnect(&mut self, conn: ConnId) {
        self.conns.remove(&conn);
    }

    fn stop_requested(&self) -> bool {
        self.stop
    }
}
