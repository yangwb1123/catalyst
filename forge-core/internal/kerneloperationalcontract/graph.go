package kerneloperationalcontract

import (
	"fmt"
	"reflect"
	"sort"
)

func invocationReference(value CapabilityInvocation) CapabilityInvocationRef {
	return CapabilityInvocationRef{InvocationID: value.InvocationID,
		InvocationSHA256: value.InvocationSHA256}
}

func artifactReceiptReference(value ArtifactReceipt) ArtifactReceiptRef {
	return ArtifactReceiptRef{ArtifactReceiptID: value.ArtifactReceiptID,
		ArtifactReceiptSHA256: value.ArtifactReceiptSHA256}
}

func eventReference(value InteractionEvent) InteractionEventRef {
	return InteractionEventRef{EventID: value.EventID, EventSHA256: value.EventSHA256}
}

func executionReference(value ExecutionReceipt) ExecutionReceiptRef {
	return ExecutionReceiptRef{ExecutionReceiptID: value.ExecutionReceiptID,
		ExecutionReceiptSHA256: value.ExecutionReceiptSHA256}
}

func validateCommonContext(value *KernelOperationalReferenceClosure) error {
	base := value.CapabilityInvocations[0]
	for index := range value.ArtifactReceipts {
		if !reflect.DeepEqual(value.ArtifactReceipts[index].Bindings, base.Bindings) ||
			!reflect.DeepEqual(value.ArtifactReceipts[index].TaskBinding, base.TaskBinding) {
			return fmt.Errorf("every record must carry identical bindings and TaskBinding")
		}
	}
	for index := range value.CapabilityInvocations {
		if !reflect.DeepEqual(value.CapabilityInvocations[index].Bindings, base.Bindings) ||
			!reflect.DeepEqual(value.CapabilityInvocations[index].TaskBinding, base.TaskBinding) {
			return fmt.Errorf("every record must carry identical bindings and TaskBinding")
		}
	}
	for index := range value.InteractionEvents {
		if !reflect.DeepEqual(value.InteractionEvents[index].Bindings, base.Bindings) ||
			!reflect.DeepEqual(value.InteractionEvents[index].TaskBinding, base.TaskBinding) {
			return fmt.Errorf("every record must carry identical bindings and TaskBinding")
		}
	}
	for index := range value.ExecutionReceipts {
		if !reflect.DeepEqual(value.ExecutionReceipts[index].Bindings, base.Bindings) ||
			!reflect.DeepEqual(value.ExecutionReceipts[index].TaskBinding, base.TaskBinding) {
			return fmt.Errorf("every record must carry identical bindings and TaskBinding")
		}
	}
	return nil
}

func sameRetryStatic(first, next CapabilityInvocation) bool {
	return reflect.DeepEqual(first.Bindings, next.Bindings) &&
		reflect.DeepEqual(first.Capability, next.Capability) &&
		reflect.DeepEqual(first.CapabilityGrantRef, next.CapabilityGrantRef) &&
		first.CorrelationID == next.CorrelationID &&
		reflect.DeepEqual(first.DeclaredOutputSlots, next.DeclaredOutputSlots) &&
		first.IdempotencyKey == next.IdempotencyKey &&
		reflect.DeepEqual(first.InputArtifactReceiptRefs, next.InputArtifactReceiptRefs) &&
		first.RequestedActionSHA256 == next.RequestedActionSHA256 &&
		reflect.DeepEqual(first.Subject, next.Subject) &&
		reflect.DeepEqual(first.TaskBinding, next.TaskBinding)
}

func validateRetryChain(value *KernelOperationalReferenceClosure) error {
	if len(value.CapabilityInvocations) != len(value.ExecutionReceipts) {
		return fmt.Errorf("each invocation requires exactly one ExecutionReceipt")
	}
	base := value.CapabilityInvocations[0]
	for index := range value.CapabilityInvocations {
		invocation := value.CapabilityInvocations[index]
		receipt := value.ExecutionReceipts[index]
		expectedAttempt := int64(index + 1)
		if invocation.Attempt != expectedAttempt || receipt.Attempt != expectedAttempt {
			return fmt.Errorf("invocations and receipts must be ordered contiguous attempts 1..N")
		}
		if !sameRetryStatic(base, invocation) {
			return fmt.Errorf("retry invocations must preserve the complete static request")
		}
		var prior *ExecutionReceiptRef
		if index > 0 {
			item := executionReference(value.ExecutionReceipts[index-1])
			prior = &item
		}
		if !reflect.DeepEqual(invocation.PriorExecutionReceiptRef, prior) ||
			!reflect.DeepEqual(receipt.PriorExecutionReceiptRef, prior) {
			return fmt.Errorf("prior receipt must be the preceding attempt")
		}
		if index > 0 && value.ExecutionReceipts[index-1].Outcome == "succeeded" {
			return fmt.Errorf("a succeeded execution cannot be retried")
		}
		if index > 0 && invocation.RequestedAtUnixMS < value.ExecutionReceipts[index-1].EndedAtUnixMS {
			return fmt.Errorf("retry request precedes the prior attempt end")
		}
	}
	return nil
}

