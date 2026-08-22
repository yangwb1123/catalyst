package sessionworktree

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/execbound"
)

type repository struct {
	current   string
	primary   string
	commonDir string
}

func resolveRepository(ctx context.Context, root string) (repository, error) {
	absolute, err := filepath.Abs(root)
	if err != nil {
		return repository{}, fmt.Errorf("resolve repository path: %w", err)
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return repository{}, fmt.Errorf("resolve repository aliases: %w", err)
	}
	top, err := gitText(ctx, resolved, "rev-parse", "--show-toplevel")
	if err != nil {
		return repository{}, fmt.Errorf("resolve Git worktree: %w", err)
	}
	primary, err := primaryWorktree(ctx, top)
	if err != nil {
		return repository{}, err
	}
	common, err := gitText(ctx, top, "rev-parse", "--path-format=absolute", "--git-common-dir")
	if err != nil {
		return repository{}, fmt.Errorf("resolve Git common directory: %w", err)
	}
	return repository{
		current: filepath.Clean(top), primary: filepath.Clean(primary),
		commonDir: filepath.Clean(common),
	}, nil
}

func primaryWorktree(ctx context.Context, root string) (string, error) {
	output, err := gitBytes(ctx, root, "worktree", "list", "--porcelain", "-z")
	if err != nil {
		return "", fmt.Errorf("list Git worktrees: %w", err)
	}
	fields := strings.Split(string(output), "\x00")
	if len(fields) == 0 || !strings.HasPrefix(fields[0], "worktree ") {
		return "", fmt.Errorf("Git worktree list omitted the primary worktree")
	}
	primary, err := filepath.EvalSymlinks(strings.TrimPrefix(fields[0], "worktree "))
	if err != nil {
		return "", fmt.Errorf("resolve primary worktree: %w", err)
	}
	return primary, nil
}

func gitText(ctx context.Context, dir string, args ...string) (string, error) {
	output, err := gitBytes(ctx, dir, args...)
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

func gitBytes(ctx context.Context, dir string, args ...string) ([]byte, error) {
	argv := append([]string{"git", "-C", dir}, args...)
	result := execbound.Run(ctx, argv, execbound.Options{
		Timeout: 2 * time.Minute, MaxOutputBytes: 1 << 20,
	}, execbound.CaptureCombined, execbound.Spec{Env: os.Environ()})
	if result.Err != nil {
		detail := result.Rendered()
		if detail != "" {
			return nil, fmt.Errorf("git %s failed: %s", args[0], detail)
		}
		return nil, fmt.Errorf("git %s failed: %w", args[0], result.Err)
	}
	return append([]byte(nil), result.Merged...), nil
}

func gitCommit(ctx context.Context, dir, revision string) (string, error) {
	commit, err := gitText(ctx, dir, "rev-parse", "--verify", revision+"^{commit}")
	if err != nil {
		return "", fmt.Errorf("resolve Git commit %q: %w", revision, err)
	}
	if !isGitObjectID(commit) {
		return "", fmt.Errorf("resolve Git commit %q: invalid object id", revision)
	}
	return commit, nil
}

func currentBranch(ctx context.Context, dir string) (string, error) {
	return gitText(ctx, dir, "symbolic-ref", "--quiet", "--short", "HEAD")
}

func requireClean(ctx context.Context, dir, label string) error {
	output, err := gitBytes(ctx, dir, "status", "--porcelain=v1", "-z",
		"--untracked-files=all", "--", ".", ":(exclude).forge")
	if err != nil {
		return fmt.Errorf("inspect %s: %w", label, err)
	}
	if len(output) != 0 {
		return fmt.Errorf("%s must be clean", label)
	}
	return nil
}

func requireBranch(ctx context.Context, dir, expected string) error {
	actual, err := currentBranch(ctx, dir)
	if err != nil || actual != expected {
		return fmt.Errorf("worktree must be on branch %q", expected)
	}
	return nil
}

func isAncestor(ctx context.Context, dir, ancestor, descendant string) bool {
	_, err := gitBytes(ctx, dir, "merge-base", "--is-ancestor", ancestor, descendant)
	return err == nil
}
