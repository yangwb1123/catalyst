//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"io/fs"
	"os"
)

func readProtectedTrustRoot(config Config) ([]byte, error) {
	if err := validateConfig(config); err != nil {
		return nil, err
	}
	authorityInfo, err := validateAuthorityRoot(config.AuthorityRoot)
	if err != nil {
		return nil, err
	}
	repository, err := bindRepository(config.RepositoryRoot)
	if err != nil {
		return nil, err
	}
	overlap, err := rootsOverlap(config.AuthorityRoot, authorityInfo, repository)
	if err != nil || overlap {
		_ = repository.file.Close()
		return nil, fmt.Errorf("authority and repository roots overlap or changed")
	}
	authority, authorityDir, err := openBoundRoot(config.AuthorityRoot, authorityInfo)
	if err != nil {
		_ = repository.file.Close()
		return nil, err
	}
	defer closeRoots(authority, authorityDir, repository)
	raw, err := readAuthorityLeaf(authority, config.TrustRootPath,
		maxTrustRootBytes, privateMode)
	if err != nil {
		return nil, err
	}
	if err = verifyPrelockBindings(config.AuthorityRoot, authorityInfo, authority,
		authorityDir, repository); err != nil {
		return discardBytes(raw), err
	}
	return raw, nil
}

func verifyPrelockBindings(authorityPath string, authorityInfo fs.FileInfo,
	authority *os.Root, authorityDir *os.File, repository *directoryBinding) error {
	if err := verifyAbsoluteDirectoryPath(authorityPath, authorityInfo,
		"authority root"); err != nil {
		return err
	}
	current, err := os.Lstat(authorityPath)
	opened, openErr := authority.Lstat(".")
	dirInfo, dirErr := authorityDir.Stat()
	if err != nil || openErr != nil || dirErr != nil ||
		!os.SameFile(authorityInfo, current) || !os.SameFile(current, opened) ||
		!os.SameFile(opened, dirInfo) {
		return fmt.Errorf("authority root changed during preflight")
	}
	if err = requireOwnedDirectory(opened, privateDirMode, "authority root"); err != nil {
		return err
	}
	if err = verifyDirectoryBinding(repository); err != nil {
		return err
	}
	overlap, err := rootsOverlap(authorityPath, authorityInfo, repository)
	if err != nil || overlap {
		return fmt.Errorf("authority and repository roots overlap or changed")
	}
	return nil
}
