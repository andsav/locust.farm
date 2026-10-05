//! Workspace lineage and checkpoint verification over signed events only.
//! No content lookup is permitted in this module or the authority fold.

use std::collections::BTreeSet;

use locust_proto::event::{Body, Context, DecisionAction, Event, Scope, WorkspaceCheckpoint};
use locust_proto::id::{EventId, PublicKey};

use super::DefinitionLookup;
use super::chain::Chain;
use super::fold::Verifier;
use super::history::History;
use super::rules::invalid;
use super::standing::{Dependency, Standing, Waiting};
use super::state::WorkspaceRevision;

fn epoch<'a>(
    history: &'a History,
    chain: &Chain,
    id: EventId,
    missing: &mut BTreeSet<Dependency>,
) -> Result<&'a Event, Standing> {
    let event = history.get(&id).ok_or_else(|| {
        missing.insert(Dependency::Event(id));
        Standing::Pending(Waiting::Reference)
    })?;
    if !matches!(event.header().body, Body::WorkspaceEpoch { .. }) {
        return Err(invalid("workspace epoch reference has the wrong kind"));
    }
    if chain
        .snapshot(&id)
        .and_then(|snapshot| snapshot.workspace_epoch)
        != Some(id)
    {
        return Err(invalid(
            "workspace epoch is outside the structural governance chain",
        ));
    }
    Ok(event)
}

pub(super) fn is_epoch_ancestor(
    history: &History,
    chain: &Chain,
    ancestor: EventId,
    mut current: Option<EventId>,
    missing: &mut BTreeSet<Dependency>,
) -> Result<bool, Standing> {
    let mut seen = BTreeSet::new();
    while let Some(id) = current {
        if !seen.insert(id) {
            return Err(invalid("cyclic workspace epoch ancestry"));
        }
        let event = epoch(history, chain, id, missing)?;
        if id == ancestor {
            return Ok(true);
        }
        let Body::WorkspaceEpoch { expected_epoch, .. } = event.header().body else {
            unreachable!()
        };
        current = expected_epoch;
    }
    Ok(false)
}

/// Resolve a structural boundary without checking the attempted checkpoint's
/// authority. Restoration skips canceled attempts, including unavailable ones.
pub(super) fn boundary(
    history: &History,
    chain: &Chain,
    start: EventId,
    missing: &mut BTreeSet<Dependency>,
) -> Result<Option<EventId>, Standing> {
    let mut current = Some(start);
    let mut seen = BTreeSet::new();
    let mut must_be_empty = false;
    while let Some(id) = current {
        if !seen.insert(id) {
            return Err(invalid("cyclic workspace boundary"));
        }
        let event = epoch(history, chain, id, missing)?;
        let Body::WorkspaceEpoch {
            expected_epoch,
            checkpoint,
            ..
        } = &event.header().body
        else {
            unreachable!()
        };
        match checkpoint {
            WorkspaceCheckpoint::Revision(revision) => {
                return if must_be_empty {
                    Err(invalid("unseeded epoch would discard a retained revision"))
                } else {
                    Ok(Some(*revision))
                };
            }
            WorkspaceCheckpoint::Unseeded => {
                must_be_empty = true;
                current = *expected_epoch;
            }
            WorkspaceCheckpoint::RetainBefore { epoch: target } => {
                if !is_epoch_ancestor(history, chain, *target, *expected_epoch, missing)? {
                    return Err(invalid(
                        "restoration target is not an ancestor workspace epoch",
                    ));
                }
                let Body::WorkspaceEpoch { expected_epoch, .. } =
                    epoch(history, chain, *target, missing)?.header().body
                else {
                    unreachable!()
                };
                current = expected_epoch;
            }
        }
    }
    Ok(None)
}

