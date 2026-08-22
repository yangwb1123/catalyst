// Package adrv2 validates proposed-only Architecture Decision Record v2 files.
// A successful validation is structural and digest-bound; it conveys no approval.
package adrv2

const (
	APIVersion       = "forgeos.architecture-decision-record/v2"
	Kind             = "ArchitectureDecisionRecord"
	Canonicalization = "forgeos.canonical-json/v1"
	Status           = "proposed"
	MaxDocumentBytes = 262144
	MaxFrontmatter   = 65536
	MaxBodyBytes     = 196608
	bodyDomain       = "forgeos.architecture-decision-record-body.v2"
	selfDomain       = "forgeos.architecture-decision-record.v2"
)

type Alternative struct {
	AlternativeID string `json:"alternative_id"`
	Description   string `json:"description"`
	Disposition   string `json:"disposition"`
	Rationale     string `json:"rationale"`
}

type Risk struct {
	Description string `json:"description"`
	Mitigation  string `json:"mitigation"`
	RiskID      string `json:"risk_id"`
}

type ValidationItem struct {
	Description      string   `json:"description"`
	DueTrigger       string   `json:"due_trigger"`
	EvidenceRequired []string `json:"evidence_required"`
	OwnerRef         string   `json:"owner_ref"`
	SuccessCriteria  string   `json:"success_criteria"`
	ValidationID     string   `json:"validation_id"`
}

type RevisitTrigger struct {
	Condition        string   `json:"condition"`
	EvidenceRequired []string `json:"evidence_required"`
	TriggerID        string   `json:"trigger_id"`
}

// Frontmatter is the exact proposed-only v2 metadata shape.
type Frontmatter struct {
	AcceptedAtUnixMS       *int64           `json:"accepted_at_unix_ms"`
	AcceptanceID           *string          `json:"acceptance_id"`
	ADRID                  string           `json:"adr_id"`
	AffectedNodeIDs        []string         `json:"affected_node_ids"`
	Alternatives           []Alternative    `json:"alternatives"`
	APIVersion             string           `json:"api_version"`
	ApproverRefs           []string         `json:"approver_refs"`
	AssumptionClaimIDs     []string         `json:"assumption_claim_ids"`
	BodySHA256             string           `json:"body_sha256"`
	Canonicalization       string           `json:"canonicalization"`
	Compatibility          string           `json:"compatibility"`
	Consequences           []string         `json:"consequences"`
	ContextClaimIDs        []string         `json:"context_claim_ids"`
	Decision               string           `json:"decision"`
	DecisionDriverClaimIDs []string         `json:"decision_driver_claim_ids"`
	DocumentName           string           `json:"document_name"`
	EvidenceRecordIDs      []string         `json:"evidence_record_ids"`
	ExpiresAtUnixMS        *int64           `json:"expires_at_unix_ms"`
	ImplementationRefs     []string         `json:"implementation_refs"`
	Kind                   string           `json:"kind"`
	OwnerRefs              []string         `json:"owner_refs"`
	ProposedAtUnixMS       int64            `json:"proposed_at_unix_ms"`
	RevisitTriggers        []RevisitTrigger `json:"revisit_triggers"`
	Risks                  []Risk           `json:"risks"`
	Rollback               string           `json:"rollback"`
	Rollout                string           `json:"rollout"`
	ScopeRefs              []string         `json:"scope_refs"`
	SelfSHA256             string           `json:"self_sha256"`
	Status                 string           `json:"status"`
	SupersededBy           []string         `json:"superseded_by"`
	Supersedes             []string         `json:"supersedes"`
	Title                  string           `json:"title"`
	ValidationPlan         []ValidationItem `json:"validation_plan"`
}

// Document is a detached result. Frontmatter and Body are copies.
type Document struct {
	Frontmatter Frontmatter
	Body        []byte
}
