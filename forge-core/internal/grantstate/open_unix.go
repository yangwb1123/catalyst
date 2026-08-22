//go:build unix

package grantstate

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"syscall"
)

type unixSession struct {
	authority        *os.Root
	authorityDir     *os.File
	authorityInfo    fs.FileInfo
	authorityPath    string
	lock             *os.File
	lockInfo         fs.FileInfo
	layout           stateLayout
	maxBytes         int64
	port             commitPort
	repositoryDir    *os.File
	repositoryInfo   fs.FileInfo
	repositoryPath   string
	repositorySource string
	state            *os.Root
	stateDirFile     *os.File
	stateDir         string
	stateInfo        fs.FileInfo
}

func openPlatform(config Config, port commitPort) (*Session, error) {
	if err := validateConfig(config); err != nil {
		return nil, newError(CodeInvalid, "open", err.Error(), nil)
	}
	authorityInfo, repository, err := validateAuthority(config.AuthorityRoot, config.RepositoryRoot)
	if err != nil {
		return nil, newError(CodeUnsafe, "open", "authority root rejected", err)
	}
	authority, authorityDir, err := openBoundRoot(config.AuthorityRoot, authorityInfo)
	if err != nil {
		_ = repository.file.Close()
		return nil, newError(CodeUnsafe, "open", "bind authority root", err)
	}
	backend, err := bindState(config.AuthorityRoot, config.StateDir, config.MaxBytes,
		issuanceLayout, authority, authorityDir, authorityInfo, repository, port)
	if err != nil {
		_ = authority.Close()
		_ = authorityDir.Close()
		_ = repository.file.Close()
		return nil, err
	}
	return &Session{backend: backend}, nil
}

func openUsagePlatform(config Config, port commitPort) (*Session, error) {
	if err := validateUsageConfig(config); err != nil {
		return nil, newError(CodeInvalid, "open usage", err.Error(), nil)
	}
	authorityInfo, err := validateAuthorityRoot(config.AuthorityRoot)
	if err != nil {
		return nil, newError(CodeUnsafe, "open usage", "authority root rejected", err)
	}
	authority, authorityDir, err := openBoundRoot(config.AuthorityRoot, authorityInfo)
	if err != nil {
		return nil, newError(CodeUnsafe, "open usage", "bind authority root", err)
	}
	backend, err := bindState(config.AuthorityRoot, config.StateDir, config.MaxBytes,
		usageLayout, authority, authorityDir, authorityInfo, nil, port)
	if err != nil {
		_ = authority.Close()
		_ = authorityDir.Close()
		return nil, err
	}
	return &Session{backend: backend}, nil
}

func validateConfig(config Config) error {
	if err := validateAbsolute(config.AuthorityRoot, "authority root"); err != nil {
		return err
	}
	if config.RepositoryRoot == "" {
		return fmt.Errorf("repository root is required")
	}
	if err := validateRelative(config.StateDir, "state directory"); err != nil {
		return err
	}
	if config.MaxBytes <= 0 || config.MaxBytes > AbsoluteMaxBytes {
		return fmt.Errorf("max bytes must be between 1 and %d", AbsoluteMaxBytes)
	}
	return nil
}

func validateUsageConfig(config Config) error {
	if err := validateAbsolute(config.AuthorityRoot, "authority root"); err != nil {
		return err
	}
	if err := validateRelative(config.StateDir, "state directory"); err != nil {
		return err
	}
	if config.RepositoryRoot != "" {
		return fmt.Errorf("usage state repository must be bound lazily")
	}
	if config.MaxBytes <= 0 || config.MaxBytes > AbsoluteMaxBytes {
		return fmt.Errorf("max bytes must be between 1 and %d", AbsoluteMaxBytes)
	}
	return nil
}

func validateAuthority(authority, repository string) (fs.FileInfo, *directoryBinding, error) {
	info, err := validateAuthorityRoot(authority)
	if err != nil {
		return nil, nil, err
	}
	boundRepository, err := bindRepository(repository)
	if err != nil {
		return nil, nil, err
	}
	if err := requireRootsDisjoint(authority, info, boundRepository); err != nil {
		_ = boundRepository.file.Close()
		return nil, nil, err
	}
	return info, boundRepository, nil
}

func validateAuthorityRoot(authority string) (fs.FileInfo, error) {
	info, err := inspectNoSymlinkAbsolute(authority)
	if err != nil {
		return nil, err
	}
	if err := requireOwnedDirectory(info, 0o700, "authority root"); err != nil {
		return nil, err
	}
	resolvedAuthority, err := filepath.EvalSymlinks(authority)
	if err != nil || resolvedAuthority != authority {
		return nil, fmt.Errorf("authority root must resolve to itself")
	}
	return info, nil
}

