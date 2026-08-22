package kerneloperationalcontract

import (
	"fmt"
	"strings"
	"testing"
)

func emptyProfile(t *testing.T) *KernelOperationalReferenceClosure {
	t.Helper()
	golden, _ := goldenClosure(t)
	invocation := golden.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.InputArtifactReceiptRefs = []ArtifactReceiptRef{}
	invocation.DeclaredOutputSlots = []string{}
	sealedInvocation, err := SealCapabilityInvocation(&invocation)
	if err != nil {
		t.Fatal(err)
	}
	receipt := golden.ExecutionReceipts[0]
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.InvocationRef = refInvocation(*sealedInvocation)
	receipt.EventRefs, receipt.InputArtifacts = []InteractionEventRef{}, []ArtifactRef{}
	receipt.OutputArtifactReceiptRefs, receipt.ReasonCodes = []ArtifactReceiptRef{}, []string{}
	receipt.Outcome = "succeeded"
	sealedReceipt, err := SealExecutionReceipt(&receipt)
	if err != nil {
		t.Fatal(err)
	}
	candidate := *golden
	candidate.ClosureID, candidate.ClosureSHA256 = "", ""
	candidate.Artifacts, candidate.ArtifactReceipts = []ArtifactRef{}, []ArtifactReceipt{}
	candidate.CapabilityInvocations = []CapabilityInvocation{*sealedInvocation}
	candidate.InteractionEvents = []InteractionEvent{}
	candidate.ExecutionReceipts = []ExecutionReceipt{*sealedReceipt}
	sealed, err := SealClosure(&candidate)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func TestEmptyIOOutputAndEventProfile(t *testing.T) {
	closure := emptyProfile(t)
	if len(closure.Artifacts) != 0 || len(closure.ArtifactReceipts) != 0 ||
		len(closure.InteractionEvents) != 0 {
		t.Fatal("empty profile invented semantic records")
	}
	raw, err := CanonicalJSON(closure)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeClosure(raw)
	if err != nil || decoded.ClosureSHA256 != closure.ClosureSHA256 {
		t.Fatalf("empty profile round trip: %v", err)
	}
}

func TestRecordNAndNPlusOneBounds(t *testing.T) {
	closure, _ := goldenClosure(t)
	invocation := closure.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.DeclaredOutputSlots = make([]string, maxIOItems)
	for index := range invocation.DeclaredOutputSlots {
		invocation.DeclaredOutputSlots[index] = fmt.Sprintf("slot-%02d", index)
	}
	if _, err := SealCapabilityInvocation(&invocation); err != nil {
		t.Fatalf("N output slots: %v", err)
	}
	invocation.DeclaredOutputSlots = append(invocation.DeclaredOutputSlots, "slot-32")
	if _, err := SealCapabilityInvocation(&invocation); err == nil {
		t.Fatal("N+1 output slots accepted")
	}
	testEventArtifactBounds(t, closure.InteractionEvents[0])
}

func testEventArtifactBounds(t *testing.T, event InteractionEvent) {
	t.Helper()
	event.EventID, event.EventSHA256 = "", ""
	event.ArtifactRefs = make([]ArtifactRef, maxIOItems)
	for index := range event.ArtifactRefs {
		event.ArtifactRefs[index] = ArtifactRef{ArtifactKind: "fixture",
			ArtifactRef:    fmt.Sprintf("fixture/%02d", index),
			ArtifactSHA256: fmt.Sprintf("%064x", index)}
	}
	if _, err := SealInteractionEvent(&event); err != nil {
		t.Fatalf("N artifact refs: %v", err)
	}
	event.ArtifactRefs = append(event.ArtifactRefs, ArtifactRef{ArtifactKind: "fixture",
		ArtifactRef: "fixture/32", ArtifactSHA256: fmt.Sprintf("%064x", 32)})
	if _, err := SealInteractionEvent(&event); err == nil {
		t.Fatal("N+1 artifact refs accepted")
	}
}

func TestAttemptElapsedAndUsageBounds(t *testing.T) {
	closure, _ := goldenClosure(t)
	invocation := closure.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.Attempt = maxAttempt
	digest := strings.Repeat("d", 64)
	invocation.PriorExecutionReceiptRef = &ExecutionReceiptRef{
		ExecutionReceiptID: executionReceiptPrefix + digest, ExecutionReceiptSHA256: digest}
	if _, err := SealCapabilityInvocation(&invocation); err != nil {
		t.Fatalf("max attempt: %v", err)
	}
	invocation.Attempt++
	if _, err := SealCapabilityInvocation(&invocation); err == nil {
		t.Fatal("N+1 attempt accepted")
	}
	testElapsedAndUsageBounds(t, closure.ExecutionReceipts[0])
}

func testElapsedAndUsageBounds(t *testing.T, receipt ExecutionReceipt) {
	t.Helper()
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.EndedAtUnixMS = receipt.StartedAtUnixMS + maxElapsedMS
	receipt.ObservedUsage.ElapsedMS = maxElapsedMS
	receipt.ObservedUsage.CallCount = maxCallCount
	receipt.ObservedUsage.CostUSDMicros = maxCostMicros
	receipt.ObservedUsage.InputTokens = maxTokenCount
	receipt.ObservedUsage.NetworkBytes = maxNetworkBytes
	receipt.ObservedUsage.OutputBytes = maxOutputBytes
	receipt.ObservedUsage.OutputTokens = maxTokenCount
	if _, err := SealExecutionReceipt(&receipt); err != nil {
		t.Fatalf("max elapsed and usage: %v", err)
	}
	receipt.EndedAtUnixMS++
	receipt.ObservedUsage.ElapsedMS++
	if _, err := SealExecutionReceipt(&receipt); err == nil {
		t.Fatal("N+1 elapsed accepted")
	}
	receipt.EndedAtUnixMS--
	receipt.ObservedUsage.ElapsedMS -= 2
	if _, err := SealExecutionReceipt(&receipt); err == nil {
		t.Fatal("elapsed wall mismatch accepted")
	}
}

func TestClosureCardinalityBoundaries(t *testing.T) {
	valid := &KernelOperationalReferenceClosure{
		Artifacts:             make([]ArtifactRef, maxArtifacts),
		ArtifactReceipts:      make([]ArtifactReceipt, maxArtifactReceipts),
		CapabilityInvocations: make([]CapabilityInvocation, maxInvocations),
		InteractionEvents:     make([]InteractionEvent, maxEvents),
		ExecutionReceipts:     make([]ExecutionReceipt, maxExecutionReceipts)}
	if err := validateClosureCounts(valid); err != nil {
		t.Fatal(err)
	}
	cases := []*KernelOperationalReferenceClosure{}
	for index := 0; index < 5; index++ {
		copy := *valid
		switch index {
		case 0:
			copy.Artifacts = make([]ArtifactRef, maxArtifacts+1)
		case 1:
			copy.ArtifactReceipts = make([]ArtifactReceipt, maxArtifactReceipts+1)
		case 2:
			copy.CapabilityInvocations = make([]CapabilityInvocation, maxInvocations+1)
		case 3:
			copy.InteractionEvents = make([]InteractionEvent, maxEvents+1)
		case 4:
			copy.ExecutionReceipts = make([]ExecutionReceipt, maxExecutionReceipts+1)
		}
		cases = append(cases, &copy)
	}
	for index, candidate := range cases {
		if err := validateClosureCounts(candidate); err == nil {
			t.Fatalf("cardinality N+1 case %d accepted", index)
		}
	}
}

func futureArtifactCandidate(t *testing.T, future bool) *KernelOperationalReferenceClosure {
	t.Helper()
	golden, _ := goldenClosure(t)
	invocation := golden.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.InputArtifactReceiptRefs = []ArtifactReceiptRef{}
	sealedInvocation, err := SealCapabilityInvocation(&invocation)
	if err != nil {
		t.Fatal(err)
	}
	output := golden.ArtifactReceipts[0]
	output.ArtifactReceiptID, output.ArtifactReceiptSHA256 = "", ""
	invocationRef := refInvocation(*sealedInvocation)
	output.ProducerInvocationRef = &invocationRef
	sealedOutput, err := SealArtifactReceipt(&output)
	if err != nil {
		t.Fatal(err)
	}
	event := golden.InteractionEvents[0]
	event.EventID, event.EventSHA256 = "", ""
	event.InvocationRef, event.ArtifactRefs = invocationRef, []ArtifactRef{sealedOutput.Artifact}
	event.OccurredAtUnixMS = sealedOutput.CreatedAtUnixMS
	if future {
		event.OccurredAtUnixMS--
	}
	sealedEvent, err := SealInteractionEvent(&event)
	if err != nil {
		t.Fatal(err)
	}
	return assembleSingleAttempt(t, golden, *sealedInvocation, *sealedOutput, *sealedEvent)
}

func assembleSingleAttempt(t *testing.T, golden *KernelOperationalReferenceClosure,
	invocation CapabilityInvocation, output ArtifactReceipt,
	event InteractionEvent) *KernelOperationalReferenceClosure {
	t.Helper()
	receipt := golden.ExecutionReceipts[0]
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.InvocationRef = refInvocation(invocation)
	receipt.EventRefs, receipt.InputArtifacts = []InteractionEventRef{refEvent(event)}, []ArtifactRef{}
	receipt.OutputArtifactReceiptRefs = []ArtifactReceiptRef{refReceipt(output)}
	sealedReceipt, err := SealExecutionReceipt(&receipt)
	if err != nil {
		t.Fatal(err)
	}
	candidate := *golden
	candidate.ClosureID, candidate.ClosureSHA256 = "", ""
	candidate.Artifacts = []ArtifactRef{output.Artifact}
	candidate.ArtifactReceipts = []ArtifactReceipt{output}
	candidate.CapabilityInvocations = []CapabilityInvocation{invocation}
	candidate.InteractionEvents = []InteractionEvent{event}
	candidate.ExecutionReceipts = []ExecutionReceipt{*sealedReceipt}
	return &candidate
}

func TestEventArtifactRequiresNonfutureReceipt(t *testing.T) {
	boundary, err := SealClosure(futureArtifactCandidate(t, false))
	if err != nil || boundary == nil {
		t.Fatalf("equal timestamp boundary: %v", err)
	}
	if _, err := SealClosure(futureArtifactCandidate(t, true)); err == nil {
		t.Fatal("event accepted an ArtifactReceipt created in its future")
	}
}

func TestReferenceGraphRejectsProjectionRetryAndContextDrift(t *testing.T) {
	base, _ := goldenClosure(t)
	cases := coreGraphDrifts(base)
	for index, candidate := range cases {
		if err := validateReferenceGraph(candidate); err == nil {
			t.Fatalf("reference graph drift %d accepted", index)
		}
	}
}

func TestEventTimesAreNondecreasingWithEqualityAllowed(t *testing.T) {
	base, _ := goldenClosure(t)
	equal, _ := cloneValue(base)
	equal.InteractionEvents[0].OccurredAtUnixMS = equal.InteractionEvents[1].OccurredAtUnixMS
	if err := validateReferenceGraph(equal); err != nil {
		t.Fatalf("equal adjacent event times rejected: %v", err)
	}
	reversed, _ := cloneValue(base)
	reversed.InteractionEvents[1].OccurredAtUnixMS =
		reversed.InteractionEvents[0].OccurredAtUnixMS - 1
	if err := validateReferenceGraph(reversed); err == nil {
		t.Fatal("reverse-causal event time accepted")
	}
}

func coreGraphDrifts(base *KernelOperationalReferenceClosure) []*KernelOperationalReferenceClosure {
	cases := make([]*KernelOperationalReferenceClosure, 0, 8)
	first, _ := cloneValue(base)
	first.ExecutionReceipts[0].InputArtifacts = []ArtifactRef{}
	cases = append(cases, first)
	second, _ := cloneValue(base)
	second.CapabilityInvocations[0].DeclaredOutputSlots = []string{}
	cases = append(cases, second)
	third, _ := cloneValue(base)
	third.ExecutionReceipts[0].Outcome, third.ExecutionReceipts[0].ReasonCodes = "succeeded", []string{}
	cases = append(cases, third)
	fourth, _ := cloneValue(base)
	fourth.InteractionEvents[0].Bindings.PolicySHA256 = strings.Repeat("e", 64)
	cases = append(cases, fourth)
	fifth, _ := cloneValue(base)
	digest := strings.Repeat("d", 64)
	fifth.CapabilityInvocations[0].InputArtifactReceiptRefs[0] = ArtifactReceiptRef{
		ArtifactReceiptID: artifactReceiptPrefix + digest, ArtifactReceiptSHA256: digest}
	cases = append(cases, fifth)
	sixth, _ := cloneValue(base)
	orphan := sixth.ArtifactReceipts[2]
	orphan.ArtifactReceiptID, orphan.ArtifactReceiptSHA256 = artifactReceiptPrefix+digest, digest
	sixth.ArtifactReceipts = append(sixth.ArtifactReceipts, orphan)
	cases = append(cases, sixth)
	seventh, _ := cloneValue(base)
	seventh.InteractionEvents[1].CausationEventRef = nil
	cases = append(cases, seventh)
	eighth, _ := cloneValue(base)
	eighth.ArtifactReceipts[0].CreatedAtUnixMS = eighth.ExecutionReceipts[0].StartedAtUnixMS - 1
	cases = append(cases, eighth)
	return cases
}
