//! A node stand-in: replicas, who speaks for members, joins and invitations.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{Body, WireEvent};
use locust_proto::id::{EndpointId, EventId, GoalId, PublicKey};
use locust_proto::invite::JoinRequest;
use locust_proto::sync::Refusal;
use locust_proto::testkit::Author;

use super::replica::TestReplica;
use crate::sync::{Ended, Host, Joining, Replica, Report};

pub struct TestHost {
    pub endpoint: EndpointId,
    pub replicas: BTreeMap<GoalId, TestReplica>,
    /// Endpoints that speak for a current member, per goal, this one included.
    pub members: BTreeMap<GoalId, BTreeSet<EndpointId>>,
    pub joins: Vec<Joining>,
    /// Digests of live invitation secrets, per goal.
    pub invites: BTreeMap<GoalId, BTreeSet<[u8; 32]>>,
    /// Keys admitted through a join, with the endpoint bound to them.
    pub admitted: BTreeMap<(GoalId, PublicKey), EndpointId>,
    /// Signs the admission of a joined key, anchored at the given event.
    pub administrator: Option<(Author, EventId)>,
    pub reports: Vec<Report>,
    /// Halt-proof evidence this host would deliver, per (goal, endpoint).
    pub halt_proofs: BTreeMap<(GoalId, EndpointId), [WireEvent; 2]>,
    /// Endpoints allowed to deliver halt proofs to this host, per goal.
    pub halt_accepts: BTreeSet<(GoalId, EndpointId)>,
    /// Halt proofs this host has received and durably held.
    pub received_halt_proofs: Vec<(GoalId, EndpointId, [WireEvent; 2])>,
    /// What [`Host::random`] returns, in turn and then again from the start.
    pub random: Vec<u64>,
    draws: usize,
    seen: BTreeMap<GoalId, u64>,
}

impl TestHost {
    pub fn new(endpoint: u8) -> Self {
        Self {
            endpoint: EndpointId([endpoint; 32]),
            replicas: BTreeMap::new(),
            members: BTreeMap::new(),
            joins: Vec::new(),
            invites: BTreeMap::new(),
            admitted: BTreeMap::new(),
            administrator: None,
            reports: Vec::new(),
            halt_proofs: BTreeMap::new(),
            halt_accepts: BTreeSet::new(),
            received_halt_proofs: Vec::new(),
            random: vec![0],
            draws: 0,
            seen: BTreeMap::new(),
        }
    }

    /// Holds `replica`, with these endpoints speaking for members.
    pub fn hold(&mut self, replica: TestReplica, members: &[EndpointId]) {
        let goal = replica.goal;
        self.members
            .entry(goal)
            .or_default()
            .extend(members.iter().copied());
        self.replicas.insert(goal, replica);
    }

    pub fn replica_mut(&mut self, goal: &GoalId) -> &mut TestReplica {
        self.replicas.get_mut(goal).unwrap()
    }

    pub fn ids(&self, goal: &GoalId) -> BTreeSet<EventId> {
        self.replicas
            .get(goal)
            .map(TestReplica::ids)
            .unwrap_or_default()
    }
}

impl Host for TestHost {
    fn peers(&self) -> Vec<(GoalId, EndpointId)> {
        self.replicas
            .keys()
            .flat_map(|goal| {
                self.members
                    .get(goal)
                    .into_iter()
                    .flatten()
                    .filter(|endpoint| **endpoint != self.endpoint)
                    .map(move |endpoint| (*goal, *endpoint))
            })
            .collect()
    }

    fn hints(&self, endpoint: &EndpointId) -> Vec<String> {
        vec![format!("test:{}", endpoint.0[0])]
    }

    fn halt_proofs(&self) -> Vec<(GoalId, EndpointId, [WireEvent; 2])> {
        self.halt_proofs
            .iter()
            .map(|((goal, endpoint), proof)| (*goal, *endpoint, proof.clone()))
            .collect()
    }

    fn accepts_halt_proof(&self, goal: &GoalId, remote: &EndpointId) -> bool {
        self.halt_accepts.contains(&(*goal, *remote))
    }

    fn receive_halt_proof(
        &mut self,
        goal: &GoalId,
        remote: &EndpointId,
        proof: [WireEvent; 2],
    ) -> Result<(), Refusal> {
        if !self.accepts_halt_proof(goal, remote) {
            return Err(Refusal::NotAMember);
        }
        self.received_halt_proofs.push((*goal, *remote, proof));
        Ok(())
    }

    fn speaks_for_member(&self, goal: &GoalId, endpoint: &EndpointId) -> bool {
        self.replicas.contains_key(goal)
            && self
                .members
                .get(goal)
                .is_some_and(|members| members.contains(endpoint))
    }

    fn replica(&mut self, goal: &GoalId) -> Option<&mut dyn Replica> {
        self.replicas
            .get_mut(goal)
            .map(|replica| replica as &mut dyn Replica)
    }

    fn joins(&self) -> Vec<Joining> {
        self.joins.clone()
    }

    fn join(
        &mut self,
        remote: &EndpointId,
        request: &JoinRequest,
        _now_ms: u64,
    ) -> Result<(), Refusal> {
        let goal = request.goal;
        if !request.verify() || request.endpoint != *remote {
            return Err(Refusal::InvitationRefused);
        }
        if self.admitted.get(&(goal, request.member)) == Some(remote) {
            return Ok(());
        }
        let digest = request.secret.digest();
        if !self
            .invites
            .get_mut(&goal)
            .is_some_and(|live| live.remove(&digest))
        {
            return Err(Refusal::InvitationRefused);
        }
        self.admitted.insert((goal, request.member), *remote);
        self.members.entry(goal).or_default().insert(*remote);
        if let Some((author, anchor)) = &mut self.administrator {
            let admission = author.event(
                goal,
                Some(*anchor),
                Body::MemberAdmitted {
                    member: request.member,
                    endpoint: *remote,
                },
            );
            self.replicas.get_mut(&goal).unwrap().insert(&[admission]);
        }
        Ok(())
    }

    fn take_changed(&mut self) -> Vec<GoalId> {
        let mut changed = Vec::new();
        for (goal, replica) in &self.replicas {
            let seen = self.seen.entry(*goal).or_default();
            if *seen != replica.revision {
                *seen = replica.revision;
                changed.push(*goal);
            }
        }
        changed
    }

    /// A join is over once presented successfully (the inviter becomes a
    /// peer) or refused.
    fn exchange_ended(&mut self, report: Report) {
        if report.join {
            match report.ended {
                Ended::Completed => {
                    self.joins.retain(|join| join.goal != report.goal);
                    self.members
                        .entry(report.goal)
                        .or_default()
                        .extend([report.endpoint, self.endpoint]);
                }
                Ended::Refused(Refusal::InvitationRefused) => {
                    self.joins.retain(|join| join.goal != report.goal);
                }
                _ => {}
            }
        }
        self.reports.push(report);
    }

    fn random(&mut self) -> u64 {
        let value = self.random[self.draws % self.random.len()];
        self.draws += 1;
        value
    }
}
