package projectsnapshot

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"time"

	"forgeos/forge-core/internal/execbound"
)

const boundRepositoryPath = "/proc/self/fd/3"

type gitRepositoryRoot struct {
	file     *os.File
	identity os.FileInfo
}

func openGitRepositoryRoot(anchor *treeRoot) (*gitRepositoryRoot, error) {
	if err := anchor.verify(); err != nil {
		return nil, err
	}
	file, err := anchor.handle.Open(".")
	if err != nil {
		return nil, fmt.Errorf("open bound Git repository root: %w", err)
	}
	info, statErr := file.Stat()
	if statErr != nil || !stableDirectory(anchor.identity, info) {
		_ = file.Close()
		return nil, fmt.Errorf("bound Git repository root differs from capture anchor")
	}
	return &gitRepositoryRoot{file: file, identity: info}, nil
}

func (root *gitRepositoryRoot) verify() error {
	current, err := root.file.Stat()
	if err != nil || !stableDirectory(root.identity, current) {
		return fmt.Errorf("bound Git repository root changed during capture")
	}
	return nil
}

func (root *gitRepositoryRoot) close() { _ = root.file.Close() }

func canonicalRepositoryRoot(
	ctx context.Context,
	root string,
	repository *gitRepositoryRoot,
	git *observedGit,
	environment []string,
) (string, error) {
	absolute, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("resolve repository root: %w", err)
	}
	canonical := filepath.Clean(absolute)
	top, err := gitOutput(ctx, repository, git, environment, "rev-parse", "--show-toplevel")
	if err != nil {
		return "", fmt.Errorf("resolve Git worktree root: %w", err)
	}
	resolvedTop, lineErr := exactGitLine(top, maxPathBytes)
	if lineErr != nil || resolvedTop != canonical {
		return "", fmt.Errorf("Git worktree root output is invalid")
	}
	return canonical, nil
}

func gitOutput(
	ctx context.Context,
	root *gitRepositoryRoot,
	git *observedGit,
	environment []string,
	args ...string,
) ([]byte, error) {
	if err := git.verifyIdentity(ctx); err != nil {
		return nil, err
	}
	if root != nil {
		if err := root.verify(); err != nil {
			return nil, err
		}
	}
	commandArgs := []string{
		"-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null",
		"-c", "core.excludesFile=/dev/null", "-c", "core.pager=cat", "--no-pager",
	}
	if root != nil {
		commandArgs = append(commandArgs, "-C", boundRepositoryPath,
			"--work-tree="+boundRepositoryPath)
	}
	commandArgs = append(commandArgs, args...)
	argv := append([]string{"git"}, commandArgs...)
	result := execbound.Run(ctx, argv, execbound.Options{
		Timeout: gitTimeout, MaxOutputBytes: maxGitOutputBytes,
	}, execbound.CaptureSplit, gitCommandSpec(root, git, environment))
	if result.Err != nil {
		return nil, fmt.Errorf("hardened Git command failed: %w", result.Err)
	}
	if result.Total > maxGitOutputBytes || result.Total > int64(result.Retained) {
		return nil, fmt.Errorf("hardened Git output exceeds %d bytes", maxGitOutputBytes)
	}
	if err := git.verifyIdentity(ctx); err != nil {
		return nil, err
	}
	if root != nil {
		if err := root.verify(); err != nil {
			return nil, err
		}
	}
	return append([]byte(nil), result.Stdout...), nil
}

func gitCommandSpec(
	root *gitRepositoryRoot,
	git *observedGit,
	environment []string,
) execbound.Spec {
	spec := execbound.Spec{
		Env: hardenedGitEnvironment(environment), ExecutablePath: git.path,
	}
	if root != nil {
		spec.ExtraFiles = []*os.File{root.file}
	}
	return spec
}

func hardenedGitEnvironment(environment []string) []string {
	pathValue, _ := exactPATH(environment)
	values := map[string]string{
		"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
		"GIT_OPTIONAL_LOCKS": "0", "GIT_PAGER": "cat", "GIT_TERMINAL_PROMPT": "0",
		"HOME": "/", "LANG": "C", "LC_ALL": "C", "PATH": pathValue,
	}
	keys := make([]string, 0, len(values))
	for name := range values {
		keys = append(keys, name)
	}
	sort.Strings(keys)
	result := make([]string, len(keys))
	for index, name := range keys {
		result[index] = name + "=" + values[name]
	}
	return result
}

const gitTimeout = 30 * time.Second