func artifactReceiptMap(values []ArtifactReceipt) (map[string]ArtifactReceipt, error) {
	result := make(map[string]ArtifactReceipt, len(values))
	for _, value := range values {
		if _, exists := result[value.ArtifactReceiptID]; exists {
			return nil, fmt.Errorf("duplicate ArtifactReceipt identity")
		}
		result[value.ArtifactReceiptID] = value
	}
	return result, nil
}

func resolveInputs(invocation CapabilityInvocation, receipt ExecutionReceipt,
	available map[string]ArtifactReceipt, used map[string]bool) error {
	artifacts := make([]ArtifactRef, 0, len(invocation.InputArtifactReceiptRefs))
	seen := make(map[ArtifactRef]bool)
	for _, reference := range invocation.InputArtifactReceiptRefs {
		member, found := available[reference.ArtifactReceiptID]
		if !found || reference != artifactReceiptReference(member) {
			return fmt.Errorf("input ArtifactReceipt reference is unresolved")
		}
		if member.ReceiptRole != "declared_input" ||
			member.CreatedAtUnixMS > invocation.RequestedAtUnixMS {
			return fmt.Errorf("invocation input role or time is invalid")
		}
		if seen[member.Artifact] {
			return fmt.Errorf("invocation inputs must project distinct ArtifactRefs")
		}
		seen[member.Artifact], used[member.ArtifactReceiptID] = true, true
		artifacts = append(artifacts, member.Artifact)
	}
	sortArtifacts(artifacts)
	if !reflect.DeepEqual(receipt.InputArtifacts, artifacts) {
		return fmt.Errorf("ExecutionReceipt input_artifacts must exactly project inputs")
	}
	return nil
}

func outputsFor(invocation CapabilityInvocation, receipt ExecutionReceipt,
	values []ArtifactReceipt, used map[string]bool) error {
	reference := invocationReference(invocation)
	outputs := make([]ArtifactReceipt, 0)
	for _, value := range values {
		if value.ProducerInvocationRef != nil && *value.ProducerInvocationRef == reference {
			outputs = append(outputs, value)
		}
	}
	slots := make([]string, len(outputs))
	refs := make([]ArtifactReceiptRef, len(outputs))
	for index, output := range outputs {
		slots[index], refs[index] = output.Slot, artifactReceiptReference(output)
		used[output.ArtifactReceiptID] = true
		if output.CreatedAtUnixMS < receipt.StartedAtUnixMS ||
			output.CreatedAtUnixMS > receipt.EndedAtUnixMS {
			return fmt.Errorf("output ArtifactReceipt time is outside its execution")
		}
	}
	sort.Strings(slots)
	if hasDuplicate(slots) || !reflect.DeepEqual(slots, invocation.DeclaredOutputSlots) {
		return fmt.Errorf("declared_output_slots must exactly cover output receipts")
	}
	sort.Slice(refs, func(i, j int) bool { return refs[i].ArtifactReceiptID < refs[j].ArtifactReceiptID })
	if !reflect.DeepEqual(refs, receipt.OutputArtifactReceiptRefs) {
		return fmt.Errorf("ExecutionReceipt must reference every exact output receipt")
	}
	return nil
}

func eventsFor(invocation CapabilityInvocation, receipt ExecutionReceipt,
	values []InteractionEvent) ([]InteractionEvent, error) {
	reference := invocationReference(invocation)
	members := make([]InteractionEvent, 0)
	for _, value := range values {
		if value.InvocationRef == reference {
			members = append(members, value)
		}
	}
	refs := make([]InteractionEventRef, len(members))
	for index, event := range members {
		if event.LogicalSequence != int64(index+1) {
			return nil, fmt.Errorf("events must be contiguous sequences per invocation")
		}
		refs[index] = eventReference(event)
		var cause *InteractionEventRef
		if index > 0 {
			cause = &refs[index-1]
		}
		if !reflect.DeepEqual(event.CausationEventRef, cause) {
			return nil, fmt.Errorf("event causation must be immediately preceding")
		}
		if event.OccurredAtUnixMS < receipt.StartedAtUnixMS ||
			event.OccurredAtUnixMS > receipt.EndedAtUnixMS {
			return nil, fmt.Errorf("event time is outside its execution")
		}
		if index > 0 && event.OccurredAtUnixMS < members[index-1].OccurredAtUnixMS {
			return nil, fmt.Errorf("event times must be nondecreasing by logical sequence")
		}
	}
	if !reflect.DeepEqual(refs, receipt.EventRefs) {
		return nil, fmt.Errorf("ExecutionReceipt event_refs must exactly cover its events")
	}
	return members, nil
}

