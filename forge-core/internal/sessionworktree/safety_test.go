package sessionworktree

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/runlock"
)

func TestConflictingRebaseRetainsSessionAndDoesNotMoveMergedMain(t *testing.T) {
	root := newTestRepository(t)
	worktrees := t.TempDir()
	first := startTestSession(t, root, worktrees, "sess-conflict-a", time.Unix(1, 0))
	second := startTestSession(t, root, worktrees, "sess-conflict-b", time.Unix(1, 0))
	commitSessionFile(t, first, "README.md", "first\n")
	commitSessionFile(t, second, "README.md", "second\n")
	readyTestSession(t, first, time.Unix(2, 0))
	readyTestSession(t, second, time.Unix(3, 0))

	merged, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(), KeepWorktree: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	mainAfterFirst := merged.MergedCommit
	conflict, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(),
	})
	if err == nil || conflict.Status != StatusConflict {
		t.Fatalf("conflict integration = %#v, %v", conflict, err)
	}
	if got := runTestGit(t, root, "rev-parse", "HEAD"); got != mainAfterFirst {
		t.Fatalf("conflict moved main: %s != %s", got, mainAfterFirst)
	}
	if info, statErr := os.Stat(second.Worktree); statErr != nil || !info.IsDir() {
		t.Fatalf("conflicted worktree was not retained: %v", statErr)
	}
}

func TestControllerContentionLeavesReadyItemQueued(t *testing.T) {
	root := newTestRepository(t)
	started := startTestSession(t, root, t.TempDir(), "sess-locked", time.Unix(1, 0))
	commitSessionFile(t, started, "locked.txt", "locked\n")
	readyTestSession(t, started, time.Unix(2, 0))
	lock, err := runlock.Acquire(root)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = lock.Release() }()
	if _, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(),
	}); err == nil {
		t.Fatal("second merge controller acquired the repository lock")
	}
	persisted, err := Get(context.Background(), root, started.SessionID)
	if err != nil || persisted.Status != StatusReady {
		t.Fatalf("contention consumed queue item: %#v, %v", persisted, err)
	}
}

func TestStartRejectsWorktreeRootInsideRepository(t *testing.T) {
	root := newTestRepository(t)
	_, err := Start(context.Background(), StartOptions{
		RepositoryRoot: root,
		WorktreeRoot:   filepath.Join(root, "nested-worktrees"),
		SessionID:      "sess-inside",
		BaseBranch:     "main",
	})
	if err == nil {
		t.Fatal("worktree root inside the primary repository was accepted")
	}
	if output := runTestGit(t, root, "branch", "--list", "session/sess-inside"); output != "" {
		t.Fatalf("rejected start left a branch: %q", output)
	}
}

func TestStartRejectsSymlinkAncestorBeforeCreatingInsideRepository(t *testing.T) {
	root := newTestRepository(t)
	outside := t.TempDir()
	alias := filepath.Join(outside, "repo-alias")
	if err := os.Symlink(root, alias); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	_, err := Start(context.Background(), StartOptions{
		RepositoryRoot: root, WorktreeRoot: filepath.Join(alias, "injected"),
		SessionID: "sess-alias", BaseBranch: "main",
	})
	if err == nil {
		t.Fatal("symlinked worktree root inside the repository was accepted")
	}
	if _, statErr := os.Lstat(filepath.Join(root, "injected")); !os.IsNotExist(statErr) {
		t.Fatalf("rejected path created a repository directory: %v", statErr)
	}
}

func TestMergeControllerRejectsLinkedSessionWorktreeAsRepositoryRoot(t *testing.T) {
	root := newTestRepository(t)
	started := startTestSession(t, root, t.TempDir(), "sess-agent", time.Unix(1, 0))
	commitSessionFile(t, started, "agent.txt", "agent\n")
	readyTestSession(t, started, time.Unix(2, 0))
	if _, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: started.Worktree, Validation: passingValidation(),
	}); err == nil {
		t.Fatal("merge controller ran from a Coding Agent worktree")
	}
}
