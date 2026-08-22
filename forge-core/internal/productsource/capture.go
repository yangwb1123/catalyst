package productsource

import (
	"context"
	"fmt"
	"strings"

	"forgeos/forge-core/internal/gitworktreesource"
)

// Capture records one bounded-interval product source observation. The
// underlying worktree reader rejects unstable files, parent symlinks, gitlinks,
// non-regular entries and inventory ambiguity. This remains an observation, not
// an atomic repository snapshot or execution pin.
func Capture(ctx context.Context, root string, environment []string) (Snapshot, error) {
	worktree, err := gitworktreesource.Capture(ctx, root, environment)
	if err != nil {
		return Snapshot{}, fmt.Errorf("capture product source: %w", err)
	}
	manifest, err := projectManifest(worktree.Manifest)
	if err != nil {
		return Snapshot{}, fmt.Errorf("capture product source: %w", err)
	}
	digest, err := Digest(manifest)
	if err != nil {
		return Snapshot{}, fmt.Errorf("seal product source: %w", err)
	}
	return Snapshot{
		Root: worktree.Root, Manifest: CloneManifest(manifest), SHA256: digest,
		worktree: worktree,
	}, nil
}

func projectManifest(worktree gitworktreesource.SourceManifest) (Manifest, error) {
	entries := make([]gitworktreesource.SourceEntry, 0, len(worktree.Entries))
	for _, entry := range worktree.Entries {
		if err := validateProductPath(entry.Path); err != nil {
			return Manifest{}, err
		}
		if forgeControlPath(entry.Path) && entry.Tracking == "tracked" {
			return Manifest{}, fmt.Errorf("tracked Forge control path %q cannot be excluded", entry.Path)
		}
		if excludedProductPath(entry.Path) {
			continue
		}
		entries = append(entries, entry)
	}
	manifest := Manifest{
		APIVersion: APIVersion, Canonicalization: Canonicalization,
		Entries: entries, ProfileID: ProfileID,
		SourceRevision: worktree.SourceRevision,
	}
	return manifest, nil
}

func excludedProductPath(path string) bool {
	return forgeControlPath(path) ||
		path == "docs/release" || strings.HasPrefix(path, "docs/release/")
}

func forgeControlPath(path string) bool {
	return path == ".forge" || strings.HasPrefix(path, ".forge/")
}

func validateProductPath(value string) error {
	folded := foldASCII(value)
	if (folded == "docs/release" || strings.HasPrefix(folded, "docs/release/")) &&
		!excludedProductPath(value) {
		return fmt.Errorf("product source path %q is a portable alias of docs/release", value)
	}
	return nil
}

func foldASCII(value string) string {
	folded := []byte(value)
	for index, character := range folded {
		if character >= 'A' && character <= 'Z' {
			folded[index] = character + ('a' - 'A')
		}
	}
	return string(folded)
}
