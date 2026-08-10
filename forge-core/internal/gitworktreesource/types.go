// Package gitworktreesource captures and validates the bounded Git worktree
// source profile shared by local command observations and Evolve evidence.
package gitworktreesource

import "os"

const (
	APIVersion       = "forgeos.command-capture.source-tree/v1"
	Canonicalization = "forgeos.canonical-json/v1"
	ProfileID        = "git-worktree-source-tree-v1"
	DigestDomain     = "forgeos.governance.local-command-source-tree-profile.v1"
)

type SourceEntry struct {
	Bytes         int64   `json:"bytes"`
	ContentSHA256 *string `json:"content_sha256"`
	Executable    *bool   `json:"executable"`
	IndexMode     *string `json:"index_mode"`
	Kind          string  `json:"kind"`
	Path          string  `json:"path"`
	SymlinkTarget *string `json:"symlink_target"`
	Tracking      string  `json:"tracking"`
}

type SourceManifest struct {
	APIVersion       string        `json:"api_version"`
	Canonicalization string        `json:"canonicalization"`
	Entries          []SourceEntry `json:"entries"`
	ProfileID        string        `json:"profile_id"`
	SourceRevision   string        `json:"source_revision"`
}

type Snapshot struct {
	Root     string
	Manifest SourceManifest
	SHA256   string

	captureManifestSHA256 string
	captureRootIdentity   os.FileInfo
}

// SameCapturedRoot reports whether two snapshots were captured from the same
// stable repository root directory identity, not merely the same path.
func SameCapturedRoot(first, second Snapshot) bool {
	return first.Root == second.Root &&
		stableSourceDirectory(first.captureRootIdentity, second.captureRootIdentity)
}

func CloneManifest(value SourceManifest) SourceManifest {
	entries := make([]SourceEntry, len(value.Entries))
	for index, entry := range value.Entries {
		entry.ContentSHA256 = cloneString(entry.ContentSHA256)
		entry.Executable = cloneBool(entry.Executable)
		entry.IndexMode = cloneString(entry.IndexMode)
		entry.SymlinkTarget = cloneString(entry.SymlinkTarget)
		entries[index] = entry
	}
	value.Entries = entries
	return value
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}

func cloneBool(value *bool) *bool {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
