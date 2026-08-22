package sessionworktree

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestMergeQueueUsesQueuedTimeAndRebasesOntoLatestMain(t *testing.T) {
	root := newTestRepository(t)
	worktrees := t.TempDir()
	first := startTestSession(t, root, worktrees, "sess-first", time.Unix(1, 0))
	second := startTestSession(t, root, worktrees, "sess-second", time.Unix(1, 0))
	firstHeadBeforeRebase := commitSessionFile(t, first, "first.txt", "first\n")
	commitSessionFile(t, second, "second.txt", "second\n")
	readySecond := readyTestSession(t, second, time.Unix(4, 0))
	readyTestSession(t, first, time.Unix(3, 0))

	mergedFirst, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(), KeepWorktree: true,
	})
	if err != nil || mergedFirst.SessionID != first.SessionID ||
		mergedFirst.MergedCommit != firstHeadBeforeRebase {
		t.Fatalf("first queue integration = %#v, %v", mergedFirst, err)
	}
	mergedSecond, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(), KeepWorktree: true,
	})
	if err != nil || mergedSecond.SessionID != second.SessionID {
		t.Fatalf("second queue integration = %#v, %v", mergedSecond, err)
	}
	if mergedSecond.HeadCommit == readySecond.HeadCommit {
		t.Fatal("second session was not rebased after main advanced")
	}
	for _, name := range []string{"first.txt", "second.txt"} {
		if _, statErr := os.Stat(filepath.Join(root, name)); statErr != nil {
			t.Fatalf("merged main missing %s: %v", name, statErr)
		}
	}
}

func TestReadyRejectsUncommittedAndEmptySessions(t *testing.T) {
	root := newTestRepository(t)
	empty := startTestSession(t, root, t.TempDir(), "sess-empty", time.Unix(1, 0))
	if _, err := Ready(context.Background(), ReadyOptions{
		SessionID: empty.SessionID, Worktree: empty.Worktree,
	}); err == nil {
		t.Fatal("empty session was enqueued")
	}
	dirty := startTestSession(t, root, t.TempDir(), "sess-uncommitted", time.Unix(1, 0))
	writeTestFile(t, filepath.Join(dirty.Worktree, "dirty.txt"), "dirty\n")
	if _, err := Ready(context.Background(), ReadyOptions{
		SessionID: dirty.SessionID, Worktree: dirty.Worktree,
	}); err == nil {
		t.Fatal("uncommitted session was enqueued")
	}
}

func TestIntegrateNextReportsEmptyQueue(t *testing.T) {
	root := newTestRepository(t)
	_, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: root, Validation: passingValidation(),
	})
	if err != ErrNoReadySession {
		t.Fatalf("empty queue error = %v", err)
	}
}
