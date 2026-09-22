// Package executionattempt contains authority-neutral Attempt declarations.
//
// Request validation only checks supplied values and their declared relations;
// it does not resolve references, persist an Attempt, reserve a device, or
// authorize execution.
package executionattempt

import (
	"fmt"
	"sort"
	"strings"
	"unicode/utf8"

	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/receipt"
)

const (
	MaxApprovalRefs               = 16
	MaxRequestedEffects           = 32
	MaxRequestedEffectBytes       = 64
	MaxControlAggregateVersion    = int64(1_000_000_000)
	MaxAttemptDurationMS          = int64(31_536_000_000)
	MaxAttemptTimeoutMS           = MaxAttemptDurationMS
	MaxAttemptCostUSDMicros       = int64(1_000_000_000_000_000)
	MaxAttemptModelCalls          = int64(1_000_000_000)
	MaxAttemptToolCalls           = MaxAttemptModelCalls
	MaxAttemptInputTokens         = MaxAttemptCostUSDMicros
	MaxAttemptOutputTokens        = MaxAttemptCostUSDMicros
	MaxAttemptOutputBytes         = MaxAttemptCostUSDMicros
	MaxAttemptNetworkBytes        = MaxAttemptCostUSDMicros
	WorkspaceCapabilityRecordType = "forge.runtime.workspace_capability"
	CapabilityGrantRecordType     = "forge.control.capability_grant"
	ApprovalRecordType            = "forge.control.approval_record"
)

type ErrorCode string

const (
	InvalidValue      ErrorCode = "invalid_value"
	ReferenceMismatch ErrorCode = "reference_mismatch"
)

type Error struct {
	code    ErrorCode
	message string
}

func (e *Error) Error() string       { return string(e.code) + ": " + e.message }
func (e *Error) Code() ErrorCode     { return e.code }
func invalid(message string) error   { return &Error{code: InvalidValue, message: message} }
func reference(message string) error { return &Error{code: ReferenceMismatch, message: message} }

func classify(err error) error {
	if err == nil {
		return nil
	}
	code, ok := core.RejectionCodeOf(err)
	if ok && (code == "pc_reference_mismatch" || code == "pc_relation_mismatch") {
		return reference(err.Error())
	}
	return invalid(err.Error())
}

type AttemptBudget struct {
	MaxDurationMS    int64 `json:"max_duration_ms"`
	MaxCostUSDMicros int64 `json:"max_cost_usd_micros"`
	MaxModelCalls    int64 `json:"max_model_calls"`
	MaxToolCalls     int64 `json:"max_tool_calls"`
	MaxInputTokens   int64 `json:"max_input_tokens"`
	MaxOutputTokens  int64 `json:"max_output_tokens"`
	MaxOutputBytes   int64 `json:"max_output_bytes"`
	MaxNetworkBytes  int64 `json:"max_network_bytes"`
}

type ControlVersionBinding struct {
	ObjectiveVersion int64 `json:"objective_version"`
	ChangeVersion    int64 `json:"change_version"`
	WorkGraphVersion int64 `json:"work_graph_version"`
	WorkItemVersion  int64 `json:"work_item_version"`
}

type AttemptRequestInput struct {
	ScopeRef               core.ScopeRef              `json:"scope_ref"`
	AttemptRef             core.EntityRef             `json:"attempt_ref"`
	WorkItemRef            core.EntityRef             `json:"work_item_ref"`
	ProjectRef             core.EntityRef             `json:"project_ref"`
	ProjectSnapshotRef     core.EntityRef             `json:"project_snapshot_ref"`
	ControlVersions        ControlVersionBinding      `json:"control_versions"`
	Executor               receipt.ExecutorDescriptor `json:"executor"`
	ContextArtifactRef     *core.ArtifactRef          `json:"context_artifact_ref"`
	WorkspaceCapabilityRef *core.RecordRef            `json:"workspace_capability_ref"`
	GrantRef               *core.RecordRef            `json:"grant_ref"`
	ApprovalRefs           []core.RecordRef           `json:"approval_refs"`
	RequestedEffects       []string                   `json:"requested_effects"`
	Budget                 AttemptBudget              `json:"budget"`
	TimeoutMS              int64                      `json:"timeout_ms"`
	IdempotencyKey         string                     `json:"idempotency_key"`
}

