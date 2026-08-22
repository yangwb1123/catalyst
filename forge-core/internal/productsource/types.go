// Package productsource captures the bounded product-source projection used by
// local output receipts. It intentionally excludes Forge control state and
// generated release documents from the existing hardened Git worktree capture.
package productsource

import "forgeos/forge-core/internal/gitworktreesource"

const (
	APIVersion       = "forgeos.product-source-state/v1"
	Canonicalization = "forgeos.canonical-json/v1"
	ProfileID        = "product-source-state-v1"
	DigestDomain     = "forgeos.local-product-source-state.v1"
)

type Manifest struct {
	APIVersion       string                          `json:"api_version"`
	Canonicalization string                          `json:"canonicalization"`
	Entries          []gitworktreesource.SourceEntry `json:"entries"`
	ProfileID        string                          `json:"profile_id"`
	SourceRevision   string                          `json:"source_revision"`
}

type Snapshot struct {
	Root     string
	Manifest Manifest
	SHA256   string

	worktree gitworktreesource.Snapshot
}

type RegularFile = gitworktreesource.RegularFile
type RegularReadLimits = gitworktreesource.RegularReadLimits

func CloneManifest(value Manifest) Manifest {
	base := gitworktreesource.SourceManifest{Entries: value.Entries}
	value.Entries = gitworktreesource.CloneManifest(base).Entries
	return value
}

func SameCapturedRoot(first, second Snapshot) bool {
	return gitworktreesource.SameCapturedRoot(first.worktree, second.worktree)
}
