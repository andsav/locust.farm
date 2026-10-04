//! Running one seed, many seeds, and making a failing seed smaller.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use super::check::settle;
use super::run::{Fail, Run};
use super::scenario::{setup, t1};
use super::storm::{joins, lag, prompt, storm};
use super::world::{Micros, SEC, Stats};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario {
    /// The guide, then writes under faults, then quiet and the invariants.
    #[default]
    Guide,
    /// No faults; every write must show everywhere promptly.
    Prompt,
    /// No injected faults; how long text lags after a laptop wakes.
    Lag,
    /// No injected faults; how long joining takes.
    Joins,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub scenario: Scenario,
    /// No fault injection.
    pub calm: bool,
    /// Keep a line for every input delivered.
    pub trace: bool,
    /// At most this many injected faults over the whole run.
    pub max_faults: Option<u32>,
    /// Every machine's clock tells true time.
    pub true_clocks: bool,
    /// No delivery stalls for seconds.
    pub no_stalls: bool,
    /// No exchanges cut or connection attempts failed at a steady rate
    /// outside the injected faults.
    pub no_background: bool,
    /// Injected faults also step wall clocks forward and back.
    pub clock_steps: bool,
}

impl Options {
    /// From the environment; see the module documentation of `tests`.
    pub fn from_env() -> Self {
        let set = |name: &str| std::env::var_os(name).is_some();
        Self {
            scenario: match std::env::var("LOCUST_SIM_SCENARIO").as_deref() {
                Ok("prompt") => Scenario::Prompt,
                Ok("lag") => Scenario::Lag,
                Ok("joins") => Scenario::Joins,
                _ => Scenario::Guide,
            },
            calm: set("LOCUST_SIM_CALM"),
            trace: set("LOCUST_SIM_TRACE"),
            max_faults: std::env::var("LOCUST_SIM_MAX_FAULTS")
                .ok()
                .map(|n| n.parse().expect("LOCUST_SIM_MAX_FAULTS is a number")),
            true_clocks: set("LOCUST_SIM_TRUE_CLOCKS"),
            no_stalls: set("LOCUST_SIM_NO_STALLS"),
            no_background: set("LOCUST_SIM_NO_BACKGROUND"),
            clock_steps: set("LOCUST_SIM_CLOCK_STEPS"),
        }
    }

    /// The environment that replays a seed with these options.
    fn replay(&self, seed: u64) -> String {
        let mut text = format!("LOCUST_SIM_SEED={seed}");
        match self.scenario {
            Scenario::Guide => {}
            Scenario::Prompt => text.push_str(" LOCUST_SIM_SCENARIO=prompt"),
            Scenario::Lag => text.push_str(" LOCUST_SIM_SCENARIO=lag"),
            Scenario::Joins => text.push_str(" LOCUST_SIM_SCENARIO=joins"),
        }
        if self.calm {
            text.push_str(" LOCUST_SIM_CALM=1");
        }
        if let Some(n) = self.max_faults {
            text.push_str(&format!(" LOCUST_SIM_MAX_FAULTS={n}"));
        }
        if self.true_clocks {
            text.push_str(" LOCUST_SIM_TRUE_CLOCKS=1");
        }
        if self.no_stalls {
            text.push_str(" LOCUST_SIM_NO_STALLS=1");
        }
        if self.no_background {
            text.push_str(" LOCUST_SIM_NO_BACKGROUND=1");
        }
        if self.clock_steps {
            text.push_str(" LOCUST_SIM_CLOCK_STEPS=1");
        }
        text
    }
}

pub struct Report {
    pub seed: u64,
    pub options: Options,
    /// For the guide, how long after the last fault ended every invariant
    /// held; for the prompt scenario, the slowest burst; for the lag
    /// scenario, how long the text took.
    pub result: Result<Micros, Fail>,
    pub digest: u64,
    pub stats: Stats,
    pub simulated: Micros,
    pub faults: Vec<String>,
    pub longest_wait: Micros,
    pub longest_wait_for: &'static str,
    pub log: Vec<String>,
}