// AttemptRequest is a validated, defensive copy of one caller declaration.
// Its fields remain private so callers cannot mutate the value after
// construction or mistake it for durable Attempt state.
type AttemptRequest struct{ input AttemptRequestInput }

func NewRequest(value AttemptRequestInput) (AttemptRequest, error) {
	if err := validate(value); err != nil {
		return AttemptRequest{}, err
	}
	value = cloneInput(value)
	sort.Slice(value.ApprovalRefs, func(i, j int) bool {
		left, right := value.ApprovalRefs[i], value.ApprovalRefs[j]
		if left.RecordID != right.RecordID {
			return left.RecordID < right.RecordID
		}
		if left.RecordSHA256 != right.RecordSHA256 {
			return left.RecordSHA256 < right.RecordSHA256
		}
		return left.RecordType < right.RecordType
	})
	value.RequestedEffects = append([]string(nil), value.RequestedEffects...)
	sort.Strings(value.RequestedEffects)
	return AttemptRequest{input: value}, nil
}

func (value AttemptRequest) InitialState() string { return "requested" }

func (value AttemptRequest) ScopeRef() core.ScopeRef { return cloneScope(value.input.ScopeRef) }

func (value AttemptRequest) AttemptRef() core.EntityRef { return value.input.AttemptRef }

func (value AttemptRequest) WorkItemRef() core.EntityRef { return value.input.WorkItemRef }

func (value AttemptRequest) ProjectRef() core.EntityRef { return value.input.ProjectRef }

func (value AttemptRequest) ProjectSnapshotRef() core.EntityRef {
	return value.input.ProjectSnapshotRef
}

func (value AttemptRequest) ControlVersions() ControlVersionBinding {
	return value.input.ControlVersions
}

func (value AttemptRequest) Executor() receipt.ExecutorDescriptor { return value.input.Executor }

func (value AttemptRequest) ContextArtifactRef() *core.ArtifactRef {
	if value.input.ContextArtifactRef == nil {
		return nil
	}
	copy := *value.input.ContextArtifactRef
	return &copy
}

func (value AttemptRequest) WorkspaceCapabilityRef() *core.RecordRef {
	return cloneRecord(value.input.WorkspaceCapabilityRef)
}

func (value AttemptRequest) GrantRef() *core.RecordRef { return cloneRecord(value.input.GrantRef) }

func (value AttemptRequest) ApprovalRefs() []core.RecordRef {
	return append([]core.RecordRef(nil), value.input.ApprovalRefs...)
}

func (value AttemptRequest) RequestedEffects() []string {
	return append([]string(nil), value.input.RequestedEffects...)
}

func (value AttemptRequest) Budget() AttemptBudget { return value.input.Budget }

func (value AttemptRequest) TimeoutMS() int64 { return value.input.TimeoutMS }

func (value AttemptRequest) IdempotencyKey() string { return value.input.IdempotencyKey }

func cloneInput(value AttemptRequestInput) AttemptRequestInput {
	value.ScopeRef = cloneScope(value.ScopeRef)
	if value.ContextArtifactRef != nil {
		copy := *value.ContextArtifactRef
		value.ContextArtifactRef = &copy
	}
	value.WorkspaceCapabilityRef = cloneRecord(value.WorkspaceCapabilityRef)
	value.GrantRef = cloneRecord(value.GrantRef)
	value.ApprovalRefs = append([]core.RecordRef(nil), value.ApprovalRefs...)
	value.RequestedEffects = append([]string(nil), value.RequestedEffects...)
	return value
}

