# Lane A review reproductions — 2026-10-03

Read the [review](../lane-a-review-2026-10-03.md) for interpretation, priorities and
limits. The [capture manifest](lane-a-review-snapshot.json) identifies the frozen
working files at base commit `bc4c25f2c2da44df74e288eafbe8ed76687ca409`.
The committed base alone cannot reproduce WIP findings: matching uncommitted
sources are also needed. Paths below describe the scratch copies used during the
review, not a new supported test harness. No production source was changed.

## Workspace export-root exclusion regression

Store this test as `crates/locust-workspace/tests/reviewer_root_selection.rs` in a
scratch copy of the committed base or matching working snapshot. The support path
below is normalized to that placement; the test body is the executed probe.
Run `cargo test --locked -p locust-workspace --test reviewer_root_selection -- --nocapture`
with a scratch `CARGO_TARGET_DIR`. This test asserts desired behavior and **fails**
on the reviewed code. All credential contents are fake.

```rust
#[path = "support/mod.rs"]
mod support;
use locust_workspace::export;
use support::{MemBlobs, TestRepo};

#[test]
fn denied_directory_as_export_root_does_not_upload_credentials() {
    let repo = TestRepo::new();
    repo.write(".aws/credentials", b"REVIEW_FAKE_AWS_SECRET");
    repo.write(".aws/nested/credentials", b"REVIEW_FAKE_NESTED_SECRET");
    repo.write(".kube/config", b"REVIEW_FAKE_KUBE_SECRET");
    repo.commit_all();
    let mut top = MemBlobs::default();
    let top_report = export(repo.path(), "HEAD", &mut top).unwrap();
    assert!(top_report.manifest.entries.is_empty());
    let mut leaked = Vec::new();
    for root in [".aws", ".aws/nested", ".kube"] {
        let mut store = MemBlobs::default();
        let report = export(&repo.path().join(root), "HEAD", &mut store).unwrap();
        eprintln!("root={root}, shared={:?}, left_out={:?}, objects={}", report.manifest.entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), report.left_out, store.objects.len());
        if !report.manifest.entries.is_empty() { leaked.push(root); }
    }
    assert!(leaked.is_empty(), "denied ancestors lost when selection rebases paths: {leaked:?}");
}
```

Captured result:

```text
root=.aws, shared=["credentials", "nested/credentials"], left_out=[], objects=3
root=.aws/nested, shared=["credentials"], left_out=[], objects=2
root=.kube, shared=["config"], left_out=[], objects=2
denied ancestors lost when selection rebases paths: [".aws", ".aws/nested", ".kube"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
```

## Authority rule probes

These tests assert the observed defects and **pass** on the reviewed code. They
exercise History, screening and the full fold, not public Goal/Node/daemon entry
points. Progress disappearing from pending review is a source-backed consequence
of the folded task state, not an executed public API observation.

In a separate copy of the matching working source, save the following diff and
apply it using `patch -p1`. It declares otherwise-unwired goal modules for tests,
copies three task IDs to shorten mutable borrows, and disables unrelated sync
tests that do not compile. It does not repair the rules under review.

```diff
--- frozen/crates/locust-core/src/goal/mod.rs
+++ probe/crates/locust-core/src/goal/mod.rs
@@ -174,0 +175,8 @@
+
+#[cfg(test)] mod history;
+#[cfg(test)] mod chain;
+#[cfg(test)] mod fold;
+#[cfg(test)] mod transition;
+#[cfg(test)] mod append;
+#[cfg(test)] mod screen;
+#[cfg(test)] mod review_probe;
--- frozen/crates/locust-core/src/goal/transition.rs
+++ probe/crates/locust-core/src/goal/transition.rs
@@ -155 +155,2 @@
-    let task = task_at(state, &entry.task).ok_or("the assignment has no task")?;
+    let task_id = entry.task;
+    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
@@ -342 +343,2 @@
-    let task = task_at(state, &entry.task).ok_or("the assignment has no task")?;
+    let task_id = entry.task;
+    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
@@ -384 +386,2 @@
-    let task = task_at(state, &entry.task).ok_or("the assignment has no task")?;
+    let task_id = entry.task;
+    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
--- frozen/crates/locust-core/src/sync/mod.rs
+++ probe/crates/locust-core/src/sync/mod.rs
@@ -30 +30 @@
-#[cfg(test)]
+# [cfg(any())]
```

Then write `crates/locust-core/src/goal/review_probe.rs` with:

```rust
use locust_proto::event::{AuthorPoint, Body, Event, Header};
use locust_proto::id::EndpointId;
use locust_proto::testkit::Author;
use super::{history::History, fold::fold, screen::screen, Standing};

fn held(events: &[Event]) -> History {
    let mut h=History::default();
    for e in events { h.insert(e); }
    h
}

#[test]
fn duplicate_admission_changes_endpoint() {
    let mut c=Author::new(1);
    let (g,a)=c.found_goal(EndpointId([1;32]));
    let rebind=c.event(g.header().goal,Some(a.id()),Body::MemberAdmitted{member:c.key.public(),endpoint:EndpointId([9;32])});
    let (f,_)=fold(&held(&[g,a,rebind]));
    assert_eq!(f.state.members[&c.key.public()],EndpointId([9;32]));
    assert_eq!(f.standings[2],Standing::Effective);
}

#[test]
fn removed_member_result_can_finalize() {
    let mut c=Author::new(1);
    let mut w=Author::new(2);
    let (g,a)=c.found_goal(EndpointId([1;32]));
    let goal=g.header().goal;
    let admit=c.event(goal,Some(a.id()),Body::MemberAdmitted{member:w.key.public(),endpoint:EndpointId([2;32])});
    let task=c.event(goal,Some(admit.id()),Body::TaskProposed{input:None,depends_on:vec![],deadline_ms:None,max_attempts:None});
    let assign=c.event(goal,Some(admit.id()),Body::TaskAssigned{task:task.id(),assignee:w.key.public(),attempt:1});
    let take=w.event(goal,Some(assign.id()),Body::AssignmentAccepted{assignment:assign.id()});
    let result=w.event(goal,Some(assign.id()),Body::ResultSubmitted{assignment:assign.id(),base:None,patch:None,artifacts:vec![]});
    let remove=c.event(goal,Some(assign.id()),Body::MemberRemoved{member:w.key.public(),last_accepted:Some(AuthorPoint{seq:result.header().seq,id:result.id()})});
    let accept=c.event(goal,Some(remove.id()),Body::ResultAccepted{result:result.id(),head:None});
    let (f,_)=fold(&held(&[g,a,admit,task,assign,take,result.clone(),remove,accept]));
    assert!(!f.state.is_member(&w.key.public()));
    assert_eq!(f.state.tasks[0].accepted,Some(result.id()));
    assert_eq!(f.standings[6],Standing::Effective);
    assert_eq!(f.standings[7],Standing::Effective);
    assert_eq!(f.standings[8],Standing::Effective);
}

#[test]
fn full_waiting_buffer_discards_fork_evidence() {
    let mut c=Author::new(1);
    let (g,a)=c.found_goal(EndpointId([1;32]));
    let mut h=held(&[g.clone(),a.clone()]);
    // Author seq2 but do not deliver it; every later prev link is genuine.
    let _missing=c.event(g.header().goal,Some(a.id()),Body::Note{about:None,supersedes:None});
    let backlog:Vec<_>=(3..1027).map(|_| c.event(g.header().goal,Some(a.id()),Body::Note{about:None,supersedes:None})).collect();
    let admitted=screen(g.header().goal,&h,backlog);
    assert_eq!(admitted.len(),1024);
    for event in admitted { h.insert(&event); }
    let mut header:Header=a.header().clone();
    header.at_ms+=1;
    let fork=Event::sign(header,&c.key).unwrap();
    assert_eq!(h.log(&c.key.public()).unwrap().waiting(),1024);
    assert!(screen(g.header().goal,&h,vec![fork.clone()]).is_empty());
    assert!(fold(&h).0.chain.halt.is_none());
    h.insert(&fork);
    assert!(matches!(fold(&h).0.chain.halt,Some(super::Halt::Fork{seq:1,..})));
}

#[test]
fn later_progress_hides_submitted_result_from_review() {
    let mut c=Author::new(1);
    let mut w=Author::new(2);
    let (g,a)=c.found_goal(EndpointId([1;32]));
    let goal=g.header().goal;
    let admit=c.event(goal,Some(a.id()),Body::MemberAdmitted{member:w.key.public(),endpoint:EndpointId([2;32])});
    let task=c.event(goal,Some(admit.id()),Body::TaskProposed{input:None,depends_on:vec![],deadline_ms:None,max_attempts:None});
    let assign=c.event(goal,Some(admit.id()),Body::TaskAssigned{task:task.id(),assignee:w.key.public(),attempt:1});
    let take=w.event(goal,Some(assign.id()),Body::AssignmentAccepted{assignment:assign.id()});
    let result=w.event(goal,Some(assign.id()),Body::ResultSubmitted{assignment:assign.id(),base:None,patch:None,artifacts:vec![]});
    let progress=w.event(goal,Some(assign.id()),Body::Progress{assignment:assign.id()});
    let (f,_)=fold(&held(&[g,a,admit,task,assign,take,result.clone(),progress]));
    assert_eq!(f.state.tasks[0].result,Some(result.id()));
    assert_eq!(f.state.tasks[0].state,super::TaskState::Taken);
    assert_eq!(f.standings[7],Standing::Effective);
}
```