/// One whole run under the faults the seed chooses.
pub fn run_seed(seed: u64, options: Options) -> Report {
    let mut r = Run::new(seed, 3);
    if options.trace {
        r.w.log = Some(Vec::new());
    }
    r.faults = !options.calm && options.scenario == Scenario::Guide;
    r.w.chaos.cap = options.max_faults;
    r.w.chaos.clock_steps = options.clock_steps;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for m in 0..3 {
            // A third of the machines keep true time; the rest are up to a
            // quarter of an hour off, either way.
            let exact = r.rng.chance(1, 3);
            let offset = r.rng.range(0, 1_800_000) as i64 - 900_000;
            if !exact && !options.true_clocks {
                r.w.machines[m].clock_offset_ms = offset;
            }
        }
        r.look = r.rng.pick(&[250, 1_000, 5_000]) * 1_000;
        if options.no_stalls {
            r.w.net.stall = 0;
        }
        match options.scenario {
            Scenario::Guide => {}
            Scenario::Prompt | Scenario::Lag | Scenario::Joins => r.look = 250_000,
        }
        match options.scenario {
            Scenario::Guide => {}
            Scenario::Prompt => return prompt(&mut r),
            Scenario::Lag => return lag(&mut r),
            Scenario::Joins => return joins(&mut r),
        }
        setup(&mut r)?;
        let none = r.rng.chance(1, 8);
        let faults = r.rng.range(1, 12) as u32;
        let gap = r.rng.pick(&[3_000, 10_000, 30_000, 90_000]);
        let cuts = r.rng.pick(&[0, 0, 2, 8]);
        let failed_opens = r.rng.pick(&[0, 0, 2, 8]);
        if r.faults && !none {
            r.w.enable_chaos(faults, gap);
            if !options.no_background {
                r.w.net.cut_rate = cuts;
                r.w.net.open_fail_rate = failed_opens;
            }
        }
        t1(&mut r)?;
        storm(&mut r)?;
        settle(&mut r)
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(panic) => {
            let text = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_else(|| "a panic without a message".into());
            Err(Fail {
                step: r.step,
                what: format!("panic: {text}"),
            })
        }
    };
    Report {
        seed,
        options,
        result,
        digest: r.w.digest,
        stats: r.w.stats,
        simulated: r.w.now,
        faults: r.w.chaos.history.clone(),
        longest_wait: r.longest_wait,
        longest_wait_for: r.longest_wait_for,
        log: r.w.log.take().unwrap_or_default(),
    }
}

pub fn describe(report: &Report) -> String {
    let seconds = |micros: Micros| micros as f64 / SEC as f64;
    let mut text = match &report.result {
        Ok(value) => format!("seed {} passed ({:.1} s)", report.seed, seconds(*value)),
        Err(fail) => format!(
            "seed {} FAILED at '{}': {}\n  replay: {} cargo test -p locust-core --lib \
             node::sim::tests::sim_one -- --ignored --nocapture",
            report.seed,
            fail.step,
            fail.what,
            report.options.replay(report.seed)
        ),
    };
    text.push_str(&format!(
        "\n  simulated {:.0} s, {} faults, {} frames, {} requests, digest {:016x}",
        seconds(report.simulated),
        report.stats.faults,
        report.stats.frames,
        report.stats.requests,
        report.digest
    ));
    for fault in &report.faults {
        text.push_str(&format!("\n  fault {fault}"));
    }
    text
}

/// The smallest replay of a failing seed that still fails: true clocks, no
/// stalls and no background faults when they do not matter, then the fewest
/// injected faults. The faults of
/// a seed are drawn in order, so a run limited to `n` faults sees the first
/// `n` of the full run.
pub fn shrink(failing: Report) -> Report {
    let mut best = failing;
    let seed = best.seed;
    let simpler: [fn(&mut Options); 3] = [
        |options| options.true_clocks = true,
        |options| options.no_stalls = true,
        |options| options.no_background = true,
    ];
    for simplify in simpler {
        let mut options = best.options;
        options.trace = false;
        simplify(&mut options);
        let report = run_seed(seed, options);
        if report.result.is_err() {
            best = report;
        }
    }
    for n in 0..best.stats.faults as u32 {
        let mut options = best.options;
        options.max_faults = Some(n);
        let report = run_seed(seed, options);
        if report.result.is_err() {
            return report;
        }
    }
    best
}

/// How many failing seeds of a sweep are made smaller.
const SHRUNK: usize = 3;

