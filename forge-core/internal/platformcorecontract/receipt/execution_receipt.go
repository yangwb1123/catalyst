package receipt

import (
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/state"
)

const (
	approvalRecordType = "forge.control.approval_record"
	grantRecordType    = "forge.control.capability_grant"
)

func validateExecutionReceipt(value *ExecutionReceipt) error {
	_, err := CanonicalExecutionReceiptJSON(value)
	return err
}

// CanonicalExecutionReceiptJSON returns exact compact canonical v1 bytes.
func CanonicalExecutionReceiptJSON(value *ExecutionReceipt) ([]byte, error) {
	if err := validateExecutionReceiptFields(value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	canonical, err := typedCanonical(value, maxReceiptBytes)
	return canonical, withRejection(err, rejectionValueInvalid)
}

// DecodeCanonicalExecutionReceipt accepts one exact canonical v1 receipt.
func DecodeCanonicalExecutionReceipt(data []byte) (*ExecutionReceipt, error) {
	var value ExecutionReceipt
	if err := decodeTypedCanonical(data, maxReceiptBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, "ExecutionReceipt"); err != nil {
		return nil, err
	}
	if err := validateExecutionReceiptFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// ExecutionReceiptSHA256 returns a domain-separated conformance digest.
func ExecutionReceiptSHA256(value *ExecutionReceipt) (string, error) {
	canonical, err := CanonicalExecutionReceiptJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(executionReceiptDigestDomain, canonical), nil
}

func validateExecutionReceiptFields(value *ExecutionReceipt) error {
	if err := validateExecutionShape(value); err != nil {
		return err
	}
	if err := validateExecutionValues(value); err != nil {
		return err
	}
	if err := validateExecutionReferences(value); err != nil {
		return err
	}
	if err := validateExecutionState(value); err != nil {
		return err
	}
	return validateExecutionRelations(value)
}

func validateExecutionShape(value *ExecutionReceipt) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "ExecutionReceipt is required")
	}
	if value.InputArtifactRefs == nil || value.OutputArtifactRefs == nil || value.ReasonCodes == nil {
		return reject(rejectionDocumentInvalid, "ExecutionReceipt required arrays must not be null")
	}
	return nil
}

func validateExecutionValues(value *ExecutionReceipt) error {
	if value.Canonicalization != core.CanonicalizationV1 ||
		value.ExecutionReceiptVersion != ReceiptVersionV1 {
		return reject(rejectionValueInvalid, "execution receipt version or canonicalization is unsupported")
	}
	if err := validateTypedID(value.ReceiptID, "rcp", "receipt_id"); err != nil {
		return err
	}
	if err := validateExecutionTimeValues(value); err != nil {
		return err
	}
	if err := validateExecutor(value.Executor); err != nil {
		return err
	}
	return validateExecutionDeclarationValues(value)
}

func validateExecutionReferences(value *ExecutionReceipt) error {
	if err := validateExecutionScope(value); err != nil {
		return err
	}
	if err := validateExecutionRecordRoles(value); err != nil {
		return err
	}
	for _, artifacts := range [][]core.ArtifactRef{value.InputArtifactRefs, value.OutputArtifactRefs} {
		for index := range artifacts {
			artifact := &artifacts[index]
			if err := validateArtifactReferences(artifact); err != nil {
				return err
			}
			if artifact.SourceSnapshotRef.EntityID != value.SourceSnapshotRef.EntityID {
				return reject(rejectionReferenceMismatch, "execution artifact source snapshot must match receipt")
			}
		}
	}
	if value.EventRange != nil && value.EventRange.AggregateRef != value.AttemptRef {
		return reject(rejectionReferenceMismatch, "event_range aggregate must equal attempt_ref")
	}
	return nil
}

func validateExecutionRecordRoles(value *ExecutionReceipt) error {
	checks := []struct {
		value    *core.RecordRef
		expected string
		label    string
	}{
		{value.ApprovalRef, approvalRecordType, "approval_ref"},
		{value.GrantRef, grantRecordType, "grant_ref"},
	}
	for _, check := range checks {
		if check.value != nil && check.value.RecordType != check.expected {
			return rejectf(rejectionReferenceMismatch, "%s must declare record_type %s", check.label, check.expected)
		}
	}
	return nil
}

