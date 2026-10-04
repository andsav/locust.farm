//! Conservative implication over pinned identities, including principals not yet
//! admitted. Unsupported completion implications fail closed with a diagnostic.
use super::rules::{self, EffectiveRules};
use locust_proto::{
    id::PublicKey,
    organization::{CompletionRule, DecisionRules, Selector, StartRule, WorkRules},
};
use std::collections::BTreeSet;

pub(super) fn inherit(parent: &EffectiveRules) -> (WorkRules, DecisionRules) {
    fn freeze(selector: &mut Selector, creator: Option<PublicKey>) {
        match selector {
            Selector::TaskCreator => {
                *selector = creator.map_or(Selector::Nobody, |key| Selector::Participant {
                    key: key.to_string(),
                })
            }
            Selector::Any { selectors } => selectors.iter_mut().for_each(|s| freeze(s, creator)),
            _ => {}
        }
    }
    fn completion(rule: &mut CompletionRule, creator: Option<PublicKey>) {
        match rule {
            CompletionRule::Contribution { by }
            | CompletionRule::Declaration { by }
            | CompletionRule::Reviews { by, .. }
            | CompletionRule::Check { by, .. } => freeze(by, creator),
            CompletionRule::All { rules } | CompletionRule::Any { rules } => {
                rules.iter_mut().for_each(|r| completion(r, creator))
            }
        }
    }
    let mut work = parent.work.clone();
    freeze(&mut work.propose, parent.creator);
    freeze(&mut work.publish, parent.creator);
    for start in &mut work.starts {
        match start {
            StartRule::Independent { by } => freeze(by, parent.creator),
            StartRule::Offered { by, to } => {
                freeze(by, parent.creator);
                freeze(to, parent.creator);
            }
        }
    }
    let mut decisions = parent.decisions.clone();
    completion(&mut decisions.completion, parent.creator);
    (work, decisions)
}

