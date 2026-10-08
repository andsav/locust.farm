//! G1's restore drills, read through the person's commands: two real
//! daemons on one computer, a data directory put back from a copy, and what
//! `status`, a refused command, an agent's post and `goal continue` print.
//! The host's computer is gated, so it opens no exchange until the drill
//! lets it, and its member's computer stays up at the address it had.
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use locust_proto::api::{Credential, GoalStatus, GuardReason, Request, Response};
use locust_proto::id::{EventId, GoalId};
use locust_proto::local;

use super::durable_tests::{
    Gated, Running, copy_stopped_home, eventually_observed, goal_status, rebind_rules, shared_goal,
    task_open,
};
use crate::cli::run_for_test as locust;
use crate::testdir::short_dir;

/// Which of the host's records the copy lacks: the drill's two forms.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lost {
    /// A task the host's agent opened: the first form.
    Post,
    /// A rule change the goal's own key signed: the second form.
    RuleChange,
}

/// The owner's command line on `home`, at the terminal.
fn owner(home: &Path, words: &[&str]) -> (u8, String) {
    let home = home.to_str().unwrap();
    let mut arguments = vec!["--home", home, "--owner"];
    arguments.extend_from_slice(words);
    locust(&arguments)
}

/// The goal's identifier as every line that continues it prints it.
fn cut(goal: GoalId) -> String {
    goal.to_string()[..8].to_owned()
}

/// Waits until the goal's status on `running`, read by its owner, meets
/// `ready`.
fn until(running: &Running, goal: GoalId, phase: &str, ready: impl Fn(&GoalStatus) -> bool) {
    let mut owner = running.owner();
    eventually_observed(
        phase,
        || goal_status(&mut owner, goal),
        |status| ready(status).then_some(()),
    );
}

/// The record the member's computer holds once it has received it.
fn received(member: &Running, goal: GoalId, event: EventId) {
    let mut agent = member.client(Credential([2; 32]), None);
    eventually_observed(
        "the member's computer holds the record",
        || agent.call(Request::Event { goal, event }),
        |answer| matches!(answer, Ok(Response::Event(_))).then_some(()),
    );
}

/// An agent credential file for the command line, readable only by its
/// owner.
fn credential_file(dir: &Path, tag: u8) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(format!("agent-{tag}.credential"));
    std::fs::write(&path, [tag; 32]).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    path.to_str().unwrap().to_owned()
}