func validateAttempt(invocation CapabilityInvocation, receipt ExecutionReceipt,
	artifactMap map[string]ArtifactReceipt, artifacts []ArtifactReceipt,
	events []InteractionEvent, used map[string]bool) ([]InteractionEvent, error) {
	if receipt.InvocationRef != invocationReference(invocation) {
		return nil, fmt.Errorf("ExecutionReceipt must reference its matching invocation")
	}
	if receipt.CorrelationID != invocation.CorrelationID {
		return nil, fmt.Errorf("receipt and invocation correlation_id differ")
	}
	if receipt.StartedAtUnixMS < invocation.RequestedAtUnixMS {
		return nil, fmt.Errorf("execution starts before its invocation request")
	}
	if err := resolveInputs(invocation, receipt, artifactMap, used); err != nil {
		return nil, err
	}
	if err := outputsFor(invocation, receipt, artifacts, used); err != nil {
		return nil, err
	}
	members, err := eventsFor(invocation, receipt, events)
	if err != nil {
		return nil, err
	}
	for _, event := range members {
		if event.CorrelationID != invocation.CorrelationID {
			return nil, fmt.Errorf("event and invocation correlation_id differ")
		}
	}
	return members, nil
}

func validateArtifactInventory(value *KernelOperationalReferenceClosure) error {
	created := make(map[ArtifactRef][]int64)
	projected := make(map[ArtifactRef]bool)
	for _, receipt := range value.ArtifactReceipts {
		created[receipt.Artifact] = append(created[receipt.Artifact], receipt.CreatedAtUnixMS)
		projected[receipt.Artifact] = true
	}
	for _, event := range value.InteractionEvents {
		for _, artifact := range event.ArtifactRefs {
			if !hasNonfutureReceipt(created[artifact], event.OccurredAtUnixMS) {
				return fmt.Errorf("Event ArtifactRef needs a non-future included ArtifactReceipt")
			}
			projected[artifact] = true
		}
	}
	for _, receipt := range value.ExecutionReceipts {
		for _, artifact := range receipt.InputArtifacts {
			projected[artifact] = true
		}
	}
	expected := make([]ArtifactRef, 0, len(projected))
	for artifact := range projected {
		expected = append(expected, artifact)
	}
	sortArtifacts(expected)
	if !reflect.DeepEqual(expected, value.Artifacts) {
		return fmt.Errorf("closure artifacts must exactly equal all referenced ArtifactRefs")
	}
	return nil
}

func hasNonfutureReceipt(times []int64, occurred int64) bool {
	for _, created := range times {
		if created <= occurred {
			return true
		}
	}
	return false
}

func sortArtifacts(values []ArtifactRef) {
	sort.Slice(values, func(i, j int) bool {
		left, _ := canonicalKey(values[i])
		right, _ := canonicalKey(values[j])
		return string(left) < string(right)
	})
}

func validateReferenceGraph(value *KernelOperationalReferenceClosure) error {
	if err := validateCommonContext(value); err != nil {
		return err
	}
	if err := validateRetryChain(value); err != nil {
		return err
	}
	artifactMap, err := artifactReceiptMap(value.ArtifactReceipts)
	if err != nil {
		return err
	}
	used := make(map[string]bool)
	flattened := make([]InteractionEvent, 0, len(value.InteractionEvents))
	for index := range value.CapabilityInvocations {
		members, err := validateAttempt(value.CapabilityInvocations[index],
			value.ExecutionReceipts[index], artifactMap, value.ArtifactReceipts,
			value.InteractionEvents, used)
		if err != nil {
			return err
		}
		flattened = append(flattened, members...)
	}
	if !reflect.DeepEqual(flattened, value.InteractionEvents) {
		return fmt.Errorf("interaction_events must be ordered by attempt then sequence")
	}
	if len(used) != len(artifactMap) {
		return fmt.Errorf("ArtifactReceipt inventory contains an orphan")
	}
	return validateArtifactInventory(value)
}
