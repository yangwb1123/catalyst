package decisioncapsulecontract

import (
	"bytes"
	"fmt"
	"sort"
	"strings"
	"testing"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func worstEscapedArtifact(index int) op.ArtifactRef {
	kind := strings.Repeat(`"`, maxShortBytes)
	if index == 0 {
		kind = "reflection_report"
	}
	return op.ArtifactRef{ArtifactKind: kind,
		ArtifactRef:    fmt.Sprintf("%03d", index) + strings.Repeat(`\`, maxReferenceBytes-3),
		ArtifactSHA256: fmt.Sprintf("%064x", index+1)}
}

func maxReceiptSets(t *testing.T, decision *kd.KernelDecisionReferenceClosure) (
	[]op.ArtifactReceipt, []op.ArtifactReceipt,
) {
	t.Helper()
	var inputBase, outputBase op.ArtifactReceipt
	for _, receipt := range decision.OperationalClosure.ArtifactReceipts {
		if receipt.ReceiptRole == "declared_input" && inputBase.ArtifactReceiptID == "" {
			inputBase = receipt
		}
		if receipt.ReceiptRole == "declared_output" && outputBase.ArtifactReceiptID == "" {
			outputBase = receipt
		}
	}
	inputs, outputs := make([]op.ArtifactReceipt, 0, 32), make([]op.ArtifactReceipt, 0, 32)
	for index := 0; index < 32; index++ {
		member := inputBase
		member.ArtifactReceiptID, member.ArtifactReceiptSHA256 = "", ""
		member.Artifact, member.Slot = worstEscapedArtifact(index), fmt.Sprintf("input-%02d", index)
		sealed, err := op.SealArtifactReceipt(&member)
		if err != nil {
			t.Fatal(err)
		}
		inputs = append(inputs, *sealed)
	}
	for index := 32; index < 64; index++ {
		member := outputBase
		member.ArtifactReceiptID, member.ArtifactReceiptSHA256 = "", ""
		member.Artifact, member.Slot = worstEscapedArtifact(index), fmt.Sprintf("output-%02d", index)
		outputs = append(outputs, member)
	}
	return inputs, outputs
}

func receiptRefs(values []op.ArtifactReceipt) []op.ArtifactReceiptRef {
	refs := make([]op.ArtifactReceiptRef, len(values))
	for index, value := range values {
		refs[index] = op.ArtifactReceiptRef{ArtifactReceiptID: value.ArtifactReceiptID,
			ArtifactReceiptSHA256: value.ArtifactReceiptSHA256}
	}
	sort.Slice(refs, func(left, right int) bool {
		leftRaw, _ := op.CanonicalJSON(&refs[left])
		rightRaw, _ := op.CanonicalJSON(&refs[right])
		return bytes.Compare(leftRaw, rightRaw) < 0
	})
	return refs
}

func maxDecisionTransaction(t *testing.T, decision *kd.KernelDecisionReferenceClosure,
	inputs []op.ArtifactReceipt) *kd.DecisionTransaction {
	t.Helper()
	transaction := decision.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	transaction.ReadArtifactReceiptRefs = receiptRefs(inputs)
	transaction.WriteSlots = make([]string, 32)
	for index := range transaction.WriteSlots {
		transaction.WriteSlots[index] = fmt.Sprintf("output-%02d", index+32)
	}
	sealed, err := kd.SealDecisionTransaction(&transaction)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func maxInvocation(t *testing.T, decision *kd.KernelDecisionReferenceClosure,
	transaction *kd.DecisionTransaction) *op.CapabilityInvocation {
	t.Helper()
	invocation := decision.OperationalClosure.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.CorrelationID = transaction.DecisionTransactionID
	invocation.DeclaredOutputSlots = append([]string{}, transaction.WriteSlots...)
	invocation.InputArtifactReceiptRefs = append([]op.ArtifactReceiptRef{},
		transaction.ReadArtifactReceiptRefs...)
	sealed, err := op.SealCapabilityInvocation(&invocation)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func sealMaxOutputs(t *testing.T, pending []op.ArtifactReceipt,
	invocation *op.CapabilityInvocation) []op.ArtifactReceipt {
	t.Helper()
	invocationRef := op.CapabilityInvocationRef{InvocationID: invocation.InvocationID,
		InvocationSHA256: invocation.InvocationSHA256}
	outputs := make([]op.ArtifactReceipt, len(pending))
	for index := range pending {
		pending[index].ProducerInvocationRef = &invocationRef
		sealed, err := op.SealArtifactReceipt(&pending[index])
		if err != nil {
			t.Fatal(err)
		}
		outputs[index] = *sealed
	}
	return outputs
}

func sortArtifacts(values []op.ArtifactRef) {
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := op.CanonicalJSON(&values[left])
		rightRaw, _ := op.CanonicalJSON(&values[right])
		return bytes.Compare(leftRaw, rightRaw) < 0
	})
}

func maxExecutionReceipt(t *testing.T, decision *kd.KernelDecisionReferenceClosure,
	transaction *kd.DecisionTransaction, invocation *op.CapabilityInvocation,
	inputs, outputs []op.ArtifactReceipt) *op.ExecutionReceipt {
	t.Helper()
	receipt := decision.OperationalClosure.ExecutionReceipts[0]
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.CorrelationID = transaction.DecisionTransactionID
	receipt.EventRefs = []op.InteractionEventRef{}
	receipt.InputArtifacts = make([]op.ArtifactRef, len(inputs))
	for index := range inputs {
		receipt.InputArtifacts[index] = inputs[index].Artifact
	}
	sortArtifacts(receipt.InputArtifacts)
	receipt.InvocationRef = op.CapabilityInvocationRef{InvocationID: invocation.InvocationID,
		InvocationSHA256: invocation.InvocationSHA256}
	receipt.Outcome, receipt.ReasonCodes = "succeeded", []string{}
	receipt.OutputArtifactReceiptRefs = receiptRefs(outputs)
	sealed, err := op.SealExecutionReceipt(&receipt)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func maxOperationalClosure(t *testing.T, decision *kd.KernelDecisionReferenceClosure,
	inputs, outputs []op.ArtifactReceipt, invocation *op.CapabilityInvocation,
	receipt *op.ExecutionReceipt) *op.KernelOperationalReferenceClosure {
	t.Helper()
	closure := decision.OperationalClosure
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	closure.ArtifactReceipts = append(append([]op.ArtifactReceipt{}, inputs...), outputs...)
	sort.Slice(closure.ArtifactReceipts, func(left, right int) bool {
		return closure.ArtifactReceipts[left].ArtifactReceiptID < closure.ArtifactReceipts[right].ArtifactReceiptID
	})
	closure.Artifacts = make([]op.ArtifactRef, 0, len(inputs)+len(outputs))
	for _, item := range append(append([]op.ArtifactReceipt{}, inputs...), outputs...) {
		closure.Artifacts = append(closure.Artifacts, item.Artifact)
	}
	sortArtifacts(closure.Artifacts)
	closure.CapabilityInvocations = []op.CapabilityInvocation{*invocation}
	closure.ExecutionReceipts = []op.ExecutionReceipt{*receipt}
	closure.InteractionEvents = []op.InteractionEvent{}
	sealed, err := op.SealClosure(&closure)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func worstEscapedDecisionClosure(t *testing.T,
	decision *kd.KernelDecisionReferenceClosure) *kd.KernelDecisionReferenceClosure {
	t.Helper()
	inputs, pending := maxReceiptSets(t, decision)
	transaction := maxDecisionTransaction(t, decision, inputs)
	invocation := maxInvocation(t, decision, transaction)
	outputs := sealMaxOutputs(t, pending, invocation)
	receipt := maxExecutionReceipt(t, decision, transaction, invocation, inputs, outputs)
	operational := maxOperationalClosure(t, decision, inputs, outputs, invocation, receipt)
	closure := *decision
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	closure.DecisionTransaction, closure.OperationalClosure = *transaction, *operational
	closure.CognitiveAtoms = make([]kd.CognitiveAtom, 0)
	for _, atom := range decision.CognitiveAtoms {
		if atom.Source.SourcePhase == "predecision" {
			closure.CognitiveAtoms = append(closure.CognitiveAtoms, atom)
		}
	}
	sealed, err := kd.SealClosure(&closure)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func TestFullSemanticProjectionAcceptsSixtyFourWorstEscapedArtifacts(t *testing.T) {
	outer, _ := golden(t)
	decision := worstEscapedDecisionClosure(t, &outer.DecisionCapsule.DecisionClosure)
	if len(decision.OperationalClosure.Artifacts) != 64 {
		t.Fatal("worst-valid closure does not contain 64 artifacts")
	}
	reflectionCount := 0
	for _, artifact := range decision.OperationalClosure.Artifacts {
		if artifact.ArtifactKind == "reflection_report" {
			reflectionCount++
		}
	}
	if reflectionCount != 1 {
		t.Fatal("upstream ArtifactRef kinds must remain opaque and uninterpreted")
	}
	manifest, err := DeriveStructuralReplayManifest(decision)
	if err != nil || len(manifest.ArtifactRefs) != 64 {
		t.Fatalf("worst-valid manifest: %v", err)
	}
	capsule, err := DeriveDecisionCapsule(decision)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := DeriveStructuralReplayClosure(capsule, []op.ArtifactRef{})
	if err != nil {
		t.Fatal(err)
	}
	if err := ValidateStructuralReplayClosure(closure); err != nil {
		t.Fatal(err)
	}
	raw, err := canonicalBytes(manifest, maxManifestBytes)
	if err != nil || len(raw) > maxManifestBytes {
		t.Fatalf("worst-valid manifest bytes=%d err=%v", len(raw), err)
	}
}

func TestManifestIndependentShapeAndSemanticEnvelopeBounds(t *testing.T) {
	outer, _ := golden(t)
	base := &outer.DecisionCapsule.ReplayManifest
	shape, _ := cloneValue(base, maxManifestBytes)
	shape.ManifestID, shape.ManifestSHA256 = "", ""
	shape.ArtifactRefs = worstArtifacts(256)
	if err := validateManifestShape(shape, true); err != nil {
		t.Fatal(err)
	}
	raw, err := CanonicalJSON(shape)
	if err != nil || len(raw) != 2_218_274 {
		t.Fatalf("256-ref shape size=%d err=%v", len(raw), err)
	}
	shape.ArtifactRefs = append(shape.ArtifactRefs, shape.ArtifactRefs[len(shape.ArtifactRefs)-1])
	if err := validateManifestShape(shape, true); err == nil {
		t.Fatal("257 ArtifactRefs accepted")
	}
	envelope := manifestEnvelope(base)
	if err := validateManifestShape(envelope, true); err != nil {
		t.Fatal(err)
	}
	if raw, _ := CanonicalJSON(envelope); len(raw) != 684_285 {
		t.Fatalf("semantic manifest upper envelope = %d", len(raw))
	}
	sealedEnvelope := *envelope
	sealedEnvelope.ManifestID = base.ManifestID
	sealedEnvelope.ManifestSHA256 = base.ManifestSHA256
	if raw, _ := CanonicalJSON(&sealedEnvelope); len(raw) != 684_440 {
		t.Fatalf("sealed semantic manifest upper envelope = %d", len(raw))
	}
}
