//! The tests cargo runs.
//!
//! `cargo test -p locust-core --lib node::sim` runs a few dozen seeds of
//! each scenario. Environment variables, all optional:
//!
//! - `LOCUST_SIM_SEEDS=N` or `A..B`: the seeds `sim_many` runs (an ignored
//!   test; run it with `-- --ignored sim_many --nocapture`).
//! - `LOCUST_SIM_SEED=N`: the seed `sim_one` replays, printing its faults.
//! - `LOCUST_SIM_TRACE=1`: `sim_one` also prints every input delivered.
//! - `LOCUST_SIM_SCENARIO=prompt`, `lag` or `joins`: the fault-free
//!   promptness scenario, the wake-lag measurement, or two joins and
//!   nothing else, instead of the guide.
//! - `LOCUST_SIM_CALM=1`: the guide with no fault injection.
//! - `LOCUST_SIM_MAX_FAULTS=N`, `LOCUST_SIM_TRUE_CLOCKS=1`,
//!   `LOCUST_SIM_NO_STALLS=1`, `LOCUST_SIM_NO_BACKGROUND=1`: a smaller
//!   replay of the same seed, as the report of a failing seed prints it.
//! - `LOCUST_SIM_CLOCK_STEPS=1`: faults also set a running machine's wall
//!   clock forward or back by up to ten minutes. Not part of the default
//!   run.
//!
//! Not simulated here: the Iroh transport (handshakes, relays, hole
//! punching, flow control, the two-stream budget), discovery and NAT, the
//! SQLite store and anything about disks, the CLI and its files, the local
//! socket, real time, threads, and the operating system's sleep.

use super::seed::{Options, Report, Scenario, describe, run_seed, sweep};
use super::storm::{LAG_BOUND, PROMPT};

fn failed_seeds(failures: &[Report]) -> Vec<u64> {
    failures.iter().map(|report| report.seed).collect()
}

fn seeds_from_env(default: u64) -> std::ops::Range<u64> {
    let spec = std::env::var("LOCUST_SIM_SEEDS").unwrap_or_else(|_| default.to_string());
    match spec.split_once("..") {
        Some((from, to)) => from.parse().unwrap()..to.parse().unwrap(),
        None => 0..spec.parse().unwrap(),
    }
}

/// The guide end to end with no faults at all.
#[test]
fn sim_guide_without_faults() {
    let calm = Options {
        calm: true,
        ..Options::default()
    };
    for seed in 0..4 {
        let report = run_seed(seed, calm);
        assert!(report.result.is_ok(), "{}", describe(&report));
        assert_eq!(report.stats.faults, 0);
        assert_eq!(report.stats.oversized, 0);
    }
}

/// A seed fixes the run: the same seed delivers the same inputs at the same
/// simulated times, and another seed does not.
#[test]
fn sim_a_seed_reproduces_its_run() {
    let first = run_seed(5, Options::default());
    let again = run_seed(5, Options::default());
    assert!(first.stats.faults > 0, "seed 5 injects faults");
    assert_eq!(first.digest, again.digest);
    assert_eq!(first.stats, again.stats);
    assert_eq!(first.simulated, again.simulated);
    assert_eq!(first.faults, again.faults);
    assert_ne!(first.digest, run_seed(6, Options::default()).digest);
}

/// The default run: a few dozen seeds of the guide under faults.
#[test]
fn sim_guide_under_faults() {
    let failures = sweep(0..32, Options::default());
    assert!(
        failures.is_empty(),
        "failing seeds: {:?}",
        failed_seeds(&failures)
    );
}

/// With nothing wrong, every write shows on every member within seconds.
#[test]
fn sim_writes_show_promptly() {
    let options = Options {
        scenario: Scenario::Prompt,
        ..Options::default()
    };
    let failures = sweep(0..16, options);
    assert!(
        failures.is_empty(),
        "failing seeds: {:?}",
        failed_seeds(&failures)
    );
    let _ = PROMPT;
}

/// After a laptop wakes, what it writes is readable on the machine that
/// stayed up within [`LAG_BOUND`], not after the backoff the other machine
/// built up while it slept (SIM-7).
#[test]
fn sim_text_lag_after_wake_is_bounded() {
    let options = Options {
        scenario: Scenario::Lag,
        ..Options::default()
    };
    let failures = sweep(0..16, options);
    assert!(
        failures.is_empty(),
        "failing seeds: {:?}",
        failed_seeds(&failures)
    );
    let _ = LAG_BOUND;
}

/// SIM-6, seed 5252 of the guide: every fault has ended a minute before the
/// third machine joins. The coordinator dials the joiner it has just
/// admitted and the joiner refuses, because it does not hold the goal yet.
/// Its shell used to close that connection, which is not admitted on its
/// side, taking its own join exchange with it, and both sides retried on the
/// same backoff schedule, so the order repeated and the join never completed
/// in ten simulated minutes. The shell now keeps a connection that carries
/// an exchange it opened, and the driver jitters its backoff. The shell rule
/// is modelled here from `crates/locust/src/daemon/network.rs`; confirm
/// against real daemons. The run is the one found before restores existed.
#[test]
fn sim_join_survives_the_coordinators_dial() {
    let options = Options {
        max_faults: Some(4),
        true_clocks: true,
        no_restores: true,
        ..Options::default()
    };
    let report = run_seed(5252, options);
    assert!(report.result.is_ok(), "{}", describe(&report));
}

/// The long run. `LOCUST_SIM_SEEDS=N` runs seeds `0..N`, `A..B` that range.
#[test]
#[ignore = "long: set LOCUST_SIM_SEEDS and run with --ignored"]
fn sim_many() {
    let failures = sweep(seeds_from_env(1_000), Options::from_env());
    assert!(
        failures.is_empty(),
        "failing seeds: {:?}",
        failed_seeds(&failures)
    );
}

/// Replays one seed and prints what happened.
#[test]
#[ignore = "replay: set LOCUST_SIM_SEED and run with --ignored --nocapture"]
fn sim_one() {
    let seed = std::env::var("LOCUST_SIM_SEED").map_or(0, |seed| seed.parse().unwrap());
    let report = run_seed(seed, Options::from_env());
    for line in &report.log {
        eprintln!("{line}");
    }
    eprintln!("{}", describe(&report));
    assert!(report.result.is_ok(), "seed {seed} failed");
}
