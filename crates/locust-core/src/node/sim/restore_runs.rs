//! Restores built step by step rather than drawn from a seed's faults. m1
//! hosts the goal, m2 and m3 are members.
//!
//! Five of them are the runs the restore guard is known to lose, kept so
//! that they stay lost in the way the plan says: each must end in a
//! position of a member's agent signed twice, outside the claims of
//! `a_restored_machine_signs_at_no_used_position_unless_its_owner_continued`
//! (risk (5) in the notes of G1 in `docs/host-safety-and-ending-plan.md`).

use locust_proto::api::Request;
use locust_proto::id::EventId;
use locust_proto::store::Store;

use super::check::{
    host_missed_the_same_admission, lost_with_the_marks, reuses, settle, violations,
};
use super::machine::Who;
use super::restore::Marks;
use super::run::{Acked, Run};
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

/// m2 and the host's computer each put back to a backup taken before m3 was
/// admitted, m2 with its marks kept, after m2's agent wrote a record that
/// reached only m3, which is stopped. Returns that record and its position.
fn restored_before_an_admission(seed: u64) -> (Run, EventId, u64) {
    let mut r = Run::new(seed, 3);
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
    (r, first, at)
}

/// m2 and the host's computer are each put back to a backup taken before
/// m3 was admitted, m2 with its marks kept. m2's agent's last record
/// reached only m3, which is stopped. m2's copy does not wait for the
/// member it does not know, the restored host cannot answer for it, and
/// once every computer the copy knows has answered, the mark is given up.
#[test]
fn a_member_restored_with_its_marks_kept_gives_up_what_only_a_member_admitted_since_holds() {
    let (mut r, first, at) = restored_before_an_admission(8204);
    reused_after(&mut r, first, at);
}

/// The same run, brought to quiet with m3 reachable again: what breaks is what
/// the fork explains, so the run would count as a residual. An acknowledged
/// record that no machine holds is no consequence of the fork, and beside it
/// the run would be a failure.
#[test]
fn a_failure_beside_an_unclaimed_reuse_is_residual_only_while_the_fork_explains_it() {
    let (mut r, first, at) = restored_before_an_admission(8204);
    reused_after(&mut r, first, at);
    cut(&mut r, M1, M3, false);
    r.w.start(M3);
    let failed = settle(&mut r).expect_err("m2's agent has two records at one position");
    assert!(failed.forked, "{}", failed.what);
    r.acked.push(Acked {
        what: "record held nowhere",
        machine: M1,
        event: EventId([7; 32]),
    });
    let found = violations(&mut r, false);
    assert!(found.iter().any(|violation| violation.forked));
    assert!(
        found
            .iter()
            .any(|violation| !violation.forked && violation.text.contains("record held nowhere")),
        "{found:?}"
    );
}

