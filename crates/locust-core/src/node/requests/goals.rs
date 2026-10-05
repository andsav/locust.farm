//! Goals and membership: founding, status, grants and the workspace binding.

use locust_proto::api::{ApiError, ErrorCode, GoalGrants, GoalStatus, MemberView, Response};
use locust_proto::crypto::ContentKey;
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, DefinitionRef, Genesis, RulesBinding};
use locust_proto::id::{BlobHash, DefinitionHash, EventId, GoalId, PublicKey};
use locust_proto::store::Store;
use std::collections::BTreeMap;

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::not_found;
use crate::node::authoring::{Place, seal_text, sign_at};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::key_write;
use crate::node::local;

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

    /// Founding and the initial rules binding become durable together.
    pub(super) fn goal_create(
        &self,
        actor: &Actor,
        title: String,
        formation_json: Option<String>,
        roles: BTreeMap<String, Vec<PublicKey>>,
        inputs: BTreeMap<String, BlobHash>,
        now_ms: u64,
    ) -> Plan {
        let creator = self.manages_goals(actor)?;
        let endpoint = self.own_endpoint()?.endpoint;
        let signer = self.signer(&creator)?;
        let source = formation_json.unwrap_or_else(|| "{\"schema_version\":2}".into());
        let (definition, normalized) = checked_definition(&source)?;
        let genesis = Genesis {
            administrator: creator,
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
                member: creator,
                endpoint,
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
                    roles,
                    inputs,
                },
            },
            None,
            now_ms,
            &mut tx,
        )?;
        let grants = GoalGrants {
            administer: true,
            ..GoalGrants::default()
        };
        tx.local(key_write(&goal, 0, &key))
            .local(local::title_write(&goal, &title))
            .local(local::grants_write(&goal, &creator, &grants))
            .local(local::part_write(&goal, &creator, false));
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
        roles: BTreeMap<String, Vec<PublicKey>>,
        inputs: BTreeMap<String, BlobHash>,
        now: u64,
    ) -> Plan {
        let (entry, administrator) = self.administrator(actor, &goal)?;
        if entry.state().current_rules != Some(expected) {
            return Err(crate::node::access::conflict("the rules revision changed"));
        }
        let (semantic, normalized) = checked_definition(&source)?;
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
            &administrator,
            Body::RulesBound {
                expected: Some(expected),
                binding: RulesBinding {
                    definition: DefinitionRef { semantic, object },
                    roles,
                    inputs,
                },
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
        let administrator = state
            .administrator
            .or_else(|| {
                entry
                    .local
                    .joins
                    .values()
                    .next()
                    .map(|join| join.administrator)
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
            administrator,
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
                    endpoint: record.endpoint,
                    local: self.principals.holds(member),
                })
                .collect(),
            halted: entry.halted(),
            workspace: Some(self.workspace_view(entry, actor)?),
            grants: actor
                .principal
                .map(|principal| entry.local.grants(&principal))
                .unwrap_or_default(),
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

    /// `goal.grant`: the owner replaces one local principal's standing
    /// grants in a goal.
    pub(super) fn goal_grant(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        grants: GoalGrants,
    ) -> Plan {
        self.readable(actor, &goal)?;
        if self.principals.get(&agent).is_none() {
            return Err(not_found("no enrolled principal has that key"));
        }
        let mut tx = Tx::none();
        tx.local(local::grants_write(&goal, &agent, &grants))
            .touch(goal);
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn goal_leave(&self, actor: &Actor, goal: GoalId, now: u64) -> Plan {
        self.manages_goals(actor)?;
        let (entry, principal) = self.member(actor, &goal)?;
        if entry.state().administrator == Some(principal) {
            return Err(crate::node::access::conflict(
                "the administrator cannot leave before authority handoff",
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
        tx.local(local::part_write(&goal, &principal, true));
        super::tasks::recorded(event, tx)
    }

    pub(super) fn member_remove(
        &self,
        actor: &Actor,
        goal: GoalId,
        member: PublicKey,
        now: u64,
    ) -> Plan {
        let (entry, administrator) = self.administrator(actor, &goal)?;
        if member == administrator {
            return Err(crate::node::access::conflict(
                "the administrator cannot remove itself before authority handoff",
            ));
        }
        if !entry.is_member(&member) {
            return Err(crate::node::access::conflict(
                "the principal is not a member",
            ));
        }
        let mut place = self.next_place(entry, &administrator)?;
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
            self.signer(&administrator)?,
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

fn checked_definition(source: &str) -> Result<(DefinitionHash, String), ApiError> {
    let inspection = crate::organization::inspect(source);
    if !inspection.valid {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "the formation is invalid; validate it for diagnostics",
        ));
    }
    let hash = inspection
        .semantic_hash
        .expect("valid semantic hash")
        .parse()
        .map_err(|_| ApiError::new(ErrorCode::Internal, "invalid semantic hash"))?;
    let normalized = serde_json::to_string(&inspection.normalized.expect("valid definition"))
        .expect("definition encodes");
    Ok((hash, normalized))
}