// A selector is a union of singleton identities, the candidate's author, or the
// whole membership universe. This symbolic representation is exact for the DSL;
// Members is never reduced to a currently admitted finite set.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Atom {
    Key(PublicKey),
    Subject,
}
fn atoms(selector: &Selector, effective: &EffectiveRules) -> Option<BTreeSet<Atom>> {
    match selector {
        Selector::Members => None,
        Selector::Nobody => Some(BTreeSet::new()),
        Selector::Participant { key } => {
            Some(key.parse().ok().map(Atom::Key).into_iter().collect())
        }
        Selector::Role { name } => Some(
            effective
                .roles
                .get(name)
                .into_iter()
                .flatten()
                .copied()
                .map(Atom::Key)
                .collect(),
        ),
        Selector::TaskCreator => Some(effective.creator.map(Atom::Key).into_iter().collect()),
        Selector::ContributionAuthor => Some(BTreeSet::from([Atom::Subject])),
        Selector::Any { selectors } => {
            let mut all = BTreeSet::new();
            for selector in selectors {
                all.extend(atoms(selector, effective)?);
            }
            Some(all)
        }
    }
}
fn subset(child: &Selector, parent: &Selector, c: &EffectiveRules, p: &EffectiveRules) -> bool {
    match (atoms(child, c), atoms(parent, p)) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(child), Some(parent)) => child.is_subset(&parent),
    }
}
fn implies(
    child: &CompletionRule,
    parent: &CompletionRule,
    c: &EffectiveRules,
    p: &EffectiveRules,
) -> bool {
    match (child, parent) {
        (_, CompletionRule::All { rules }) => rules.iter().all(|r| implies(child, r, c, p)),
        (CompletionRule::Any { rules }, _) => rules.iter().all(|r| implies(r, parent, c, p)),
        (CompletionRule::All { rules }, _) => rules.iter().any(|r| implies(r, parent, c, p)),
        (_, CompletionRule::Any { rules }) => rules.iter().any(|r| implies(child, r, c, p)),
        (CompletionRule::Contribution { by: cb }, CompletionRule::Contribution { by: pb })
        | (CompletionRule::Declaration { by: cb }, CompletionRule::Declaration { by: pb }) => {
            subset(cb, pb, c, p)
        }
        (
            CompletionRule::Reviews {
                by: cb,
                count: cn,
                exclude_author: ce,
            },
            CompletionRule::Reviews {
                by: pb,
                count: pn,
                exclude_author: pe,
            },
        ) => cn >= pn && (!pe || *ce) && subset(cb, pb, c, p),
        (
            CompletionRule::Check { name: cn, by: cb },
            CompletionRule::Check { name: pn, by: pb },
        ) => cn == pn && subset(cb, pb, c, p),
        _ => false,
    }
}
pub(super) fn narrows(c: &EffectiveRules, p: &EffectiveRules) -> bool {
    let authority = |child: &Option<_>, parent: &Option<_>| {
        child.as_ref().is_none_or(|child| {
            parent.as_ref().is_some_and(|parent| {
                rules::authority(child, c).is_some()
                    && rules::authority(child, c) == rules::authority(parent, p)
            })
        })
    };
    subset(&c.work.propose, &p.work.propose, c, p)
        && subset(&c.work.publish, &p.work.publish, c, p)
        && c.work.starts.iter().all(|child| {
            p.work.starts.iter().any(|parent| match (child, parent) {
                (StartRule::Independent { by: cb }, StartRule::Independent { by: pb }) => {
                    subset(cb, pb, c, p)
                }
                (StartRule::Offered { by: cb, to: ct }, StartRule::Offered { by: pb, to: pt }) => {
                    subset(cb, pb, c, p) && subset(ct, pt, c, p)
                }
                _ => false,
            })
        })
        && authority(&c.decisions.selection, &p.decisions.selection)
        && authority(&c.decisions.finish, &p.decisions.finish)
        && implies(&c.decisions.completion, &p.decisions.completion, c, p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::id::{DefinitionHash, EventId};
    fn effective() -> EffectiveRules {
        EffectiveRules {
            work: WorkRules::default(),
            decisions: DecisionRules::default(),
            roles: Default::default(),
            creator: Some(PublicKey([1; 32])),
            rules: EventId([2; 32]),
            definition: DefinitionHash([3; 32]),
        }
    }
    #[test]
    fn membership_universe_and_subject_are_not_current_members() {
        let c = effective();
        let p = effective();
        let finite = Selector::Any {
            selectors: vec![
                Selector::Participant {
                    key: PublicKey([1; 32]).to_string(),
                },
                Selector::ContributionAuthor,
            ],
        };
        assert!(!subset(&Selector::Members, &finite, &c, &p));
        assert!(subset(&finite, &Selector::Members, &c, &p));
        assert!(!subset(
            &Selector::ContributionAuthor,
            &Selector::TaskCreator,
            &c,
            &p
        ));
    }
    #[test]
    fn inherited_creator_remains_parent_identity() {
        let parent = effective();
        let mut parent = parent;
        parent.work.starts = vec![StartRule::Independent {
            by: Selector::TaskCreator,
        }];
        let mut child = parent.clone();
        child.creator = Some(PublicKey([4; 32]));
        assert!(!narrows(&child, &parent));
        (child.work, child.decisions) = inherit(&parent);
        assert!(narrows(&child, &parent));
    }
    #[test]
    fn completion_thresholds_conjunction_and_disjunction_do_not_relax() {
        let p = effective();
        let c = effective();
        let reviews = |count, exclude_author| CompletionRule::Reviews {
            by: Selector::Members,
            count,
            exclude_author,
        };
        assert!(implies(&reviews(3, true), &reviews(2, true), &c, &p));
        assert!(!implies(&reviews(1, true), &reviews(2, true), &c, &p));
        assert!(!implies(&reviews(2, false), &reviews(2, true), &c, &p));
        let both = CompletionRule::All {
            rules: vec![reviews(2, true), CompletionRule::default()],
        };
        let either = CompletionRule::Any {
            rules: vec![reviews(2, true), CompletionRule::default()],
        };
        assert!(implies(&both, &reviews(2, true), &c, &p));
        assert!(!implies(&either, &reviews(2, true), &c, &p));
    }
}
