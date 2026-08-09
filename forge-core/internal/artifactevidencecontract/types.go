// Package artifactevidencecontract deterministically adapts one exact
// forgeos.artifact.v1 provenance record into a shadow EvidenceRecord v1.
// It has no claim, atom, persistence, authority, or effect capability.
package artifactevidencecontract

import (
	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/governancecontract"
)

const (
	APIVersion       = "forgeos.governance.artifact-evidence-adapter/v1"
	Canonicalization = "forgeos.canonical-json/v1"
	AdaptedShadow    = "ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)"

	adapterPrincipalID = "forgeos.artifact-evidence-adapter"
	adapterRole        = "evidence-adapter"
	adapterVersion     = "v1"

	sourceDigestDomain   = "forgeos.governance.artifact-provenance-source.v1"
	requestDigestDomain  = "forgeos.governance.artifact-evidence-adapter.request.v1"
	evidenceDigestDomain = "forgeos.governance.evidence-record.v1"
)

// Binding supplies governance identity and snapshot bindings unavailable in
// the legacy artifact provenance record. These values remain declarations.
type Binding struct {
	AggregateID         string   `json:"aggregate_id"`
	ContextSHA256       string   `json:"context_sha256"`
	PolicySHA256        string   `json:"policy_sha256"`
	ProjectID           string   `json:"project_id"`
	Scope               string   `json:"scope"`
	Sensitivity         string   `json:"sensitivity"`
	Sequence            int64    `json:"sequence"`
	SourceRevision      string   `json:"source_revision"`
	SourceTreeSHA256    string   `json:"source_tree_sha256"`
	Subjects            []string `json:"subjects"`
	SupersedesRecordIDs []string `json:"supersedes_record_ids"`
}

// Request is the complete pure adapter input. Its canonical bytes are part of
// Evidence identity, so no ambient clock, repository read, or hidden default
// participates in adaptation.
type Request struct {
	APIVersion       string          `json:"api_version"`
	Artifact         artifact.Record `json:"artifact"`
	Binding          Binding         `json:"binding"`
	Canonicalization string          `json:"canonicalization"`
}

// Adaptation exposes exact request/source identities and the independently
// sealed Governance record. Result is always AdaptedShadow on success.
type Adaptation struct {
	CanonicalRequestJSON []byte
	CanonicalSourceJSON  []byte
	Evidence             *governancecontract.Record
	RequestSHA256        string
	Result               string
	SourceSHA256         string
}

// SourceJSON returns a defensive copy of exact canonical artifact source bytes.
func (a *Adaptation) SourceJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalSourceJSON...)
}

// RequestJSON returns a defensive copy of exact canonical request bytes.
func (a *Adaptation) RequestJSON() []byte {
	if a == nil {
		return nil
	}
	return append([]byte(nil), a.CanonicalRequestJSON...)
}

// EvidenceJSON returns exact canonical EvidenceRecord bytes.
func (a *Adaptation) EvidenceJSON() []byte {
	if a == nil || a.Evidence == nil {
		return nil
	}
	return a.Evidence.RecordJSON()
}