/// The host's computer is put back before m3 is admitted, to a copy that
/// lacks the admission only because it did not exist yet, and m3 is admitted
/// afterwards. m2 is then put back with its marks kept to a copy from before
/// the admission. The host's computer knows m3 again and answers for it, so
/// m2 waits for m3, and a reuse of m2's agent here would be inside the
/// claims: the fourth case of risk (5) needs a host restore that lost the
/// admission.
#[test]
fn a_host_restore_from_before_an_admission_existed_leaves_the_claim_standing() {
    let mut r = Run::new(8205, 3);
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
    r.w.put_back(M1, 0, Marks::Kept, true);
    r.w.start(M1);
    r.w.run_for(60 * SEC);
    assert!(
        r.signs(M1),
        "m1 is still held: {:?}",
        r.read(M1, goal_status(&r))
    );
    scenario::join(&mut r, M3, 3).unwrap();
    r.w.run_for(60 * SEC);
    r.w.put_back(M2, 0, Marks::Kept, true);
    let member = r.w.restores.last().unwrap().clone();
    assert_eq!(member.m, M2);
    assert!(!host_missed_the_same_admission(&r, &member));
    r.w.start(M2);
    r.w.run_for(120 * SEC);
    assert!(
        r.signs(M2),
        "m2 is still held: {:?}",
        r.read(M2, goal_status(&r))
    );
    scenario::finding(&mut r, M2, "written after the restore").unwrap();
    r.w.run_for(60 * SEC);
    assert!(reuses(&r).is_empty(), "{:?}", reuses(&r));
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

/// The first run, put off past a second restore. m2 is put back whole, its
/// marks lost, after its agent's last record reached m3 and not the host's
/// computer, and its hold ends once it hears from the host's computer. Its
/// agent signs nothing yet. Then m2's data directory alone goes back to the
/// same backup with the marks kept: the marks were written again from the
/// copy and name nothing it lacks, so the start holds nothing and the agent
/// signs at the position m3 holds. The marks-kept restore lost nothing the
/// marks knew; the record was lost with the marks.
#[test]
fn a_member_restored_with_its_marks_lost_and_then_kept_signs_again_what_only_another_member_holds()
{
    let mut r = joined(8207);
    scenario::finding(&mut r, M2, "in the backup").unwrap();
    r.w.run_for(60 * SEC);
    r.w.backups[M2].clear();
    r.w.backup(M2);
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
    assert!(
        r.signs(M2),
        "m2 has heard from the host's computer: {:?}",
        r.read(M2, goal_status(&r))
    );
    r.w.put_back(M2, 0, Marks::Kept, true);
    let second = r.w.restores.last().unwrap();
    let marked = second.marked.get(&(r.goal(), r.principals[M2]));
    assert!(marked.is_some_and(|marked| *marked < at), "{marked:?}");
    r.w.start(M2);
    reused_after(&mut r, first, at);
}

/// As the run before, but m3 answers m2 after the first restore and its
/// record comes back, which raises the mark. The second restore, with the
/// marks kept, then finds the record missing and m2 waits for it: a reuse
/// here would be inside the claims, since the marks knew the position.
#[test]
fn a_record_that_came_back_after_the_marks_were_lost_leaves_the_claim_standing() {
    let mut r = joined(8208);
    scenario::finding(&mut r, M2, "in the backup").unwrap();
    r.w.run_for(60 * SEC);
    r.w.backups[M2].clear();
    r.w.backup(M2);
    cut(&mut r, M1, M2, true);
    cut(&mut r, M1, M3, true);
    let first = scenario::finding(&mut r, M2, "reached m3 only").unwrap();
    let at = seq(&r, M2, first);
    r.w.run_for(60 * SEC);
    assert!(holds(&r, M3, first) && !holds(&r, M1, first));
    r.w.put_back(M2, 0, Marks::Lost, true);
    cut(&mut r, M1, M2, false);
    r.w.start(M2);
    r.w.run_for(120 * SEC);
    assert!(holds(&r, M2, first), "m3 sent the record back");
    assert!(r.signs(M2));
    r.w.put_back(M2, 0, Marks::Kept, true);
    let second = r.w.restores.last().unwrap();
    let marked = second.marked.get(&(r.goal(), r.principals[M2])).copied();
    assert!(marked.is_some_and(|marked| marked >= at), "{marked:?}");
    let records = [first, EventId([7; 32])].into();
    assert!(!lost_with_the_marks(&r, second, at, &records));
    r.w.start(M2);
    assert!(!r.signs(M2), "the record the marks name is missing");
    r.w.run_for(120 * SEC);
    assert!(
        r.signs(M2),
        "m2 is still held: {:?}",
        r.read(M2, goal_status(&r))
    );
    let after = scenario::finding(&mut r, M2, "written after the restore").unwrap();
    assert!(seq(&r, M2, after) > at);
    r.w.run_for(60 * SEC);
    assert!(reuses(&r).is_empty(), "{:?}", reuses(&r));
}
