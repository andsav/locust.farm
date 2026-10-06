//! The single write path.
//!
//! A request is planned against the node as it is (`&self`), which yields a
//! [`Tx`]: everything the request wants made durable. [`Node::land`] commits
//! it as one store commit and only then updates memory, from the commit's own
//! writes. A failed commit returns the store's error and leaves memory as it
//! was.

use locust_proto::api::{ApiError, Caller, ErrorCode, Membership, Response};
use locust_proto::codec;
use locust_proto::crypto::content_hash;
use locust_proto::engine::Entropy;
use locust_proto::id::{BlobHash, GoalId, IdempotencyKey, PublicKey};
use locust_proto::store::{Commit, LocalWrite, Space, Store};
use serde::{Deserialize, Serialize};

use super::feed::Feed;
use super::{Node, local, records};
use crate::goal::{Goal, Standing};

/// What one request or received frame wants made durable.
#[derive(Debug, Default)]
pub(super) struct Tx {
    pub commit: Commit,
    /// Goals whose revision rises with this commit: their events change, or
    /// a local record that feeds pending work does.
    pub touched: Vec<GoalId>,
    /// Objects finished through the store's streaming interface before landing.
    pub arrived: Vec<BlobHash>,
    /// True when the events were signed here for a request, so each must
    /// take effect or the request fails.
    pub authored: bool,
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
        Caller::Agent(principal) | Caller::Author(principal) => {
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

    /// Makes `tx` durable as one commit and then updates memory from it.
    ///
    /// Events are the one exception to "commit first": a goal is advanced in
    /// memory before the commit, because the feed entries the same commit
    /// must carry are an outcome of applying them. Nothing is released before
    /// the commit returns, and when it fails the goal is rebuilt from the
    /// store, so memory never keeps an uncommitted change.
    pub(super) fn land(&mut self, tx: Tx) -> Result<(), ApiError> {
        let goals: std::collections::BTreeSet<_> = tx
            .touched
            .iter()
            .copied()
            .chain(tx.commit.events.iter().map(|event| event.header().goal))
            .collect();
        self.land_once(tx)?;
        for goal in goals {
            self.drive_flow(goal)?;
        }
        Ok(())
    }

    pub(super) fn land_once(&mut self, mut tx: Tx) -> Result<(), ApiError> {
        let goals: std::collections::BTreeSet<_> = tx
            .touched
            .iter()
            .copied()
            .chain(tx.commit.events.iter().map(|event| event.header().goal))
            .collect();
        // Check local counters before advancing any in-memory projection.
        // A refused transaction must not leave even a temporarily accepted fact.
        let revisions: Vec<_> = goals
            .iter()
            .map(|goal| {
                self.goals
                    .get(goal)
                    .map_or(0, |entry| entry.local.revision)
                    .checked_add(1)
                    .map(|revision| local::revision_write(goal, revision))
                    .ok_or_else(|| {
                        ApiError::new(
                            ErrorCode::Conflict,
                            "the goal revision counter is exhausted",
                        )
                    })
            })
            .collect::<Result<_, _>>()?;
        let mut projections = std::collections::BTreeMap::new();
        for goal in &goals {
            let definitions_changed = self
                .goals
                .get(goal)
                .map(|entry| entry.definitions.changed_by(*goal, &tx.commit, &tx.arrived))
                .transpose()?
                .unwrap_or(true);
            if definitions_changed
                || tx
                    .commit
                    .events
                    .iter()
                    .any(|event| event.header().goal == *goal)
            {
                projections.insert(*goal, definitions_changed);
            }
        }
        let backups: Vec<_> = projections
            .keys()
            .map(|id| {
                (
                    *id,
                    self.goals
                        .get(id)
                        .map(|entry| (entry.goal.clone(), entry.definitions.clone())),
                )
            })
            .collect();
        for (goal, definitions_changed) in projections {
            if let Err(error) = self.advance(goal, definitions_changed, &mut tx) {
                self.rollback(backups);
                return Err(error);
            }
        }
        for goal in &goals {
            self.finish_joins(*goal, &mut tx);
            self.clear_removed(*goal, &mut tx);
            self.project_deliveries(*goal, &mut tx);
        }
        tx.commit.local.extend(revisions);
        if !tx.is_empty() {
            if let Err(error) = self.store.commit(&tx.commit) {
                self.failed = true;
                self.rollback(backups);
                return Err(error.into());
            }
            for write in &tx.commit.local {
                let result = match write {
                    LocalWrite::Put { space, key, value } => self.absorb(*space, key, Some(value)),
                    LocalWrite::Delete { space, key } => self.absorb(*space, key, None),
                };
                if let Err(error) = result {
                    self.failed = true;
                    return Err(error.into());
                }
            }
        }
        for goal in &goals {
            let events: Vec<_> = tx
                .commit
                .events
                .iter()
                .filter(|event| event.header().goal == *goal)
                .cloned()
                .collect();
            if !events.is_empty() {
                self.entry_mut(*goal).note_named(&events);
                self.outbound.insert(*goal);
            }
        }
        if let Err(error) = self.update_blob_index(&tx.commit) {
            self.failed = true;
            return Err(error.into());
        }
        self.changed.extend(tx.touched);
        self.stop |= tx.stop;
        Ok(())
    }

    fn rollback(
        &mut self,
        backups: Vec<(GoalId, Option<(Goal, super::definitions::Definitions)>)>,
    ) {
        for (id, backup) in backups {
            if let Some((goal, definitions)) = backup {
                let entry = self.entry_mut(id);
                entry.goal = goal;
                entry.definitions = definitions;
            } else {
                self.goals.remove(&id);
            }
        }
    }

    /// Applies the events of `tx` to their goal and adds what follows from
    /// that to the same commit: a feed entry for every event judged.
    fn advance(
        &mut self,
        goal: GoalId,
        definitions_changed: bool,
        tx: &mut Tx,
    ) -> Result<(), ApiError> {
        let authored = tx.authored;
        if definitions_changed {
            let entry = self.goals.get(&goal);
            let definitions = entry
                .map(|entry| &entry.definitions)
                .cloned()
                .unwrap_or_default();
            let keys = entry.map(|entry| entry.keys.clone()).unwrap_or_default();
            let updated = definitions.updated(&self.store, goal, &keys, &tx.commit)?;
            self.entry_mut(goal).definitions = updated;
        }
        let entry = self.entry_mut(goal);
        let changes = entry.goal.apply(&tx.commit.events, &entry.definitions);
        if authored
            && let Some(excluded) = tx
                .commit
                .events
                .iter()
                .filter(|event| event.header().goal == goal)
                .find_map(|event| match entry.goal.standing(&event.id()) {
                    Some(Standing::Effective) => None,
                    other => Some(other),
                })
        {
            // The node signs nothing its own copy of the goal would not
            // apply; a check before signing missed this case.
            let reason = match excluded {
                Some(Standing::Excluded(exclusion)) => {
                    exclusion.reason().unwrap_or_else(|| exclusion.name())
                }
                _ => "the event cannot be applied yet",
            };
            return Err(ApiError::new(ErrorCode::Conflict, reason));
        }
        let mut position = entry.feed.len();
        for event in &changes.judged {
            position += 1;
            tx.commit
                .local
                .push(Feed::entry_write(&goal, position, event));
        }
        tx.touch(goal);
        Ok(())
    }

    fn finish_joins(&self, goal: GoalId, tx: &mut Tx) {
        let Some(entry) = self.goals.get(&goal) else {
            return;
        };
        for (principal, join) in &entry.local.joins {
            if entry.is_member(principal)
                && super::requests::invitations::publication_matches(
                    &entry.goal,
                    join.publication.as_ref(),
                )
            {
                tx.commit.local.push(local::join_delete(&goal, principal));
                tx.commit
                    .local
                    .push(local::part_write(&goal, principal, false));
            }
        }
    }

    /// A removal and its local level cleanup land in one store commit. A
    /// later admission therefore starts with only the newly chosen level.
    fn clear_removed(&self, goal: GoalId, tx: &mut Tx) {
        let Some(entry) = self.goals.get(&goal) else {
            return;
        };
        for principal in entry
            .local
            .levels
            .keys()
            .chain(
                entry
                    .local
                    .allowances
                    .keys()
                    .map(|(_, principal)| principal),
            )
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            if entry.membership(&principal) != Some(Membership::Removed)
                || !self.principals.holds(&principal)
            {
                continue;
            }
            if entry.local.levels.contains_key(&principal) {
                tx.local(local::level_delete(&goal, &principal));
            }
            for (task, agent) in entry.local.allowances.keys() {
                if *agent == principal {
                    tx.local(local::allowance_delete(&goal, task, &principal));
                }
            }
        }
    }
}
