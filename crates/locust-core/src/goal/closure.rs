//! Closure constrains starts at their authenticated causal position. A later
//! observed close never retroactively cancels genuinely concurrent work.
use super::{DefinitionLookup, Standing, fold::Verifier, rules::invalid};
use locust_proto::{
    event::{Body, Context, DecisionAction, DecisionPurpose, Event},
    id::EventId,
};
use std::collections::BTreeSet;

impl<D: DefinitionLookup + ?Sized> Verifier<'_, D> {
    pub(super) fn open_at_observed_closure(
        &self,
        event: &Event,
        context: Context,
        declared: Option<EventId>,
        proof: Option<EventId>,
    ) -> Result<(), Standing> {
        if let Some(id) = declared {
            self.require(id, proof)?;
            if !matches!(self.event(id)?.header().body, Body::ScopeDecided { context: target, ref action, .. } if target == context && action.purpose() == DecisionPurpose::Closure)
            {
                return Err(invalid(
                    "attempt closure position belongs to another scope or purpose",
                ));
            }
        }
        // A scope without a closure authority cannot contain an effective
        // closure decision. Avoid traversing unrelated ancestry in open work.
        if self.resolve(context)?.effective.decisions.closure.is_none() {
            return Ok(());
        }
        // Traverse exact signed author and typed dependency ancestry, never
        // timestamps, arrival order, or untyped Header.parents hints.
        let h = event.header();
        let mut pending: Vec<_> = h
            .prev
            .into_iter()
            .chain(h.anchor)
            .chain(h.body.dependencies())
            .collect();
        let mut visited = BTreeSet::new();
        let mut latest: Option<(u64, EventId, DecisionAction)> = None;
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let ancestor = self.event(id)?;
            let ah = ancestor.header();
            if let Body::ScopeDecided {
                context: target,
                action,
                ..
            } = &ah.body
                && *target == context
                && action.purpose() == DecisionPurpose::Closure
                && self.status(id, proof) == Standing::Effective
                && latest.as_ref().is_none_or(|(seq, _, _)| ah.seq > *seq)
            {
                latest = Some((ah.seq, id, action.clone()));
            }
            pending.extend(ah.prev);
            pending.extend(ah.anchor);
            pending.extend(ah.body.dependencies());
        }
        if latest.as_ref().map(|(_, id, _)| *id) != declared {
            return Err(invalid(
                "attempt omits or regresses its authenticated closure position",
            ));
        }
        if matches!(latest, Some((_, _, DecisionAction::Close))) {
            return Err(invalid("attempt starts after observing a closed round"));
        }
        Ok(())
    }
}