func validateExecutionRelations(value *ExecutionReceipt) error {
	if err := validateExecutionIntervalRelation(value); err != nil {
		return err
	}
	elapsed := value.EndedAtUnixMS - value.StartedAtUnixMS
	if value.ObservedUsage.ElapsedMS != elapsed {
		return reject(rejectionRelationMismatch, "observed_usage.elapsed_ms must equal execution wall interval")
	}
	for index := range value.InputArtifactRefs {
		if err := validateArtifactRelations(&value.InputArtifactRefs[index]); err != nil {
			return err
		}
		if err := validateExecutionArtifactRelation(&value.InputArtifactRefs[index], value, false); err != nil {
			return err
		}
	}
	for index := range value.OutputArtifactRefs {
		if err := validateArtifactRelations(&value.OutputArtifactRefs[index]); err != nil {
			return err
		}
		if err := validateExecutionArtifactRelation(&value.OutputArtifactRefs[index], value, true); err != nil {
			return err
		}
	}
	if err := validateEventRangeRelation(value.EventRange); err != nil {
		return err
	}
	return validateTerminalReasonRelation(value.TerminalState, value.ReasonCodes)
}

func validateExecutionScope(value *ExecutionReceipt) error {
	if err := validateScopeReferences(value.ScopeRef); err != nil {
		return err
	}
	if value.ScopeRef.ProjectSnapshotID == nil || value.ScopeRef.AttemptID == nil ||
		value.ScopeRef.SessionID == nil || value.ScopeRef.TurnID != nil || value.ScopeRef.ActionID != nil {
		return reject(rejectionReferenceMismatch, "execution receipt requires exact session-level snapshot scope")
	}
	checks := []struct {
		value      core.EntityRef
		typeValue  core.EntityType
		identifier string
		label      string
	}{
		{value.AttemptRef, core.EntityType("attempt"), *value.ScopeRef.AttemptID, "attempt_ref"},
		{value.SessionRef, core.EntityType("session"), *value.ScopeRef.SessionID, "session_ref"},
		{value.SourceSnapshotRef, core.EntityType("project_snapshot"), *value.ScopeRef.ProjectSnapshotID, "source_snapshot_ref"},
	}
	for _, check := range checks {
		if check.value.EntityType != check.typeValue || check.value.EntityID != check.identifier {
			return rejectf(rejectionReferenceMismatch, "%s must exactly match scope_ref", check.label)
		}
	}
	return nil
}

func validateExecutionTimeValues(value *ExecutionReceipt) error {
	if err := validateUnixMS(value.StartedAtUnixMS, "started_at_unix_ms"); err != nil {
		return err
	}
	if err := validateUnixMS(value.EndedAtUnixMS, "ended_at_unix_ms"); err != nil {
		return err
	}
	return validateObservedUsage(value.ObservedUsage)
}

func validateExecutionState(value *ExecutionReceipt) error {
	if _, ok := terminalAttemptStates[value.TerminalState]; !ok {
		return rejectf(rejectionStateInvalid, "terminal_state %q is not an Attempt terminal state", value.TerminalState)
	}
	return nil
}

var terminalAttemptStates = map[state.AttemptState]struct{}{
	"interrupted": {}, "completed": {}, "failed": {}, "uncertain": {},
}

func validateExecutor(value ExecutorDescriptor) error {
	if err := validateActorRef(value.ActorRef); err != nil {
		return err
	}
	if value.ActorRef.ActorType != core.ActorType("agent") &&
		value.ActorRef.ActorType != core.ActorType("service") &&
		value.ActorRef.ActorType != core.ActorType("system") {
		return reject(rejectionValueInvalid, "executor actor_type must be agent, service, or system")
	}
	if err := validateSchemaName(value.AdapterID, "executor.adapter_id"); err != nil {
		return err
	}
	return validateAdapterVersion(value.AdapterVersion)
}

func validateObservedUsage(value ObservedUsage) error {
	checks := []struct {
		label   string
		value   int64
		maximum int64
	}{
		{"cost_usd_micros", value.CostUSDMicros, maxObservedQuantity},
		{"elapsed_ms", value.ElapsedMS, maxExecutionElapsedMS},
		{"input_tokens", value.InputTokens, maxObservedQuantity},
		{"model_calls", value.ModelCalls, maxObservedCount},
		{"network_bytes", value.NetworkBytes, maxObservedQuantity},
		{"output_bytes", value.OutputBytes, maxObservedQuantity},
		{"output_tokens", value.OutputTokens, maxObservedQuantity},
		{"tool_calls", value.ToolCalls, maxObservedCount},
	}
	for _, check := range checks {
		if check.value < 0 || check.value > check.maximum {
			return rejectf(rejectionValueInvalid, "observed_usage.%s must be in 0..%d", check.label, check.maximum)
		}
	}
	return nil
}