pub(super) fn revision(history: &History, id: EventId) -> Result<WorkspaceRevision, Standing> {
    let event = history
        .get(&id)
        .ok_or(Standing::Pending(Waiting::Reference))?;
    let Body::ScopeDecided {
        context,
        action: DecisionAction::Select { subject },
        ..
    } = event.header().body
    else {
        return Err(invalid("workspace revision is not a selection"));
    };
    if context.scope != Scope::Workspace {
        return Err(invalid("revision belongs to another scope"));
    }
    let proposal = history
        .get(&subject)
        .ok_or(Standing::Pending(Waiting::Reference))?;
    let Body::WorkspaceProposed {
        context: proposal_context,
        parent,
        result_manifest,
        ..
    } = proposal.header().body
    else {
        return Err(invalid("workspace selection subject has the wrong kind"));
    };
    if context != proposal_context {
        return Err(invalid(
            "workspace selection subject belongs to another epoch",
        ));
    }
    Ok(WorkspaceRevision {
        id,
        context,
        proposal: subject,
        parent,
        result_manifest,
    })
}

pub(super) fn lineage(
    history: &History,
    mut head: Option<EventId>,
) -> Result<Vec<WorkspaceRevision>, Standing> {
    let mut seen = BTreeSet::new();
    let mut revisions = Vec::new();
    while let Some(id) = head {
        if !seen.insert(id) {
            return Err(invalid("cyclic workspace revision lineage"));
        }
        let revision = revision(history, id)?;
        head = revision.parent;
        revisions.push(revision);
    }
    Ok(revisions)
}

