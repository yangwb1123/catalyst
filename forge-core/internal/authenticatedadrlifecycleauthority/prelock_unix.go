//go:build unix

package authenticatedadrlifecycleauthority

import (
	"fmt"
	"io/fs"
	"os"
)

func readProtectedMaterials(config Config) ([][]byte, error) {
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
		return nil, fmt.Errorf("authority and repository overlap or changed")
	}
	authority, authorityDir, err := openBoundRoot(config.AuthorityRoot, authorityInfo)
	if err != nil {
		_ = repository.file.Close()
		return nil, err
	}
	defer closeRoots(authority, authorityDir, repository)
	paths := []string{config.SignatureProfilePath, config.ApprovalTrustRootPath,
		config.LifecycleTrustRootPath}
	maxima := []int64{maxProfile, maxRoot, maxRoot}
	result := make([][]byte, len(paths))
	for index, path := range paths {
		result[index], err = readAuthorityLeaf(authority, path, maxima[index], privateMode)
		if err != nil {
			clearMatrix(result)
			return nil, err
		}
	}
	if err = verifyPrelockBindings(config.AuthorityRoot, authorityInfo, authority,
		authorityDir, repository); err != nil {
		clearMatrix(result)
		return nil, err
	}
	return result, nil
}

func verifyPrelockBindings(authorityPath string, authorityInfo fs.FileInfo,
	authority *os.Root, authorityDir *os.File, repository *directoryBinding) error {
	if err := verifyAbsoluteDirectoryPath(authorityPath, authorityInfo, "authority root"); err != nil {
		return err
	}
	current, err := os.Lstat(authorityPath)
	opened, openErr := authority.Lstat(".")
	dirInfo, dirErr := authorityDir.Stat()
	if err != nil || openErr != nil || dirErr != nil || !os.SameFile(authorityInfo, current) ||
		!os.SameFile(current, opened) || !os.SameFile(opened, dirInfo) {
		return fmt.Errorf("authority root changed")
	}
	if err = requireOwnedDirectory(opened, privateDir, "authority root"); err != nil {
		return err
	}
	if err = verifyDirectoryBinding(repository); err != nil {
		return err
	}
	overlap, err := rootsOverlap(authorityPath, authorityInfo, repository)
	if err != nil || overlap {
		return fmt.Errorf("authority and repository overlap or changed")
	}
	return nil
}
