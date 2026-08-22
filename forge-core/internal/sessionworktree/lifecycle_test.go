package sessionworktree

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSessionLifecycleMergesValidatedCommitAndCleansIsolation(t *testing.T) {
	root := newTestRepository(t)
	worktrees := t.TempDir()
	started := startTestSession(t, root, worktrees, "sess-alpha", time.Unix(1, 0))
	wantHead := commitSessionFile(t, started, "alpha.txt", "alpha\n")
	ready := readyTestSession(t, started, time.Unix(2, 0))
	if ready.Status != StatusReady || ready.HeadCommit != wantHead {
		t.Fatalf("ready state = %#v", ready)
	}
	merged, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(),
		Now: func() time.Time { return time.Unix(3, 0) },
	})
	if err != nil {
		t.Fatalf("IntegrateNext: %v", err)
	}
	if merged.Status != StatusCleaned || merged.MergedCommit != wantHead {
		t.Fatalf("merged state = %#v", merged)
	}
	if got := runTestGit(t, root, "rev-parse", "HEAD"); got != wantHead {
		t.Fatalf("main HEAD = %s, want %s", got, wantHead)
	}
	if _, err := os.Stat(started.Worktree); !os.IsNotExist(err) {
		t.Fatalf("worktree retained after cleanup: %v", err)
	}
	if output := runTestGit(t, root, "branch", "--list", started.Branch); output != "" {
		t.Fatalf("session branch retained: %q", output)
	}
}

func TestValidationFailureRetainsWorktreeAndNeverMovesMain(t *testing.T) {
	root := newTestRepository(t)
	base := runTestGit(t, root, "rev-parse", "HEAD")
	started := startTestSession(t, root, t.TempDir(), "sess-red", time.Unix(1, 0))
	commitSessionFile(t, started, "red.txt", "red\n")
	readyTestSession(t, started, time.Unix(2, 0))
	failed, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root,
		Validation: ValidationCommand{
			Program: "git", Args: []string{"rev-parse", "--verify", "refs/heads/missing"},
			Timeout: time.Minute,
		},
	})
	if err == nil || failed.Status != StatusTestFailed {
		t.Fatalf("failed integration = %#v, %v", failed, err)
	}
	if got := runTestGit(t, root, "rev-parse", "HEAD"); got != base {
		t.Fatalf("validation failure moved main: %s != %s", got, base)
	}
	if info, statErr := os.Stat(started.Worktree); statErr != nil || !info.IsDir() {
		t.Fatalf("failed worktree was not retained: %v", statErr)
	}
	persisted, loadErr := Get(context.Background(), root, started.SessionID)
	if loadErr != nil || persisted.Status != StatusTestFailed {
		t.Fatalf("persisted failed state = %#v, %v", persisted, loadErr)
	}
}

func TestDirtyMainBlocksReadySessionWithoutChangingItsState(t *testing.T) {
	root := newTestRepository(t)
	started := startTestSession(t, root, t.TempDir(), "sess-dirty", time.Unix(1, 0))
	commitSessionFile(t, started, "clean.txt", "clean\n")
	readyTestSession(t, started, time.Unix(2, 0))
	writeTestFile(t, filepath.Join(root, "dirty.txt"), "do not merge\n")
	_, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(),
	})
	if err == nil {
		t.Fatal("dirty main was accepted")
	}
	persisted, loadErr := Get(context.Background(), root, started.SessionID)
	if loadErr != nil || persisted.Status != StatusReady {
		t.Fatalf("dirty-main block consumed queue item: %#v, %v", persisted, loadErr)
	}
}