impl<D: DefinitionLookup + ?Sized> Verifier<'_, D> {
    pub(super) fn workspace_boundary(&self, epoch: EventId) -> Result<Option<EventId>, Standing> {
        boundary(
            self.history,
            self.chain,
            epoch,
            &mut self.missing.borrow_mut(),
        )
    }

    pub(super) fn checkpoint_contains(&self, owner: EventId, id: EventId) -> bool {
        if !matches!(
            self.history.get(&owner).map(|event| &event.header().body),
            Some(Body::WorkspaceEpoch { .. })
        ) {
            return false;
        }
        if !self.checkpoint_lineages.borrow().contains_key(&owner) {
            let result = self
                .workspace_boundary(owner)
                .and_then(|head| lineage(self.history, head))
                .map(|lineage| lineage.into_iter().map(|revision| revision.id).collect());
            self.checkpoint_lineages.borrow_mut().insert(owner, result);
        }
        self.checkpoint_lineages
            .borrow()
            .get(&owner)
            .is_some_and(|result| result.as_ref().is_ok_and(|lineage| lineage.contains(&id)))
    }

    /// Pinning a checkpoint may bypass only workspace selection sibling forks.
    /// A fork elsewhere in the integrator's log does not gain signing authority.
    pub(super) fn checkpoint_signer(&self, event: &Event, owner: EventId) -> Result<(), Standing> {
        let proof = self.proof(owner)?;
        let log = self
            .history
            .log(&event.header().author)
            .expect("held author has log");
        if log.fork.is_none_or(|fork| fork > event.header().seq) {
            return Ok(());
        }
        let mut ancestry = BTreeSet::new();
        let mut current = Some(event.id());
        while let Some(id) = current {
            if !ancestry.insert(id) {
                return Err(invalid("cyclic author ancestry"));
            }
            current = self.event(id)?.header().prev;
        }
        let end = log
            .points
            .partition_point(|point| point.seq <= event.header().seq);
        let mut from = 0;
        while from < end {
            let seq = log.points[from].seq;
            let to = from + log.points[from..end].partition_point(|point| point.seq == seq);
            let siblings = &log.points[from..to];
            from = to;
            if siblings.len() < 2 {
                continue;
            }
            let selected = siblings
                .iter()
                .find(|point| {
                    self.history
                        .slot(&point.id)
                        .is_some_and(|slot| proof.retained.contains(slot))
                })
                .ok_or(Standing::Pending(Waiting::ForkProof))?;
            let selected_event = self.event(selected.id)?;
            if !ancestry.contains(&selected.id) {
                return Err(Standing::Pending(Waiting::ForkProof));
            }
            let Body::ScopeDecided {
                context,
                previous,
                action: DecisionAction::Select { .. },
                ..
            } = selected_event.header().body
            else {
                return Err(Standing::Pending(Waiting::ForkProof));
            };
            if context.scope != Scope::Workspace || !self.checkpoint_contains(owner, selected.id)
                || siblings.iter().any(|point| !matches!(self.history.get(&point.id).map(|event| &event.header().body),
                    Some(Body::ScopeDecided { context: other, previous: prior, action: DecisionAction::Select { .. }, .. }) if *other == context && *prior == previous)) {
                return Err(Standing::Pending(Waiting::ForkProof));
            }
        }
        Ok(())
    }

    pub(super) fn workspace_epoch(&self, event: &Event) -> Result<(), Standing> {
        let Body::WorkspaceEpoch {
            expected_epoch,
            rules,
            checkpoint,
        } = &event.header().body
        else {
            unreachable!()
        };
        self.require(*rules, None)?;
        let anchor = event.header().anchor.expect("governance has anchor");
        if self
            .chain
            .position(rules)
            .is_none_or(|position| position > self.chain.position(&anchor).unwrap())
        {
            return Err(invalid("workspace rules are after its governance anchor"));
        }
        let _ = self.resolve(Context {
            scope: Scope::Workspace,
            round: event.id(),
        })?;
        let boundary = self.workspace_boundary(event.id())?;
        if let Some(revision) = boundary {
            if matches!(checkpoint, WorkspaceCheckpoint::Revision(_)) {
                let previous = expected_epoch.ok_or_else(|| {
                    invalid("initial epoch cannot checkpoint a nonexistent workspace")
                })?;
                let candidate = revision_record(self, revision)?;
                let in_prior = candidate.context.round == previous
                    || self
                        .workspace_boundary(previous)
                        .and_then(|head| lineage(self.history, head))?
                        .iter()
                        .any(|ancestor| ancestor.id == revision);
                if !in_prior {
                    return Err(invalid(
                        "checkpoint is outside the preceding retained workspace lineage",
                    ));
                }
            }
            self.require(revision, Some(event.id()))?;
        }
        Ok(())
    }

    pub(super) fn workspace_parent(
        &self,
        context: Context,
        parent: Option<EventId>,
        proof: Option<EventId>,
    ) -> Result<(), Standing> {
        let checkpoint = self.workspace_boundary(context.round)?;
        if parent == checkpoint {
            if let Some(parent) = parent {
                self.require(parent, Some(context.round))?;
            }
            return Ok(());
        }
        let parent = parent
            .ok_or_else(|| invalid("seed proposal cannot discard a retained workspace revision"))?;
        let revision = revision_record(self, parent)?;
        if revision.context != context {
            return Err(invalid(
                "proposal parent is outside its workspace epoch lineage",
            ));
        }
        // A typed source can retain an old proposal based on the branch
        // selected by the receiving epoch. Validate that parent with the epoch
        // proof, never with another scope's decision or an excluded sibling.
        let checkpoint_owner = proof.and_then(|owner| {
            let event = self.history.get(&owner)?;
            let epoch = match event.header().body {
                Body::WorkspaceEpoch { .. } => owner,
                Body::ScopeDecided {
                    context:
                        Context {
                            scope: Scope::Workspace,
                            round,
                        },
                    ..
                } => round,
                _ => return None,
            };
            self.checkpoint_contains(epoch, parent).then_some(epoch)
        });
        self.require(parent, checkpoint_owner.or(proof))
    }

    pub(super) fn source_authors(&self, subject: EventId) -> Result<BTreeSet<PublicKey>, Standing> {
        let event = self.event(subject)?;
        let mut authors = BTreeSet::from([event.header().author]);
        if !matches!(event.header().body, Body::WorkspaceProposed { .. }) {
            return Ok(authors);
        }
        let mut pending = vec![subject];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let event = self.event(id)?;
            let Body::WorkspaceProposed { sources, .. } = &event.header().body else {
                return Err(invalid("workspace source reference is not a proposal"));
            };
            authors.insert(event.header().author);
            pending.extend(sources.iter().copied());
        }
        Ok(authors)
    }
}

fn revision_record<D: DefinitionLookup + ?Sized>(
    v: &Verifier<'_, D>,
    id: EventId,
) -> Result<WorkspaceRevision, Standing> {
    let _ = v.event(id)?;
    revision(v.history, id)
}
