//! One administrator stream and authenticated membership tenures. Work decisions
//! are deliberately absent: a pending work scope cannot stop this chain.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{AuthorPoint, Body, Event, RulesBinding};
use locust_proto::id::{EventId, PublicKey};

use super::DefinitionLookup;
use super::history::History;
use super::standing::{Dependency, Exclusion, Halt, Standing, Waiting};
use super::state::{BoundRules, Member, State};

#[derive(Clone, Debug)]
pub(super) struct Tenure {
    pub admission: EventId,
    pub principal: PublicKey,
    pub until: Option<usize>,
    pub cutoff: Option<AuthorPoint>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Snapshot {
    pub epoch: u32,
    pub rules: Option<EventId>,
    pub members: BTreeMap<PublicKey, EventId>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Chain {
    authorization: RefCell<BTreeMap<EventId, (Standing, BTreeSet<Dependency>)>>,
    pub order: Vec<EventId>,
    pub snapshots: BTreeMap<EventId, Snapshot>,
    pub positions: BTreeMap<EventId, usize>,
    pub tenures: BTreeMap<EventId, Tenure>,
    pub standings: BTreeMap<EventId, Standing>,
    pub state: State,
    pub halt: Option<Halt>,
    pub missing: BTreeSet<Dependency>,
}

impl Chain {
    pub fn build<D: DefinitionLookup + ?Sized>(history: &History, definitions: &D) -> Self {
        let mut chain = Self::default();
        let Some(administrator) = history.administrator else {
            return chain;
        };
        chain.state.administrator = Some(administrator);
        let Some(log) = history.log(&administrator) else {
            return chain;
        };
        let mut snapshot = Snapshot::default();
        let mut initial_definition = None;
        for &slot in &log.slots[..log.usable] {
            let event = &history.events[slot];
            if !event.header().body.is_governance() {
                continue;
            }
            if event.header().anchor != chain.order.last().copied() {
                chain.halt = Some(Halt::BrokenChain { event: event.id() });
                chain
                    .standings
                    .insert(event.id(), Standing::Excluded(Exclusion::AfterHalt));
                break;
            }
            let position = chain.order.len();
            let mut status = Standing::Effective;
            let payload_epoch = if matches!(event.header().body, Body::MemberRemoved { .. }) {
                snapshot.epoch.checked_add(1)
            } else {
                Some(snapshot.epoch)
            };
            if payload_epoch.is_none()
                || event
                    .header()
                    .payload
                    .is_some_and(|payload| Some(payload.key_epoch) != payload_epoch)
            {
                status = Standing::Excluded(Exclusion::BadEpoch);
            } else {
                match &event.header().body {
                    Body::Genesis(genesis) => {
                        initial_definition = Some(genesis.definition);
                    }
                    Body::MemberAdmitted { member, endpoint } => {
                        if snapshot.members.contains_key(member) {
                            status = Standing::Excluded(Exclusion::Precondition(
                                "member is already admitted",
                            ));
                        } else {
                            snapshot.members.insert(*member, event.id());
                            chain.tenures.insert(
                                event.id(),
                                Tenure {
                                    admission: event.id(),
                                    principal: *member,
                                    until: None,
                                    cutoff: None,
                                },
                            );
                            chain.state.members.insert(
                                *member,
                                Member {
                                    principal: *member,
                                    endpoint: *endpoint,
                                    admission: event.id(),
                                    removed: None,
                                    read_epoch: snapshot.epoch,
                                },
                            );
                        }
                    }
                    Body::MemberRemoved {
                        member,
                        admission,
                        last_accepted,
                    } => {
                        if snapshot.members.get(member) != Some(admission) {
                            status = Standing::Excluded(Exclusion::Precondition(
                                "removal does not name the current admission",
                            ));
                        } else {
                            snapshot.members.remove(member);
                            let tenure = chain
                                .tenures
                                .get_mut(admission)
                                .expect("current admission exists");
                            tenure.until = Some(position);
                            tenure.cutoff = *last_accepted;
                            if let Some(member) = chain.state.members.get_mut(member) {
                                member.removed = Some(event.id());
                                member.read_epoch = snapshot.epoch;
                            }
                            snapshot.epoch += 1;
                            for member in chain
                                .state
                                .members
                                .values_mut()
                                .filter(|member| member.is_active())
                            {
                                member.read_epoch = snapshot.epoch;
                            }
                        }
                    }
                    Body::RulesBound { expected, binding } => {
                        if *expected != snapshot.rules {
                            status = Standing::Excluded(Exclusion::Precondition(
                                "rules revision compare-and-swap failed",
                            ));
                        } else if expected.is_none()
                            && initial_definition != Some(binding.definition.semantic)
                        {
                            status = Standing::Excluded(Exclusion::Precondition(
                                "first definition differs from genesis",
                            ));
                        } else if binding.definition.object.key_epoch != snapshot.epoch {
                            status = Standing::Excluded(Exclusion::BadEpoch);
                        } else {
                            status = validate_binding(
                                binding,
                                &snapshot,
                                definitions,
                                &mut chain.missing,
                            );
                            // A missing definition cannot restore the previous rules. Its pinned
                            // revision remains discoverable and work waits for validation.
                            if !matches!(status, Standing::Excluded(_)) {
                                snapshot.rules = Some(event.id());
                                chain.state.rules.insert(
                                    event.id(),
                                    BoundRules {
                                        id: event.id(),
                                        binding: binding.clone(),
                                    },
                                );
                            }
                        }
                    }
                    Body::PublicationSet(set) => {
                        if set.policy.validate().is_err()
                            || set.farm_id != locust_proto::farm::FarmId::from_key(set.upload_key)
                        {
                            status = Standing::Excluded(Exclusion::Precondition(
                                "invalid publication policy or identity",
                            ));
                        } else {
                            chain.state.publication = Some((event.id(), set.clone()));
                        }
                    }
                    Body::TaskRevised { .. } => {
                        // Exact task/round authorization is checked by the shared work evaluator.
                    }
                    _ => unreachable!("governance classified exhaustively"),
                }
            }
            chain.positions.insert(event.id(), position);
            chain.snapshots.insert(event.id(), snapshot.clone());
            chain.order.push(event.id());
            chain.standings.insert(event.id(), status);
            chain.state.head = Some(event.id());
            chain.state.epoch = snapshot.epoch;
            chain.state.current_rules = snapshot.rules;
        }
        if chain.halt.is_none()
            && let Some(seq) = log.fork
        {
            chain.halt = Some(Halt::Fork {
                seq,
                events: log
                    .points
                    .iter()
                    .filter(|point| point.seq == seq)
                    .map(|point| point.id)
                    .collect(),
            });
        }
        chain
    }

    pub fn position(&self, id: &EventId) -> Option<usize> {
        self.positions.get(id).copied()
    }
    pub fn snapshot(&self, id: &EventId) -> Option<&Snapshot> {
        self.snapshots.get(id)
    }

    pub fn tenure_at(&self, author: &PublicKey, anchor: EventId) -> Option<&Tenure> {
        let admission = self.snapshot(&anchor)?.members.get(author)?;
        self.tenures.get(admission)
    }

    /// Validates the exact same-author cutoff branch. Missing ancestry waits;
    /// a malformed cutoff preserves no old events and never reopens membership.
    pub fn cutoff(
        &self,
        history: &History,
        tenure: &Tenure,
        event: EventId,
    ) -> Result<bool, EventId> {
        if tenure.until.is_none() {
            return Ok(true);
        }
        let Some(point) = tenure.cutoff else {
            return Ok(false);
        };
        let mut next = Some(point.id);
        let mut seq = point.seq;
        let mut seen = BTreeSet::new();
        let mut found = false;
        while let Some(id) = next {
            if !seen.insert(id) {
                return Ok(false);
            }
            let Some(ancestor) = history.get(&id) else {
                return Err(id);
            };
            if ancestor.header().author != tenure.principal || ancestor.header().seq != seq {
                return Ok(false);
            }
            if id == point.id {
                let Some(anchor) = ancestor.header().anchor else {
                    return Ok(false);
                };
                if self
                    .tenure_at(&tenure.principal, anchor)
                    .map(|t| t.admission)
                    != Some(tenure.admission)
                {
                    return Ok(false);
                }
            }
            found |= id == event;
            next = ancestor.header().prev;
            if next.is_some() {
                let Some(before) = seq.checked_sub(1) else {
                    return Ok(false);
                };
                seq = before;
            }
        }
        Ok(found)
    }

    pub fn authorize(
        &self,
        history: &History,
        event: &Event,
        pins: Option<&super::commitments::EventSet>,
        missing: &mut BTreeSet<Dependency>,
    ) -> Standing {
        let cached = self.authorization.borrow().get(&event.id()).cloned();
        let (base, dependencies) = cached.unwrap_or_else(|| {
            let mut dependencies = BTreeSet::new();
            let base = self.authorize_base(history, event, &mut dependencies);
            self.authorization
                .borrow_mut()
                .insert(event.id(), (base, dependencies.clone()));
            (base, dependencies)
        });
        missing.extend(dependencies);
        let h = event.header();
        if base != Standing::Effective || h.body.is_governance() {
            return base;
        }
        let cutoff = h
            .anchor
            .and_then(|anchor| self.tenure_at(&h.author, anchor))
            .is_some_and(|tenure| tenure.until.is_some());
        let log = history.log(&h.author).expect("held author has a log");
        let usable = log.contains_usable(AuthorPoint {
            seq: h.seq,
            id: event.id(),
        });
        if !usable
            && !cutoff
            && !pins.is_some_and(|pins| {
                history
                    .slot(&event.id())
                    .is_some_and(|slot| pins.contains(slot))
            })
        {
            return Standing::Pending(if log.fork.is_some() {
                Waiting::ForkProof
            } else {
                Waiting::Predecessor
            });
        }
        Standing::Effective
    }
    fn authorize_base(
        &self,
        history: &History,
        event: &Event,
        missing: &mut BTreeSet<Dependency>,
    ) -> Standing {
        let h = event.header();
        if h.body.is_governance() {
            return if Some(h.author) != self.state.administrator {
                Standing::Excluded(Exclusion::NotAdministrator)
            } else {
                self.standings
                    .get(&event.id())
                    .copied()
                    .unwrap_or(Standing::Excluded(Exclusion::AfterHalt))
            };
        }
        let Some(anchor) = h.anchor else {
            return Standing::Excluded(Exclusion::Precondition("missing governance anchor"));
        };
        let Some(snapshot) = self.snapshot(&anchor) else {
            missing.insert(Dependency::Event(anchor));
            return Standing::Pending(Waiting::Anchor);
        };
        if h.payload
            .is_some_and(|payload| payload.key_epoch != snapshot.epoch)
        {
            return Standing::Excluded(Exclusion::BadEpoch);
        }
        let Some(tenure) = self.tenure_at(&h.author, anchor) else {
            return Standing::Excluded(Exclusion::NotAMember);
        };
        match self.cutoff(history, tenure, event.id()) {
            Ok(false) => return Standing::Excluded(Exclusion::PastRemoval),
            Ok(true) => {}
            Err(id) => {
                missing.insert(Dependency::Event(id));
                return Standing::Pending(Waiting::Reference);
            }
        };
        // Check full author ancestry and monotonic governance anchoring even when
        // a scoped proof bypasses the ordinary usable-prefix exclusion.
        let mut id = event.id();
        let mut seq = h.seq;
        let mut reach = self.position(&anchor).unwrap();
        loop {
            let Some(current) = history.get(&id) else {
                missing.insert(Dependency::Event(id));
                return Standing::Pending(Waiting::Predecessor);
            };
            let ch = current.header();
            if ch.author != h.author || ch.seq != seq {
                return Standing::Excluded(Exclusion::Precondition("broken author ancestry"));
            }
            if let Some(a) = ch.anchor {
                let Some(position) = self.position(&a) else {
                    missing.insert(Dependency::Event(a));
                    return Standing::Pending(Waiting::Anchor);
                };
                if position > reach {
                    return Standing::Excluded(Exclusion::AnchorRegressed);
                }
                reach = position;
            }
            // A previously validated ordinary event certifies its structural
            // ancestry. The edge and anchor above are still checked here; pins
            // and the target event's membership/cutoff remain independent.
            if id != event.id()
                && !ch.body.is_governance()
                && self
                    .authorization
                    .borrow()
                    .get(&id)
                    .is_some_and(|(standing, _)| *standing == Standing::Effective)
            {
                break;
            }
            let Some(prev) = ch.prev else {
                break;
            };
            let Some(before) = seq.checked_sub(1) else {
                return Standing::Excluded(Exclusion::Precondition("broken author sequence"));
            };
            seq = before;
            id = prev;
        }
        Standing::Effective
    }
}

fn validate_binding<D: DefinitionLookup + ?Sized>(
    binding: &RulesBinding,
    snapshot: &Snapshot,
    definitions: &D,
    missing: &mut BTreeSet<Dependency>,
) -> Standing {
    let Some(definition) = definitions.definition(&binding.definition.semantic) else {
        missing.insert(Dependency::Definition(binding.definition.semantic));
        return Standing::Pending(Waiting::Definition);
    };
    if !super::valid_definition(&binding.definition.semantic, definition) {
        return Standing::Excluded(Exclusion::InvalidDefinition);
    }
    for (role, principals) in &binding.roles {
        if !definition.roles.contains_key(role)
            || !principals.is_sorted_by(|a, b| a < b)
            || principals
                .iter()
                .any(|key| !snapshot.members.contains_key(key))
        {
            return Standing::Excluded(Exclusion::Precondition(
                "role bindings must name distinct admitted principals and declared roles",
            ));
        }
    }
    if definition
        .roles
        .keys()
        .any(|role| !binding.roles.contains_key(role))
    {
        return Standing::Excluded(Exclusion::Precondition("declared role is not bound"));
    }
    if binding
        .inputs
        .keys()
        .any(|name| !definition.context.inputs.contains_key(name))
        || definition
            .context
            .inputs
            .iter()
            .any(|(name, input)| input.required && !binding.inputs.contains_key(name))
    {
        return Standing::Excluded(Exclusion::Precondition(
            "input bindings do not match declared required inputs",
        ));
    }
    let authority_ok = |authority: &locust_proto::organization::Authority| match authority {
        locust_proto::organization::Authority::Role { name } => binding
            .roles
            .get(name)
            .is_some_and(|members| members.len() == 1),
        locust_proto::organization::Authority::Participant { key } => key
            .parse::<PublicKey>()
            .ok()
            .is_some_and(|key| snapshot.members.contains_key(&key)),
    };
    let decisions = std::iter::once(&definition.decisions).chain(
        definition
            .task_types
            .values()
            .filter_map(|task_type| task_type.decisions.as_ref()),
    );
    if decisions
        .flat_map(|rules| rules.selection.iter().chain(rules.finish.iter()))
        .any(|authority| !authority_ok(authority))
    {
        return Standing::Excluded(Exclusion::Precondition(
            "an authority must bind exactly one admitted principal",
        ));
    }
    Standing::Effective
}
