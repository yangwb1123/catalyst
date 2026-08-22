// Package projectsnapshot captures an authority-neutral, bounded-interval
// projection of one local Git worktree. It deliberately excludes protected
// path bytes and never claims an atomic, current, complete, or trusted view.
package projectsnapshot

type Request struct {
	APIVersion       string `json:"api_version"`
	Canonicalization string `json:"canonicalization"`
	ExtractorID      string `json:"extractor_id"`
	ExtractorVersion string `json:"extractor_version"`
	PathPolicyID     string `json:"path_policy_id"`
	ProfileID        string `json:"profile_id"`
	ProjectID        string `json:"project_id"`
	RequestSHA256    string `json:"request_sha256"`
	RunID            string `json:"run_id"`
}

type Entry struct {
	Bytes         int64   `json:"bytes"`
	ContentSHA256 *string `json:"content_sha256"`
	EntrySHA256   string  `json:"entry_sha256"`
	Executable    *bool   `json:"executable"`
	IndexMode     *string `json:"index_mode"`
	Kind          string  `json:"kind"`
	Path          string  `json:"path"`
	PathSHA256    string  `json:"path_sha256"`
	Tracking      string  `json:"tracking"`
}

type Exclusion struct {
	ExclusionSHA256        string  `json:"exclusion_sha256"`
	IndexMode              *string `json:"index_mode"`
	LeafFilesystemObserved bool    `json:"leaf_filesystem_observed"`
	PathSHA256             string  `json:"path_sha256"`
	Reason                 string  `json:"reason"`
	Tracking               string  `json:"tracking"`
}

type GitObserver struct {
	ExecutableBytes      int64  `json:"executable_bytes"`
	ExecutableSHA256     string `json:"executable_sha256"`
	IdentityAttestation  string `json:"identity_attestation"`
	LocalConfigIsolation string `json:"local_config_isolation"`
	NetworkContainment   string `json:"network_containment"`
	Version              string `json:"version"`
}

type SourceManifest struct {
	APIVersion           string      `json:"api_version"`
	Canonicalization     string      `json:"canonicalization"`
	Entries              []Entry     `json:"entries"`
	EntrySetSHA256       string      `json:"entry_set_sha256"`
	Excluded             []Exclusion `json:"excluded"`
	ExclusionSetSHA256   string      `json:"exclusion_set_sha256"`
	GitObserver          GitObserver `json:"git_observer"`
	IgnoredPathCount     int64       `json:"ignored_path_count"`
	PathPolicyID         string      `json:"path_policy_id"`
	ProfileID            string      `json:"profile_id"`
	SourceManifestSHA256 string      `json:"source_manifest_sha256"`
	SourceRevision       string      `json:"source_revision"`
	UniverseCount        int64       `json:"universe_count"`
}

type CoverageCounts struct {
	ExcludedControlCount   int64 `json:"excluded_control_count"`
	ExcludedSensitiveCount int64 `json:"excluded_sensitive_count"`
	ExcludedSymlinkCount   int64 `json:"excluded_symlink_count"`
	IgnoredPathCount       int64 `json:"ignored_path_count"`
	IncludedRegularCount   int64 `json:"included_regular_count"`
	TrackedAbsentCount     int64 `json:"tracked_absent_count"`
	TrackedCount           int64 `json:"tracked_count"`
	UniverseCount          int64 `json:"universe_count"`
	UntrackedCount         int64 `json:"untracked_count"`
}

type CoverageSurface struct {
	ObservedItemCount int64    `json:"observed_item_count"`
	ReasonCodes       []string `json:"reason_codes"`
	Status            string   `json:"status"`
	Surface           string   `json:"surface"`
}

type Coverage struct {
	APIVersion           string            `json:"api_version"`
	Canonicalization     string            `json:"canonicalization"`
	Counts               CoverageCounts    `json:"counts"`
	CoverageSHA256       string            `json:"coverage_sha256"`
	SourceManifestSHA256 string            `json:"source_manifest_sha256"`
	Surfaces             []CoverageSurface `json:"surfaces"`
}

type Extractor struct {
	ExtractorID      string `json:"extractor_id"`
	ExtractorVersion string `json:"extractor_version"`
}

type Snapshot struct {
	APIVersion             string         `json:"api_version"`
	Atomic                 bool           `json:"atomic"`
	AuthorityAttested      bool           `json:"authority_attested"`
	Canonicalization       string         `json:"canonicalization"`
	Consistency            string         `json:"consistency"`
	Coverage               Coverage       `json:"coverage"`
	CoverageSHA256         string         `json:"coverage_sha256"`
	Currentness            string         `json:"currentness"`
	EffectAttested         bool           `json:"effect_attested"`
	Extractor              Extractor      `json:"extractor"`
	Freshness              string         `json:"freshness"`
	Kind                   string         `json:"kind"`
	PermissionAttested     bool           `json:"permission_attested"`
	PersistenceAttested    bool           `json:"persistence_attested"`
	PositiveResult         string         `json:"positive_result"`
	ProfileID              string         `json:"profile_id"`
	ProjectID              string         `json:"project_id"`
	RequestSHA256          string         `json:"request_sha256"`
	RunID                  string         `json:"run_id"`
	SnapshotID             string         `json:"snapshot_id"`
	SnapshotIdentitySHA256 string         `json:"snapshot_identity_sha256"`
	SnapshotSHA256         string         `json:"snapshot_sha256"`
	SourceManifest         SourceManifest `json:"source_manifest"`
	SourceManifestSHA256   string         `json:"source_manifest_sha256"`
	SystemCompleteness     string         `json:"system_completeness"`
	TruthAttested          bool           `json:"truth_attested"`
}

type Envelope struct {
	APIVersion       string   `json:"api_version"`
	Canonicalization string   `json:"canonicalization"`
	EnvelopeSHA256   string   `json:"envelope_sha256"`
	Kind             string   `json:"kind"`
	Request          Request  `json:"request"`
	Snapshot         Snapshot `json:"snapshot"`
}

type Production struct {
	envelope Envelope
	encoded  []byte
}

func (value *Production) JSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.encoded...)
}

func (value *Production) Envelope() Envelope {
	if value == nil {
		return Envelope{}
	}
	return cloneEnvelope(value.envelope)
}
