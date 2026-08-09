// Package cognitiveatomcontract implements the deterministic, shadow-only
// CognitiveAtom v1 projection from Governance Evidence/Claim v1 record sets.
// A structurally valid atom does not attest truth, authority, instruction
// eligibility, or hard-guard eligibility.
package cognitiveatomcontract

import "forgeos/forge-core/internal/governancecontract"

const (
	APIVersion       = "forgeos.aadm.cognitive-atom/v1"
	Kind             = "CognitiveAtom"
	Canonicalization = "forgeos.canonical-json/v1"

	atomDigestDomain    = "forgeos.aadm.cognitive-atom.v1"
	atomIDDomain        = "forgeos.aadm.cognitive-atom-id.v1"
	atomSetDigestDomain = "forgeos.aadm.cognitive-atom-set.v1"
	closureDigestDomain = "forgeos.governance.record-set.v1"
)

type Integrity struct {
	CanonicalSHA256  string `json:"canonical_sha256"`
	Canonicalization string `json:"canonicalization"`
}

type Metadata struct {
	AtomID           string `json:"atom_id"`
	ContextSHA256    string `json:"context_sha256"`
	PolicySHA256     string `json:"policy_sha256"`
	ProjectID        string `json:"project_id"`
	Scope            string `json:"scope"`
	SourceRevision   string `json:"source_revision"`
	SourceTreeSHA256 string `json:"source_tree_sha256"`
	TaskID           string `json:"task_id"`
}

type Source struct {
	CanonicalSHA256    string `json:"canonical_sha256"`
	ClaimAggregateID   string `json:"claim_aggregate_id"`
	ClaimRecordID      string `json:"claim_record_id"`
	ClaimSequence      int64  `json:"claim_sequence"`
	ClosureByteCount   int64  `json:"closure_byte_count"`
	ClosureRecordCount int64  `json:"closure_record_count"`
	ClosureSHA256      string `json:"closure_sha256"`
	RecordKind         string `json:"record_kind"`
}

type Proposition struct {
	ObjectType  string                    `json:"object_type"`
	ObjectValue governancecontract.Scalar `json:"object_value"`
	Predicate   string                    `json:"predicate"`
	Subject     string                    `json:"subject"`
}

type Validity struct {
	ValidFromUnixMS  int64  `json:"valid_from_unix_ms"`
	ValidUntilUnixMS *int64 `json:"valid_until_unix_ms"`
}

type Spec struct {
	AtomType                       string      `json:"atom_type"`
	AuthorityRef                   *string     `json:"authority_ref"`
	ContradictingEvidenceRecordIDs []string    `json:"contradicting_evidence_record_ids"`
	DerivedFromClaimRecordIDs      []string    `json:"derived_from_claim_record_ids"`
	EpistemicState                 string      `json:"epistemic_state"`
	Hardness                       string      `json:"hardness"`
	InstructionAllowed             bool        `json:"instruction_allowed"`
	ProjectionConfidenceMicros     *int64      `json:"projection_confidence_micros"`
	ProjectionMode                 string      `json:"projection_mode"`
	Proposition                    Proposition `json:"proposition"`
	SupportingEvidenceRecordIDs    []string    `json:"supporting_evidence_record_ids"`
	Validity                       Validity    `json:"validity"`
}

// CognitiveAtom is a deterministic shadow projection. Its positive validation
// result is structural only and grants no authority.
type CognitiveAtom struct {
	APIVersion string    `json:"api_version"`
	Integrity  Integrity `json:"integrity"`
	Kind       string    `json:"kind"`
	Metadata   Metadata  `json:"metadata"`
	Source     Source    `json:"source"`
	Spec       Spec      `json:"spec"`

	canonicalAtom    []byte
	canonicalPayload []byte
	digest           string
}

// AtomJSON returns exact compact canonical bytes, or nil if the exported value
// was mutated after construction or decoding.
func (a *CognitiveAtom) AtomJSON() []byte {
	state, err := verifiedCanonicalState(a)
	if err != nil {
		return nil
	}
	return append([]byte(nil), state.atom...)
}

// PayloadJSON returns canonical bytes with the self digest blank, or nil if the
// atom no longer validates.
func (a *CognitiveAtom) PayloadJSON() []byte {
	state, err := verifiedCanonicalState(a)
	if err != nil {
		return nil
	}
	return append([]byte(nil), state.payload...)
}

// Digest returns the atom's verified lowercase SHA-256, or an empty string if
// the atom no longer validates.
func (a *CognitiveAtom) Digest() string {
	state, err := verifiedCanonicalState(a)
	if err != nil {
		return ""
	}
	return state.digest
}

// ID returns the atom's verified content-derived stable identifier.
func (a *CognitiveAtom) ID() string {
	if _, err := verifiedCanonicalState(a); err != nil {
		return ""
	}
	return a.Metadata.AtomID
}