Run `cargo test --locked -p locust-core --lib goal::review_probe -- --nocapture`
with a scratch target directory. The strengthened final probe run reports
**4 passed, 0 failed**. The fork probe first confirms that ordinary screening
admits all 1,024 genuinely linked backlog events with sequence 2 withheld; it then
shows the conflicting sequence-1 event is dropped, while retaining that same
conflict directly makes the fold halt. The removal probe confirms submission,
removal and later acceptance all become effective. No production-path authority
or persistence result is implied by these test-only repairs.

## Sync machine probes

Reviewed frozen snapshot: `working/`, captured in `snapshot.json` at base commit `bc4c25f2c2da44df74e288eafbe8ed76687ca409`. Probe source is a separate copy in `sync-probe/`. No primary source or frozen snapshot was edited. Commands used the pinned toolchain, offline dependency resolution, and an isolated target directory. No commits were made.

### Exact protocol wording

`working/crates/locust-proto/src/sync.rs:11-14` says:

```rust
//! [`SyncMessage`]. The transport decides nothing about membership: whether a
//! remote endpoint speaks for a member, which is whether a `MemberAdmitted`
//! decision of the goal binds a current member to it, is decided by the
//! daemon for every request.
```

The responder retains admission from `Hello`; `serve` rechecks current membership only for `KeyRequest` (`responder.rs:134-138`). The removal probe therefore tests this per-request requirement for `EventRequest`. Member removal is outside T1.

### Commands and results

Run from `/Users/andrei/Projects26/locust/output/lane-a-review-2026-10-03/working`:

```sh
CARGO_TARGET_DIR=/Users/andrei/Projects26/locust/target/lane-a-review-sync cargo test --offline --locked -p locust-core sync::
CARGO_TARGET_DIR=/Users/andrei/Projects26/locust/target/lane-a-review-sync cargo test --offline --locked -p locust-proto
```

- Unmodified frozen core: compile fails E0599, missing `Founded::clone_goal`, `sync/tests/machines.rs:42`.
- Unmodified frozen proto: 155 tests passed; zero failed; zero doc tests.

Prepare a fresh scratch copy, then save the exact fenced diff below as `sync-probe.patch` and apply it with `patch -p1 < ../sync-probe.patch` from the copy. The copy command assumes the destination does not already exist; the reviewed `sync-probe/` is currently retained with the changes applied:

```sh
cp -R /Users/andrei/Projects26/locust/output/lane-a-review-2026-10-03/working /Users/andrei/Projects26/locust/output/lane-a-review-2026-10-03/sync-probe
```

Run from `/Users/andrei/Projects26/locust/output/lane-a-review-2026-10-03/sync-probe`:

```sh
CARGO_TARGET_DIR=/Users/andrei/Projects26/locust/target/lane-a-review-sync cargo test --offline --locked -p locust-core sync::
CARGO_TARGET_DIR=/Users/andrei/Projects26/locust/target/lane-a-review-sync cargo test --offline --locked -p locust-core sync:: -- --skip review_
```

- Full scratch sync run: 20 tests; 17 passed, three failed. Exit 101 is expected from the three added desired-behavior probes.
- Scratch sync run excluding `review_`: 17 passed, zero failed, 21 filtered out. This includes expanded genuine inventory pagination and the corrected chunk-count assertion.
- Scratch warnings remain for unfinished core code; no clean clippy/fmt claim is made.

### Interpretation of scratch changes

The helper only makes the existing test fixture compile. The original chunk test asks for `2 MiB + 5` bytes starting at offset `7`, leaving `2 MiB - 2`: the correct frame count is two. Changing its assertion from three to two repairs the test expectation, not the implementation.

The existing pagination test uses 4,106 variants split roughly one-third/two-thirds, so each peer's inventory fits within 4,096 points. The scratch version uses 8,292 variants, making the larger peer exceed a page; this expanded test passes.

All three added `review_` probes **assert desired behavior and fail on the current implementation**. They do not assert the observed defect and pass:

- Completion probe: expects no `Completed` report before any delivery/finish confirmation. Observed: report is recorded immediately after emitting `Done` and `Finish`.
- Membership probe: expects `Refused(NotAMember)` after an admitted endpoint is removed. Observed: returns `Events`.
- Content probe: expects the available 257th wanted object to arrive after repeated exchanges. Observed: 256 unavailable earlier objects occupy every selection slot, and the target remains absent. This uses the supplied deterministic `TestReplica`; production `Replica` is still absent, so classify it as a machine/selection-contract integration hazard.

