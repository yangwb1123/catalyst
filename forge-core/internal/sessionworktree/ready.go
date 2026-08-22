package sessionworktree

import (
	"context"
	"fmt"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/runlock"
)

type ReadyOptions struct {
	SessionID string
	Worktree  string
	Now       func() time.Time
}

// Ready verifies the Coding Agent left a committed, clean branch and enqueues
// it. This operation cannot update or merge the base branch.
func Ready(ctx context.Context, options ReadyOptions) (Session, error) {
	if !runlock.Supported() {
		return Session{}, fmt.Errorf("session enqueue requires a real local cross-process lock")
	}
	if err := validateSessionID(options.SessionID); err != nil {
		return Session{}, err
	}
	repo, err := resolveRepository(ctx, options.Worktree)
	if err != nil {
		return Session{}, err
	}
	lock, err := runlock.Acquire(repo.primary)
	if err != nil {
		return Session{}, fmt.Errorf("serialize session enqueue: %w", err)
	}
	defer func() { _ = lock.Release() }()
	return readyLocked(ctx, repo, options)
}

func readyLocked(ctx context.Context, repo repository, options ReadyOptions) (Session, error) {
	store, err := openStore(repo)
	if err != nil {
		return Session{}, err
	}
	session, err := store.load(options.SessionID)
	if err != nil {
		return Session{}, err
	}
	if !readyTransitionAllowed(session.Status) {
		return Session{}, fmt.Errorf("session %q cannot be enqueued from %s", session.SessionID, session.Status)
	}
	if filepath.Clean(repo.current) != session.Worktree {
		return Session{}, fmt.Errorf("session worktree does not match durable state")
	}
	if err := requireBranch(ctx, session.Worktree, session.Branch); err != nil {
		return Session{}, err
	}
	if err := requireClean(ctx, session.Worktree, "session worktree"); err != nil {
		return Session{}, err
	}
	head, err := gitCommit(ctx, session.Worktree, "HEAD")
	if err != nil {
		return Session{}, err
	}
	if head == session.BaseCommit || !isAncestor(ctx, session.Worktree, session.BaseCommit, head) {
		return Session{}, fmt.Errorf("session branch must contain committed work after its base")
	}
	now := time.Now()
	if options.Now != nil {
		now = options.Now()
	}
	session.HeadCommit = head
	session.QueuedAt = now.UTC().Format(time.RFC3339Nano)
	transition(&session, StatusReady, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	return session, nil
}

func readyTransitionAllowed(status Status) bool {
	switch status {
	case StatusRunning, StatusConflict, StatusTestFailed, StatusMergeFailed:
		return true
	default:
		return false
	}
}

func List(ctx context.Context, root string) ([]Session, error) {
	repo, err := resolveRepository(ctx, root)
	if err != nil {
		return nil, err
	}
	store, err := openStore(repo)
	if err != nil {
		return nil, err
	}
	return store.list()
}

func Get(ctx context.Context, root, id string) (Session, error) {
	repo, err := resolveRepository(ctx, root)
	if err != nil {
		return Session{}, err
	}
	store, err := openStore(repo)
	if err != nil {
		return Session{}, err
	}
	return store.load(id)
}
