//! Deterministic keys and event builders for tests in this and other crates.
//! Enable with the `testkit` feature; never use these keys outside tests.

use crate::PROTOCOL_VERSION;
use crate::crypto::{ContentKey, Keypair};
use crate::event::{AuthorPoint, Body, CancelOutcome, Doc, Event, Genesis, Header, PayloadRef};
use crate::id::{BlobHash, EndpointId, EventId, GoalId};
use crate::seal;
use crate::store::Blob;

/// A fixed key derived from one byte.
pub fn keypair(n: u8) -> Keypair {
    Keypair::from_seed([n; 32])
}

/// A fixed content key derived from one byte.
pub fn content_key(n: u8) -> ContentKey {
    ContentKey([n; 32])
}

/// Seals `text` for `goal` under [`content_key`]`(1)` at `epoch`, and returns
/// the stored object with the payload reference that names it.
pub fn sealed_payload(goal: &GoalId, epoch: u32, text: &[u8]) -> (PayloadRef, Blob) {
    let sealed =
        seal::seal(goal, epoch, &content_key(1), text).expect("testkit payloads are small");
    let blob = Blob::new(sealed);
    let payload = PayloadRef {
        hash: blob.hash(),
        len: blob.bytes().len() as u32,
        key_epoch: epoch,
    };
    (payload, blob)
}

/// One principal writing a well-formed log: tracks the sequence number and
/// previous event so tests state only what they are about.
pub struct Author {
    /// The principal's signing key, [`keypair`] of the byte it was made from.
    pub key: Keypair,
    next_seq: u64,
    prev: Option<EventId>,
}

impl Author {
    /// A principal with [`keypair`]`(n)` and an empty log.
    pub fn new(n: u8) -> Self {
        Self {
            key: keypair(n),
            next_seq: 0,
            prev: None,
        }
    }

    /// The founding event of a goal this author owns and coordinates. It
    /// admits nobody; see [`Author::found_goal`] for the full founding.
    pub fn genesis(&mut self) -> Event {
        let genesis = Genesis {
            owner: self.key.public(),
            coordinator: self.key.public(),
            salt: [0; 16],
        };
        self.event(genesis.goal_id(), None, Body::Genesis(genesis))
    }

    /// Founds a goal the way the daemon does: the genesis event, then this
    /// author's first decision as coordinator, which admits itself bound to
    /// `endpoint`. The daemon commits both together.
    pub fn found_goal(&mut self, endpoint: EndpointId) -> (Event, Event) {
        let genesis = self.genesis();
        let admission = self.event(
            genesis.header().goal,
            Some(genesis.id()),
            Body::MemberAdmitted {
                member: self.key.public(),
                endpoint,
            },
        );
        (genesis, admission)
    }

    /// The author's next event with no payload and no extra parents.
    pub fn event(&mut self, goal: GoalId, anchor: Option<EventId>, body: Body) -> Event {
        self.event_with(goal, anchor, body, None)
    }

    /// The author's next event with the given payload reference and no extra
    /// parents. `at_ms` is a fixed time plus the sequence number. Panics if
    /// the header fails [`Header::check`].
    pub fn event_with(
        &mut self,
        goal: GoalId,
        anchor: Option<EventId>,
        body: Body,
        payload: Option<PayloadRef>,
    ) -> Event {
        let header = Header {
            version: PROTOCOL_VERSION,
            goal,
            author: self.key.public(),
            seq: self.next_seq,
            prev: self.prev,
            anchor,
            parents: Vec::new(),
            at_ms: 1_790_000_000_000 + self.next_seq,
            payload,
            body,
        };
        let event = Event::sign(header, &self.key).expect("testkit builds valid headers");
        self.next_seq += 1;
        self.prev = Some(event.id());
        event
    }
}

/// One value of every [`Body`] variant, in declaration order, for tests that
/// must cover them all. A new variant belongs at the end of this list.
pub fn every_body() -> Vec<Body> {
    let id = EventId([1; 32]);
    let key = keypair(1).public();
    let hash = BlobHash([2; 32]);
    vec![
        Body::Genesis(Genesis {
            owner: key,
            coordinator: key,
            salt: [0; 16],
        }),
        Body::MemberAdmitted {
            member: key,
            endpoint: EndpointId([3; 32]),
        },
        Body::MemberRemoved {
            member: key,
            last_accepted: Some(AuthorPoint { seq: 1, id }),
        },
        Body::TaskAssigned {
            task: id,
            assignee: key,
            attempt: 1,
        },
        Body::CancelRequested { assignment: id },
        Body::ResultAccepted {
            result: id,
            head: Some(hash),
        },
        Body::ResultRejected { result: id },
        Body::RevisionAccepted { revision: id },
        Body::TaskProposed {
            input: Some(hash),
            depends_on: vec![id],
            deadline_ms: Some(1),
            max_attempts: Some(2),
        },
        Body::AssignmentAccepted { assignment: id },
        Body::AssignmentDeclined { assignment: id },
        Body::Progress { assignment: id },
        Body::ResultSubmitted {
            assignment: id,
            base: Some(hash),
            patch: Some(hash),
            artifacts: vec![hash],
        },
        Body::AttemptFailed { assignment: id },
        Body::CancelAcknowledged {
            cancel: id,
            outcome: CancelOutcome::Stopped,
        },
        Body::Note {
            about: Some(id),
            supersedes: None,
        },
        Body::Revision {
            doc: Doc::Plan,
            base: None,
        },
        Body::LeaveRequested,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_founded_goal_starts_with_the_coordinator_admitting_itself() {
        let mut coordinator = Author::new(1);
        let endpoint = EndpointId([7; 32]);
        let (genesis, admission) = coordinator.found_goal(endpoint);

        let Body::Genesis(record) = genesis.header().body else {
            panic!("the first event is the genesis");
        };
        assert_eq!(record.coordinator, coordinator.key.public());
        let header = admission.header();
        assert_eq!(header.goal, genesis.header().goal);
        assert_eq!(header.author, record.coordinator);
        assert_eq!((header.seq, header.prev), (1, Some(genesis.id())));
        assert_eq!(header.anchor, Some(genesis.id()));
        assert_eq!(
            header.body,
            Body::MemberAdmitted {
                member: record.coordinator,
                endpoint,
            }
        );
        assert!(header.body.is_decision());
    }
}
