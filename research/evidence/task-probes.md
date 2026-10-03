# Task lifecycle characterization probes

Research date: 2026-10-03. These are original research probes against MoltMesh commit `707c870e3188df243e5aea3c4662daa3253270bc`. **A passing result confirms the reported gap, not correct behavior.** All test data and identities are synthetic and local. See the [validation report](../moltmesh-validation.md) for scope and limitations.

## Reproduction

In a disposable checkout of the pinned revision, save the source below as `researchprobe/task_semantics_test.go` and run with Go 1.26.5 and CGO enabled:

```sh
go test -race -v ./researchprobe
```

The upstream checkout must retain its normal module/dependencies. This probe belongs to that Go module, not Locust's Rust workspace. No upstream production file needs to be changed. The task probe source below was gofmt-formatted after its recorded run; only formatting changed.

## Captured result

```text
=== RUN   TestObservationSameIdentityClaimsReturnSameLease
    task_semantics_test.go:37: Two callers sharing an assignee identity both receive the same active lease; process exclusivity is not enforced.
--- PASS: TestObservationSameIdentityClaimsReturnSameLease (0.01s)
=== RUN   TestObservationCursorBeforeClaimLeavesSubmittedTaskBehind
    task_semantics_test.go:49: Checkpointed delivery has no lease and remains SUBMITTED; reconnect after that cursor yields no work to reclaim.
--- PASS: TestObservationCursorBeforeClaimLeavesSubmittedTaskBehind (0.00s)
=== RUN   TestObservationIdempotencyKeyCanResendToDifferentAssignee
    task_semantics_test.go:61: Same idempotency key returned the original task but queued a new request carrying its ID to another assignee.
--- PASS: TestObservationIdempotencyKeyCanResendToDifferentAssignee (0.00s)
=== RUN   TestObservationRemoteTaskOmitsRetryAndDeadlineSettings
    task_semantics_test.go:73: Initiator requested max_attempts=7 and 60s deadline; assignee stored max_attempts=3 and deadline={0 false}.
--- PASS: TestObservationRemoteTaskOmitsRetryAndDeadlineSettings (0.01s)
=== RUN   TestObservationCancellationDoesNotReachAssignee
    task_semantics_test.go:85: Initiator task is CANCELLED; assignee task remains SUBMITTED and no cancellation message is enqueued.
--- PASS: TestObservationCancellationDoesNotReachAssignee (0.01s)
PASS
ok  	github.com/sahilpohare/p2p-a2a/researchprobe	1.696s
```

## Probe source