/// A free port of the loopback address, for a host's computer that must be
/// found again where its tickets said after it is started from a copy.
fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The drill in either form, with the marks kept: the copy is put back
/// beside them, so the host's computer knows exactly what is missing. As in
/// G1's drill, the member's computer is stopped while the copy starts and
/// started afterwards; it calls the host's computer where the ticket said.
fn the_drill(lost: Lost) {
    let (original, member_home, backup, files) =
        (short_dir(), short_dir(), short_dir(), short_dir());
    let port = free_port();
    let marks = local::marks_dir(original.path()).unwrap();
    let mut host = Running::start_at(original.path(), &marks, port);
    let mut member = Running::start(member_home.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    host.stop();
    copy_stopped_home(original.path(), backup.path());
    let mut host = Running::start_at(original.path(), &marks, port);
    let record = match lost {
        Lost::Post => task_open(
            &mut host.client(Credential([1; 32]), None),
            goal,
            "After the copy",
        ),
        Lost::RuleChange => rebind_rules(&mut host.owner(), goal),
    }
    .unwrap();
    let mut member = Running::start(member_home.path());
    received(&member, goal, record);
    member.stop();
    host.stop();

    // The data directory put back from the copy, beside the marks it kept,
    // with the member's computer stopped.
    let restored = Running::start_at(backup.path(), &marks, port);
    let line = format!("locust --owner goal continue --goal {}", cut(goal));
    let (code, text) = owner(backup.path(), &["status"]);
    assert_eq!(code, 0, "{text}");
    let missing = match lost {
        Lost::Post => "1 record Host signed",
        Lost::RuleChange => "1 record this computer signed as host",
    };
    let block = format!(
        "\
  Catching up: this computer's Locust data is older than what it signed here.
    Missing: {missing}. Nothing is signed here until they come back from another computer in the goal.
    Heard from since this start: nobody yet.
    Not yet: Member's computer ("
    );
    assert!(text.starts_with("Nothing is waiting for you.\n"), "{text}");
    assert!(text.contains(&block), "{text}");
    assert!(
        text.contains(&format!(
            "\n    To continue without them: {line}\n  Restored from a copy. "
        )),
        "{text}"
    );
    // The goal's own key is never named.
    let governance = goal_status(&mut restored.owner(), goal).governance;
    assert!(!text.contains(&governance.to_string()[..8]), "{text}");

    // The person's own command: while the goal's own key is held, the
    // refusal and the continue line, and no plan.
    let (code, refused) = owner(
        backup.path(),
        &[
            "rules",
            "bind",
            "--goal",
            &cut(goal),
            "--formation",
            "directed",
            "--plan",
        ],
    );
    match lost {
        Lost::RuleChange => {
            assert_eq!(code, 9, "{refused}");
            assert_eq!(
                refused,
                format!(
                    "locust: read_only: You can't change the rules of \"Durable lifecycle\": the Locust data here is older than what this computer signed in the goal (this computer). It catches up by itself. To go on without waiting: {line}"
                )
            );
        }
        // Only the agent's record is missing: the goal's own key signs.
        Lost::Post => {
            assert_eq!(code, 0, "{refused}");
            assert!(refused.contains("\nPlan id: plan-"), "{refused}");
        }
    }

    // An agent's post: exit 9 and its own voice, with the side as data.
    let credential = credential_file(files.path(), 1);
    let (code, answer) = locust(&[
        "--home",
        backup.path().to_str().unwrap(),
        "--credential",
        &credential,
        "--json",
        "contribution",
        "publish",
        "--goal",
        &goal.to_string(),
        "notes",
    ]);
    assert_eq!(code, 9, "{answer}");
    let answer: serde_json::Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(answer["error"]["code"], "read_only");
    assert_eq!(answer["error"]["details"]["why"]["side"], "this_computer");
    assert_eq!(
        answer["error"]["message"],
        "agent-1 can't post to this goal: the Locust data here is older than what this computer signed in the goal (this computer). It catches up by itself, or agent-1's owner can continue without waiting."
    );

    // The member's computer answers: the block is gone, the restored line
    // stays until the next ordinary start.
    let member = Running::start(member_home.path());
    until(&restored, goal, "the record returned", |status| {
        status.guard.is_empty()
    });
    drop(member);
    let (code, text) = owner(backup.path(), &["status"]);
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("Catching up"), "{text}");
    assert!(text.contains("\n  Restored from a copy. "), "{text}");
}

#[test]
fn after_the_lost_rule_change_status_names_the_computers_and_the_hosts_command_is_refused() {
    the_drill(Lost::RuleChange);
}

#[test]
fn after_the_lost_post_status_names_the_hosts_agent_and_its_one_missing_record() {
    the_drill(Lost::Post);
}

