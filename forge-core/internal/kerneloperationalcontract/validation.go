package kerneloperationalcontract

import "fmt"

func validateIdentity(identity, digest, prefix, label string, blank bool) error {
	if blank && identity == "" && digest == "" {
		return nil
	}
	if err := validateHash(digest, label+"_sha256"); err != nil {
		return err
	}
	if identity != prefix+digest {
		return fmt.Errorf("%s_id must bind %s_sha256", label, label)
	}
	return nil
}

func validateArtifactReceiptRef(value ArtifactReceiptRef, label string) error {
	return validateIdentity(value.ArtifactReceiptID, value.ArtifactReceiptSHA256,
		artifactReceiptPrefix, "artifact_receipt", false)
}

func validateInvocationRef(value CapabilityInvocationRef, label string) error {
	return validateIdentity(value.InvocationID, value.InvocationSHA256,
		invocationPrefix, "invocation", false)
}

func validateEventRef(value InteractionEventRef, label string) error {
	return validateIdentity(value.EventID, value.EventSHA256, eventPrefix, "event", false)
}

func validateExecutionReceiptRef(value ExecutionReceiptRef, label string) error {
	return validateIdentity(value.ExecutionReceiptID, value.ExecutionReceiptSHA256,
		executionReceiptPrefix, "execution_receipt", false)
}

func validateObservedUsage(value ObservedUsage) error {
	bounds := []struct {
		label   string
		value   int64
		maximum int64
	}{
		{"call_count", value.CallCount, maxCallCount},
		{"cost_usd_micros", value.CostUSDMicros, maxCostMicros},
		{"elapsed_ms", value.ElapsedMS, maxElapsedMS},
		{"input_tokens", value.InputTokens, maxTokenCount},
		{"network_bytes", value.NetworkBytes, maxNetworkBytes},
		{"output_bytes", value.OutputBytes, maxOutputBytes},
		{"output_tokens", value.OutputTokens, maxTokenCount},
	}
	for _, bound := range bounds {
		if err := validateNonnegative(bound.value, "observed_usage."+bound.label,
			bound.maximum); err != nil {
			return err
		}
	}
	return nil
}

func validateRecordHeader(apiVersion, kind string, expectedAPI, expectedKind string) error {
	if apiVersion != expectedAPI {
		return fmt.Errorf("api_version must be %q", expectedAPI)
	}
	if kind != expectedKind {
		return fmt.Errorf("kind must be %q", expectedKind)
	}
	return nil
}

func validateArtifactReceiptFields(value *ArtifactReceipt, blank bool) error {
	if value == nil {
		return fmt.Errorf("ArtifactReceipt is nil")
	}
	if err := validateRecordHeader(value.APIVersion, value.Kind,
		artifactReceiptAPI, artifactReceiptKind); err != nil {
		return err
	}
	if value.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if err := validateIdentity(value.ArtifactReceiptID, value.ArtifactReceiptSHA256,
		artifactReceiptPrefix, "artifact_receipt", blank); err != nil {
		return err
	}
	if err := validateArtifact(value.Artifact, "artifact"); err != nil {
		return err
	}
	if err := validateArtifactReceiptMembers(value); err != nil {
		return err
	}
	return validateTaskBinding(value.TaskBinding)
}

func validateArtifactReceiptMembers(value *ArtifactReceipt) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if err := validateNonnegative(value.ContentBytes, "content_bytes", int64(^uint64(0)>>1)); err != nil {
		return err
	}
	if err := validateNonnegative(value.CreatedAtUnixMS, "created_at_unix_ms", int64(^uint64(0)>>1)); err != nil {
		return err
	}
	if err := validatePrincipal(value.Producer, "producer"); err != nil {
		return err
	}
	if !oneOf(value.ReceiptRole, "declared_input", "declared_output") {
		return fmt.Errorf("receipt_role is unsupported")
	}
	if (value.ReceiptRole == "declared_input") != (value.ProducerInvocationRef == nil) {
		return fmt.Errorf("declared_input requires null producer; output requires one")
	}
	if value.ProducerInvocationRef != nil {
		if err := validateInvocationRef(*value.ProducerInvocationRef,
			"producer_invocation_ref"); err != nil {
			return err
		}
	}
	return validateIdentifier(value.Slot, "slot")
}

