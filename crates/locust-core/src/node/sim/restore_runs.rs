//! Restores built step by step rather than drawn from a seed's faults. m1
//! hosts the goal, m2 and m3 are members.
//!
//! Four of them are the runs the restore guard is known to lose, kept so
//! that they stay lost in the way the plan says: each must end in a
//! position of a member's agent signed twice, outside the claims of
//! `a_restored_machine_signs_at_no_used_position_unless_its_owner_continued`
//! (residuals 5 and 6 in the notes of G1 in
//! `docs/host-safety-and-ending-plan.md`).

use locust_proto::api::Request;
use locust_proto::id::EventId;
use locust_proto::store::Store;

use super::check::reuses;
use super::machine::Who;
use super::restore::Marks;
use super::run::Run;
use super::scenario;
use super::world::SEC;

const M1: usize = 0;
const M2: usize = 1;
const M3: usize = 2;

/// Three members of one goal on three machines, with no faults, and a
/// backup of m2 and of m3 taken once all three are members.
fn joined(seed: u64) -> Run {
    let mut r = Run::new(seed, 3);
    r.faults = false;
    r.w.net.stall = 0;
    r.w.chaos.allow_restores = true;
    scenario::setup(&mut r).unwrap();
    scenario::create(&mut r).unwrap();
    scenario::join(&mut r, M2, 2).unwrap();
    scenario::join(&mut r, M3, 3).unwrap();
    r.w.run_for(60 * SEC);
    for m in [M2, M3] {
        r.w.ready[m] = true;
        r.w.backup(m);
        assert_eq!(r.w.backups[m].len(), 1);
    }
    r
}

fn cut(r: &mut Run, a: usize, b: usize, blocked: bool) {
    r.w.set_blocked(a, b, blocked);
    r.w.set_blocked(b, a, blocked);
}

fn holds(r: &Run, m: usize, event: EventId) -> bool {
    r.w.machines[m].store.event(&event).unwrap().is_some()
}

fn seq(r: &Run, m: usize, event: EventId) -> u64 {
    let event = r.w.machines[m].store.event(&event).unwrap().unwrap();
    event.header().seq
}

/// m2 writes again once the guard lets it, and the new record takes the
/// position of `first`, which m3 still holds.
fn reused_after(r: &mut Run, first: EventId, at: u64) {
    assert!(
        r.signs(M2),
        "m2 is still held: {:?}",
        r.read(M2, goal_status(r))
    );
    let again = scenario::finding(r, M2, "written again after the restore").unwrap();
    assert_eq!(seq(r, M2, again), at);
    let found = reuses(r);
    let reuse = found
        .iter()
        .find(|reuse| reuse.key == r.principals[M2] && reuse.seq == at)
        .unwrap_or_else(|| panic!("no reuse at {at}: {found:?}"));
    assert!(!reuse.claimed, "{}", reuse.describe(r));
    assert!(reuse.records.contains(&first) && reuse.records.contains(&again));
    assert!(holds(r, M3, first));
}

fn goal_status(r: &Run) -> Request {
    Request::GoalStatus { goal: r.goal() }
}

/// m2 is put back with its marks lost. Its agent's last record reached m3
/// and not the host's computer, and the hold of a member's computer after a
/// copy of unknown age ends once it hears from the host's computer.
#[test]
fn a_member_restored_with_its_marks_lost_signs_again_what_only_another_member_holds() {
    let mut r = joined(8201);
    cut(&mut r, M1, M2, true);
    cut(&mut r, M1, M3, true);
    let first = scenario::finding(&mut r, M2, "reached m3 only").unwrap();
    let at = seq(&r, M2, first);
    r.w.run_for(60 * SEC);
    assert!(holds(&r, M3, first) && !holds(&r, M1, first));
    r.w.stop(M3, true);
    r.w.put_back(M2, 0, Marks::Lost, true);
    cut(&mut r, M1, M2, false);
    r.w.start(M2);
    assert!(!r.signs(M2), "the restored m2 is held at its start");
    r.w.run_for(120 * SEC);
    reused_after(&mut r, first, at);
}

/// m2 is put back with its marks kept, to a copy that does not hold m3's
/// removal. Its agent's last record reached only m3. Once m2 hears from the
/// host it learns of the removal, every other computer has answered, and
/// the mark is given up.
#[test]
fn a_member_restored_with_its_marks_kept_gives_up_what_only_a_removed_member_holds() {
    let mut r = joined(8202);
    cut(&mut r, M1, M2, true);
    cut(&mut r, M1, M3, true);
    let first = scenario::finding(&mut r, M2, "reached m3 only").unwrap();
    let at = seq(&r, M2, first);
    r.w.run_for(60 * SEC);
    assert!(holds(&r, M3, first) && !holds(&r, M1, first));
    r.w.stop(M3, true);
    let goal = r.goal();
    let member = r.principals[M3];
    r.record(
        M1,
        Who::Owner,
        "remove",
        Request::MemberRemove { goal, member },
    )
    .unwrap();
    r.w.put_back(M2, 0, Marks::Kept, true);
    cut(&mut r, M1, M2, false);
    r.w.start(M2);
    assert!(!r.signs(M2), "the restored m2 is held at its start");
    r.w.run_for(120 * SEC);
    reused_after(&mut r, first, at);
}

