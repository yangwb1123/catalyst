//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

type protectedSession struct {
	mu      sync.Mutex
	backend *unixState
}

type unixState struct {
	authority       *os.Root
	authorityLeaves map[string]authorityLeafBinding
	authorityDir    *os.File
	authorityInfo   fs.FileInfo
	authorityPath   string
	lock            *os.File
	lockInfo        fs.FileInfo
	maxBytes        int64
	port            commitPort
	repository      *directoryBinding
	state           *os.Root
	stateDir        *os.File
	stateInfo       fs.FileInfo
	stateRelative   string
}

func checkStatePlatform() error { return nil }

func openProtectedState(config Config) (stateSession, error) {
	session, err := openProtectedStateWith(config, nil)
	if err != nil {
		return nil, err
	}
	return session, nil
}

func openProtectedStateWith(config Config, port commitPort) (*protectedSession, error) {
	if err := validateConfig(config); err != nil {
		return nil, err
	}
	authorityInfo, err := validateAuthorityRoot(config.AuthorityRoot)
	if err != nil {
		return nil, err
	}
	repository, err := bindRepository(config.RepositoryRoot)
	if err != nil {
		return nil, fmt.Errorf("bind repository identity: %w", err)
	}
	overlap, err := rootsOverlap(config.AuthorityRoot, authorityInfo, repository)
	if err != nil || overlap {
		_ = repository.file.Close()
		return nil, fmt.Errorf("authority and repository roots overlap or are unstable")
	}
	authority, authorityDir, err := openBoundRoot(config.AuthorityRoot, authorityInfo)
	if err != nil {
		_ = repository.file.Close()
		return nil, err
	}
	return bindApprovalState(config, authority, authorityDir, authorityInfo, repository, port)
}

func validateAuthorityRoot(value string) (fs.FileInfo, error) {
	info, err := inspectNoSymlinkAbsolute(value)
	if err != nil {
		return nil, err
	}
	if err := requireOwnedDirectory(info, privateDirMode, "authority root"); err != nil {
		return nil, err
	}
	resolved, err := filepath.EvalSymlinks(value)
	if err != nil || resolved != value {
		return nil, fmt.Errorf("authority root must resolve to itself")
	}
	return info, nil
}

func openBoundRoot(value string, expected fs.FileInfo) (*os.Root, *os.File, error) {
	root, err := os.OpenRoot(value)
	if err != nil {
		return nil, nil, err
	}
	opened, openErr := root.Lstat(".")
	current, currentErr := os.Lstat(value)
	if openErr != nil || currentErr != nil || !os.SameFile(expected, opened) ||
		!os.SameFile(opened, current) {
		_ = root.Close()
		return nil, nil, fmt.Errorf("authority root changed while opening")
	}
	dir, err := root.Open(".")
	if err != nil {
		_ = root.Close()
		return nil, nil, err
	}
	return root, dir, nil
}

func bindApprovalState(config Config, authority *os.Root, authorityDir *os.File,
	authorityInfo fs.FileInfo, repository *directoryBinding, port commitPort,
) (*protectedSession, error) {
	state, stateDir, stateInfo, err := openRelativeDirectory(authority, config.StateDir)
	if err != nil {
		closeRoots(authority, authorityDir, repository)
		return nil, err
	}
	if err := requireOwnedDirectory(stateInfo, privateDirMode, "approval state directory"); err != nil {
		_ = state.Close()
		_ = stateDir.Close()
		closeRoots(authority, authorityDir, repository)
		return nil, err
	}
	if port == nil {
		port = osCommitPort{}
	}
	backend := &unixState{authority: authority, authorityDir: authorityDir,
		authorityInfo: authorityInfo, authorityPath: config.AuthorityRoot,
		authorityLeaves: map[string]authorityLeafBinding{},
		maxBytes:        maxLedgerBytes, port: port, repository: repository, state: state,
		stateDir: stateDir, stateInfo: stateInfo, stateRelative: config.StateDir}
	if err := backend.acquireLock(); err != nil {
		_ = backend.close()
		return nil, err
	}
	if _, _, err := readStateLeaf(backend.state, stateLedgerFile, maxLedgerBytes); err != nil {
		_ = backend.close()
		return nil, fmt.Errorf("existing approval ledger rejected: %w", err)
	}
	return &protectedSession{backend: backend}, nil
}

func openRelativeDirectory(root *os.Root, relative string) (*os.Root, *os.File, fs.FileInfo, error) {
	current, owned := root, false
	for _, component := range strings.Split(relative, "/") {
		next, info, err := openChildDirectory(current, component)
		if owned {
			_ = current.Close()
		}
		if err != nil || requireOwnedDirectory(info, privateDirMode, "authority directory") != nil {
			if next != nil {
				_ = next.Close()
			}
			return nil, nil, nil, fmt.Errorf("authority directory component rejected")
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
	if openErr != nil || afterErr != nil || !os.SameFile(before, opened) ||
		!os.SameFile(opened, after) {
		_ = child.Close()
		return nil, nil, fmt.Errorf("directory component changed while opening")
	}
	return child, opened, nil
}

func closeRoots(authority *os.Root, authorityDir *os.File, repository *directoryBinding) {
	_ = authority.Close()
	_ = authorityDir.Close()
	if repository != nil && repository.file != nil {
		_ = repository.file.Close()
	}
}