func validateInvocationFields(value *CapabilityInvocation, blank bool) error {
	if value == nil {
		return fmt.Errorf("CapabilityInvocation is nil")
	}
	if err := validateRecordHeader(value.APIVersion, value.Kind,
		invocationAPI, invocationKind); err != nil {
		return err
	}
	if value.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if err := validateIdentity(value.InvocationID, value.InvocationSHA256,
		invocationPrefix, "invocation", blank); err != nil {
		return err
	}
	if value.Attempt < 1 || value.Attempt > maxAttempt {
		return fmt.Errorf("attempt must be in 1..%d", maxAttempt)
	}
	if err := validateInvocationMembers(value); err != nil {
		return err
	}
	return validateTaskBinding(value.TaskBinding)
}

func validateInvocationMembers(value *CapabilityInvocation) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if err := validateCapability(value.Capability); err != nil {
		return err
	}
	if err := validateGrantRef(value.CapabilityGrantRef); err != nil {
		return err
	}
	if err := validateIdentifier(value.CorrelationID, "correlation_id"); err != nil {
		return err
	}
	if err := validateStringSet(value.DeclaredOutputSlots,
		"declared_output_slots", maxIOItems, false); err != nil {
		return err
	}
	if err := validateIdentifier(value.IdempotencyKey, "idempotency_key"); err != nil {
		return err
	}
	if err := validateSortedUnique(value.InputArtifactReceiptRefs,
		"input_artifact_receipt_refs", maxIOItems, false, validateArtifactReceiptRef); err != nil {
		return err
	}
	return validateInvocationTail(value)
}

func validateInvocationTail(value *CapabilityInvocation) error {
	if (value.Attempt == 1) != (value.PriorExecutionReceiptRef == nil) {
		return fmt.Errorf("attempt one requires null prior receipt; retry requires one")
	}
	if value.PriorExecutionReceiptRef != nil {
		if err := validateExecutionReceiptRef(*value.PriorExecutionReceiptRef,
			"prior_execution_receipt_ref"); err != nil {
			return err
		}
	}
	if err := validateHash(value.RequestedActionSHA256, "requested_action_sha256"); err != nil {
		return err
	}
	if err := validateNonnegative(value.RequestedAtUnixMS, "requested_at_unix_ms",
		int64(^uint64(0)>>1)); err != nil {
		return err
	}
	return validatePrincipal(value.Subject, "subject")
}

func validateEventFields(value *InteractionEvent, blank bool) error {
	if value == nil {
		return fmt.Errorf("InteractionEvent is nil")
	}
	if err := validateRecordHeader(value.APIVersion, value.Kind, eventAPI, eventKind); err != nil {
		return err
	}
	if value.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if err := validateIdentity(value.EventID, value.EventSHA256,
		eventPrefix, "event", blank); err != nil {
		return err
	}
	if err := validatePrincipal(value.Actor, "actor"); err != nil {
		return err
	}
	if err := validateSortedUnique(value.ArtifactRefs, "artifact_refs",
		maxIOItems, false, validateArtifact); err != nil {
		return err
	}
	if err := validateEventMembers(value); err != nil {
		return err
	}
	return validateTaskBinding(value.TaskBinding)
}

func validateEventMembers(value *InteractionEvent) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if value.LogicalSequence < 1 || value.LogicalSequence > maxEvents {
		return fmt.Errorf("logical_sequence must be in 1..%d", maxEvents)
	}
	if (value.LogicalSequence == 1) != (value.CausationEventRef == nil) {
		return fmt.Errorf("sequence one requires null cause; later events require one")
	}
	if value.CausationEventRef != nil {
		if err := validateEventRef(*value.CausationEventRef, "causation_event_ref"); err != nil {
			return err
		}
	}
	if value.ConfidenceMicros != nil {
		if err := validateNonnegative(*value.ConfidenceMicros, "confidence_micros",
			maxConfidenceMicros); err != nil {
			return err
		}
	}
	return validateEventTail(value)
}

