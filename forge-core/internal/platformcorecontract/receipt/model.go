package receipt

import (
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/state"
)

type VerificationStatus string
type VerificationApplicability string

type ExecutorDescriptor struct {
	ActorRef       core.ActorRef `json:"actor_ref"`
	AdapterID      string        `json:"adapter_id"`
	AdapterVersion string        `json:"adapter_version"`
}

type ObservedUsage struct {
	CostUSDMicros int64 `json:"cost_usd_micros"`
	ElapsedMS     int64 `json:"elapsed_ms"`
	InputTokens   int64 `json:"input_tokens"`
	ModelCalls    int64 `json:"model_calls"`
	NetworkBytes  int64 `json:"network_bytes"`
	OutputBytes   int64 `json:"output_bytes"`
	OutputTokens  int64 `json:"output_tokens"`
	ToolCalls     int64 `json:"tool_calls"`
}

type EventRange struct {
	AggregateRef  core.EntityRef `json:"aggregate_ref"`
	FirstEventID  string         `json:"first_event_id"`
	FirstSequence int64          `json:"first_sequence"`
	LastEventID   string         `json:"last_event_id"`
	LastSequence  int64          `json:"last_sequence"`
}

type ExecutionReceipt struct {
	ApprovalRef             *core.RecordRef    `json:"approval_ref"`
	AttemptRef              core.EntityRef     `json:"attempt_ref"`
	Canonicalization        string             `json:"canonicalization"`
	EndedAtUnixMS           int64              `json:"ended_at_unix_ms"`
	EventRange              *EventRange        `json:"event_range"`
	ExecutionReceiptVersion int64              `json:"execution_receipt_version"`
	Executor                ExecutorDescriptor `json:"executor"`
	GrantRef                *core.RecordRef    `json:"grant_ref"`
	InputArtifactRefs       []core.ArtifactRef `json:"input_artifact_refs"`
	ObservedUsage           ObservedUsage      `json:"observed_usage"`
	OutputArtifactRefs      []core.ArtifactRef `json:"output_artifact_refs"`
	ReasonCodes             []string           `json:"reason_codes"`
	ReceiptID               string             `json:"receipt_id"`
	ScopeRef                core.ScopeRef      `json:"scope_ref"`
	SessionRef              core.EntityRef     `json:"session_ref"`
	SourceSnapshotRef       core.EntityRef     `json:"source_snapshot_ref"`
	StartedAtUnixMS         int64              `json:"started_at_unix_ms"`
	TerminalState           state.AttemptState `json:"terminal_state"`
}

type VerificationCheckRequest struct {
	CheckID          string `json:"check_id"`
	CheckName        string `json:"check_name"`
	DeclaredRequired bool   `json:"declared_required"`
}

type VerificationRequest struct {
	Canonicalization           string                     `json:"canonicalization"`
	Checks                     []VerificationCheckRequest `json:"checks"`
	InputArtifactRef           core.ArtifactRef           `json:"input_artifact_ref"`
	RequestedAtUnixMS          int64                      `json:"requested_at_unix_ms"`
	RequestedBy                core.ActorRef              `json:"requested_by"`
	ScopeRef                   core.ScopeRef              `json:"scope_ref"`
	VerificationID             string                     `json:"verification_id"`
	VerificationRequestVersion int64                      `json:"verification_request_version"`
}

type VerificationCheckResult struct {
	Applicability       VerificationApplicability `json:"applicability"`
	ApplicabilityReason *string                   `json:"applicability_reason"`
	CheckID             string                    `json:"check_id"`
	EvidenceRefs        []core.RecordRef          `json:"evidence_refs"`
	ReasonCodes         []string                  `json:"reason_codes"`
	Status              VerificationStatus        `json:"status"`
}

type VerificationReceipt struct {
	Canonicalization           string                    `json:"canonicalization"`
	EndedAtUnixMS              int64                     `json:"ended_at_unix_ms"`
	InputArtifactRef           core.ArtifactRef          `json:"input_artifact_ref"`
	OverallStatus              VerificationStatus        `json:"overall_status"`
	ProducedBy                 core.ActorRef             `json:"produced_by"`
	ReceiptID                  string                    `json:"receipt_id"`
	RequestSHA256              string                    `json:"request_sha256"`
	Results                    []VerificationCheckResult `json:"results"`
	ScopeRef                   core.ScopeRef             `json:"scope_ref"`
	StartedAtUnixMS            int64                     `json:"started_at_unix_ms"`
	VerificationID             string                    `json:"verification_id"`
	VerificationReceiptVersion int64                     `json:"verification_receipt_version"`
}
