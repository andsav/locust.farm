//! The transport's side of the node, including durable invitation redemption.
use locust_proto::api::PeerView;
use locust_proto::engine::{Entropy, PeerEngine, PeerInput, PeerOutput, PeerTime};
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
            .members
            .values()
            .filter(|member| member.is_active())
            .map(|member| &member.endpoint)
            .filter(|endpoint| Some(**endpoint) != own)
            .peekable();
        peers.peek().is_none() || peers.any(|endpoint| self.peer_view(endpoint).connected)
    }
}

impl<S: Store, E: Entropy> PeerEngine for Node<S, E> {
    fn peer_readable(&self, exchange: locust_proto::engine::ExchangeId) -> bool {
        self.peer_driver.readable(exchange)
    }

    fn endpoint_secret(&self) -> [u8; 32] {
        self.identity.endpoint_secret
    }
    fn peer(&mut self, input: PeerInput, time: PeerTime, out: &mut Vec<PeerOutput>) {
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
        driver.handle(self, input, time, out);
        self.peer_driver = driver;
        self.replica_goal = None;
    }
}

impl<S: Store, E: Entropy> Host for Node<S, E> {
    fn halt_proofs(&self) -> Vec<(GoalId, EndpointId, [locust_proto::event::WireEvent; 2])> {
        let own = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint);
        let mut proofs = Vec::new();
        for (goal, entry) in &self.goals {
            for author in entry.goal.authors() {
                let Some(seq) = entry.goal.fork_point(author) else {
                    continue;
                };
                let mut conflicts = entry
                    .goal
                    .points(author)
                    .iter()
                    .filter(|point| point.seq == seq)
                    .filter_map(|point| entry.goal.event(&point.id));
                let (Some(first), Some(second)) = (conflicts.next(), conflicts.next()) else {
                    continue;
                };
                for endpoint in historical_endpoints(entry) {
                    if Some(endpoint) != own {
                        proofs.push((*goal, endpoint, [first.to_wire(), second.to_wire()]));
                    }
                }
            }
        }
        proofs
    }

    fn accepts_halt_proof(&self, goal: &GoalId, remote: &EndpointId) -> bool {
        !self.failed
            && self.goals.get(goal).is_some_and(|entry| {
                historical_endpoints(entry).contains(remote)
                    || entry
                        .local
                        .joins
                        .values()
                        .any(|join| join.endpoint == *remote)
            })
    }

    fn receive_halt_proof(
        &mut self,
        goal: &GoalId,
        remote: &EndpointId,
        proof: [locust_proto::event::WireEvent; 2],
    ) -> Result<(), Refusal> {
        if !self.accepts_halt_proof(goal, remote) {
            return Err(Refusal::NotAMember);
        }
        let entry = self.goals.get(goal).ok_or(Refusal::NotAMember)?;
        let first =
            locust_proto::event::Event::from_wire(&proof[0]).map_err(|_| Refusal::NotAMember)?;
        let second =
            locust_proto::event::Event::from_wire(&proof[1]).map_err(|_| Refusal::NotAMember)?;
        if first.header().goal != *goal
            || second.header().goal != *goal
            || first.header().author != second.header().author
            || !(entry.state().governance == Some(first.header().author)
                || entry.state().members.contains_key(&first.header().author)
                || entry.state().governance.is_some_and(|governance| {
                    entry.goal.points(&governance).iter().any(|point| {
                        entry.goal.event(&point.id).is_some_and(|event| {
                            matches!(event.header().body, Body::MemberAdmitted { member, .. } if member == first.header().author)
                        })
                    })
                })
                || entry.local.joins.values().any(|join| join.governance == first.header().author))
            || first.header().seq != second.header().seq
            || first.id() == second.id()
        {
            return Err(Refusal::NotAMember);
        }
        let mut tx = Tx::none();
        tx.commit.events.extend(
            [first, second]
                .into_iter()
                .filter(|event| !entry.goal.holds(&event.id())),
        );
        if tx.commit.events.is_empty() {
            return Ok(());
        }
        self.land(tx).map_err(|_| Refusal::NotAMember)
    }

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
                    .members
                    .values()
                    .filter(|member| member.is_active())
                    .map(|member| &member.endpoint)
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
            && self.goals.get(goal).is_some_and(|entry| {
                entry
                    .state()
                    .members
                    .values()
                    .any(|member| member.is_active() && member.endpoint == *endpoint)
            })
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
                        request: JoinRequest::sign(
                            *goal,
                            endpoint,
                            join.name.clone(),
                            join.secret,
                            signer,
                        ),
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
        let tx = self.plan_join(remote, request, now_ms)?;
        self.land(tx).map_err(|_| Refusal::InvitationRefused)
    }
    fn take_changed(&mut self) -> Vec<GoalId> {
        std::mem::take(&mut self.outbound).into_iter().collect()
    }
    fn exchange_ended(&mut self, report: Report) {
        let mut tx = Tx::none();
        if report.ended == Ended::Completed {
            tx.local(records::put(
                Space::Goal,
                records::key(b'Y', &[&report.goal.0, &report.endpoint.0]),
                &report.at_ms,
            ));
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
    fn random(&mut self) -> u64 {
        let mut bytes = [0; 8];
        self.entropy.borrow_mut().fill(&mut bytes);
        u64::from_le_bytes(bytes)
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Shared validation for network and same-daemon invitation redemption.
    /// A valid, pending invitation authorizes admission without a local grant.
    pub(super) fn plan_join(
        &self,
        remote: &EndpointId,
        request: &JoinRequest,
        now_ms: u64,
    ) -> Result<Tx, Refusal> {
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
        if invite.goal != request.goal || invite.revoked_ms.is_some() {
            return Err(refused);
        }
        let entry = self.goals.get(&request.goal).ok_or(refused)?;
        if let Some((member, endpoint)) = invite.redeemed {
            return if member == request.member
                && endpoint == *remote
                && entry
                    .state()
                    .members
                    .get(&member)
                    .is_some_and(|member| member.is_active() && member.endpoint == *remote)
            {
                Ok(Tx::none())
            } else {
                Err(refused)
            };
        }
        if !self.hosts(entry)
            || invite.expires_ms.is_some_and(|expires| now_ms >= expires)
            || entry.state().governance != Some(invite.governance)
            || request.member == invite.governance
            || entry.is_member(&request.member)
        {
            return Err(refused);
        }
        let mut tx = Tx::none();
        self.author_alone(
            entry,
            &invite.governance,
            Body::MemberAdmitted {
                member: request.member,
                endpoint: *remote,
                name: request.name.clone(),
                role: invite.role.clone(),
            },
            &mut tx,
        )
        .map_err(|_| refused)?;
        invite.redeemed = Some((request.member, *remote));
        invite.redeemed_ms = Some(now_ms);
        tx.local(records::put(Space::Invite, digest.to_vec(), &invite));
        Ok(tx)
    }
}

/// Signed admission contacts remain eligible for conflict evidence only.
/// This never changes the current member/endpoint projection.
fn historical_endpoints(entry: &super::entry::Entry) -> std::collections::BTreeSet<EndpointId> {
    let Some(governance) = entry.state().governance else {
        return Default::default();
    };
    entry
        .goal
        .points(&governance)
        .iter()
        .filter_map(|point| match entry.goal.event(&point.id)?.header().body {
            Body::MemberAdmitted { endpoint, .. } => Some(endpoint),
            _ => None,
        })
        .collect()
}
