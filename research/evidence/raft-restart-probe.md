# Unsnapshotted Raft restart characterization probe

Research date: 2026-10-03. These are original research probes against MoltMesh commit `707c870e3188df243e5aea3c4662daa3253270bc`. **A passing result confirms the reported gap, not correct behavior.** All test data and identities are synthetic and local. See the [validation report](../moltmesh-validation.md) for scope and limitations.

## Reproduction

In a disposable checkout of the pinned revision, save the source below as `daemon/thread/research_restart_test.go` and run with Go 1.26.5 and CGO enabled:

```sh
go test -race -count=1 -run '^TestResearchRestartReplaysCommittedApplicationEntry$' -v ./daemon/thread
```

The upstream checkout must retain its normal module/dependencies. This probe belongs to that Go module, not Locust's Rust workspace. No upstream production file needs to be changed.

## Captured result

```text
=== RUN   TestResearchRestartReplaysCommittedApplicationEntry
    research_restart_test.go:94: before restart: height=1 payload_count=1 snapshot_index=0 raft_commit=3 raft_log_entries=3 pending=0
    research_restart_test.go:113: after restart without enqueue/tick/network: height=2 payload_count=2 applied_index=3
--- PASS: TestResearchRestartReplaysCommittedApplicationEntry (0.03s)
PASS
ok  	github.com/sahilpohare/p2p-a2a/daemon/thread	1.682s
```

## Probe source

```go
package thread

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/sahilpohare/p2p-a2a/daemon/identity"
	pb "github.com/sahilpohare/p2p-a2a/gen/a2a/v1"
	"go.etcd.io/raft/v3"
	"go.uber.org/zap"
)

// Characterization probe: application state and the Raft log are persisted,
// but no Snapshot is taken before reconstructing the backend. No network or
// external identities are used, and nothing is re-enqueued after reopening.
func TestResearchRestartReplaysCommittedApplicationEntry(t *testing.T) {
	id, err := identity.Generate()
	if err != nil {
		t.Fatal(err)
	}
	th := &pb.Thread{Id: "research-restart", CreatorDid: id.DID, ReplicaDids: []string{id.DID}, N: 1, EpochMs: 100}
	dbPath := filepath.Join(t.TempDir(), "thread.db")
	store, err := NewStore(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.SaveThread(th); err != nil {
		t.Fatal(err)
	}
	backend, err := newRaftBackend(th, id, store, zap.NewNop(), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		backend.Stop()
		store.Close()
	}()
	const payload = "synthetic-committed-once"
	if err := store.EnqueueEntry(th.Id, &pb.ThreadEntry{AuthorDid: id.DID, Payload: []byte(payload), Kind: "message"}); err != nil {
		t.Fatal(err)
	}
	count := func() (int64, int) {
		t.Helper()
		blocks, err := store.GetBlocksSince(th.Id, 0, 0)
		if err != nil {
			t.Fatal(err)
		}
		var height int64
		n := 0
		for _, block := range blocks {
			height = block.Height
			for _, entry := range block.Entries {
				if string(entry.Payload) == payload {
					n++
				}
			}
		}
		return height, n
	}
	pumpUntil := func(want int, tick bool) {
		t.Helper()
		deadline := time.Now().Add(10 * time.Second)
		for time.Now().Before(deadline) {
			backend.Pump(context.Background(), nil)
			if tick {
				backend.Tick(context.Background())
			}
			if _, n := count(); n >= want {
				return
			}
			time.Sleep(time.Millisecond)
		}
		height, n := count()
		t.Fatalf("timed out awaiting %d copies: height=%d payload_count=%d", want, height, n)
	}
	pumpUntil(1, true)
	height, n := count()
	if n != 1 || height != 1 {
		t.Fatalf("before restart: height=%d payload_count=%d", height, n)
	}
	snapshot, hardState, entries, err := store.LoadRaftState(th.Id)
	if err != nil {
		t.Fatal(err)
	}
	if !raft.IsEmptySnap(snapshot) || hardState.Commit == 0 || len(entries) == 0 {
		t.Fatalf("invalid fixture: snapshot_index=%d commit=%d log_entries=%d", snapshot.Metadata.Index, hardState.Commit, len(entries))
	}
	pending, err := store.PendingEntryCount(th.Id)
	if err != nil || pending != 0 {
		t.Fatalf("pending entries before restart: count=%d err=%v", pending, err)
	}
	t.Logf("before restart: height=%d payload_count=%d snapshot_index=%d raft_commit=%d raft_log_entries=%d pending=%d", height, n, snapshot.Metadata.Index, hardState.Commit, len(entries), pending)
	backend.Stop() // Deliberately omit Snapshot: characterize unsnapshotted restart.
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}
	store, err = NewStore(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	backend, err = newRaftBackend(th, id, store, zap.NewNop(), nil)
	if err != nil {
		t.Fatal(err)
	}
	// Pump only; no new clock ticks, proposals, enqueue calls, or network input.
	pumpUntil(2, false)
	height, n = count()
	if n != 2 || height != 2 {
		t.Fatalf("after restart: height=%d payload_count=%d", height, n)
	}
	t.Logf("after restart without enqueue/tick/network: height=%d payload_count=%d applied_index=%d", height, n, backend.appliedIndex)
}
```
