package decisioncapsulecontract

import (
	"encoding/json"
	"reflect"
	"sort"
	"strings"
	"testing"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

type retryMappings struct {
	artifactReceipts map[string]op.ArtifactReceiptRef
	events           map[string]op.InteractionEventRef
	sourceRefs       map[string]json.RawMessage
}

func newRetryMappings() *retryMappings {
	return &retryMappings{artifactReceipts: make(map[string]op.ArtifactReceiptRef),
		events:     make(map[string]op.InteractionEventRef),
		sourceRefs: make(map[string]json.RawMessage)}
}

func addSourceRef(t *testing.T, mappings *retryMappings, oldID string, value any) {
	t.Helper()
	raw, err := op.CanonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	mappings.sourceRefs[oldID] = append(json.RawMessage{}, raw...)
}

func lostFirstReceipt(t *testing.T, old op.ExecutionReceipt) *op.ExecutionReceipt {
	t.Helper()
	old.ExecutionReceiptID, old.ExecutionReceiptSHA256 = "", ""
	old.Outcome, old.ReasonCodes = "lost", []string{"fixture_lost"}
	sealed, err := op.SealExecutionReceipt(&old)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func retryInvocation(t *testing.T, old op.CapabilityInvocation,
	first *op.ExecutionReceipt) *op.CapabilityInvocation {
	t.Helper()
	old.InvocationID, old.InvocationSHA256 = "", ""
	old.PriorExecutionReceiptRef = &op.ExecutionReceiptRef{
		ExecutionReceiptID: first.ExecutionReceiptID, ExecutionReceiptSHA256: first.ExecutionReceiptSHA256}
	sealed, err := op.SealCapabilityInvocation(&old)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func resealRetryArtifactReceipts(t *testing.T, closure *op.KernelOperationalReferenceClosure,
	oldRef, newRef op.CapabilityInvocationRef, mappings *retryMappings) {
	t.Helper()
	for index := range closure.ArtifactReceipts {
		old := closure.ArtifactReceipts[index]
		if old.ProducerInvocationRef == nil || !reflect.DeepEqual(*old.ProducerInvocationRef, oldRef) {
			continue
		}
		oldID := old.ArtifactReceiptID
		old.ArtifactReceiptID, old.ArtifactReceiptSHA256 = "", ""
		old.ProducerInvocationRef = &newRef
		sealed, err := op.SealArtifactReceipt(&old)
		if err != nil {
			t.Fatal(err)
		}
		closure.ArtifactReceipts[index] = *sealed
		ref := op.ArtifactReceiptRef{ArtifactReceiptID: sealed.ArtifactReceiptID,
			ArtifactReceiptSHA256: sealed.ArtifactReceiptSHA256}
		mappings.artifactReceipts[oldID] = ref
		addSourceRef(t, mappings, oldID, &ref)
	}
}

func resealRetryEvents(t *testing.T, closure *op.KernelOperationalReferenceClosure,
	oldRef, newRef op.CapabilityInvocationRef, mappings *retryMappings) {
	t.Helper()
	var prior *op.InteractionEventRef
	for index := range closure.InteractionEvents {
		old := closure.InteractionEvents[index]
		if !reflect.DeepEqual(old.InvocationRef, oldRef) {
			continue
		}
		oldID := old.EventID
		old.EventID, old.EventSHA256 = "", ""
		old.InvocationRef, old.CausationEventRef = newRef, prior
		sealed, err := op.SealInteractionEvent(&old)
		if err != nil {
			t.Fatal(err)
		}
		closure.InteractionEvents[index] = *sealed
		ref := op.InteractionEventRef{EventID: sealed.EventID, EventSHA256: sealed.EventSHA256}
		mappings.events[oldID], prior = ref, &ref
		addSourceRef(t, mappings, oldID, &ref)
	}
}

func resealRetryExecution(t *testing.T, old op.ExecutionReceipt,
	first *op.ExecutionReceipt, invocation op.CapabilityInvocationRef,
	mappings *retryMappings) *op.ExecutionReceipt {
	t.Helper()
	oldID := old.ExecutionReceiptID
	old.ExecutionReceiptID, old.ExecutionReceiptSHA256 = "", ""
	old.InvocationRef = invocation
	old.PriorExecutionReceiptRef = &op.ExecutionReceiptRef{
		ExecutionReceiptID: first.ExecutionReceiptID, ExecutionReceiptSHA256: first.ExecutionReceiptSHA256}
	for index, reference := range old.EventRefs {
		if replacement, ok := mappings.events[reference.EventID]; ok {
			old.EventRefs[index] = replacement
		}
	}
	for index, reference := range old.OutputArtifactReceiptRefs {
		if replacement, ok := mappings.artifactReceipts[reference.ArtifactReceiptID]; ok {
			old.OutputArtifactReceiptRefs[index] = replacement
		}
	}
	sealed, err := op.SealExecutionReceipt(&old)
	if err != nil {
		t.Fatal(err)
	}
	ref := op.ExecutionReceiptRef{ExecutionReceiptID: sealed.ExecutionReceiptID,
		ExecutionReceiptSHA256: sealed.ExecutionReceiptSHA256}
	addSourceRef(t, mappings, oldID, &ref)
	return sealed
}

func lostOperationalClosure(t *testing.T, source *op.KernelOperationalReferenceClosure) (
	*op.KernelOperationalReferenceClosure, *retryMappings,
) {
	t.Helper()
	closure, err := cloneValue(source, maxCapsuleBytes)
	if err != nil {
		t.Fatal(err)
	}
	oldFirst, oldSecond := closure.ExecutionReceipts[0], closure.ExecutionReceipts[1]
	oldInvocation := closure.CapabilityInvocations[1]
	first := lostFirstReceipt(t, oldFirst)
	invocation := retryInvocation(t, oldInvocation, first)
	oldRef := op.CapabilityInvocationRef{InvocationID: oldInvocation.InvocationID,
		InvocationSHA256: oldInvocation.InvocationSHA256}
	newRef := op.CapabilityInvocationRef{InvocationID: invocation.InvocationID,
		InvocationSHA256: invocation.InvocationSHA256}
	mappings := newRetryMappings()
	addSourceRef(t, mappings, oldInvocation.InvocationID, &newRef)
	resealRetryArtifactReceipts(t, closure, oldRef, newRef, mappings)
	resealRetryEvents(t, closure, oldRef, newRef, mappings)
	second := resealRetryExecution(t, oldSecond, first, newRef, mappings)
	closure.CapabilityInvocations[1] = *invocation
	closure.ExecutionReceipts = []op.ExecutionReceipt{*first, *second}
	sort.Slice(closure.ArtifactReceipts, func(left, right int) bool {
		return closure.ArtifactReceipts[left].ArtifactReceiptID < closure.ArtifactReceipts[right].ArtifactReceiptID
	})
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	sealed, err := op.SealClosure(closure)
	if err != nil {
		t.Fatal(err)
	}
	return sealed, mappings
}

func sourceReferenceID(raw json.RawMessage) string {
	fields := make(map[string]string)
	if err := json.Unmarshal(raw, &fields); err != nil {
		return ""
	}
	for field, value := range fields {
		if strings.HasSuffix(field, "_id") {
			return value
		}
	}
	return ""
}

func lostDecisionClosure(t *testing.T,
	source *kd.KernelDecisionReferenceClosure) *kd.KernelDecisionReferenceClosure {
	t.Helper()
	closure, err := cloneValue(source, maxCapsuleBytes)
	if err != nil {
		t.Fatal(err)
	}
	operational, mappings := lostOperationalClosure(t, &closure.OperationalClosure)
	closure.OperationalClosure = *operational
	for index := range closure.CognitiveAtoms {
		atom := closure.CognitiveAtoms[index]
		replacement, ok := mappings.sourceRefs[sourceReferenceID(atom.Source.SourceRef)]
		if atom.Source.SourcePhase != "postdecision" || !ok {
			continue
		}
		atom.AtomID, atom.AtomSHA256 = "", ""
		atom.Source.SourceRef = append(json.RawMessage{}, replacement...)
		sealed, sealErr := kd.SealCognitiveAtom(&atom)
		if sealErr != nil {
			t.Fatal(sealErr)
		}
		closure.CognitiveAtoms[index] = *sealed
	}
	sort.Slice(closure.CognitiveAtoms, func(left, right int) bool {
		return closure.CognitiveAtoms[left].AtomID < closure.CognitiveAtoms[right].AtomID
	})
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	sealed, err := kd.SealClosure(closure)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func TestLostRetryAndSuccessAttemptsAreNeverTrimmed(t *testing.T) {
	goldenOuter, _ := golden(t)
	decision := lostDecisionClosure(t, &goldenOuter.DecisionCapsule.DecisionClosure)
	operational := &decision.OperationalClosure
	if operational.ExecutionReceipts[0].Outcome != "lost" ||
		operational.ExecutionReceipts[1].Outcome != "succeeded" ||
		operational.CapabilityInvocations[1].PriorExecutionReceiptRef == nil {
		t.Fatal("lost/retry/success graph was not retained")
	}
	manifest, err := DeriveStructuralReplayManifest(decision)
	if err != nil {
		t.Fatal(err)
	}
	if len(manifest.CapabilityInvocationRefs) != 2 || len(manifest.ExecutionReceiptRefs) != 2 {
		t.Fatal("manifest trimmed the lost or retry attempt")
	}
	capsule, err := DeriveDecisionCapsule(decision)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DeriveStructuralReplayClosure(capsule, []op.ArtifactRef{}); err != nil {
		t.Fatal(err)
	}
}

func TestCrossCapsuleBranchSubstitutionFails(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	otherDecision := lostDecisionClosure(t, &capsule.DecisionClosure)
	other, err := DeriveDecisionCapsule(otherDecision)
	if err != nil {
		t.Fatal(err)
	}
	if err := ValidateEvaluationBranch(&outer.EvaluationBranch, other); err == nil {
		t.Fatal("branch validated against a different capsule")
	}
	otherBranch, err := DeriveEvaluationBranch(other)
	if err != nil {
		t.Fatal(err)
	}
	changed, _ := cloneValue(outer, maxClosureBytes)
	changed.ClosureID, changed.ClosureSHA256 = "", ""
	changed.EvaluationBranch = *otherBranch
	if _, err := SealStructuralReplayClosure(changed); err == nil {
		t.Fatal("outer accepted another capsule's branch")
	}
}
