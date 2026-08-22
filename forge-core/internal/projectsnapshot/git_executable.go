package projectsnapshot

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type observedGit struct {
	bytes  int64
	file   *os.File
	info   os.FileInfo
	path   string
	sha256 string
}

func openObservedGit(
	ctx context.Context,
	environment []string,
	observer captureObserver,
) (*observedGit, error) {
	pathValue, err := exactPATH(environment)
	if err != nil {
		return nil, err
	}
	resolved, err := resolveGit(ctx, pathValue)
	if err != nil {
		return nil, err
	}
	before, err := os.Lstat(resolved)
	if err != nil || !before.Mode().IsRegular() || before.Mode().Perm()&0o111 == 0 {
		return nil, fmt.Errorf("resolved Git executable is not an executable regular file")
	}
	observe(observer, observeBeforeGitOpen, resolved)
	file, err := openGitExecutable(resolved)
	if err != nil {
		return nil, fmt.Errorf("open resolved Git executable: %w", err)
	}
	opened, openErr := file.Stat()
	after, afterErr := os.Lstat(resolved)
	if openErr != nil || afterErr != nil || !stableFile(before, opened) || !stableFile(opened, after) {
		_ = file.Close()
		return nil, fmt.Errorf("resolved Git executable changed while opening")
	}
	if err := requireSingleLink(file, opened); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("resolved Git executable: %w", err)
	}
	digest, count, err := hashOpenFile(ctx, file, maxGitExecutableBytes)
	if err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("hash resolved Git executable: %w", err)
	}
	return &observedGit{bytes: count, file: file, info: opened, path: resolved, sha256: digest}, nil
}

func resolveGit(ctx context.Context, pathValue string) (string, error) {
	directories := filepath.SplitList(pathValue)
	for _, directory := range directories {
		if err := ctx.Err(); err != nil {
			return "", fmt.Errorf("resolve Git executable: %w", err)
		}
		if directory == "" || !filepath.IsAbs(directory) || filepath.Clean(directory) != directory {
			return "", fmt.Errorf("PATH contains a noncanonical directory")
		}
	}
	for _, directory := range directories {
		if err := ctx.Err(); err != nil {
			return "", fmt.Errorf("resolve Git executable: %w", err)
		}
		candidate := filepath.Join(directory, "git")
		info, err := os.Stat(candidate)
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			return "", fmt.Errorf("inspect Git executable candidate: %w", err)
		}
		if !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
			continue
		}
		resolved, err := filepath.EvalSymlinks(candidate)
		if err != nil || !filepath.IsAbs(resolved) || filepath.Clean(resolved) != resolved {
			return "", fmt.Errorf("resolve Git executable symlinks")
		}
		return resolved, nil
	}
	return "", fmt.Errorf("Git executable is unavailable on scrubbed PATH")
}

func exactPATH(environment []string) (string, error) {
	value, count := "", 0
	for _, entry := range environment {
		name, candidate, ok := strings.Cut(entry, "=")
		if ok && name == "PATH" {
			value, count = candidate, count+1
		}
	}
	if count != 1 || value == "" {
		return "", fmt.Errorf("environment must contain exactly one nonempty PATH")
	}
	return value, nil
}

func (git *observedGit) verify(ctx context.Context) error {
	if err := git.verifyIdentity(ctx); err != nil {
		return err
	}
	digest, count, err := hashOpenFile(ctx, git.file, maxGitExecutableBytes)
	if err != nil || digest != git.sha256 || count != git.bytes {
		return fmt.Errorf("resolved Git executable bytes changed during capture")
	}
	return git.verifyIdentity(ctx)
}

func (git *observedGit) verifyIdentity(ctx context.Context) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	opened, openErr := git.file.Stat()
	named, namedErr := os.Lstat(git.path)
	if openErr != nil || namedErr != nil || !stableFile(git.info, opened) ||
		!stableFile(git.info, named) {
		return fmt.Errorf("resolved Git executable changed during capture")
	}
	if err := requireSingleLink(git.file, opened); err != nil {
		return fmt.Errorf("resolved Git executable: %w", err)
	}
	return nil
}

func hashOpenFile(ctx context.Context, file *os.File, maximum int64) (string, int64, error) {
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return "", 0, err
	}
	reader := &contextReader{ctx: ctx, reader: io.LimitReader(file, maximum+1)}
	content, err := io.ReadAll(reader)
	if err != nil || int64(len(content)) > maximum {
		return "", 0, fmt.Errorf("file is unreadable or exceeds %d bytes", maximum)
	}
	return sha256Bytes(content), int64(len(content)), nil
}

func (git *observedGit) close() { _ = git.file.Close() }

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (reader *contextReader) Read(buffer []byte) (int, error) {
	if err := reader.ctx.Err(); err != nil {
		return 0, err
	}
	return reader.reader.Read(buffer)
}