```go
package researchprobe

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"

	"github.com/sahilpohare/p2p-a2a/daemon/identity"
	"github.com/sahilpohare/p2p-a2a/daemon/outbox"
	"github.com/sahilpohare/p2p-a2a/daemon/rpc"
	"github.com/sahilpohare/p2p-a2a/daemon/tasks"
	pb "github.com/sahilpohare/p2p-a2a/gen/a2a/v1"
	"go.uber.org/zap"
)

// These are characterization probes: success confirms the printed observation,
// not that the observed behavior is desirable. All calls and data stay local.
type env struct {
	server *rpc.Server
	tasks  *tasks.Store
	outbox *outbox.Outbox
	path   string
}

func environment(t *testing.T) *env {
	t.Helper()
	id, err := identity.Generate()
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "tasks.db")
	ts, err := tasks.New(path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ts.Close() })
	ob, err := outbox.New(":memory:", nil, zap.NewNop())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ob.Close() })
	s := rpc.New(id, nil, ob, ts, nil, nil, nil, nil, nil, nil, nil, nil, nil, nil, zap.NewNop())
	return &env{s, ts, ob, path}
}

func TestObservationSameIdentityClaimsReturnSameLease(t *testing.T) {
	e := environment(t)
	task, err := e.tasks.Create("initiator", "worker", "", "research", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	one, err := e.tasks.Claim(task.Id, "worker", time.Minute)
	if err != nil {
		t.Fatal(err)
	}
	two, err := e.tasks.Claim(task.Id, "worker", time.Minute)
	if err != nil {
		t.Fatal(err)
	}
	if one.LeaseToken != two.LeaseToken {
		t.Fatal("observation changed: second claimant received a distinct lease")
	}
	t.Log("Two callers sharing an assignee identity both receive the same active lease; process exclusivity is not enforced.")
}

func TestObservationCursorBeforeClaimLeavesSubmittedTaskBehind(t *testing.T) {
	e := environment(t)
	task, err := e.tasks.Create("initiator", "worker", "", "research", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	first, err := e.tasks.ListDeliveries("worker", nil, 0, 0)
	if err != nil || len(first) != 1 {
		t.Fatalf("initial: %v %v", first, err)
	}
	// Emulate the Python SDK checkpoint just before its first ClaimTask call.
	after := first[0].Sequence
	second, err := e.tasks.ListDeliveries("worker", nil, after, 0)
	if err != nil {
		t.Fatal(err)
	}
	state, err := e.tasks.Get(task.Id)
	if err != nil {
		t.Fatal(err)
	}
	if len(second) != 0 || state.Status != pb.TaskStatus_TASK_STATUS_SUBMITTED {
		t.Fatal("observation changed")
	}
	t.Log("Checkpointed delivery has no lease and remains SUBMITTED; reconnect after that cursor yields no work to reclaim.")
}

func TestObservationIdempotencyKeyCanResendToDifferentAssignee(t *testing.T) {
	e := environment(t)
	ctx := context.Background()
	one, err := e.server.CreateTask(ctx, &pb.CreateTaskRequest{ToDid: "worker-one", IdempotencyKey: "retry-key", Task: &pb.TaskRequest{Skill: "research", Metadata: map[string]string{"input": "first"}}})
	if err != nil {
		t.Fatal(err)
	}
	two, err := e.server.CreateTask(ctx, &pb.CreateTaskRequest{ToDid: "worker-two", IdempotencyKey: "retry-key", Task: &pb.TaskRequest{Skill: "research", Metadata: map[string]string{"input": "second"}}})
	if err != nil {
		t.Fatal(err)
	}
	queued, err := e.outbox.List("pending", 0)
	if err != nil {
		t.Fatal(err)
	}
	if one.Id != two.Id || two.Assignee != "worker-one" || len(queued) != 2 {
		t.Fatalf("observation changed: task=%v messages=%v", two, queued)
	}
	var redirected bool
	for _, msg := range queued {
		if msg.TaskId == one.Id && msg.ToDid == "worker-two" {
			redirected = true
		}
	}
	if !redirected {
		t.Fatal("observation changed: no message to second assignee")
	}
	t.Log("Same idempotency key returned the original task but queued a new request carrying its ID to another assignee.")
}

func TestObservationRemoteTaskOmitsRetryAndDeadlineSettings(t *testing.T) {
	a, b := environment(t), environment(t)
	task, err := a.server.CreateTask(context.Background(), &pb.CreateTaskRequest{ToDid: "worker", IdempotencyKey: "settings", MaxAttempts: 7, TimeoutMs: 60000, Task: &pb.TaskRequest{Skill: "research"}})
	if err != nil {
		t.Fatal(err)
	}
	queued, err := a.outbox.List("pending", 0)
	if err != nil || len(queued) != 1 {
		t.Fatal(err)
	}
	if err := b.server.HandleIncoming(queued[0]); err != nil {
		t.Fatal(err)
	}
	db, err := sql.Open("sqlite3", b.path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	var attempts int
	var deadline sql.NullInt64
	if err := db.QueryRow("SELECT max_attempts,deadline_at FROM tasks WHERE id=?", task.Id).Scan(&attempts, &deadline); err != nil {
		t.Fatal(err)
	}
	if attempts != 3 || deadline.Valid {
		t.Fatalf("observation changed: attempts=%d deadline=%v", attempts, deadline)
	}
	t.Logf("Initiator requested max_attempts=7 and 60s deadline; assignee stored max_attempts=%d and deadline=%v.", attempts, deadline)
}

func TestObservationCancellationDoesNotReachAssignee(t *testing.T) {
	a, b := environment(t), environment(t)
	task, err := a.server.CreateTask(context.Background(), &pb.CreateTaskRequest{ToDid: "worker", Task: &pb.TaskRequest{Skill: "research"}})
	if err != nil {
		t.Fatal(err)
	}
	queued, err := a.outbox.List("pending", 0)
	if err != nil || len(queued) != 1 {
		t.Fatal(err)
	}
	if err := b.server.HandleIncoming(queued[0]); err != nil {
		t.Fatal(err)
	}
	if _, err := a.server.CancelTask(context.Background(), &pb.TaskID{Id: task.Id}); err != nil {
		t.Fatal(err)
	}
	local, _ := a.tasks.Get(task.Id)
	remote, _ := b.tasks.Get(task.Id)
	queued, err = a.outbox.List("pending", 0)
	if err != nil {
		t.Fatal(err)
	}
	if local.Status != pb.TaskStatus_TASK_STATUS_CANCELLED || remote.Status != pb.TaskStatus_TASK_STATUS_SUBMITTED || len(queued) != 1 {
		t.Fatal("observation changed")
	}
	t.Log("Initiator task is CANCELLED; assignee task remains SUBMITTED and no cancellation message is enqueued.")
}
```