/// m2 and the host's computer are each put back to a backup taken before
/// m3 was admitted, m2 with its marks kept. m2's agent's last record
/// reached only m3, which is stopped. m2's copy does not wait for the
/// member it does not know, the restored host cannot answer for it, and
/// once every computer the copy knows has answered, the mark is given up.
#[test]
fn a_member_restored_with_its_marks_kept_gives_up_what_only_a_member_admitted_since_holds() {
    let mut r = Run::new(8204, 3);
    r.faults = false;
    r.w.net.stall = 0;
    r.w.chaos.allow_restores = true;
    scenario::setup(&mut r).unwrap();
    scenario::create(&mut r).unwrap();
    scenario::join(&mut r, M2, 2).unwrap();
    r.w.run_for(60 * SEC);
    for m in [M1, M2] {
        r.w.ready[m] = true;
        r.w.backup(m);
        assert_eq!(r.w.backups[m].len(), 1);
    }
    scenario::join(&mut r, M3, 3).unwrap();
    r.w.run_for(60 * SEC);
    cut(&mut r, M1, M2, true);
    cut(&mut r, M1, M3, true);
    let first = scenario::finding(&mut r, M2, "reached m3 only").unwrap();
    let at = seq(&r, M2, first);
    r.w.run_for(60 * SEC);
    assert!(holds(&r, M3, first) && !holds(&r, M1, first));
    r.w.stop(M3, true);
    r.w.put_back(M2, 0, Marks::Kept, true);
    r.w.put_back(M1, 0, Marks::Kept, true);
    cut(&mut r, M1, M2, false);
    r.w.start(M2);
    r.w.start(M1);
    assert!(!r.signs(M2), "the restored m2 is held at its start");
    r.w.run_for(120 * SEC);
    reused_after(&mut r, first, at);
}

/// m3's agent has signed nothing in the goal when m3 is put back with its
/// marks lost. Once it has heard from the host's computer the goal is
/// caught up, and a restart after that is ordinary: the start that found
/// the copy wrote the marks again although there was no mark to write, so
/// they no longer read as lost.
#[test]
fn a_restart_after_a_copy_of_unknown_age_was_caught_up_holds_nothing() {
    let mut r = joined(8203);
    let goal = r.goal();
    let agent = r.principals[M3];
    let log = r.w.machines[M3].store.log(&goal, 0, usize::MAX).unwrap();
    assert!(log.iter().all(|(_, event)| event.header().author != agent));
    r.w.put_back(M3, 0, Marks::Lost, true);
    r.w.start(M3);
    assert!(!r.signs(M3), "the restored m3 is held at its start");
    r.w.run_for(60 * SEC);
    assert!(
        r.signs(M3),
        "m3 is still held: {:?}",
        r.read(M3, goal_status(&r))
    );
    r.w.stop(M3, true);
    r.w.start(M3);
    assert!(
        r.signs(M3),
        "m3 is held again: {:?}",
        r.read(M3, goal_status(&r))
    );
}

/// m3's agent signs a record after m3's backup, and it reaches m2 only. m3
/// is put back whole, its marks lost: the goal is a copy of unknown age, and
/// no mark carries that, since the agent has no record in the copy. Before
/// m3 hears from anyone its data directory alone goes back to the same
/// backup with the marks kept. Nothing marks the agent, so it signs at once
/// at the position m2 holds.
#[test]
fn a_member_whose_agent_had_no_record_in_the_copy_signs_again_after_a_second_restore() {
    let mut r = joined(8206);
    let goal = r.goal();
    let agent = r.principals[M3];
    let log = r.w.machines[M3].store.log(&goal, 0, usize::MAX).unwrap();
    assert!(log.iter().all(|(_, event)| event.header().author != agent));
    cut(&mut r, M1, M2, true);
    cut(&mut r, M1, M3, true);
    let first = scenario::finding(&mut r, M3, "reached m2 only").unwrap();
    let at = seq(&r, M3, first);
    r.w.run_for(60 * SEC);
    assert!(holds(&r, M2, first) && !holds(&r, M1, first));
    cut(&mut r, M2, M3, true);
    r.w.put_back(M3, 0, Marks::Lost, true);
    r.w.start(M3);
    assert!(!r.signs(M3), "the restored m3 is held at its start");
    r.w.run_for(10 * SEC);
    assert!(!r.signs(M3), "m3 has heard from nobody");
    r.w.put_back(M3, 0, Marks::Kept, true);
    r.w.start(M3);
    assert!(r.signs(M3), "no mark holds the agent");
    let again = scenario::finding(&mut r, M3, "written again after the restore").unwrap();
    assert_eq!(seq(&r, M3, again), at);
    let found = reuses(&r);
    let reuse = found
        .iter()
        .find(|reuse| reuse.key == agent && reuse.seq == at)
        .unwrap_or_else(|| panic!("no reuse at {at}: {found:?}"));
    assert!(reuse.records.contains(&first) && reuse.records.contains(&again));
    assert!(!reuse.claimed, "{}", reuse.describe(&r));
}
