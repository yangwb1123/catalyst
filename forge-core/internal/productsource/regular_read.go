package productsource

import (
	"context"
	"fmt"

	"forgeos/forge-core/internal/gitworktreesource"
)

// ReadRegularFiles returns exact bytes for sorted declared product paths and
// revalidates them against the hardened worktree snapshot captured by Capture.
func ReadRegularFiles(
	ctx context.Context,
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
) ([]RegularFile, error) {
	selected, err := boundedPathCopy(paths, limits)
	if err != nil {
		return nil, err
	}
	if err := validateProductRegularFileSet(snapshot, selected); err != nil {
		return nil, err
	}
	files, err := gitworktreesource.ReadRegularFiles(ctx, snapshot.worktree, selected, limits)
	if err != nil {
		return nil, fmt.Errorf("read declared product artifacts: %w", err)
	}
	return files, nil
}

// ReadSingleLinkRegularFiles is the product-only reader that additionally
// requires hard-link count verification before and after the exact-byte read.
func ReadSingleLinkRegularFiles(
	ctx context.Context,
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
) ([]RegularFile, error) {
	selected, err := boundedPathCopy(paths, limits)
	if err != nil {
		return nil, err
	}
	if err := validateProductRegularFileSet(snapshot, selected); err != nil {
		return nil, err
	}
	files, err := gitworktreesource.ReadSingleLinkRegularFiles(ctx, snapshot.worktree, selected, limits)
	if err != nil {
		return nil, fmt.Errorf("read single-link declared product artifacts: %w", err)
	}
	return files, nil
}

// ReadSingleLinkDeclaredFiles reads a mixed set of product and canonical
// docs/release artifacts from Capture's retained hardened worktree snapshot.
// It rejects control paths, portable release aliases, non-regular leaves and
// hard links without adding release bytes to the product-source projection.
func ReadSingleLinkDeclaredFiles(
	ctx context.Context,
	snapshot Snapshot,
	paths []string,
	limits RegularReadLimits,
) ([]RegularFile, error) {
	selected, err := boundedPathCopy(paths, limits)
	if err != nil {
		return nil, err
	}
	if err := ValidateDeclaredRegularFileSet(snapshot, selected); err != nil {
		return nil, err
	}
	files, err := gitworktreesource.ReadSingleLinkRegularFiles(ctx, snapshot.worktree, selected, limits)
	if err != nil {
		return nil, fmt.Errorf("read single-link declared artifacts: %w", err)
	}
	return files, nil
}

func boundedPathCopy(paths []string, limits RegularReadLimits) ([]string, error) {
	if paths == nil || limits.MaxFiles < 1 || len(paths) > limits.MaxFiles {
		return nil, fmt.Errorf("regular file path set is invalid or exceeds %d files", limits.MaxFiles)
	}
	return append([]string{}, paths...), nil
}
