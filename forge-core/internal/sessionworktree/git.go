package sessionworktree

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"forgeos/forge-core/internal/execbound"
)

type repository struct {
	current   string
	primary   string
	commonDir string
}

const maxSessionGitDiagnosticBytes = 4 << 10
const maxSessionGitDiagnosticRaw = 960
const maxSessionGitOutputBytes = 1 << 20

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
	top, err = canonicalRepositoryRoot(top, resolved)
	if err != nil {
		return repository{}, err
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
	path, err := parsePrimaryWorktree(output)
	if err != nil {
		return "", err
	}
	primary, err := filepath.EvalSymlinks(path)
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
	return decodeGitText(output)
}

func gitBytes(ctx context.Context, dir string, args ...string) ([]byte, error) {
	if len(args) == 0 {
		return nil, fmt.Errorf("Git operation is required")
	}
	argv := []string{
		"git", "-c", "core.fsmonitor=false", "-c", "core.hooksPath=" + os.DevNull,
		"-c", "core.pager=cat", "--no-pager", "-C", dir,
	}
	argv = append(argv, args...)
	result := execbound.Run(ctx, argv, execbound.Options{
		Timeout: 2 * time.Minute, MaxOutputBytes: maxSessionGitOutputBytes,
	}, execbound.CaptureSplit, execbound.Spec{Env: sessionGitEnvironment(os.Environ())})
	if err := validateGitCapture(result); err != nil {
		return nil, fmt.Errorf(
			"git %s returned unusable output: %w", strconv.QuoteToASCII(args[0]), err,
		)
	}
	if result.CtxErr != nil {
		return nil, fmt.Errorf("git %s canceled: %w", strconv.QuoteToASCII(args[0]), result.CtxErr)
	}
	if result.Err != nil {
		if len(result.Stderr) != 0 {
			return nil, fmt.Errorf(
				"git %s failed: cause=%s stderr=%s", strconv.QuoteToASCII(args[0]),
				boundedSessionGitDiagnostic([]byte(result.Err.Error())),
				boundedSessionGitDiagnostic(result.Stderr),
			)
		}
		return nil, fmt.Errorf(
			"git %s failed: cause=%s", strconv.QuoteToASCII(args[0]),
			boundedSessionGitDiagnostic([]byte(result.Err.Error())),
		)
	}
	return append([]byte(nil), result.Stdout...), nil
}

func boundedSessionGitDiagnostic(source []byte) string {
	truncated := len(source) > maxSessionGitDiagnosticRaw
	if truncated {
		source = source[:maxSessionGitDiagnosticRaw]
	}
	result := strconv.QuoteToASCII(string(source))
	if truncated {
		result += " …[Git stderr diagnostic truncated]"
	}
	return result
}

func decodeGitText(output []byte) (string, error) {
	if len(output) < 2 || output[len(output)-1] != '\n' {
		return "", fmt.Errorf("Git text output omitted its terminal LF")
	}
	value := output[:len(output)-1]
	if bytes.IndexAny(value, "\x00\r\n") >= 0 {
		return "", fmt.Errorf("Git text output contains framing bytes")
	}
	return string(value), nil
}

func parsePrimaryWorktree(output []byte) (string, error) {
	if len(output) < 2 || !bytes.HasSuffix(output, []byte{0, 0}) {
		return "", fmt.Errorf("Git worktree list has invalid terminal framing")
	}
	records := bytes.Split(output[:len(output)-2], []byte{0, 0})
	primary := ""
	for index, record := range records {
		fields := bytes.Split(record, []byte{0})
		if len(fields) == 0 || len(fields[0]) <= len("worktree ") ||
			!bytes.HasPrefix(fields[0], []byte("worktree ")) {
			return "", fmt.Errorf("Git worktree record %d is malformed", index)
		}
		for _, field := range fields {
			if len(field) == 0 {
				return "", fmt.Errorf("Git worktree record %d contains an empty field", index)
			}
		}
		if index == 0 {
			primary = string(fields[0][len("worktree "):])
		}
	}
	return primary, nil
}

func canonicalRepositoryRoot(top, requested string) (string, error) {
	absolute, err := filepath.Abs(top)
	if err != nil {
		return "", fmt.Errorf("resolve Git top-level path: %w", err)
	}
	canonical, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", fmt.Errorf("resolve Git top-level aliases: %w", err)
	}
	relative, err := filepath.Rel(canonical, requested)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("Git top-level does not contain the requested repository path")
	}
	return filepath.Clean(canonical), nil
}

func sessionGitEnvironment(parent []string) []string {
	overrides := map[string]string{
		"GIT_CONFIG_GLOBAL": os.DevNull, "GIT_CONFIG_NOSYSTEM": "1",
		"GIT_CONFIG_SYSTEM": os.DevNull, "GIT_OPTIONAL_LOCKS": "0",
		"GIT_PAGER": "cat", "GIT_TERMINAL_PROMPT": "0", "LANG": "C", "LC_ALL": "C",
	}
	result := make([]string, 0, len(parent)+len(overrides))
	for _, declaration := range parent {
		name, _, ok := strings.Cut(declaration, "=")
		upper := strings.ToUpper(name)
		if !ok || strings.HasPrefix(upper, "GIT_") {
			continue
		}
		if _, replaced := overrides[upper]; !replaced {
			result = append(result, declaration)
		}
	}
	keys := make([]string, 0, len(overrides))
	for name := range overrides {
		keys = append(keys, name)
	}
	sort.Strings(keys)
	for _, name := range keys {
		result = append(result, name+"="+overrides[name])
	}
	return result
}

func validateGitCapture(result execbound.Result) error {
	switch {
	case result.DrainIncomplete:
		return fmt.Errorf("output drain incomplete")
	case result.CountOverflow:
		return fmt.Errorf("total byte count overflowed after retaining %d bytes", result.Retained)
	case result.Total > maxSessionGitOutputBytes:
		return fmt.Errorf(
			"output exceeded aggregate limit: observed %d bytes, limit %d",
			result.Total, maxSessionGitOutputBytes,
		)
	case result.Total > int64(result.Retained):
		return fmt.Errorf("output truncated: retained %d of %d bytes", result.Retained, result.Total)
	default:
		return nil
	}
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
