//! Daemon-wide requests: status, stopping, and enrolling principals.

use locust_proto::api::{ApiError, DaemonStatus, ErrorCode, Grants, Response};
use locust_proto::crypto::Keypair;
use locust_proto::engine::Entropy;
use locust_proto::store::Store;

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::identity::{PrincipalRecord, Principals};

impl<S: Store, E: Entropy> Node<S, E> {
    /// `status`: the daemon, and as much of its principals and goals as the
    /// caller may see.
    pub(super) fn status(&self, actor: &Actor) -> Plan {
        let agents = match actor.principal {
            None => self.principals.iter().map(|found| found.view()).collect(),
            Some(key) => self
                .principals
                .get(&key)
                .map(|found| found.view())
                .into_iter()
                .collect(),
        };
        answer(Response::Status(DaemonStatus {
            daemon_version: self.daemon_version.clone(),
            endpoint: self
                .identity
                .endpoint
                .as_ref()
                .map(|record| record.endpoint),
            agents,
            goals: self.goal_summaries(actor.principal),
        }))
    }

    /// `daemon.stop`: answered first, then the shell stops the daemon.
    pub(super) fn shutdown(&self) -> Plan {
        Ok(Planned {
            response: Response::Done,
            tx: Tx {
                stop: true,
                ..Tx::none()
            },
        })
    }

    /// `agent.enroll`: a new principal under a credential the client already
    /// stored. The daemon generates and keeps the signing key.
    pub(super) fn agent_enroll(&self, name: String, grants: Grants, credential: [u8; 32]) -> Plan {
        if let Some(existing) = self.principals.by_name(&name) {
            return if existing.record.credential == credential {
                answer(Response::AgentEnrolled {
                    agent: existing.key.public(),
                })
            } else {
                Err(ApiError::new(
                    ErrorCode::Conflict,
                    "the name is taken by a principal with another credential",
                ))
            };
        }
        if credential == self.identity.owner || self.principals.credential(&credential).is_some() {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "the credential already names something else",
            ));
        }
        let key = Keypair::from_seed(self.random());
        let agent = key.public();
        let record = PrincipalRecord {
            name,
            seed: key.seed(),
            credential,
            grants,
            revoked: false,
        };
        let mut tx = Tx::none();
        tx.local(Principals::principal_write(&agent, &record));
        Ok(Planned {
            response: Response::AgentEnrolled { agent },
            tx,
        })
    }
}
