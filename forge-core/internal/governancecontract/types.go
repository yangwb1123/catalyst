// Package governancecontract implements the shadow-only Governance Evidence/Claim v1 wire contract.
package governancecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strconv"
)

const (
	APIVersion       = "forgeos.governance/v1"
	Canonicalization = "forgeos.canonical-json/v1"
	EvidenceKind     = "EvidenceRecord"
	ClaimKind        = "KnowledgeClaim"
)

type Principal struct {
	AuthorityDomain string `json:"authority_domain"`
	PrincipalID     string `json:"principal_id"`
	PrincipalType   string `json:"principal_type"`
	Role            string `json:"role"`
	RunID           string `json:"run_id"`
}

type Metadata struct {
	AggregateID         string    `json:"aggregate_id"`
	ContextSHA256       string    `json:"context_sha256"`
	CreatedAtUnixMS     int64     `json:"created_at_unix_ms"`
	CreatedBy           Principal `json:"created_by"`
	PolicySHA256        string    `json:"policy_sha256"`
	ProjectID           string    `json:"project_id"`
	RecordID            string    `json:"record_id"`
	Scope               string    `json:"scope"`
	Sequence            int64     `json:"sequence"`
	SourceRevision      string    `json:"source_revision"`
	SourceTreeSHA256    string    `json:"source_tree_sha256"`
	SupersedesRecordIDs []string  `json:"supersedes_record_ids"`
}

type Integrity struct {
	CanonicalSHA256  string `json:"canonical_sha256"`
	Canonicalization string `json:"canonicalization"`
}

type Collector struct {
	CollectorID      string `json:"collector_id"`
	CollectorType    string `json:"collector_type"`
	CollectorVersion string `json:"collector_version"`
	ParametersSHA256 string `json:"parameters_sha256"`
	RunID            string `json:"run_id"`
}

type SourceSnapshot struct {
	SnapshotID     string `json:"snapshot_id"`
	SnapshotSHA256 string `json:"snapshot_sha256"`
	SnapshotType   string `json:"snapshot_type"`
}

type Locator struct {
	ContentSHA256 string `json:"content_sha256"`
	ExitCode      *int64 `json:"exit_code"`
	LineEnd       *int64 `json:"line_end"`
	LineStart     *int64 `json:"line_start"`
	LocatorRef    string `json:"locator_ref"`
	LocatorType   string `json:"locator_type"`
}

type EvidenceSpec struct {
	ArtifactSHA256   *string        `json:"artifact_sha256"`
	Collector        Collector      `json:"collector"`
	ContentRole      string         `json:"content_role"`
	Directness       string         `json:"directness"`
	EvidenceType     string         `json:"evidence_type"`
	Locator          Locator        `json:"locator"`
	ObservedAtUnixMS int64          `json:"observed_at_unix_ms"`
	Sensitivity      string         `json:"sensitivity"`
	SourceSnapshot   SourceSnapshot `json:"source_snapshot"`
	SourceTrust      string         `json:"source_trust"`
	Subjects         []string       `json:"subjects"`
}

type Status struct {
	ReasonCodes      []string `json:"reason_codes"`
	State            string   `json:"state"`
	ValidFromUnixMS  int64    `json:"valid_from_unix_ms"`
	ValidUntilUnixMS *int64   `json:"valid_until_unix_ms"`
}

type EvidenceRecord struct {
	APIVersion string       `json:"api_version"`
	Integrity  Integrity    `json:"integrity"`
	Kind       string       `json:"kind"`
	Metadata   Metadata     `json:"metadata"`
	Spec       EvidenceSpec `json:"spec"`
	Status     Status       `json:"status"`
}

type ClaimOwner struct {
	PrincipalID   string `json:"principal_id"`
	PrincipalType string `json:"principal_type"`
}

type ValidationPlan struct {
	DueAtUnixMS           int64    `json:"due_at_unix_ms"`
	ImpactIfFalse         string   `json:"impact_if_false"`
	Method                string   `json:"method"`
	OwnerID               string   `json:"owner_id"`
	RequiredEvidenceTypes []string `json:"required_evidence_types"`
}