func bindState(
	authorityPath, stateDir string,
	maxBytes int64,
	layout stateLayout,
	authority *os.Root,
	authorityDir *os.File,
	authorityInfo fs.FileInfo,
	repository *directoryBinding,
	port commitPort,
) (*unixSession, error) {
	if !layout.valid() {
		return nil, newError(CodeInvalid, "open", "state layout is invalid", nil)
	}
	state, stateDirFile, info, err := openRelativeDirectory(authority, stateDir)
	if err != nil {
		return nil, newError(CodeUnsafe, "open", "bind state directory", err)
	}
	if err := requireOwnedDirectory(info, 0o700, "state directory"); err != nil {
		_ = state.Close()
		_ = stateDirFile.Close()
		return nil, newError(CodeUnsafe, "open", "state directory rejected", err)
	}
	if port == nil {
		port = osCommitPort{}
	}
	backend := &unixSession{
		authority: authority, authorityDir: authorityDir, authorityInfo: authorityInfo,
		authorityPath: authorityPath, layout: layout, maxBytes: maxBytes, port: port,
		state: state, stateDir: stateDir, stateDirFile: stateDirFile, stateInfo: info,
	}
	if repository != nil {
		backend.setRepository(repository)
	}
	if err := backend.acquireLock(); err != nil {
		_ = state.Close()
		_ = stateDirFile.Close()
		return nil, err
	}
	if err := backend.validateInitialLedger(); err != nil {
		_ = backend.close()
		return nil, err
	}
	return backend, nil
}

func (s *unixSession) validateInitialLedger() error {
	if _, _, err := readStateLeaf(s.state, s.layout.ledger, s.maxBytes); err != nil {
		return newError(CodeUnsafe, "open", "existing ledger rejected", err)
	}
	return nil
}

func openBoundRoot(path string, expected fs.FileInfo) (*os.Root, *os.File, error) {
	root, err := os.OpenRoot(path)
	if err != nil {
		return nil, nil, err
	}
	opened, openErr := root.Lstat(".")
	current, currentErr := os.Lstat(path)
	if openErr != nil || currentErr != nil || !os.SameFile(expected, opened) || !os.SameFile(opened, current) {
		_ = root.Close()
		return nil, nil, fmt.Errorf("root changed while opening")
	}
	dir, err := root.Open(".")
	if err != nil {
		_ = root.Close()
		return nil, nil, err
	}
	return root, dir, nil
}

func openRelativeDirectory(root *os.Root, relative string) (*os.Root, *os.File, fs.FileInfo, error) {
	current := root
	owned := false
	for _, component := range splitRelative(relative) {
		next, _, err := openChildDirectory(current, component)
		if owned {
			_ = current.Close()
		}
		if err != nil {
			return nil, nil, nil, err
		}
		current, owned = next, true
	}
	info, err := current.Lstat(".")
	if err != nil {
		_ = current.Close()
		return nil, nil, nil, err
	}
	dir, err := current.Open(".")
	if err != nil {
		_ = current.Close()
		return nil, nil, nil, err
	}
	return current, dir, info, nil
}

func openChildDirectory(parent *os.Root, name string) (*os.Root, fs.FileInfo, error) {
	before, err := parent.Lstat(name)
	if err != nil || before.Mode()&os.ModeSymlink != 0 || !before.IsDir() {
		return nil, nil, fmt.Errorf("directory component is missing, aliased, or not a directory")
	}
	child, err := parent.OpenRoot(name)
	if err != nil {
		return nil, nil, err
	}
	opened, openErr := child.Lstat(".")
	after, afterErr := parent.Lstat(name)
	if openErr != nil || afterErr != nil || !os.SameFile(before, opened) || !os.SameFile(opened, after) {
		_ = child.Close()
		return nil, nil, fmt.Errorf("directory component changed while opening")
	}
	return child, opened, nil
}

func (s *unixSession) acquireLock() error {
	lock, created, err := s.openOrCreateLock()
	if err != nil {
		return newError(CodeUnsafe, "lock", "open stable lock", err)
	}
	if created {
		if err := persistNewLock(lock, s.stateDirFile); err != nil {
			_ = lock.Close()
			return newError(CodePersistence, "lock", "persist stable lock", err)
		}
	}
	info, err := validateOpenStateLeaf(lock, 0o600, "lock file")
	if err != nil {
		_ = lock.Close()
		return newError(CodeUnsafe, "lock", "lock file rejected", err)
	}
	if err := lockExclusive(lock); err != nil {
		_ = lock.Close()
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EAGAIN) ||
			errors.Is(err, syscall.EACCES) {
			return newError(CodeBusy, "lock", "another issuer holds the lock", ErrBusy)
		}
		return newError(CodeUnsafe, "lock", "acquire nonblocking lock", err)
	}
	s.lock, s.lockInfo = lock, info
	if err := s.verifyBindings(); err != nil {
		_ = unlock(lock)
		_ = lock.Close()
		s.lock = nil
		return err
	}
	return nil
}

func (s *unixSession) openOrCreateLock() (*os.File, bool, error) {
	lock, err := openExistingRegular(s.state, s.layout.lock, true)
	if err == nil {
		return lock, false, nil
	}
	if !errors.Is(err, fs.ErrNotExist) {
		return nil, false, err
	}
	lock, err = s.state.OpenFile(s.layout.lock, os.O_RDWR|os.O_CREATE|os.O_EXCL|syscall.O_NONBLOCK, 0o600)
	if errors.Is(err, fs.ErrExist) {
		lock, err = openExistingRegular(s.state, s.layout.lock, true)
		return lock, false, err
	}
	if err != nil {
		return nil, false, err
	}
	if err := lock.Chmod(0o600); err != nil {
		_ = lock.Close()
		return nil, true, err
	}
	return lock, true, nil
}

func persistNewLock(lock, state *os.File) error {
	if err := lock.Sync(); err != nil {
		return err
	}
	return state.Sync()
}
