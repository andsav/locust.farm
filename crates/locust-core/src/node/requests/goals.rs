//! Goals and membership: founding, status, levels and the workspace binding.

use crate::goal::DefinitionLookup;
use locust_proto::api::{ApiError, ErrorCode, GoalStatus, Level, MemberView, Response};
use locust_proto::crypto::{ContentKey, Keypair};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, DefinitionRef, Genesis, RulesBinding, WorkspaceCheckpoint};
use locust_proto::id::{BlobHash, DefinitionHash, EventId, GoalId, PublicKey};
use locust_proto::organization::Formation;
use locust_proto::store::Store;
use std::collections::{BTreeMap, BTreeSet};

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::{conflict, not_found};
use crate::node::authoring::{Place, seal_text, sign_at};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::{Entry, key_write};
use crate::node::local;

pub(super) struct GoalCreateInput {
    pub agent: PublicKey,
    pub title: String,
    pub formation_json: Option<String>,
    pub name: String,
    pub inputs: BTreeMap<String, BlobHash>,
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// This daemon's endpoint, which admissions and invitations name.
    pub(super) fn own_endpoint(&self) -> Result<&crate::node::identity::EndpointRecord, ApiError> {
        self.identity.endpoint.as_ref().ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "the transport has not reported this daemon's endpoint yet",
            )
        })
    }

    /// Founding, the admission of the host's agent and the initial rules
    /// binding become durable together, signed by a governance key drawn for
    /// this goal and stored beside it. The named agent is an ordinary member.
    pub(super) fn goal_create(&self, actor: &Actor, input: GoalCreateInput, now_ms: u64) -> Plan {
        let GoalCreateInput {
            agent,
            title,
            formation_json,
            name,
            inputs,
        } = input;
        let host = self.local_agent(actor, agent)?.principal()?;
        let endpoint = self.own_endpoint()?.endpoint;
        let seed: [u8; 32] = self.random();
        let governance = Keypair::from_seed(seed);
        let signer = &governance;
        let source = formation_json.unwrap_or_else(|| {
            serde_json::to_string(
                &locust_proto::organization::presets()
                    .into_iter()
                    .find(|preset| preset.name == "peer-review")
                    .expect("peer-review preset")
                    .formation,
            )
            .expect("formation encodes")
        });
        let (definition, normalized, _) = checked_definition(&source)?;
        let genesis = Genesis {
            governance: governance.public(),
            host,
            definition,
            salt: self.random(),
        };
        let goal = genesis.goal_id();
        let key = ContentKey(self.random());
        let mut tx = Tx::none();
        let founding = sign_at(
            goal,
            signer,
            Place {
                seq: 0,
                prev: None,
                anchor: None,
                epoch: 0,
            },
            Body::Genesis(genesis),
            Some((&key, &title)),
            now_ms,
            &mut tx,
        )?;
        let admission = sign_at(
            goal,
            signer,
            Place {
                seq: 1,
                prev: Some(founding.id()),
                anchor: Some(founding.id()),
                epoch: 0,
            },
            Body::MemberAdmitted {
                member: host,
                endpoint,
                name,
                role: None,
            },
            None,
            now_ms,
            &mut tx,
        )?;
        let (object, blob) = seal_text(&goal, 0, &key, normalized.as_bytes())?;
        tx.commit.blobs.push(blob);
        sign_at(
            goal,
            signer,
            Place {
                seq: 2,
                prev: Some(admission.id()),
                anchor: Some(admission.id()),
                epoch: 0,
            },
            Body::RulesBound {
                expected: None,
                binding: RulesBinding {
                    definition: DefinitionRef {
                        semantic: definition,
                        object,
                    },
                    inputs,
                },
            },
            None,
            now_ms,
            &mut tx,
        )?;
        tx.local(key_write(&goal, 0, &key))
            .local(local::governance_write(&goal, &seed))
            .local(local::title_write(&goal, &title))
            .local(local::part_write(&goal, &host, false))
            .local(local::level_write(&goal, &host, &Level::Auto));
        Ok(Planned {
            response: Response::GoalCreated { goal },
            tx,
        })
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    pub(super) fn rules_bind(
        &self,
        actor: &Actor,
        goal: GoalId,
        expected: EventId,
        source: String,
        inputs: BTreeMap<String, BlobHash>,
        now: u64,
    ) -> Plan {
        let (entry, governance) = self.host(actor, &goal)?;
        if entry.state().current_rules != Some(expected) {
            return Err(crate::node::access::conflict("the rules revision changed"));
        }
        let (semantic, normalized, formation) = checked_definition(&source)?;
        let deciding = Self::deciding(entry)?;
        for role in formation
            .roles
            .keys()
            .filter(|role| entry.state().roles.contains_key(*role))
        {
            let was_deciding = deciding.contains(role);
            if was_deciding != crate::organization::is_authority_role(&formation, role) {
                let reason = if was_deciding {
                    "this role picks or closes in this goal and has one holder; these rules make it a group. Use another role name."
                } else {
                    "this role is a group in this goal; these rules make it pick or close. Use another role name."
                };
                return Err(conflict(reason).with_details(serde_json::json!({"role": role})));
            }
        }
        let epoch = entry.state().epoch;
        let key = entry.keys.get(&epoch).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "the current content key is unavailable",
            )
        })?;
        let (object, blob) = seal_text(&goal, epoch, key, normalized.as_bytes())?;
        let mut tx = Tx::none();
        tx.commit.blobs.push(blob);
        let event = self.author(
            entry,
            &governance,
            Body::RulesBound {
                expected: Some(expected),
                binding: RulesBinding {
                    definition: DefinitionRef { semantic, object },
                    inputs,
                },
            },
            None,
            now,
            &mut tx,
        )?;
        if let Some(workspace) = entry
            .state()
            .workspace
            .as_ref()
            .filter(|workspace| workspace.enabled)
            && formation.workspace.is_some()
        {
            // Extend the binding in this transaction, never sign twice at the
            // unchanged durable tip. Replay applies both records on landing.
            let binding = tx.commit.events.last().expect("binding was signed");
            let place = Place {
                seq: binding
                    .header()
                    .seq
                    .checked_add(1)
                    .ok_or_else(|| conflict("the author's log is exhausted"))?,
                prev: Some(event),
                anchor: Some(event),
                epoch,
            };
            sign_at(
                goal,
                self.key_for(entry, &governance)?,
                place,
                Body::WorkspaceEpoch {
                    expected_epoch: Some(workspace.epoch),
                    rules: event,
                    checkpoint: workspace
                        .head
                        .map_or(WorkspaceCheckpoint::Unseeded, WorkspaceCheckpoint::Revision),
                },
                None,
                now,
                &mut tx,
            )?;
        }
        super::tasks::recorded(event, tx)
    }

    /// The name is shared evidence once admitted, and the ticket's claim
    /// while a joining copy is still waiting for that evidence.
    pub(in crate::node) fn host_name(entry: &Entry) -> Option<String> {
        entry
            .state()
            .host
            .and_then(|host| entry.state().members.get(&host))
            .map(|member| member.name.clone())
            .or_else(|| {
                entry
                    .local
                    .joins
                    .values()
                    .next()
                    .map(|join| join.host_name.clone())
            })
    }

    /// Every binding matters: a role keeps its kind even when only old work
    /// refers to it. Missing definitions prevent any change of roles or kind.
    pub(in crate::node) fn deciding(entry: &Entry) -> Result<BTreeSet<String>, ApiError> {
        let (roles, complete) = Self::deciding_known(entry);
        complete.then_some(roles).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "the goal's earlier rules have not arrived yet",
            )
        })
    }

    fn deciding_known(entry: &Entry) -> (BTreeSet<String>, bool) {
        let mut roles = BTreeSet::new();
        let mut complete = true;
        for rules in entry.state().rules.values() {
            if let Some(formation) = entry
                .definitions
                .definition(&rules.binding.definition.semantic)
            {
                roles.extend(
                    formation
                        .roles
                        .keys()
                        .filter(|name| crate::organization::is_authority_role(formation, name))
                        .cloned(),
                );
            } else {
                complete = false;
            }
        }
        (roles, complete)
    }

    #[allow(clippy::too_many_arguments)] // The API's compare-and-set role change.
    pub(super) fn role_change(
        &self,
        actor: &Actor,
        goal: GoalId,
        role: String,
        member: PublicKey,
        expected: Vec<PublicKey>,
        give: bool,
        now: u64,
    ) -> Plan {
        let (entry, governance) = self.host(actor, &goal)?;
        if entry.goal.evaluation().host_halt.is_some() {
            return Err(ApiError::new(
                ErrorCode::Halted,
                "the goal's authority is halted",
            ));
        }
        let holders = entry.state().roles.get(&role).ok_or_else(||
            ApiError::new(ErrorCode::Invalid, "this goal has no such role")
                .with_details(serde_json::json!({"role": role, "roles": entry.state().roles.keys().collect::<Vec<_>>()})))?;
        if *holders != expected {
            return Err(conflict(
                "the holders of this role changed; look again and repeat",
            ));
        }
        if !entry.is_member(&member) {
            return Err(conflict("the principal is not a member of this goal"));
        }
        let deciding = Self::deciding(entry)?.contains(&role);
        let host = entry
            .state()
            .host
            .ok_or_else(|| not_found("the host's agent is not known"))?;
        let mut next = holders.clone();
        if give {
            if holders.contains(&member) {
                return Err(conflict("the member already holds this role"));
            }
            if deciding {
                next.clear();
            }
            next.push(member);
            next.sort();
        } else {
            if !holders.contains(&member) {
                return Err(conflict("the member does not hold this role"));
            }
            if deciding && member == host {
                return Err(conflict(
                    "this role has one holder; give it to another member instead of taking it from the host's agent",
                ));
            }
            next.retain(|holder| *holder != member);
            if next.is_empty() {
                next.push(host);
            }
            if next == *holders {
                return answer(Response::Done);
            }
        }
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &governance,
            Body::RoleHolders {
                role,
                holders: next,
            },
            None,
            now,
            &mut tx,
        )?;
        super::tasks::recorded(event, tx)
    }

    /// `goal.status`: the goal as this daemon holds it, with the parts that
    /// are about the calling principal.
    pub(in crate::node) fn goal_status(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let state = entry.state();
        // Before any history has arrived the ticket's word is all there is.
        let governance = state
            .governance
            .or_else(|| {
                entry
                    .local
                    .joins
                    .values()
                    .next()
                    .map(|join| join.governance)
            })
            .ok_or_else(|| not_found("no such goal"))?;
        let own = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint);
        answer(Response::GoalStatus(GoalStatus {
            goal,
            title: self.title(entry, actor.principal.as_ref()),
            governance,
            hosted_here: self.hosts(entry),
            host: state.host,
            host_name: Self::host_name(entry),
            roles: state.roles.clone(),
            deciding: Self::deciding_known(entry).0,
            governance_head: state.head,
            current_rules: state.current_rules,
            scope_halts: entry
                .goal
                .evaluation()
                .scope_halts
                .keys()
                .map(|key| locust_proto::api::ScopeHalt {
                    context: key.context,
                    reason: locust_proto::api::Halt::AuthorityConflict,
                })
                .collect(),
            members: state
                .members
                .iter()
                .filter(|(_, member)| member.is_active())
                .map(|(member, record)| MemberView {
                    member: *member,
                    name: record.name.clone(),
                    endpoint: record.endpoint,
                    local: self.principals.holds(member),
                })
                .collect(),
            halted: entry.halted(),
            workspace: Some(self.workspace_view(entry, actor)?),
            abilities: actor.principal.map_or_else(
                || {
                    entry
                        .local
                        .part
                        .keys()
                        .chain(entry.local.joins.keys())
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .map(|agent| self.abilities(entry, agent))
                        .collect()
                },
                |agent| vec![self.abilities(entry, agent)],
            ),
            stalled: self.stalled(entry),
            peers: state
                .members
                .values()
                .filter(|member| member.is_active())
                .map(|member| member.endpoint)
                .collect::<std::collections::BTreeSet<_>>()
                .iter()
                .filter(|endpoint| Some(**endpoint) != own)
                .map(|endpoint| self.peer_view(endpoint))
                .collect(),
        }))
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn goal_leave(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        now: u64,
    ) -> Plan {
        let local_actor = self.local_agent(actor, agent)?;
        let (entry, principal) = self.member(&local_actor, &goal)?;
        if entry.state().host == Some(principal) {
            return Err(crate::node::access::conflict(
                "the host's agent cannot leave its own goal",
            ));
        }
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::LeaveRequested {
                admission: entry.state().members[&principal].admission,
            },
            None,
            now,
            &mut tx,
        )?;
        tx.local(local::part_write(&goal, &principal, true))
            .local(local::level_delete(&goal, &principal));
        for (task, agent) in entry.local.allowances.keys() {
            if *agent == principal {
                tx.local(local::allowance_delete(&goal, task, agent));
            }
        }
        super::tasks::recorded(event, tx)
    }

    pub(super) fn member_remove(
        &self,
        actor: &Actor,
        goal: GoalId,
        member: PublicKey,
        now: u64,
    ) -> Plan {
        let (entry, governance) = self.host(actor, &goal)?;
        if entry.state().host == Some(member) {
            return Err(crate::node::access::conflict(
                "the host's agent cannot be removed from its own goal",
            ));
        }
        if !entry.is_member(&member) {
            return Err(crate::node::access::conflict(
                "the principal is not a member",
            ));
        }
        let mut place = self.next_place(entry, &governance)?;
        place.epoch = place
            .epoch
            .checked_add(1)
            .ok_or_else(|| crate::node::access::conflict("the key epoch is exhausted"))?;
        let last_accepted = entry
            .goal
            .points(&member)
            .iter()
            .filter(|point| {
                entry
                    .goal
                    .standing(&point.id)
                    .is_some_and(|standing| standing.is_effective())
            })
            .max_by_key(|point| point.seq)
            .copied();
        let key = ContentKey(self.random());
        let mut tx = Tx::none();
        let event = sign_at(
            goal,
            self.key_for(entry, &governance)?,
            place,
            Body::MemberRemoved {
                member,
                admission: entry.state().members[&member].admission,
                last_accepted,
            },
            Some((&key, "")),
            now,
            &mut tx,
        )?;
        tx.local(key_write(&goal, place.epoch, &key));
        super::tasks::recorded(event.id(), tx)
    }
}

fn checked_definition(source: &str) -> Result<(DefinitionHash, String, Formation), ApiError> {
    let inspection = crate::organization::inspect(source);
    if !inspection.valid {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            inspection
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{}: {} {}",
                        diagnostic.code, diagnostic.message, diagnostic.correction
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    let hash = inspection
        .semantic_hash
        .expect("valid semantic hash")
        .parse()
        .map_err(|_| ApiError::new(ErrorCode::Internal, "invalid semantic hash"))?;
    let formation = inspection.normalized.expect("valid definition");
    let normalized = serde_json::to_string(&formation).expect("definition encodes");
    Ok((hash, normalized, formation))
}
