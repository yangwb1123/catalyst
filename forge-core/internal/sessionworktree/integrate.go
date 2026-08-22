package sessionworktree

import (
	"context"
	"errors"
	"fmt"
	"os"
	"time"

	"forgeos/forge-core/internal/execbound"
	"forgeos/forge-core/internal/runlock"
)

var ErrNoReadySession = errors.New("session merge queue is empty")

type ValidationCommand struct {
	Program string
	Args    []string
	Timeout time.Duration
}

type IntegrateOptions struct {
	RepositoryRoot string
	Validation     ValidationCommand
	KeepWorktree   bool
	Now            func() time.Time
}

// IntegrateNext is the only session operation that may update the base branch.
// A repository-wide process lock makes selection, rebase, validation, and the
// final fast-forward one serialized merge-queue transaction.
func IntegrateNext(ctx context.Context, options IntegrateOptions) (Session, error) {
	if !runlock.Supported() {
		return Session{}, fmt.Errorf("merge controller requires a real local cross-process lock")
	}
	repo, err := resolveRepository(ctx, options.RepositoryRoot)
	if err != nil {
		return Session{}, err
	}
	if repo.current != repo.primary {
		return Session{}, fmt.Errorf("merge controller must run from the primary worktree")
	}
	if options.Validation.Program == "" {
		return Session{}, fmt.Errorf("merge controller requires a validation program")
	}
	lock, err := runlock.Acquire(repo.primary)
	if err != nil {
		return Session{}, fmt.Errorf("serialize merge controller: %w", err)
	}
	defer func() { _ = lock.Release() }()
	return integrateLocked(ctx, repo, options)
}

func integrateLocked(ctx context.Context, repo repository, options IntegrateOptions) (Session, error) {
	store, err := openStore(repo)
	if err != nil {
		return Session{}, err
	}
	sessions, err := store.list()
	if err != nil {
		return Session{}, err
	}
	queued := readySessions(sessions)
	if len(queued) == 0 {
		return Session{}, ErrNoReadySession
	}
	session := queued[0]
	if err := integrationPreflight(ctx, repo, session); err != nil {
		return Session{}, err
	}
	return rebaseValidateMerge(ctx, repo, store, session, options)
}

func integrationPreflight(ctx context.Context, repo repository, session Session) error {
	if err := requireSessionRepository(ctx, repo, session); err != nil {
		return err
	}
	if err := requireBranch(ctx, repo.primary, session.BaseBranch); err != nil {
		return fmt.Errorf("base worktree preflight: %w", err)
	}
	if err := requireClean(ctx, repo.primary, "base worktree"); err != nil {
		return err
	}
	if err := requireBranch(ctx, session.Worktree, session.Branch); err != nil {
		return fmt.Errorf("session preflight: %w", err)
	}
	if err := requireClean(ctx, session.Worktree, "session worktree"); err != nil {
		return err
	}
	head, err := gitCommit(ctx, session.Worktree, "HEAD")
	if err != nil || head != session.HeadCommit {
		return fmt.Errorf("session HEAD changed after enqueue")
	}
	return nil
}

func requireSessionRepository(ctx context.Context, repo repository, session Session) error {
	sessionRepo, err := resolveRepository(ctx, session.Worktree)
	if err != nil {
		return fmt.Errorf("resolve session worktree: %w", err)
	}
	if sessionRepo.primary != repo.primary || sessionRepo.commonDir != repo.commonDir ||
		sessionRepo.current != session.Worktree {
		return fmt.Errorf("session worktree no longer belongs to the recorded repository")
	}
	return nil
}