type DecisionAuthority struct {
	ADRRef      string `json:"adr_ref"`
	ApprovalRef string `json:"approval_ref"`
}

type Scalar struct {
	Kind    string
	String  string
	Integer int64
	Boolean bool
}

func (s *Scalar) UnmarshalJSON(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		return err
	}
	switch typed := value.(type) {
	case nil:
		s.Kind = "null"
	case string:
		s.Kind, s.String = "string", typed
	case bool:
		s.Kind, s.Boolean = "boolean", typed
	case json.Number:
		integer, err := parseInt64Number(typed.String())
		if err != nil {
			return err
		}
		s.Kind, s.Integer = "integer", integer
	default:
		return fmt.Errorf("object_value must be a scalar")
	}
	return nil
}

func (s Scalar) MarshalJSON() ([]byte, error) {
	switch s.Kind {
	case "null":
		return []byte("null"), nil
	case "string":
		if err := validateString(s.String); err != nil {
			return nil, err
		}
		buffer := bytes.NewBuffer(nil)
		appendJSONString(buffer, s.String)
		return buffer.Bytes(), nil
	case "boolean":
		return []byte(strconv.FormatBool(s.Boolean)), nil
	case "integer":
		return []byte(strconv.FormatInt(s.Integer, 10)), nil
	default:
		return nil, fmt.Errorf("unsupported scalar kind %q", s.Kind)
	}
}

type ClaimSpec struct {
	ClaimType                      string             `json:"claim_type"`
	ConfidenceMicros               *int64             `json:"confidence_micros"`
	ContradictingEvidenceRecordIDs []string           `json:"contradicting_evidence_record_ids"`
	DecisionAuthority              *DecisionAuthority `json:"decision_authority"`
	DerivedFromClaimRecordIDs      []string           `json:"derived_from_claim_record_ids"`
	ObjectType                     string             `json:"object_type"`
	ObjectValue                    Scalar             `json:"object_value"`
	Owner                          ClaimOwner         `json:"owner"`
	Predicate                      string             `json:"predicate"`
	QueueRef                       *string            `json:"queue_ref"`
	Reasoning                      string             `json:"reasoning"`
	ReviewByUnixMS                 *int64             `json:"review_by_unix_ms"`
	Subject                        string             `json:"subject"`
	SupportingEvidenceRecordIDs    []string           `json:"supporting_evidence_record_ids"`
	ValidationPlan                 *ValidationPlan    `json:"validation_plan"`
}

type KnowledgeClaim struct {
	APIVersion string    `json:"api_version"`
	Integrity  Integrity `json:"integrity"`
	Kind       string    `json:"kind"`
	Metadata   Metadata  `json:"metadata"`
	Spec       ClaimSpec `json:"spec"`
	Status     Status    `json:"status"`
}

// Record is a decoded union. Its positive validation result is structural only.
type Record struct {
	Evidence         *EvidenceRecord
	Claim            *KnowledgeClaim
	node             any
	canonicalRecord  []byte
	canonicalPayload []byte
	digest           string
}

func (r *Record) Kind() string {
	if r.Evidence != nil {
		return EvidenceKind
	}
	if r.Claim != nil {
		return ClaimKind
	}
	return ""
}

func (r *Record) Header() *Metadata {
	if r.Evidence != nil {
		return &r.Evidence.Metadata
	}
	if r.Claim != nil {
		return &r.Claim.Metadata
	}
	return nil
}

func (r *Record) RecordJSON() []byte {
	state, err := verifiedCanonicalState(r)
	if err != nil {
		return nil
	}
	return append([]byte(nil), state.record...)
}

func (r *Record) PayloadJSON() []byte {
	state, err := verifiedCanonicalState(r)
	if err != nil {
		return nil
	}
	return append([]byte(nil), state.payload...)
}

func (r *Record) Digest() string {
	state, err := verifiedCanonicalState(r)
	if err != nil {
		return ""
	}
	return state.digest
}
