//! In-memory tests of the exchange machines and the driver: a minimal
//! replica over `MemStore`, a host standing in for the node, and a network
//! that carries outputs between drivers.

mod content;
mod convergence;
mod driver;
mod host;
mod machines;
mod net;
mod replica;

use locust_proto::event::{Body, Event};
use locust_proto::id::{EndpointId, EventId, GoalId};
use locust_proto::testkit::Author;

use host::TestHost;
use replica::TestReplica;

pub fn endpoint(n: u8) -> EndpointId {
    EndpointId([n; 32])
}

pub fn contribution() -> Body {
    Body::ContributionPublished {
        context: locust_proto::event::Context {
            scope: locust_proto::event::Scope::Goal,
            round: locust_proto::id::EventId([1; 32]),
        },
        attempt: None,
        sources: Vec::new(),
        artifacts: vec![],
    }
}

/// A goal founded by author 1, which wrote the genesis.
pub struct Founded {
    pub goal: GoalId,
    pub genesis: Event,
    pub owner: Author,
}

impl Founded {
    pub fn new() -> Self {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        Self {
            goal: genesis.header().goal,
            genesis,
            owner,
        }
    }

    pub fn anchor(&self) -> Option<EventId> {
        Some(self.genesis.id())
    }

    /// `count` notes by `author`, anchored at the genesis.
    pub fn notes(&self, author: &mut Author, count: usize) -> Vec<Event> {
        (0..count)
            .map(|_| author.event(self.goal, self.anchor(), contribution()))
            .collect()
    }

    /// A replica holding the genesis and `events`.
    pub fn replica(&self, events: &[Event]) -> TestReplica {
        let mut replica = TestReplica::new(self.goal);
        replica.insert(std::slice::from_ref(&self.genesis));
        replica.insert(events);
        replica
    }
}

/// A host at endpoint `n` holding `replica`, with `members` speaking for
/// members of its goal.
pub fn host(n: u8, replica: TestReplica, members: &[u8]) -> TestHost {
    let mut host = TestHost::new(n);
    let members: Vec<EndpointId> = members.iter().copied().map(endpoint).collect();
    host.hold(replica, &members);
    host
}