func rebaseValidateMerge(
	ctx context.Context,
	repo repository,
	store *sessionStore,
	session Session,
	options IntegrateOptions,
) (Session, error) {
	now := integrationNow(options)
	baseHead, err := gitCommit(ctx, repo.primary, "refs/heads/"+session.BaseBranch)
	if err != nil {
		return Session{}, err
	}
	transition(&session, StatusRebasing, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	if _, err := gitBytes(ctx, session.Worktree, "rebase", "refs/heads/"+session.BaseBranch); err != nil {
		return failSession(store, session, StatusConflict, "rebase failed", now, err)
	}
	return validateAndMerge(ctx, repo, store, session, baseHead, options, now)
}

func validateAndMerge(
	ctx context.Context,
	repo repository,
	store *sessionStore,
	session Session,
	baseHead string,
	options IntegrateOptions,
	now time.Time,
) (Session, error) {
	head, err := gitCommit(ctx, session.Worktree, "HEAD")
	if err != nil {
		return Session{}, err
	}
	session.HeadCommit = head
	transition(&session, StatusValidating, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	if err := runValidation(ctx, session.Worktree, options.Validation); err != nil {
		return failSession(store, session, StatusTestFailed, "validation failed", now, err)
	}
	if err := postValidationCheck(ctx, repo, session, baseHead); err != nil {
		return failSession(store, session, StatusMergeFailed, "post-validation check failed", now, err)
	}
	return mergeValidated(ctx, repo, store, session, options, now)
}

func runValidation(ctx context.Context, worktree string, command ValidationCommand) error {
	argv := append([]string{command.Program}, command.Args...)
	timeout := command.Timeout
	if timeout == 0 {
		timeout = 3 * time.Hour
	}
	result := execbound.Run(ctx, argv, execbound.Options{
		Timeout: timeout, MaxOutputBytes: 10 << 20,
	}, execbound.CaptureCombined, execbound.Spec{Dir: worktree, Env: os.Environ()})
	if result.Err == nil {
		return nil
	}
	if output := result.Rendered(); output != "" {
		return fmt.Errorf("validation command failed: %s", output)
	}
	return fmt.Errorf("validation command failed: %w", result.Err)
}

func postValidationCheck(ctx context.Context, repo repository, session Session, baseHead string) error {
	if err := requireClean(ctx, session.Worktree, "session worktree after validation"); err != nil {
		return err
	}
	if err := requireBranch(ctx, session.Worktree, session.Branch); err != nil {
		return err
	}
	head, err := gitCommit(ctx, session.Worktree, "HEAD")
	if err != nil || head != session.HeadCommit {
		return fmt.Errorf("validation changed session HEAD")
	}
	currentBase, err := gitCommit(ctx, repo.primary, "refs/heads/"+session.BaseBranch)
	if err != nil || currentBase != baseHead {
		return fmt.Errorf("base branch changed during validation")
	}
	return requireClean(ctx, repo.primary, "base worktree after validation")
}

func mergeValidated(
	ctx context.Context,
	repo repository,
	store *sessionStore,
	session Session,
	options IntegrateOptions,
	now time.Time,
) (Session, error) {
	transition(&session, StatusMerging, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	if _, err := gitBytes(ctx, repo.primary, "merge", "--ff-only", session.Branch); err != nil {
		return failSession(store, session, StatusMergeFailed, "fast-forward merge failed", now, err)
	}
	merged, err := gitCommit(ctx, repo.primary, "HEAD")
	if err != nil || merged != session.HeadCommit {
		return failSession(store, session, StatusMergeFailed, "merged HEAD mismatch", now, err)
	}
	session.MergedCommit = merged
	transition(&session, StatusMerged, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	if options.KeepWorktree {
		return session, nil
	}
	return cleanupMerged(ctx, repo, store, session, now)
}

func cleanupMerged(
	ctx context.Context,
	repo repository,
	store *sessionStore,
	session Session,
	now time.Time,
) (Session, error) {
	if _, err := gitBytes(ctx, repo.primary, "worktree", "remove", session.Worktree); err != nil {
		return failSession(store, session, StatusMerged, "worktree cleanup failed", now, err)
	}
	if _, err := gitBytes(ctx, repo.primary, "branch", "-d", session.Branch); err != nil {
		return failSession(store, session, StatusMerged, "branch cleanup failed", now, err)
	}
	transition(&session, StatusCleaned, "", now)
	if err := store.save(session); err != nil {
		return Session{}, err
	}
	return session, nil
}

func failSession(
	store *sessionStore,
	session Session,
	status Status,
	failure string,
	now time.Time,
	cause error,
) (Session, error) {
	transition(&session, status, failure, now)
	if err := store.save(session); err != nil {
		return Session{}, fmt.Errorf("%v; persist failure state: %w", cause, err)
	}
	return session, cause
}

func integrationNow(options IntegrateOptions) time.Time {
	if options.Now != nil {
		return options.Now()
	}
	return time.Now()
}