/// The drill with the marks directory copied too: a copy of unknown age.
/// On the host's computer the goal waits for the person, also after the
/// member's computer answered; `goal continue` shows its plan and needs the
/// plan it showed.
#[test]
fn a_host_restored_with_its_marks_waits_for_the_person_and_continues_on_a_yes() {
    let (original, member_home, backup) = (short_dir(), short_dir(), short_dir());
    let port = free_port();
    let mut host = Running::start_at(
        original.path(),
        &local::marks_dir(original.path()).unwrap(),
        port,
    );
    let mut member = Running::start(member_home.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    host.stop();
    // Both folders copied: a copied marks file reads as lost.
    copy_stopped_home(original.path(), backup.path());
    let marks = local::marks_dir(backup.path()).unwrap();
    copy_stopped_home(&local::marks_dir(original.path()).unwrap(), &marks);
    let restored = Running::start_at(backup.path(), &marks, port);
    let line = format!("locust --owner goal continue --goal {}", cut(goal));
    let waiting = format!(
        "Waiting for you\n  \"Durable lifecycle\" is catching up: this computer's Locust data may be an old copy, and only you can say it is the newest\n    {line}\n"
    );
    let block = "  Catching up: this computer's Locust data may be an old copy.\n    Waiting for you: only you can say this is the newest copy of this computer's data.\n";
    // From the start.
    let (_, text) = owner(backup.path(), &["status"]);
    assert!(text.starts_with(&waiting), "{text}");
    assert!(text.contains(block), "{text}");
    assert!(
        text.contains(&format!("\n    To continue: {line}\n")),
        "{text}"
    );
    // And after the member's computer answered.
    let member = Running::start(member_home.path());
    until(&restored, goal, "the member's computer heard", |status| {
        status
            .guard
            .iter()
            .all(|hold| hold.reason == GuardReason::Unheard && !hold.heard.is_empty())
    });
    let (_, text) = owner(backup.path(), &["status"]);
    assert!(text.starts_with(&waiting), "{text}");
    assert!(
        text.contains("\n    Heard from since this start: Member's computer ("),
        "{text}"
    );

    let (code, plan) = owner(
        backup.path(),
        &["goal", "continue", "--goal", &cut(goal), "--plan"],
    );
    assert_eq!(code, 0, "{plan}");
    assert!(
        plan.starts_with(&format!(
            "Goal: Durable lifecycle ({}) · host: you\nContinue: sign in this goal from this computer's copy of the data.\n  This copy may be older than what this computer signed here, and nothing on this computer can tell.\n  They may include a removal, a rule change or the end of the goal.\n",
            cut(goal)
        )),
        "{plan}"
    );
    let id = plan
        .lines()
        .find_map(|line| line.strip_prefix("Plan id: "))
        .unwrap()
        .to_owned();
    let (code, stale) = owner(
        backup.path(),
        &[
            "goal",
            "continue",
            "--goal",
            &cut(goal),
            "--confirm",
            "plan-0000000000000000",
        ],
    );
    assert_eq!(code, 7, "{stale}");
    assert_eq!(
        stale,
        "locust: conflict: the plan changed; run --plan again"
    );
    let (code, done) = owner(
        backup.path(),
        &["goal", "continue", "--goal", &cut(goal), "--confirm", &id],
    );
    assert_eq!(code, 0, "{done}");
    assert_eq!(
        done,
        "Continued \"Durable lifecycle\". This computer signs here again."
    );
    let (_, text) = owner(backup.path(), &["status"]);
    assert!(text.starts_with("Nothing is waiting for you.\n"), "{text}");
    assert!(!text.contains("Catching up"), "{text}");
    let (code, again) = owner(backup.path(), &["goal", "continue", "--goal", &cut(goal)]);
    assert_eq!(
        (code, again.as_str()),
        (
            0,
            "\"Durable lifecycle\" is not catching up. Nothing changed."
        )
    );
    drop(member);
}

/// The same copy on the member's computer: its block waits for the host's
/// computer, and nothing waits for the person.
#[test]
fn a_member_restored_with_its_marks_waits_to_hear_from_the_hosts_computer() {
    let (host_home, original, copy) = (short_dir(), short_dir(), short_dir());
    let host = Running::start(host_home.path());
    let mut member = Running::start(original.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    copy_stopped_home(original.path(), copy.path());
    let marks = local::marks_dir(copy.path()).unwrap();
    copy_stopped_home(&local::marks_dir(original.path()).unwrap(), &marks);
    let open = Arc::new(AtomicBool::new(false));
    let gate = Arc::clone(&open);
    let restored =
        Running::start_observed(copy.path(), &marks, move |node| Gated { node, open: gate });
    let (_, text) = owner(copy.path(), &["status"]);
    assert!(text.starts_with("Nothing is waiting for you.\n"), "{text}");
    assert!(
        text.contains("\n  Catching up: this computer's Locust data may be an old copy.\n    Waiting to hear from the host's computer (Host), last seen "),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            ". Nothing is needed from you.\n    To continue without it: locust --owner goal continue --goal {}\n",
            cut(goal)
        )),
        "{text}"
    );
    // The host's computer answers and the hold ends with no command.
    open.store(true, Ordering::Release);
    until(&restored, goal, "the host's computer heard", |status| {
        status.guard.is_empty()
    });
    let (_, text) = owner(copy.path(), &["status"]);
    assert!(!text.contains("Catching up"), "{text}");
    drop(host);
}
