//! Deterministic keys and event builders for tests in this and other crates.
//! Enable with the `testkit` feature; never use these keys outside tests.

use crate::PROTOCOL_VERSION;
use crate::crypto::Keypair;
use crate::event::{Body, Event, Genesis, Header, PayloadRef};
use crate::id::{EventId, GoalId};

/// A fixed key derived from one byte.
pub fn keypair(n: u8) -> Keypair {
    Keypair::from_seed([n; 32])
}

/// One principal writing a well-formed log: tracks the sequence number and
/// previous event so tests state only what they are about.
pub struct Author {
    pub key: Keypair,
    next_seq: u64,
    prev: Option<EventId>,
}

impl Author {
    pub fn new(n: u8) -> Self {
        Self {
            key: keypair(n),
            next_seq: 0,
            prev: None,
        }
    }

    /// The founding event of a goal this author owns and coordinates.
    pub fn genesis(&mut self) -> Event {
        let genesis = Genesis {
            owner: self.key.public(),
            coordinator: self.key.public(),
            salt: [0; 16],
        };
        self.event(genesis.goal_id(), None, Body::Genesis(genesis))
    }

    /// The author's next event with no payload and no extra parents.
    pub fn event(&mut self, goal: GoalId, anchor: Option<EventId>, body: Body) -> Event {
        self.event_with(goal, anchor, body, None)
    }

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