func cloneScope(value core.ScopeRef) core.ScopeRef {
	value.ActionID = cloneString(value.ActionID)
	value.AttemptID = cloneString(value.AttemptID)
	value.ChangeID = cloneString(value.ChangeID)
	value.ObjectiveID = cloneString(value.ObjectiveID)
	value.ProjectID = cloneString(value.ProjectID)
	value.ProjectSnapshotID = cloneString(value.ProjectSnapshotID)
	value.SessionID = cloneString(value.SessionID)
	value.TurnID = cloneString(value.TurnID)
	value.WorkGraphID = cloneString(value.WorkGraphID)
	value.WorkItemID = cloneString(value.WorkItemID)
	return value
}

func cloneRecord(value *core.RecordRef) *core.RecordRef {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func validate(value AttemptRequestInput) error {
	if err := classify(core.ValidateReference(value.ScopeRef)); err != nil {
		return err
	}
	scope := value.ScopeRef
	if scope.ProjectID == nil || scope.ProjectSnapshotID == nil || scope.ObjectiveID == nil ||
		scope.ChangeID == nil || scope.WorkGraphID == nil || scope.WorkItemID == nil || scope.AttemptID == nil ||
		scope.SessionID != nil || scope.TurnID != nil || scope.ActionID != nil {
		return reference("scope_ref must be an exact full Attempt scope")
	}
	if err := exactRef(value.AttemptRef, "attempt", scope.AttemptID, "attempt_ref"); err != nil {
		return err
	}
	if err := exactRef(value.WorkItemRef, "work_item", scope.WorkItemID, "work_item_ref"); err != nil {
		return err
	}
	if err := exactRef(value.ProjectRef, "project", scope.ProjectID, "project_ref"); err != nil {
		return err
	}
	if err := exactRef(value.ProjectSnapshotRef, "project_snapshot", scope.ProjectSnapshotID, "project_snapshot_ref"); err != nil {
		return err
	}
	if err := classify(receipt.ValidateExecutorDescriptor(value.Executor)); err != nil {
		return err
	}
	if value.ContextArtifactRef != nil {
		if err := classify(core.ValidateArtifactRef(value.ContextArtifactRef)); err != nil {
			return err
		}
		if value.ContextArtifactRef.SourceSnapshotRef != value.ProjectSnapshotRef {
			return reference("context_artifact_ref must bind the exact project_snapshot_ref")
		}
		if value.ContextArtifactRef.ProducerAttemptID == value.AttemptRef.EntityID {
			return reference("context_artifact_ref cannot be produced by the requested Attempt")
		}
	}
	if err := optionalRecord(value.WorkspaceCapabilityRef, WorkspaceCapabilityRecordType, "workspace_capability_ref"); err != nil {
		return err
	}
	if err := optionalRecord(value.GrantRef, CapabilityGrantRecordType, "grant_ref"); err != nil {
		return err
	}
	if len(value.RequestedEffects) > MaxRequestedEffects {
		return invalid("requested_effects exceed the bounded entry limit")
	}
	if len(value.RequestedEffects) > 0 && value.GrantRef == nil {
		return reference("requested_effects require a declared grant_ref")
	}
	seenIDs := make(map[string]struct{}, len(value.ApprovalRefs))
	if len(value.ApprovalRefs) > MaxApprovalRefs {
		return invalid("approval_refs exceed the bounded entry limit")
	}
	for _, ref := range value.ApprovalRefs {
		if err := optionalRecord(&ref, ApprovalRecordType, "approval_ref"); err != nil {
			return err
		}
		if _, exists := seenIDs[ref.RecordID]; exists {
			return invalid("approval_refs contain a duplicate record_id")
		}
		seenIDs[ref.RecordID] = struct{}{}
	}
	seenEffects := make(map[string]struct{}, len(value.RequestedEffects))
	for _, effect := range value.RequestedEffects {
		if !validEffect(effect) {
			return invalid("requested_effect is not a bounded lowercase token")
		}
		if _, exists := seenEffects[effect]; exists {
			return invalid("requested_effects must be unique")
		}
		seenEffects[effect] = struct{}{}
	}
	if err := validateBudget(value.Budget, value.TimeoutMS); err != nil {
		return err
	}
	if len(value.IdempotencyKey) < 16 || len(value.IdempotencyKey) > 128 || !visibleASCII(value.IdempotencyKey) {
		return invalid("idempotency_key must contain 16..128 visible ASCII bytes")
	}
	for label, version := range map[string]int64{"objective_version": value.ControlVersions.ObjectiveVersion, "change_version": value.ControlVersions.ChangeVersion, "work_graph_version": value.ControlVersions.WorkGraphVersion, "work_item_version": value.ControlVersions.WorkItemVersion} {
		if version < 1 || version > MaxControlAggregateVersion {
			return invalid(fmt.Sprintf("control_versions.%s is invalid", label))
		}
	}
	return nil
}

func exactRef(value core.EntityRef, typ string, expected *string, label string) error {
	if string(value.EntityType) != typ {
		return reference(label + " must exactly match its scope_ref identity")
	}
	if err := classify(core.ValidateReference(value)); err != nil {
		return err
	}
	if expected == nil || value.EntityID != *expected {
		return reference(label + " must exactly match its scope_ref identity")
	}
	return nil
}

func optionalRecord(value *core.RecordRef, expected, label string) error {
	if value == nil {
		return nil
	}
	if err := classify(core.ValidateReference(*value)); err != nil {
		return err
	}
	if value.RecordType != expected {
		return reference(label + ".record_type must equal " + expected)
	}
	return nil
}

func validEffect(value string) bool {
	if value == "" || len(value) > MaxRequestedEffectBytes || !utf8.ValidString(value) {
		return false
	}
	for index, character := range []byte(value) {
		if (index == 0 && (character < 'a' || character > 'z')) ||
			(index == len(value)-1 && !((character >= 'a' && character <= 'z') || (character >= '0' && character <= '9'))) ||
			!((character >= 'a' && character <= 'z') || (character >= '0' && character <= '9') || strings.ContainsRune("._-", rune(character))) {
			return false
		}
	}
	return true
}

func visibleASCII(value string) bool {
	for _, character := range []byte(value) {
		if character < '!' || character > '~' {
			return false
		}
	}
	return true
}

func validateBudget(value AttemptBudget, timeout int64) error {
	if value.MaxDurationMS < 1 || value.MaxDurationMS > MaxAttemptDurationMS {
		return invalid("budget.max_duration_ms is outside its bounded range")
	}
	checks := map[string]struct{ number, maximum int64 }{
		"max_cost_usd_micros": {value.MaxCostUSDMicros, MaxAttemptCostUSDMicros},
		"max_model_calls":     {value.MaxModelCalls, MaxAttemptModelCalls},
		"max_tool_calls":      {value.MaxToolCalls, MaxAttemptToolCalls},
		"max_input_tokens":    {value.MaxInputTokens, MaxAttemptInputTokens},
		"max_output_tokens":   {value.MaxOutputTokens, MaxAttemptOutputTokens},
		"max_output_bytes":    {value.MaxOutputBytes, MaxAttemptOutputBytes},
		"max_network_bytes":   {value.MaxNetworkBytes, MaxAttemptNetworkBytes},
	}
	for label, check := range checks {
		if check.number < 0 || check.number > check.maximum {
			return invalid("budget." + label + " is outside its bounded range")
		}
	}
	if timeout < 1 || timeout > MaxAttemptTimeoutMS || timeout > value.MaxDurationMS {
		return invalid("timeout_ms must be positive and at most budget.max_duration_ms")
	}
	return nil
}
