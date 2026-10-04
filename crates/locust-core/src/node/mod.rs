//! The daemon's state machine: local requests and peer exchanges over one store.
//!
//! [`Node`] owns the store, the daemon's identity, the enrolled principals
//! and every goal, and is the only writer. One request or one received frame
//! produces at most one [`Commit`](locust_proto::store::Commit); what the
//! node remembers is then updated from that commit's own writes
//! (`Node::absorb`), so memory is always what a restart would load.
//!
//! Module map: `identity` (who the daemon is, its principals), `callers`
//! (connections, who a request acts as), `sessions` (sessions and claims),
//! `entry`, `local` and `feed` (one goal as the node holds it), `authoring`
//! (signing events), `commit` (the single write path), `views` (what reads
//! answer with), `requests` (one module per family of operations), `peers`
//! (the transport's side).

mod access;
mod authoring;
mod callers;
mod commit;
mod content_graph;
mod definitions;
mod delivery;
mod entry;
mod feed;
mod flow;
mod identity;
mod local;
mod peers;
mod records;
mod replica;
mod requests;
mod sessions;
mod views;

#[cfg(test)]
mod sim;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use locust_proto::API_VERSION;
use locust_proto::api::{ApiError, Caller, ClientHello, ErrorCode, RequestFrame, ServerHello};
use locust_proto::engine::{ConnId, Engine, Entropy, Parked, Step};
use locust_proto::id::GoalId;
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::store::{Commit, Space, Store, StoreError};

use crate::goal::Goal;
use callers::Conn;
use entry::Entry;
use identity::{Identity, Principals};
use sessions::Sessions;

/// Every space whose records the node keeps in memory, in the order they
/// are loaded.
const SPACES: [Space; 8] = [
    Space::Identity,
    Space::Agent,
    Space::Session,
    Space::Goal,
    Space::Key,
    Space::Claim,
    Space::Cursor,
    Space::Pending,
];

/// The daemon's state machine over a store `S` and a random source `E`.
pub struct Node<S, E> {
    store: S,
    /// Shared so that planning a request, which only reads the node, can
    /// still draw keys and secrets.
    entropy: RefCell<E>,
    daemon_version: String,
    identity: Identity,
    principals: Principals,
    sessions: Sessions,
    /// Every goal held or being joined.
    goals: BTreeMap<GoalId, Entry>,
    conns: HashMap<ConnId, Conn>,
    /// Goals whose revision rose since `take_changed` was last called.
    changed: BTreeSet<GoalId>,
    /// Goals whose events changed since the peer driver last asked.
    outbound: BTreeSet<GoalId>,
    /// Set when a failed commit could not be undone in memory: the node
    /// answers nothing more until it is reopened.
    peer_driver: crate::sync::Driver,
    peer_connections: BTreeSet<locust_proto::id::EndpointId>,
    replica_goal: Option<GoalId>,
    blob_index: content_graph::BlobIndex,
    failed: bool,
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
            sessions: Sessions::default(),
            goals: BTreeMap::new(),
            conns: HashMap::new(),
            changed: BTreeSet::new(),
            outbound: BTreeSet::new(),
            peer_driver: crate::sync::Driver::default(),
            peer_connections: BTreeSet::new(),
            replica_goal: None,
            blob_index: content_graph::BlobIndex::default(),
            failed: false,
            stop: false,
        };
        for id in node.store.goals()? {
            node.goals.insert(id, Entry::new(Goal::new(id)));
        }
        for space in SPACES {
            for (key, value) in node.store.scan(space, &[])? {
                node.absorb(space, &key, Some(&value))?;
            }
        }
        let ids: Vec<_> = node.goals.keys().copied().collect();
        for id in ids {
            let definitions = definitions::Definitions::load(
                &node.store,
                id,
                &node.goals[&id].keys,
                &Commit::default(),
            )?;
            let goal = Goal::load(&node.store, id, &definitions)?;
            let entry = node.goals.get_mut(&id).expect("known goal");
            entry.goal = goal;
            entry.definitions = definitions;
            let events: Vec<_> = entry
                .goal
                .authors()
                .flat_map(|author| entry.goal.points(author))
                .filter_map(|point| entry.goal.event(&point.id).cloned())
                .collect();
            entry.note_named(&events);
        }
        node.rebuild_blob_index()?;
        let ids: Vec<_> = node.goals.keys().copied().collect();
        for id in ids {
            let mut tx = commit::Tx::none();
            node.project_deliveries(id, &mut tx);
            node.land_once(tx)
                .map_err(|error| StoreError::Failed(error.to_string()))?;
            node.drive_flow(id)
                .map_err(|error| StoreError::Failed(error.to_string()))?;
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
            Space::Session => self.sessions.absorb(key, value),
            Space::Goal => {
                let goal = local::goal_of(key)?;
                self.entry_mut(goal).local.absorb(key, value)
            }
            Space::Key => {
                let (goal, epoch) = entry::key_subject(key)?;
                if let Some(value) = value {
                    let content_key = records::read(value)?;
                    self.entry_mut(goal).keys.insert(epoch, content_key);
                } else {
                    self.entry_mut(goal).keys.remove(&epoch);
                }
                Ok(())
            }
            Space::Claim => {
                let (goal, assignment) = sessions::claim_subject(key)?;
                let claims = &mut self.entry_mut(goal).claims;
                match value {
                    Some(value) => {
                        claims.insert(assignment, records::read(value)?);
                    }
                    None => {
                        claims.remove(&assignment);
                    }
                }
                Ok(())
            }
            Space::Pending => {
                let (goal, effect, recipient) = delivery::subject(key)?;
                let records = &mut self.entry_mut(goal).deliveries;
                match value {
                    Some(value) => {
                        records.insert((effect, recipient), records::read(value)?);
                    }
                    None => {
                        records.remove(&(effect, recipient));
                    }
                }
                Ok(())
            }
            Space::Cursor => match value {
                Some(value) if feed::is_entry(key) => {
                    let goal = feed::goal_of(key)?;
                    self.entry_mut(goal).feed.absorb(key, value)
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// The entry of `goal`, created with nothing held when a local record
    /// is the first thing known about it (a join in progress).
    fn entry_mut(&mut self, goal: GoalId) -> &mut Entry {
        self.goals
            .entry(goal)
            .or_insert_with(|| Entry::new(Goal::new(goal)))
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
                Some(caller @ (Caller::Agent(key) | Caller::Viewer(key) | Caller::Author(key)))
                    if self.principals.active(&key).is_some() =>
                {
                    caller
                }
                _ => return self.refuse(ErrorCode::Denied, "the credential is unknown or revoked"),
            }
        };
        if matches!(caller, Caller::Viewer(_) | Caller::Author(_)) && hello.session.is_some() {
            return self.refuse(
                ErrorCode::Invalid,
                "this credential cannot represent an execution session",
            );
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

    fn failure(&self) -> Option<locust_proto::api::ApiError> {
        self.failed.then(|| {
            locust_proto::api::ApiError::new(
                ErrorCode::Internal,
                "storage failed; restart the daemon",
            )
        })
    }

    fn stop_requested(&self) -> bool {
        self.stop || self.failed
    }
}
