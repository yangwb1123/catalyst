// Package evolverepolocatorevidencecontract deterministically adapts one
// exact Evolve repository locator observation into a shadow repo_locator
// EvidenceRecord v1. It does not read files or reports and does not attest a
// scan judgment, completion, truth, authority, persistence, or effects.
package evolverepolocatorevidencecontract

import "forgeos/forge-core/internal/governancecontract"

const (
	APIVersion            = "forgeos.governance.evolve-repo-locator-evidence-adapter/v1"
	ObservationAPIVersion = "forgeos.evolve-repo-locator/v1"
	Canonicalization      = "forgeos.canonical-json/v1"
	ScanContract          = "evolve_scan_v1"
	AdaptedShadow         = "ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)"

	adapterPrincipalID = "forgeos.evolve-repo-locator-evidence-adapter"
	adapterRole        = "evidence-adapter"

	locatorDigestDomain  = "forgeos.governance.evolve-repo-locator.locator.v1"
	sourceDigestDomain   = "forgeos.governance.evolve-repo-locator-source.v1"
	requestDigestDomain  = "forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1"
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

type Content struct {
	Bytes  int64  `json:"bytes"`
	SHA256 string `json:"sha256"`
}

type Locator struct {
	Detail string `json:"detail"`
	Line   int64  `json:"line"`
	Path   string `json:"path"`
}

type Producer struct {
	ParametersSHA256 string `json:"parameters_sha256"`
	ProducerID       string `json:"producer_id"`
	ProducerType     string `json:"producer_type"`
	ProducerVersion  string `json:"producer_version"`
	RunID            string `json:"run_id"`
}

type ScanContext struct {
	Contract      string  `json:"contract"`
	Depth         string  `json:"depth"`
	Dimension     string  `json:"dimension"`
	OpportunityID *string `json:"opportunity_id"`
	Relation      string  `json:"relation"`
	ReportSHA256  string  `json:"report_sha256"`
}

type Source struct {
	SourceRevision   string `json:"source_revision"`
	SourceTreeSHA256 string `json:"source_tree_sha256"`
}

type Observation struct {
	APIVersion       string      `json:"api_version"`
	Canonicalization string      `json:"canonicalization"`
	Content          Content     `json:"content"`
	Locator          Locator     `json:"locator"`
	ObservedAtUnixMS int64       `json:"observed_at_unix_ms"`
	Producer         Producer    `json:"producer"`
	ScanContext      ScanContext `json:"scan_context"`
	Source           Source      `json:"source"`
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
	CanonicalLocatorJSON     []byte
	CanonicalObservationJSON []byte
	CanonicalRequestJSON     []byte
	Evidence                 *governancecontract.Record
	LocatorSHA256            string
	RequestSHA256            string
	Result                   string
	SourceSnapshotSHA256     string
}

func (a *Adaptation) LocatorJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalLocatorJSON...)
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
