package productsource

import (
	"fmt"
	"sort"
	"strings"

	"forgeos/forge-core/internal/gitworktreesource"
)

// ValidateDeclaredRegularFileSet validates a strictly sorted set that may
// contain product files and exact canonical docs/release descendants. Release
// files stay outside the product-source digest, but must be regular entries in
// the same sealed worktree observation retained by Capture.
func ValidateDeclaredRegularFileSet(snapshot Snapshot, paths []string) error {
	if err := validateSnapshotProjection(snapshot); err != nil {
		return err
	}
	if paths == nil {
		return fmt.Errorf("declared regular file set must be non-nil")
	}
	for index, value := range paths {
		if index > 0 && paths[index-1] >= value {
			return fmt.Errorf("declared regular file paths must be strictly sorted and unique")
		}
		if err := validateDeclaredPathPolicy(snapshot.Manifest, value); err != nil {
			return err
		}
		if err := requireRegularWorktreeEntry(snapshot.worktree.Manifest, value); err != nil {
			return err
		}
	}
	return nil
}

func validateProductRegularFileSet(snapshot Snapshot, paths []string) error {
	if err := validateSnapshotProjection(snapshot); err != nil {
		return err
	}
	if paths == nil {
		return fmt.Errorf("product regular file set must be non-nil")
	}
	for index, value := range paths {
		if index > 0 && paths[index-1] >= value {
			return fmt.Errorf("product regular file paths must be strictly sorted and unique")
		}
		if !manifestContains(snapshot.Manifest.Entries, value) {
			return fmt.Errorf("regular file %q is absent from product source manifest", value)
		}
		if err := requireRegularWorktreeEntry(snapshot.worktree.Manifest, value); err != nil {
			return err
		}
	}
	return nil
}

func validateSnapshotProjection(snapshot Snapshot) error {
	if err := Validate(snapshot.Manifest, snapshot.SHA256); err != nil {
		return fmt.Errorf("validate product source snapshot: %w", err)
	}
	if snapshot.Root == "" || snapshot.Root != snapshot.worktree.Root {
		return fmt.Errorf("product source snapshot root differs from captured worktree")
	}
	if err := gitworktreesource.Validate(snapshot.worktree.Manifest, snapshot.worktree.SHA256); err != nil {
		return fmt.Errorf("validate captured worktree snapshot: %w", err)
	}
	projected, err := projectManifest(snapshot.worktree.Manifest)
	if err != nil {
		return fmt.Errorf("validate captured product projection: %w", err)
	}
	digest, err := Digest(projected)
	if err != nil || digest != snapshot.SHA256 {
		return fmt.Errorf("product source snapshot differs from captured worktree projection")
	}
	return nil
}

func validateDeclaredPathPolicy(manifest Manifest, value string) error {
	folded := foldASCII(value)
	if pathWithin(folded, ".forge") {
		return fmt.Errorf("declared regular file %q is forbidden Forge control state", value)
	}
	if pathWithin(folded, "docs/release") && !releaseArtifactPath(value) {
		return fmt.Errorf("declared regular file %q is not a canonical docs/release artifact", value)
	}
	if releaseArtifactPath(value) || manifestContains(manifest.Entries, value) {
		return nil
	}
	return fmt.Errorf("declared regular file %q is neither product source nor a release artifact", value)
}

func releaseArtifactPath(value string) bool {
	return strings.HasPrefix(value, "docs/release/") && len(value) > len("docs/release/")
}

func requireRegularWorktreeEntry(manifest gitworktreesource.SourceManifest, value string) error {
	position := sort.Search(len(manifest.Entries), func(index int) bool {
		return manifest.Entries[index].Path >= value
	})
	if position == len(manifest.Entries) || manifest.Entries[position].Path != value {
		return fmt.Errorf("declared regular file %q is absent from captured worktree", value)
	}
	entry := manifest.Entries[position]
	if entry.Kind != "regular" || entry.ContentSHA256 == nil {
		return fmt.Errorf("declared regular file %q is not a manifest-bound regular file", value)
	}
	return nil
}

func manifestContains(entries []gitworktreesource.SourceEntry, value string) bool {
	position := sort.Search(len(entries), func(index int) bool { return entries[index].Path >= value })
	return position < len(entries) && entries[position].Path == value
}

func pathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}
