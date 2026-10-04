//! The transport's side of the node, including durable invitation redemption.
use locust_proto::api::PeerView;
use locust_proto::engine::{Entropy, PeerEngine, PeerInput, PeerOutput};
use locust_proto::event::Body;
use locust_proto::id::{EndpointId, GoalId};
use locust_proto::invite::JoinRequest;
use locust_proto::store::{Space, Store};
use locust_proto::sync::Refusal;

use super::Node;
use super::commit::Tx;
use super::identity::{EndpointRecord, Identity};
use super::records;
use crate::sync::{Ended, Host, Joining, Replica, Report};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn peer_view(&self, endpoint: &EndpointId) -> PeerView {
        let last_sync_ms = self
            .store
            .get(Space::Peer, &records::key(b's', &[&endpoint.0]))
            .ok()
            .flatten()
            .and_then(|bytes| records::read(&bytes).ok());
        PeerView {
            endpoint: *endpoint,
            connected: self.peer_connections.contains(endpoint),
            last_sync_ms,
        }
    }
    pub(super) fn reachable(&self, goal: &GoalId) -> bool {
        let own = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint);
        let Some(entry) = self.goals.get(goal) else {
            return true;
        };
        let mut peers = entry
            .state()
            .endpoints
            .keys()
            .filter(|endpoint| Some(**endpoint) != own)
            .peekable();
        peers.peek().is_none() || peers.any(|endpoint| self.peer_view(endpoint).connected)
    }
}

impl<S: Store, E: Entropy> PeerEngine for Node<S, E> {
    fn endpoint_secret(&self) -> [u8; 32] {
        self.identity.endpoint_secret
    }
    fn peer(&mut self, input: PeerInput, now_ms: u64, out: &mut Vec<PeerOutput>) {
        if self.failed {
            return;
        }
        if let PeerInput::Endpoint { endpoint, hints } = input {
            let record = EndpointRecord { endpoint, hints };
            if self.identity.endpoint.as_ref() != Some(&record) {
                let mut tx = Tx::none();
                tx.local(Identity::endpoint_write(&record));
                let _ = self.land(tx);
            }
            return;
        }
        if let PeerInput::Connection {
            endpoint,
            connected,
        } = input
        {
            if connected {
                self.peer_connections.insert(endpoint);
            } else {
                self.peer_connections.remove(&endpoint);
            }
            return;
        }
        let mut driver = std::mem::take(&mut self.peer_driver);
        driver.handle(self, input, now_ms, out);
        self.peer_driver = driver;
        self.replica_goal = None;
    }
}

impl<S: Store, E: Entropy> Host for Node<S, E> {
    fn peers(&self) -> Vec<(GoalId, EndpointId)> {
        let own = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint);
        self.goals
            .iter()
            .flat_map(|(goal, entry)| {
                entry
                    .state()
                    .endpoints
                    .keys()
                    .filter(move |endpoint| Some(**endpoint) != own)
                    .map(move |endpoint| (*goal, *endpoint))
            })
            .collect()
    }
    fn hints(&self, endpoint: &EndpointId) -> Vec<String> {
        self.store
            .get(Space::Peer, &endpoint.0)
            .ok()
            .flatten()
            .and_then(|bytes| records::read(&bytes).ok())
            .unwrap_or_default()
    }
    fn speaks_for_member(&self, goal: &GoalId, endpoint: &EndpointId) -> bool {
        !self.failed
            && self
                .goals
                .get(goal)
                .is_some_and(|entry| entry.state().endpoints.contains_key(endpoint))
    }
    fn replica(&mut self, goal: &GoalId) -> Option<&mut dyn Replica> {
        if self.failed || !self.goals.contains_key(goal) {
            return None;
        }
        self.replica_goal = Some(*goal);
        Some(self)
    }
    fn joins(&self) -> Vec<Joining> {
        let Some(endpoint) = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint)
        else {
            return Vec::new();
        };
        self.goals
            .iter()
            .flat_map(|(goal, entry)| {
                entry.local.joins.iter().filter_map(move |(member, join)| {
                    if join.refused {
                        return None;
                    }
                    let signer = self.signer(member).ok()?;
                    Some(Joining {
                        goal: *goal,
                        endpoint: join.endpoint,
                        hints: join.hints.clone(),
                        request: JoinRequest::sign(*goal, endpoint, join.secret, signer),
                    })
                })
            })
            .collect()
    }
    fn join(
        &mut self,
        remote: &EndpointId,
        request: &JoinRequest,
        now_ms: u64,
    ) -> Result<(), Refusal> {
        use super::requests::invitations::InviteRecord;
        let refused = Refusal::InvitationRefused;
        if self.failed || *remote != request.endpoint || !request.verify() {
            return Err(refused);
        }
        let digest = request.secret.digest();
        let bytes = self
            .store
            .get(Space::Invite, &digest)
            .map_err(|_| refused)?
            .ok_or(refused)?;
        let mut invite: InviteRecord = records::read(&bytes).map_err(|_| refused)?;
        if invite.goal != request.goal {
            return Err(refused);
        }
        let entry = self.goals.get(&request.goal).ok_or(refused)?;
        if let Some((member, endpoint)) = invite.redeemed {
            return if member == request.member
                && endpoint == *remote
                && entry.state().members.get(&member) == Some(remote)
            {
                Ok(())
            } else {
                Err(refused)
            };
        }
        if invite.expires_ms.is_some_and(|expires| now_ms >= expires)
            || entry.state().coordinator != Some(invite.coordinator)
            || entry.is_member(&request.member)
        {
            return Err(refused);
        }
        let mut tx = Tx::none();
        self.author(
            entry,
            &invite.coordinator,
            Body::MemberAdmitted {
                member: request.member,
                endpoint: *remote,
            },
            None,
            now_ms,
            &mut tx,
        )
        .map_err(|_| refused)?;
        invite.redeemed = Some((request.member, *remote));
        tx.local(records::put(Space::Invite, digest.to_vec(), &invite));
        self.land(tx).map_err(|_| refused)
    }
    fn take_changed(&mut self) -> Vec<GoalId> {
        std::mem::take(&mut self.outbound).into_iter().collect()
    }
    fn exchange_ended(&mut self, report: Report) {
        let mut tx = Tx::none();
        if report.ended == Ended::Completed {
            tx.local(records::put(
                Space::Peer,
                records::key(b's', &[&report.endpoint.0]),
                &report.at_ms,
            ));
        }
        if report.join
            && matches!(report.ended, Ended::Refused(Refusal::InvitationRefused))
            && let Some(entry) = self.goals.get(&report.goal)
        {
            for (member, join) in &entry.local.joins {
                if join.endpoint == report.endpoint && report.joining_member == Some(*member) {
                    let mut join = join.clone();
                    join.refused = true;
                    tx.local(super::local::join_write(&report.goal, member, &join))
                        .touch(report.goal);
                }
            }
        }
        let _ = self.land(tx);
    }
}