The tests use fake hosts/replicas, not deployed daemons or the real transport. They do not prove production persistence or three-machine behavior.

### Exact scratch diff

Paths are relative to the scratch copy; apply only there.

```diff
--- a/crates/locust-core/src/sync/tests/mod.rs
+++ b/crates/locust-core/src/sync/tests/mod.rs
@@ -49,0 +50,4 @@
+    pub fn clone_goal(&self) -> Self {
+        Self { goal: self.goal, genesis: self.genesis.clone(), owner: Author::new(0) }
+    }
+
--- a/crates/locust-core/src/sync/tests/machines.rs
+++ b/crates/locust-core/src/sync/tests/machines.rs
@@ -262 +262 @@
-    assert_eq!((at, chunks.len()), (total, 3));
+    assert_eq!((at, chunks.len()), (total, 2));
@@ -318,0 +319,12 @@
+
+#[test]
+fn review_revoked_member_gets_no_event_or_blob() {
+    let (founded, mut host, held) = served();
+    let mut responder = Responder::new(endpoint(2));
+    let mut out = Vec::new();
+    responder.receive(&mut host, hello(founded.goal), 0, &mut out);
+    assert!(responder.is_admitted());
+    host.members.get_mut(&founded.goal).unwrap().remove(&endpoint(2));
+    responder.receive(&mut host, SyncMessage::EventRequest(vec![held[0]]), 1, &mut out);
+    assert_eq!(out, [SyncMessage::Refused(Refusal::NotAMember)]);
+}
--- a/crates/locust-core/src/sync/tests/convergence.rs
+++ b/crates/locust-core/src/sync/tests/convergence.rs
@@ -180 +180 @@
-    let variants = MAX_INVENTORY_POINTS + 10;
+    let variants = MAX_INVENTORY_POINTS * 2 + 100;
--- a/crates/locust-core/src/sync/tests/driver.rs
+++ b/crates/locust-core/src/sync/tests/driver.rs
@@ -1 +1,22 @@
-//! Placeholder.
+use locust_proto::engine::{ExchangeId, PeerInput, PeerOutput};
+use locust_proto::sync::SyncMessage;
+use crate::sync::{Driver, Ended, Host};
+use super::{Founded, host};
+
+#[test]
+fn review_completion_waits_for_finish_acknowledgement() {
+    let founded = Founded::new();
+    let mut host = host(1, founded.replica(&[]), &[1, 2]);
+    let mut driver = Driver::new();
+    let mut out = Vec::new();
+    driver.handle(&mut host, PeerInput::Poll, 0, &mut out);
+    let exchange = ExchangeId::Dialed(0);
+    assert!(matches!(out[0], PeerOutput::Open { .. }));
+    out.clear();
+    driver.handle(&mut host, PeerInput::Opened(exchange), 1, &mut out);
+    out.clear();
+    let frontier = host.replica(&founded.goal).unwrap().frontier();
+    driver.handle(&mut host, PeerInput::Frame { exchange, frame: SyncMessage::Frontier(frontier) }, 2, &mut out);
+    assert!(out.iter().any(|item| matches!(item, PeerOutput::Finish(_))));
+    assert!(host.reports.iter().all(|r| r.ended != Ended::Completed), "completed recorded before shell delivered or acknowledged Done");
+}
--- a/crates/locust-core/src/sync/tests/content.rs
+++ b/crates/locust-core/src/sync/tests/content.rs
@@ -1 +1,25 @@
-//! Placeholder.
+use locust_proto::id::BlobHash;
+use locust_proto::store::Blob;
+use super::{Founded, host};
+use super::net::Net;
+
+#[test]
+fn review_unavailable_early_blobs_do_not_starve_later_available_content() {
+    let founded = Founded::new();
+    let blob = Blob::new(b"available later content".to_vec());
+    let target = blob.hash();
+    assert!(target.0[0] > 0);
+    let mut left = founded.replica(&[]);
+    for n in 0..256u16 {
+        let mut bytes = [0u8; 32];
+        bytes[30..].copy_from_slice(&n.to_be_bytes());
+        left.wants.insert(BlobHash(bytes), 10);
+    }
+    left.wants.insert(target, b"available later content".len() as u64);
+    let mut right = founded.replica(&[]);
+    right.add_blob(blob);
+    let mut net = Net::new(vec![host(1, left, &[1,2]), host(2, right, &[1,2])]);
+    net.poll(&[0,1]);
+    for _ in 0..5 { net.tick(61_000); }
+    assert!(net.host(0).replica_mut(&founded.goal).holds_blob(&target), "available object after first 256 never requested");
+}
```