/// Runs `seeds` on every core and returns the failures, the first few made as
/// small as they go, after printing a summary.
pub fn sweep(seeds: std::ops::Range<u64>, options: Options) -> Vec<Report> {
    let started = Instant::now();
    let count = seeds.end.saturating_sub(seeds.start);
    let next = AtomicU64::new(seeds.start);
    let reports = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..threads.min(count as usize).max(1) {
            scope.spawn(|| {
                loop {
                    let seed = next.fetch_add(1, Ordering::Relaxed);
                    if seed >= seeds.end {
                        break;
                    }
                    let report = run_seed(seed, options);
                    reports.lock().expect("no panic holds it").push(report);
                }
            });
        }
    });
    let mut reports = reports.into_inner().expect("no panic holds it");
    reports.sort_by_key(|report| report.seed);
    let seconds = started.elapsed().as_secs_f64();

    let mut totals = Stats::default();
    let mut simulated = 0;
    let (mut value_max, mut value_seed) = (0, 0);
    let mut values = Vec::new();
    let (mut wait_max, mut wait_seed, mut wait_for) = (0, 0, "");
    let mut failures = Vec::new();
    for report in reports {
        totals.faults += report.stats.faults;
        totals.frames += report.stats.frames;
        totals.requests += report.stats.requests;
        totals.closed += report.stats.closed;
        totals.open_failed += report.stats.open_failed;
        totals.oversized += report.stats.oversized;
        simulated += report.simulated;
        if report.longest_wait > wait_max {
            wait_max = report.longest_wait;
            (wait_seed, wait_for) = (report.seed, report.longest_wait_for);
        }
        match report.result {
            Ok(value) => {
                values.push(value);
                if value > value_max {
                    (value_max, value_seed) = (value, report.seed);
                }
            }
            Err(_) => failures.push(report),
        }
    }
    values.sort_unstable();
    let at = |per_cent: usize| {
        values
            .get(values.len() * per_cent / 100)
            .copied()
            .unwrap_or(0)
    };
    let (median, ninetieth) = (at(50), at(90));
    let secs = |micros: Micros| micros as f64 / SEC as f64;
    eprintln!(
        "sim {:?}: {count} seeds in {seconds:.1} s ({:.1} seeds/s on {threads} threads), \
         {} failed; {} faults, {} frames, {} requests, {} exchanges closed, \
         {} failed to open, {} oversized frames, {:.1} simulated hours",
        options.scenario,
        count as f64 / seconds,
        failures.len(),
        totals.faults,
        totals.frames,
        totals.requests,
        totals.closed,
        totals.open_failed,
        totals.oversized,
        secs(simulated) / 3_600.0,
    );
    match options.scenario {
        Scenario::Guide => eprintln!(
            "sim: time to quiet after the last fault ended: median {:.1} s, 90th percentile \
             {:.1} s, longest {:.1} s (seed {value_seed}); longest fault-free wait in the \
             guide: {:.1} s (seed {wait_seed}, for {wait_for})",
            secs(median),
            secs(ninetieth),
            secs(value_max),
            secs(wait_max),
        ),
        Scenario::Prompt => eprintln!(
            "sim: slowest burst to show everywhere: median {:.1} s, longest {:.1} s \
             (seed {value_seed}); longest wait: {:.1} s (seed {wait_seed}, for {wait_for})",
            secs(median),
            secs(value_max),
            secs(wait_max),
        ),
        Scenario::Lag => eprintln!(
            "sim: wait for text written after waking: median {:.1} s, 90th percentile \
             {:.1} s, longest {:.1} s (seed {value_seed})",
            secs(median),
            secs(ninetieth),
            secs(value_max)
        ),
        Scenario::Joins => eprintln!(
            "sim: slower of two joins: median {:.1} s, 90th percentile {:.1} s, 99th \
             percentile {:.1} s, longest {:.1} s (seed {value_seed})",
            secs(median),
            secs(ninetieth),
            secs(at(99)),
            secs(value_max)
        ),
    }
    // Shrinking replays a seed many times, so only the first few get it.
    let failures: Vec<Report> = failures
        .into_iter()
        .enumerate()
        .map(|(index, report)| {
            if index < SHRUNK {
                shrink(report)
            } else {
                report
            }
        })
        .collect();
    for report in failures.iter().take(SHRUNK) {
        eprintln!("{}", describe(report));
    }
    for report in failures.iter().skip(SHRUNK).take(40) {
        if let Err(fail) = &report.result {
            eprintln!(
                "seed {} FAILED at '{}': {}",
                report.seed, fail.step, fail.what
            );
        }
    }
    failures
}
