package sessionworktree

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/runlock"
	"forgeos/forge-core/internal/statefs"
)

type StartOptions struct {
	RepositoryRoot string
	WorktreeRoot   string
	BaseBranch     string
	SessionID      string
	Now            func() time.Time
}

// Start creates exactly one session branch, worktree, and durable state record.
func Start(ctx context.Context, options StartOptions) (Session, error) {
	if !runlock.Supported() {
		return Session{}, fmt.Errorf("session worktrees require a real local cross-process lock")
	}
	repo, err := resolveRepository(ctx, options.RepositoryRoot)
	if err != nil {
		return Session{}, err
	}
	if repo.current != repo.primary {
		return Session{}, fmt.Errorf("session start must target the primary worktree")
	}
	lock, err := runlock.Acquire(repo.primary)
	if err != nil {
		return Session{}, fmt.Errorf("serialize session start: %w", err)
	}
	defer func() { _ = lock.Release() }()
	return startLocked(ctx, repo, options)
}

func startLocked(ctx context.Context, repo repository, options StartOptions) (Session, error) {
	id, base, now, err := normalizeStartOptions(options)
	if err != nil {
		return Session{}, err
	}
	_, target, err := prepareWorktreePath(repo, options.WorktreeRoot, id)
	if err != nil {
		return Session{}, err
	}
	if _, err := gitBytes(ctx, repo.primary, "check-ref-format", "--branch", base); err != nil {
		return Session{}, fmt.Errorf("base branch %q is invalid", base)
	}
	baseCommit, err := gitCommit(ctx, repo.primary, "refs/heads/"+base)
	if err != nil {
		return Session{}, err
	}
	session := newSession(id, repo, base, target, baseCommit, now)
	store, err := openStore(repo)
	if err != nil {
		return Session{}, err
	}
	if _, err := store.load(id); err == nil {
		return Session{}, fmt.Errorf("session %q already exists", id)
	}
	if _, err := gitCommit(ctx, repo.primary, "refs/heads/"+session.Branch); err == nil {
		return Session{}, fmt.Errorf("session branch %q already exists", session.Branch)
	}
	if _, err := gitBytes(ctx, repo.primary, "worktree", "add", "-b", session.Branch,
		target, "refs/heads/"+base); err != nil {
		return Session{}, fmt.Errorf("create session worktree: %w", err)
	}
	if err := store.create(session); err != nil {
		rollbackStartedWorktree(ctx, repo, session)
		return Session{}, err
	}
	return session, nil
}

func normalizeStartOptions(options StartOptions) (string, string, time.Time, error) {
	id := options.SessionID
	if id == "" {
		id = "sess-" + runlock.NewRunID()
	}
	if err := validateSessionID(id); err != nil {
		return "", "", time.Time{}, err
	}
	base := options.BaseBranch
	if base == "" {
		base = "main"
	}
	now := time.Now()
	if options.Now != nil {
		now = options.Now()
	}
	return id, base, now, nil
}

func prepareWorktreePath(repo repository, requested, id string) (string, string, error) {
	root := requested
	if root == "" {
		root = filepath.Join(filepath.Dir(repo.primary), ".forge-worktrees", filepath.Base(repo.primary))
	}
	absolute, err := filepath.Abs(root)
	if err != nil {
		return "", "", err
	}
	planned, err := resolvePlannedPath(absolute)
	if err != nil {
		return "", "", fmt.Errorf("resolve worktree root: %w", err)
	}
	if pathWithin(repo.primary, planned) || pathWithin(repo.commonDir, planned) {
		return "", "", fmt.Errorf("worktree root must be outside the primary worktree")
	}
	if err := statefs.EnsurePrivateDirTree(planned); err != nil {
		return "", "", fmt.Errorf("secure worktree root: %w", err)
	}
	resolved, err := filepath.EvalSymlinks(planned)
	if err != nil || pathWithin(repo.primary, resolved) || pathWithin(repo.commonDir, resolved) {
		return "", "", fmt.Errorf("worktree root resolves inside the primary worktree")
	}
	target := filepath.Join(resolved, id)
	if _, err := os.Lstat(target); err == nil || !os.IsNotExist(err) {
		return "", "", fmt.Errorf("worktree path %q already exists", target)
	}
	return resolved, target, nil
}

func resolvePlannedPath(path string) (string, error) {
	cursor := filepath.Clean(path)
	missing := make([]string, 0, 4)
	for {
		info, err := os.Lstat(cursor)
		if err == nil {
			if !info.IsDir() {
				return "", fmt.Errorf("existing ancestor %q is not a directory", cursor)
			}
			break
		}
		if !os.IsNotExist(err) {
			return "", err
		}
		parent := filepath.Dir(cursor)
		if parent == cursor {
			return "", fmt.Errorf("no existing directory ancestor")
		}
		missing = append(missing, filepath.Base(cursor))
		cursor = parent
	}
	resolved, err := filepath.EvalSymlinks(cursor)
	if err != nil {
		return "", err
	}
	for index := len(missing) - 1; index >= 0; index-- {
		resolved = filepath.Join(resolved, missing[index])
	}
	return resolved, nil
}

func pathWithin(parent, candidate string) bool {
	relative, err := filepath.Rel(parent, candidate)
	return err == nil && relative != ".." && !filepath.IsAbs(relative) &&
		!strings.HasPrefix(relative, ".."+string(filepath.Separator))
}

func rollbackStartedWorktree(ctx context.Context, repo repository, session Session) {
	_, _ = gitBytes(ctx, repo.primary, "worktree", "remove", session.Worktree)
	_, _ = gitBytes(ctx, repo.primary, "branch", "-D", session.Branch)
}
