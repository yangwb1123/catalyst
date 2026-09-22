package deviceplacement

import (
	"forgeos/forge-core/internal/auditprojection"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// RunExecutionEvidenceSchemaVersion identifies a pure binding of an existing
// Run observation to an existing session-bound terminal receipt observation.
const RunExecutionEvidenceSchemaVersion = "forge.run.execution-evidence.v1"

const RunExecutionEvidenceEvaluationMode = "pure_run_execution_evidence_binding"

// RunExecutionEvidenceAuthority enumerates capabilities deliberately absent
// from this value. A valid evidence binding never proves any of these facts.
type RunExecutionEvidenceAuthority struct {
	IdentityVerified    bool `json:"identity_verified"`
	OwnerAuthorized     bool `json:"owner_authorized"`
	RunAuthoritative    bool `json:"run_authoritative"`
	ReceiptPersisted    bool `json:"receipt_persisted"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// RunExecutionEvidenceInput joins two already validated, caller-supplied
// observations. Neither input is loaded from a service by this package.
type RunExecutionEvidenceInput struct {
	Run     auditprojection.RunObserved     `json:"run_observed"`
	Receipt SessionRunnerReceiptObservation `json:"session_receipt_observed"`
}

// RunExecutionEvidence is bounded, content-free metadata for one observed Run
// and one terminal receipt. It is suitable for display or a later reviewed
// adapter, but is not a durable receipt or execution result.
type RunExecutionEvidence struct {
	SchemaVersion          string                        `json:"api_version"`
	EvaluationMode         string                        `json:"evaluation_mode"`
	OwnerRef               string                        `json:"owner_ref"`
	ConversationID         string                        `json:"conversation_id"`
	RunID                  string                        `json:"run_id"`
	PromptID               string                        `json:"prompt_id"`
	RunStatus              string                        `json:"run_status"`
	AttemptID              string                        `json:"attempt_id"`
	TargetID               string                        `json:"target_id"`
	CommandID              string                        `json:"command_id"`
	CommandSHA256          string                        `json:"command_sha256"`
	DispositionKind        string                        `json:"disposition_kind"`
	ReceiptObservedAtMS    uint64                        `json:"receipt_observed_at_ms"`
	Uncertain              bool                          `json:"uncertain"`
	ReconciliationRequired bool                          `json:"reconciliation_required"`
	MetadataObserved       bool                          `json:"metadata_observed"`
	ContentIncluded        bool                          `json:"content_included"`
	Authority              RunExecutionEvidenceAuthority `json:"authority"`
}

// RunExecutionEvidenceError is a stable value-level rejection class.
type RunExecutionEvidenceError string

const (
	ErrInvalidRunExecutionEvidence RunExecutionEvidenceError = "invalid_run_execution_evidence"
	ErrRunExecutionOwnerMismatch   RunExecutionEvidenceError = "run_execution_owner_mismatch"
	ErrRunExecutionBindingMismatch RunExecutionEvidenceError = "run_execution_binding_mismatch"
)

func (e RunExecutionEvidenceError) Error() string { return string(e) }

// ObserveRunExecutionEvidence binds an owner-scoped Run metadata value to a
// session-bound terminal receipt observation. It does not read a clock,
// persist a receipt, issue a lease, select a target, or authorize execution.
func ObserveRunExecutionEvidence(input RunExecutionEvidenceInput) (RunExecutionEvidence, error) {
	if !validRunObservedForEvidence(input.Run) || input.Receipt.Validate() != nil {
		return RunExecutionEvidence{}, ErrInvalidRunExecutionEvidence
	}
	if input.Run.OwnerRef != observedOwnerReferenceForReceipt(input.Receipt) {
		return RunExecutionEvidence{}, ErrRunExecutionOwnerMismatch
	}
	receipt := input.Receipt
	if input.Run.ConversationID != receipt.ConversationID || input.Run.RunID != receipt.RunID || input.Run.PromptID != receipt.PromptID {
		return RunExecutionEvidence{}, ErrRunExecutionBindingMismatch
	}
	nested := receipt.ReceiptObservation
	return RunExecutionEvidence{
		SchemaVersion:          RunExecutionEvidenceSchemaVersion,
		EvaluationMode:         RunExecutionEvidenceEvaluationMode,
		OwnerRef:               input.Run.OwnerRef,
		ConversationID:         input.Run.ConversationID,
		RunID:                  input.Run.RunID,
		PromptID:               input.Run.PromptID,
		RunStatus:              input.Run.Status,
		AttemptID:              nested.AttemptID,
		TargetID:               nested.TargetID,
		CommandID:              nested.CommandID,
		CommandSHA256:          nested.CommandSHA256,
		DispositionKind:        nested.DispositionKind,
		ReceiptObservedAtMS:    nested.ObservedAtMS,
		Uncertain:              nested.Uncertain,
		ReconciliationRequired: nested.ReconciliationRequired,
		MetadataObserved:       true,
		ContentIncluded:        false,
		Authority:              RunExecutionEvidenceAuthority{},
	}, nil
}

func observedOwnerReferenceForReceipt(receipt SessionRunnerReceiptObservation) string {
	return auditprojection.ObservedOwnerReference(model.Owner{
		Issuer: receipt.Owner.Issuer, Subject: receipt.Owner.Subject, TenantID: receipt.Owner.TenantID,
	})
}

func validRunObservedForEvidence(value auditprojection.RunObserved) bool {
	if value.APIVersion != auditprojection.ForgeRunObservedV1 || value.OwnerRef == "" ||
		len(value.OwnerRef) != 64 || !lowerHex(value.OwnerRef) ||
		!validEvidenceIdentifier(value.ConversationID) || !validEvidenceIdentifier(value.RunID) ||
		!validEvidenceIdentifier(value.PromptID) || value.CreatedAtMS > uint64(MaxSafeIntegerMS) ||
		value.LatestSequence == 0 || value.LatestSequence > uint64(MaxSafeIntegerMS) ||
		!value.MetadataObserved || value.ContentIncluded || value.Authority != (auditprojection.RunObservedAuthority{}) {
		return false
	}
	switch value.Status {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}

func validEvidenceIdentifier(value string) bool {
	if value == "" || len(value) > 85 {
		return false
	}
	for _, character := range value {
		if character <= 0x20 || character == ':' || character == '/' || character == '\\' {
			return false
		}
	}
	return true
}

func lowerHex(value string) bool {
	for _, character := range value {
		if !(character >= '0' && character <= '9') && !(character >= 'a' && character <= 'f') {
			return false
		}
	}
	return true
}
