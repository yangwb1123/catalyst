// Package commandobservationevidencecontract deterministically adapts one
// exact command observation into a shadow gate/test EvidenceRecord v1.
// It does not execute commands or attest pass, completion, truth, authority,
// persistence, claims, atoms, or effects.
package commandobservationevidencecontract

import "forgeos/forge-core/internal/governancecontract"

const (
	APIVersion            = "forgeos.governance.command-observation-evidence-adapter/v1"
	ObservationAPIVersion = "forgeos.command-observation/v1"
	Canonicalization      = "forgeos.canonical-json/v1"
	AdaptedShadow         = "ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, truth, authority, claim, atom, persistence, or effect attestation)"

	adapterPrincipalID = "forgeos.command-observation-evidence-adapter"
	adapterRole        = "evidence-adapter"

	commandDigestDomain  = "forgeos.governance.command-observation.command.v1"
	sourceDigestDomain   = "forgeos.governance.command-observation-source.v1"
	requestDigestDomain  = "forgeos.governance.command-observation-evidence-adapter.request.v1"
	evidenceDigestDomain = "forgeos.governance.evidence-record.v1"
)

type Binding struct {
	AggregateID         string   `json:"aggregate_id"`
	ContextSHA256       string   `json:"context_sha256"`
	PolicySHA256        string   `json:"policy_sha256"`
	ProjectID           string   `json:"project_id"`
	Scope               string   `json:"scope"`
	Sensitivity         string   `json:"sensitivity"`
	Sequence            int64    `json:"sequence"`
	Subjects            []string `json:"subjects"`
	SupersedesRecordIDs []string `json:"supersedes_record_ids"`
}

type Command struct {
	Argv               []string `json:"argv"`
	CWD                string   `json:"cwd"`
	EnvironmentSHA256  string   `json:"environment_sha256"`
	StdinBytes         int64    `json:"stdin_bytes"`
	StdinSHA256        string   `json:"stdin_sha256"`
	TimeoutMS          *int64   `json:"timeout_ms"`
	ToolSnapshotSHA256 string   `json:"tool_snapshot_sha256"`
}

type Producer struct {
	ProducerID      string `json:"producer_id"`
	ProducerType    string `json:"producer_type"`
	ProducerVersion string `json:"producer_version"`
	RunID           string `json:"run_id"`
}

type Source struct {
	SourceRevision   string `json:"source_revision"`
	SourceTreeSHA256 string `json:"source_tree_sha256"`
}

type Stream struct {
	Bytes          int64  `json:"bytes"`
	RetainedBytes  int64  `json:"retained_bytes"`
	RetainedSHA256 string `json:"retained_sha256"`
	SHA256         string `json:"sha256"`
}

type Streams struct {
	Combined Stream `json:"combined"`
	Stderr   Stream `json:"stderr"`
	Stdout   Stream `json:"stdout"`
}

type Termination struct {
	ExitCode *int64 `json:"exit_code"`
	Kind     string `json:"kind"`
}

type Observation struct {
	APIVersion       string      `json:"api_version"`
	Canonicalization string      `json:"canonicalization"`
	Command          Command     `json:"command"`
	EndedAtUnixMS    int64       `json:"ended_at_unix_ms"`
	EvidenceType     string      `json:"evidence_type"`
	Producer         Producer    `json:"producer"`
	Source           Source      `json:"source"`
	StartedAtUnixMS  int64       `json:"started_at_unix_ms"`
	Streams          Streams     `json:"streams"`
	Termination      Termination `json:"termination"`
}

type Request struct {
	APIVersion       string      `json:"api_version"`
	Binding          Binding     `json:"binding"`
	Canonicalization string      `json:"canonicalization"`
	Observation      Observation `json:"observation"`
}

// Adaptation exposes the three independent input identities and one sealed
// Governance record. Result is always AdaptedShadow on success.
type Adaptation struct {
	CanonicalCommandJSON     []byte
	CanonicalObservationJSON []byte
	CanonicalRequestJSON     []byte
	CommandSHA256            string
	Evidence                 *governancecontract.Record
	RequestSHA256            string
	Result                   string
	SourceSnapshotSHA256     string
}

func (a *Adaptation) CommandJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalCommandJSON...)
}

func (a *Adaptation) ObservationJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalObservationJSON...)
}

func (a *Adaptation) RequestJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalRequestJSON...)
}

func (a *Adaptation) EvidenceJSON() []byte {
	if a == nil || a.Evidence == nil {
		return nil
	}
	return a.Evidence.RecordJSON()
}
