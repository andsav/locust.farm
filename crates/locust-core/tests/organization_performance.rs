//! Local comparison workload; no numerical gate or product limit.
use locust_core::goal::Goal;
use locust_proto::{
    event::*,
    id::{DefinitionHash, EndpointId},
    organization::{Authority, Formation},
    store::{Commit, MemStore, Store},
    testkit::{self, Author},
};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

fn history(tasks: usize, forked: bool) -> (Vec<Event>, BTreeMap<DefinitionHash, Formation>) {
    let mut admin = Author::new(1);
    let mut worker = Author::new(2);
    let mut formation = Formation::default();
    formation.decisions.selection = Some(Authority::Participant {
        key: admin.key.public().to_string(),
    });
    let genesis = admin.genesis_with(&formation);
    let goal = genesis.header().goal;
    let self_admission = admin.event(
        goal,
        Some(genesis.id()),
        Body::MemberAdmitted {
            member: admin.key.public(),
            endpoint: EndpointId([1; 32]),
        },
    );
    let admission = admin.event(
        goal,
        Some(self_admission.id()),
        Body::MemberAdmitted {
            member: worker.key.public(),
            endpoint: EndpointId([2; 32]),
        },
    );
    let (binding, _) = testkit::rules_binding(&goal, 0, &formation, BTreeMap::new());
    let bound = admin.event(
        goal,
        Some(admission.id()),
        Body::RulesBound {
            expected: None,
            binding,
        },
    );
    let anchor = bound.id();
    let mut events = vec![genesis, self_admission, admission, bound];
    let mut fork = None;
    for index in 0..tasks {
        let task = worker.event(
            goal,
            Some(anchor),
            Body::TaskOpened {
                binding: TaskBinding {
                    rules: anchor,
                    task_type: None,
                    inputs: BTreeMap::new(),
                    parent: None,
                    stage: None,
                },
            },
        );
        let context = Context {
            scope: Scope::Task(TaskId::Authored(task.id())),
            round: task.id(),
        };
        let start = worker.event(
            goal,
            Some(anchor),
            Body::AttemptStarted {
                context,
                offer: None,
                closure: None,
            },
        );
        let result = worker.event(
            goal,
            Some(anchor),
            Body::ContributionPublished {
                context,
                attempt: Some(start.id()),
                base: None,
                patch: None,
                artifacts: vec![],
            },
        );
        if forked && index == tasks / 2 {
            let mut header = result.header().clone();
            header.at_ms += 1;
            fork = Some(Event::sign(header, &worker.key).unwrap());
        }
        let complete = worker.event(
            goal,
            Some(anchor),
            Body::CompletionDeclared {
                context,
                subject: result.id(),
            },
        );
        let select = admin.event(
            goal,
            Some(anchor),
            Body::ScopeDecided {
                context,
                previous: None,
                action: DecisionAction::Select {
                    subject: result.id(),
                },
                evidence: vec![complete.id()],
            },
        );
        events.extend([task, start, result, complete, select]);
    }
    events.extend(fork);
    (
        events,
        BTreeMap::from([(testkit::definition_hash(&formation), formation)]),
    )
}
fn sample(mut run: impl FnMut()) -> (u128, u128, u128) {
    run();
    let mut times: Vec<_> = (0..7)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed().as_micros()
        })
        .collect();
    times.sort_unstable();
    (times[0], times[3], times[6])
}
#[test]
#[ignore = "measurement only; no numeric performance gate"]
fn measure_organization_goal() {
    println!("kind,tasks,events,mode,min_us,median_us,max_us");
    for tasks in [16, 128, 512] {
        for forked in [false, true] {
            let (events, definitions) = history(tasks, forked);
            let id = events[0].header().goal;
            let mut store = MemStore::new();
            store
                .commit(&Commit {
                    events: events.clone(),
                    ..Commit::default()
                })
                .unwrap();
            let expected = Goal::load(&store, id, &definitions).unwrap();
            assert_eq!(expected.state().decisions.len(), tasks);
            assert_eq!(expected.state().selections.len(), tasks);
            assert!(expected.state().selections.values().all(|selection| {
                let Scope::Task(task) = selection.context.scope else {
                    return false;
                };
                expected.selected_task(task).is_some_and(|task| {
                    task.rounds[&selection.context.round].selected == Some(selection.subject.id())
                })
            }));
            assert!(
                expected
                    .state()
                    .decisions
                    .values()
                    .all(|decisions| decisions.len() == 1)
            );
            let wire: Vec<_> = events.iter().map(Event::to_wire).collect();
            let kind = if forked {
                "member_fork_pinned"
            } else {
                "healthy"
            };
            for mode in [
                "store_replay",
                "decoded_batch_ingest",
                "wire_decode_batch_ingest",
            ] {
                let (low, median, high) = sample(|| {
                    let actual = match mode {
                        "store_replay" => Goal::load(black_box(&store), id, &definitions).unwrap(),
                        "decoded_batch_ingest" => {
                            let mut goal = Goal::new(id);
                            for batch in events.chunks(128) {
                                goal.apply(black_box(batch), &definitions);
                            }
                            goal
                        }
                        _ => {
                            let mut goal = Goal::new(id);
                            for batch in wire.chunks(128) {
                                let decoded: Vec<_> = batch
                                    .iter()
                                    .map(|event| Event::from_wire(black_box(event)).unwrap())
                                    .collect();
                                goal.apply(&decoded, &definitions);
                            }
                            goal
                        }
                    };
                    assert_eq!(actual.state(), expected.state());
                    black_box(actual);
                });
                println!(
                    "{kind},{tasks},{},{mode},{low},{median},{high}",
                    events.len()
                );
            }
        }
    }
}
