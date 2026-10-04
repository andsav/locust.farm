//! Four scenarios beyond the guide, all through the local API only.
//!
//! [`storm`] runs after the guide: members keep writing while faults are in
//! effect and nobody waits for anything to arrive, so that when the faults
//! end there is real work left and the time to quiet means something.
//!
//! [`prompt`] is the opposite: no faults at all, and every write must show
//! on every member within seconds. Anti-entropy repairs a lost push within
//! half a minute, so only a promptness bound can tell that the push path
//! itself works.

//!
//! [`lag`] measures one thing the guide does not ask: after a laptop wakes,
//! how long until the machine that stayed up can read what the woken one
//! writes. The text of a pushed event is fetched by an exchange the receiver
//! opens, which it opens as soon as the push completes, whatever backoff it
//! built up while the laptop slept.

//!
//! [`joins`] is only the start of the guide, without faults: how long a
//! join takes when nothing is wrong. It is cheap enough to run by the
//! hundred thousand, which is what finding a rare ordering takes.

use locust_proto::api::{Request, Response};
use locust_proto::id::EventId;

use super::check::settle;
use super::machine::Who;
use super::run::{Acked, Fail, Run};
use super::scenario::{create, finding, join, setup};
use super::world::{MS, Micros, SEC};

/// How soon a write must show everywhere when nothing is wrong.
pub const PROMPT: Micros = 10 * SEC;

/// Members write findings at seeded moments under a fresh set of faults; in a
/// third of the seeds the coordinator is stopped throughout.
pub fn storm(r: &mut Run) -> Result<(), Fail> {
    r.step = "write under faults";
    if r.faults {
        let budget = r.rng.range(2, 8) as u32;
        let gap = r.rng.pick(&[2_000, 5_000, 15_000]);
        r.w.enable_chaos(budget, gap);
    }
    let without_coordinator = r.rng.chance(1, 3);
    if without_coordinator {
        r.hold_stopped(0)?;
    }
    let goal = r.goal();
    for round in 0..r.rng.range(3, 10) {
        let m = r.rng.below(3) as usize;
        r.w.chaos_point();
        // Someone can only type on a machine that is up.
        if r.w.machines[m].running() {
            let text = format!("storm finding {round} from m{}", m + 1);
            let request = Request::ContributionPublish {
                goal,
                task: None,
                attempt: None,
                generation: None,
                summary: text.clone(),
                base: None,
                patch: None,
                sources: Vec::new(),
                artifacts: Vec::new(),
            };
            match r.w.call(m, Who::Agent, request) {
                Ok(Response::Recorded { event }) => {
                    r.acked.push(Acked {
                        what: "finding",
                        machine: m,
                        event,
                    });
                    r.findings.push((event, text));
                }
                other => return r.fail(format!("m{} finding.add answered {other:?}", m + 1)),
            }
        }
        let pause = r.rng.range(0, 15_000);
        r.w.run_for(pause * MS);
    }
    if without_coordinator {
        r.release(0);
    }
    Ok(())
}

/// Three members in steady state with nothing wrong: bursts of findings from
/// seeded members, each visible everywhere within [`PROMPT`].
pub fn prompt(r: &mut Run) -> Result<Micros, Fail> {
    setup(r)?;
    r.w.net.stall = 0;
    create(r)?;
    r.step = "m2 and m3 join";
    join(r, 1, 2)?;
    join(r, 2, 3)?;
    r.w.run_for(5 * SEC);
    r.step = "prompt delivery";
    let mut slowest = 0;
    for burst in 0..r.rng.range(4, 10) {
        let mut written: Vec<(EventId, String)> = Vec::new();
        for n in 0..r.rng.range(1, 4) {
            let m = r.rng.below(3) as usize;
            let text = format!("burst {burst} finding {n} from m{}", m + 1);
            written.push((finding(r, m, &text)?, text));
            let gap = r.rng.range(0, 400);
            r.w.run_for(gap * MS);
        }
        let from = r.w.now;
        r.wait("every member to show a burst of findings", |r| {
            written
                .iter()
                .all(|(id, text)| (0..3).all(|m| r.shows_finding(m, *id, text)))
        })?;
        let took = r.w.now - from;
        slowest = slowest.max(took);
        if took > PROMPT {
            return r.fail(format!(
                "with nothing wrong, findings took {:.1} s to show everywhere",
                took as f64 / SEC as f64
            ));
        }
        // Land the next burst anywhere in the anti-entropy period.
        let pause = r.rng.range(0, 40_000);
        r.w.run_for(pause * MS);
    }
    settle(r)?;
    Ok(slowest)
}

/// The longest [`lag`] may be: the push, then the fetch it starts.
pub const LAG_BOUND: Micros = 10 * SEC;

/// No injected faults. Machine 2 sleeps for minutes while machine 1 keeps
/// trying it, wakes, and writes a finding some seconds later. Returns how long
/// machine 1 took to show the finding's text.
pub fn lag(r: &mut Run) -> Result<Micros, Fail> {
    setup(r)?;
    r.w.net.stall = 0;
    create(r)?;
    r.step = "m2 and m3 join";
    join(r, 1, 2)?;
    join(r, 2, 3)?;
    r.w.run_for(5 * SEC);
    r.step = "m2 sleeps for minutes";
    r.hold_asleep(1)?;
    let asleep = r.rng.range(240, 900);
    r.w.run_for(asleep * SEC);
    r.release(1);
    let awake = r.rng.range(1, 40);
    r.w.run_for(awake * SEC);
    r.step = "text written after waking";
    let text = "M2 wrote this after waking";
    let id = finding(r, 1, text)?;
    let from = r.w.now;
    r.wait(
        "m1 to show the text of a finding m2 wrote after waking",
        |r| r.shows_finding(0, id, text),
    )?;
    let took = r.w.now - from;
    if took > LAG_BOUND {
        return r.fail(format!(
            "the text took {:.1} s to show on m1",
            took as f64 / SEC as f64
        ));
    }
    settle(r)?;
    Ok(took)
}

/// No injected faults: a goal is founded and two machines join in turn.
/// Returns how long the slower join took to show on every member.
pub fn joins(r: &mut Run) -> Result<Micros, Fail> {
    setup(r)?;
    create(r)?;
    let mut slowest = 0;
    for (m, step) in [(1, "m2 joins"), (2, "m3 joins")] {
        r.step = step;
        let from = r.w.now;
        join(r, m, m + 1)?;
        slowest = slowest.max(r.w.now - from);
    }
    Ok(slowest)
}