func validateEventTail(value *InteractionEvent) error {
	if err := validateIdentifier(value.CorrelationID, "correlation_id"); err != nil {
		return err
	}
	if err := validateInvocationRef(value.InvocationRef, "invocation_ref"); err != nil {
		return err
	}
	if err := validateText(value.ObjectRef, "object_ref", maxReferenceBytes); err != nil {
		return err
	}
	if err := validateNonnegative(value.OccurredAtUnixMS, "occurred_at_unix_ms",
		int64(^uint64(0)>>1)); err != nil {
		return err
	}
	if value.Target != nil {
		if err := validatePrincipal(*value.Target, "target"); err != nil {
			return err
		}
	}
	if !oneOf(value.Verb, "approve", "execute", "observe", "propose",
		"reject", "request", "rollback", "verify") {
		return fmt.Errorf("verb is unsupported")
	}
	return nil
}

func validateEventRefs(values []InteractionEventRef) error {
	if values == nil || len(values) > maxEvents {
		return fmt.Errorf("event_refs cardinality is outside the frozen bound")
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateEventRef(value, "event_refs"); err != nil {
			return err
		}
		key := value.EventID + value.EventSHA256
		if _, found := seen[key]; found {
			return fmt.Errorf("event_refs must be unique")
		}
		seen[key] = struct{}{}
	}
	return nil
}

func validateExecutionReceiptFields(value *ExecutionReceipt, blank bool) error {
	if value == nil {
		return fmt.Errorf("ExecutionReceipt is nil")
	}
	if err := validateRecordHeader(value.APIVersion, value.Kind,
		executionReceiptAPI, executionReceiptKind); err != nil {
		return err
	}
	if value.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if err := validateIdentity(value.ExecutionReceiptID, value.ExecutionReceiptSHA256,
		executionReceiptPrefix, "execution_receipt", blank); err != nil {
		return err
	}
	if value.Attempt < 1 || value.Attempt > maxAttempt {
		return fmt.Errorf("attempt must be in 1..%d", maxAttempt)
	}
	if err := validateExecutionMembers(value); err != nil {
		return err
	}
	return validateTaskBinding(value.TaskBinding)
}

func validateExecutionMembers(value *ExecutionReceipt) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if err := validateIdentifier(value.CorrelationID, "correlation_id"); err != nil {
		return err
	}
	if value.StartedAtUnixMS < 0 || value.EndedAtUnixMS < value.StartedAtUnixMS ||
		value.EndedAtUnixMS-value.StartedAtUnixMS > maxElapsedMS {
		return fmt.Errorf("execution wall interval is invalid or exceeds max")
	}
	if err := validateEventRefs(value.EventRefs); err != nil {
		return err
	}
	if err := validateSortedUnique(value.InputArtifacts, "input_artifacts",
		maxIOItems, false, validateArtifact); err != nil {
		return err
	}
	return validateExecutionTail(value)
}

func validateExecutionTail(value *ExecutionReceipt) error {
	if err := validateInvocationRef(value.InvocationRef, "invocation_ref"); err != nil {
		return err
	}
	if err := validateObservedUsage(value.ObservedUsage); err != nil {
		return err
	}
	if value.ObservedUsage.ElapsedMS != value.EndedAtUnixMS-value.StartedAtUnixMS {
		return fmt.Errorf("observed_usage.elapsed_ms must equal the execution wall interval")
	}
	if !oneOf(value.Outcome, "cancelled", "failed", "inconclusive", "lost", "succeeded") {
		return fmt.Errorf("outcome is unsupported")
	}
	if err := validateSortedUnique(value.OutputArtifactReceiptRefs,
		"output_artifact_receipt_refs", maxIOItems, false, validateArtifactReceiptRef); err != nil {
		return err
	}
	return validateExecutionFinal(value)
}

func validateExecutionFinal(value *ExecutionReceipt) error {
	if (value.Attempt == 1) != (value.PriorExecutionReceiptRef == nil) {
		return fmt.Errorf("attempt one requires null prior receipt; retry requires one")
	}
	if value.PriorExecutionReceiptRef != nil {
		if err := validateExecutionReceiptRef(*value.PriorExecutionReceiptRef,
			"prior_execution_receipt_ref"); err != nil {
			return err
		}
	}
	if err := validateStringSet(value.ReasonCodes, "reason_codes", maxReasonCodes, false); err != nil {
		return err
	}
	if (value.Outcome == "succeeded") != (len(value.ReasonCodes) == 0) {
		return fmt.Errorf("succeeded requires no reasons; every other outcome requires one")
	}
	return validatePrincipal(value.Executor, "executor")
}