func validateExecutionDeclarationValues(value *ExecutionReceipt) error {
	for _, reference := range []struct {
		value *core.RecordRef
		label string
	}{{value.ApprovalRef, "approval_ref"}, {value.GrantRef, "grant_ref"}} {
		if reference.value != nil {
			if err := validateRecordRef(*reference.value, reference.label); err != nil {
				return err
			}
		}
	}
	if err := validateExecutionReferenceValues(value); err != nil {
		return err
	}
	if err := validateArtifactSetValues(value.InputArtifactRefs, "input_artifact_refs"); err != nil {
		return err
	}
	if err := validateArtifactSetValues(value.OutputArtifactRefs, "output_artifact_refs"); err != nil {
		return err
	}
	if err := validateEventRangeValues(value.EventRange); err != nil {
		return err
	}
	return validateReasonCodes(value.ReasonCodes, "reason_codes")
}

func validateExecutionReferenceValues(value *ExecutionReceipt) error {
	if err := validateScopeValues(value.ScopeRef); err != nil {
		return err
	}
	checks := []struct {
		value core.EntityRef
		label string
	}{
		{value.AttemptRef, "attempt_ref"}, {value.SessionRef, "session_ref"},
		{value.SourceSnapshotRef, "source_snapshot_ref"},
	}
	for _, check := range checks {
		if err := validateEntityRef(check.value, check.label); err != nil {
			return err
		}
	}
	return nil
}

func validateArtifactSetValues(values []core.ArtifactRef, label string) error {
	if len(values) > maxReceiptArtifacts {
		return rejectf(rejectionValueInvalid, "%s exceeds %d items", label, maxReceiptArtifacts)
	}
	previous := ""
	for index := range values {
		artifact := &values[index]
		if err := validateArtifactValues(artifact); err != nil {
			return err
		}
		if artifact.LogicalID <= previous {
			return rejectf(rejectionValueInvalid, "%s must be strictly sorted by logical_id", label)
		}
		previous = artifact.LogicalID
	}
	return nil
}

func validateExecutionArtifactRelation(artifact *core.ArtifactRef, receipt *ExecutionReceipt, output bool) error {
	if !output && artifact.CreatedAtUnixMS > receipt.StartedAtUnixMS {
		return reject(rejectionRelationMismatch, "input artifact cannot postdate execution start")
	}
	if output && (artifact.ProducerAttemptID != receipt.AttemptRef.EntityID ||
		artifact.CreatedAtUnixMS < receipt.StartedAtUnixMS || artifact.CreatedAtUnixMS > receipt.EndedAtUnixMS) {
		return reject(rejectionRelationMismatch, "output artifact must be produced by the attempt during execution")
	}
	return nil
}

func validateEventRangeValues(value *EventRange) error {
	if value == nil {
		return nil
	}
	if err := validateEntityRef(value.AggregateRef, "event_range.aggregate_ref"); err != nil {
		return err
	}
	if err := validateTypedID(value.FirstEventID, "evt", "event_range.first_event_id"); err != nil {
		return err
	}
	if err := validateTypedID(value.LastEventID, "evt", "event_range.last_event_id"); err != nil {
		return err
	}
	if value.FirstSequence < 1 || value.LastSequence < 1 {
		return reject(rejectionValueInvalid, "event_range sequences must be positive")
	}
	return nil
}

func validateEventRangeRelation(value *EventRange) error {
	if value == nil {
		return nil
	}
	if value.LastSequence < value.FirstSequence {
		return reject(rejectionRelationMismatch, "event_range sequence interval is reversed")
	}
	if (value.FirstSequence == value.LastSequence) != (value.FirstEventID == value.LastEventID) {
		return reject(rejectionRelationMismatch, "event_range identity and sequence cardinality disagree")
	}
	return nil
}

func validateExecutionIntervalRelation(value *ExecutionReceipt) error {
	elapsed := value.EndedAtUnixMS - value.StartedAtUnixMS
	if elapsed < 0 || elapsed > maxExecutionElapsedMS {
		return reject(rejectionRelationMismatch, "execution wall interval is invalid or exceeds maximum")
	}
	return nil
}

func validateTerminalReasonRelation(attemptState state.AttemptState, values []string) error {
	if (attemptState == state.AttemptState("completed")) != (len(values) == 0) {
		return reject(rejectionRelationMismatch, "completed requires no reasons; other terminals require reasons")
	}
	return nil
}

func validateReasonCodes(values []string, label string) error {
	if values == nil {
		return rejectf(rejectionDocumentInvalid, "%s must be a non-null array", label)
	}
	if len(values) > maxReasonCodes {
		return rejectf(rejectionValueInvalid, "%s must contain at most %d items", label, maxReasonCodes)
	}
	previous := ""
	for _, value := range values {
		if err := validateLowerToken(value, label, 64); err != nil {
			return err
		}
		if value <= previous {
			return rejectf(rejectionValueInvalid, "%s must be strictly sorted and unique", label)
		}
		previous = value
	}
	return nil
}
