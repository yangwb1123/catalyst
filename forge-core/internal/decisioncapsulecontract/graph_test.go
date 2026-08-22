package decisioncapsulecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"sort"
	"testing"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

var exactReplayAttestationFields = []string{
	"approval_authentication_attestation", "attempt_history_completeness_attestation",
	"authority_attestation", "authorization_attestation", "binding_authentication_attestation",
	"capsule_completeness_attestation", "cas_attestation", "completion_attestation",
	"content_provenance_attestation", "effect_attestation", "evaluation_execution_attestation",
	"evaluator_independence_attestation", "event_append_attestation", "execution_attestation",
	"external_history_resolution_attestation", "grant_authentication_attestation",
	"hard_guard_attestation", "instruction_attestation", "outcome_attestation",
	"permission_attestation", "persistence_attestation", "principal_authentication_attestation",
	"reflection_completeness_attestation", "replay_equivalence_attestation",
	"result_authentication_attestation", "rule_evaluation_attestation",
	"source_resolution_attestation", "transition_attestation", "truth_attestation",
	"usage_measurement_attestation", "verifier_independence_attestation",
	"world_state_resolution_attestation",
}

func jsonFields(value any) []string {
	typeOfValue := reflect.TypeOf(value)
	fields := make([]string, 0, typeOfValue.NumField())
	for index := 0; index < typeOfValue.NumField(); index++ {
		fields = append(fields, typeOfValue.Field(index).Tag.Get("json"))
	}
	sort.Strings(fields)
	return fields
}

func TestFourObjectsHaveExactFrozenFields(t *testing.T) {
	cases := []struct {
		value    any
		expected []string
	}{
		{StructuralReplayManifest{}, []string{"api_version", "artifact_receipt_refs", "artifact_refs", "attestations", "canonicalization", "capability_invocation_refs", "decision_closure_ref", "decision_transaction_ref", "effect_replay_allowed", "execution_receipt_refs", "history_rewrite_allowed", "interaction_event_refs", "kind", "manifest_id", "manifest_sha256", "operational_closure_ref", "postdecision_atom_refs", "predecision_atom_refs", "replay_mode"}},
		{DecisionCapsule{}, []string{"api_version", "attestations", "canonicalization", "capsule_id", "capsule_mode", "capsule_sha256", "decision_closure", "kind", "replay_manifest", "result"}},
		{EvaluationBranch{}, []string{"api_version", "attestations", "branch_id", "branch_mode", "branch_sha256", "canonicalization", "capsule_ref", "comparison_result", "decision_closure_ref", "effect_replay_allowed", "history_rewrite_allowed", "kind", "manifest_ref"}},
		{StructuralReplayClosure{}, []string{"api_version", "attestations", "canonicalization", "closure_id", "closure_sha256", "decision_capsule", "evaluation_branch", "kind", "reflection_report_artifact_refs", "result"}},
	}
	for _, test := range cases {
		if fields := jsonFields(test.value); !reflect.DeepEqual(fields, test.expected) {
			t.Errorf("%T fields = %v", test.value, fields)
		}
	}
	if fields := jsonFields(ReplayAttestations{}); !reflect.DeepEqual(fields, exactReplayAttestationFields) {
		t.Fatalf("replay attestations fields = %v", fields)
	}
}

type manifestInventoryCase struct {
	name  string
	apply func(*StructuralReplayManifest, string)
}

func mutateInventory[T any](values []T, mutation string, foreign func(T) T) []T {
	result := append([]T{}, values...)
	switch mutation {
	case "omit":
		return result[:len(result)-1]
	case "duplicate":
		return append(result, result[0])
	case "foreign":
		return append(result, foreign(result[0]))
	case "reorder":
		result[0], result[len(result)-1] = result[len(result)-1], result[0]
	}
	return result
}

func foreignArtifact(value op.ArtifactRef) op.ArtifactRef {
	value.ArtifactRef += "/foreign"
	value.ArtifactSHA256 = fmt.Sprintf("%064x", 0)
	return value
}

func manifestInventoryCases() []manifestInventoryCase {
	zero := func(value string) string { return value[:len(value)-64] + fmt.Sprintf("%064x", 0) }
	return []manifestInventoryCase{
		{"artifact_refs", func(value *StructuralReplayManifest, mutation string) {
			value.ArtifactRefs = mutateInventory(value.ArtifactRefs, mutation, foreignArtifact)
		}},
		{"artifact_receipt_refs", func(value *StructuralReplayManifest, mutation string) {
			value.ArtifactReceiptRefs = mutateInventory(value.ArtifactReceiptRefs, mutation, func(item op.ArtifactReceiptRef) op.ArtifactReceiptRef {
				item.ArtifactReceiptSHA256 = fmt.Sprintf("%064x", 0)
				item.ArtifactReceiptID = zero(item.ArtifactReceiptID)
				return item
			})
		}},
		{"capability_invocation_refs", func(value *StructuralReplayManifest, mutation string) {
			value.CapabilityInvocationRefs = mutateInventory(value.CapabilityInvocationRefs, mutation, func(item op.CapabilityInvocationRef) op.CapabilityInvocationRef {
				item.InvocationSHA256 = fmt.Sprintf("%064x", 0)
				item.InvocationID = zero(item.InvocationID)
				return item
			})
		}},
		{"interaction_event_refs", func(value *StructuralReplayManifest, mutation string) {
			value.InteractionEventRefs = mutateInventory(value.InteractionEventRefs, mutation, func(item op.InteractionEventRef) op.InteractionEventRef {
				item.EventSHA256 = fmt.Sprintf("%064x", 0)
				item.EventID = zero(item.EventID)
				return item
			})
		}},
		{"execution_receipt_refs", func(value *StructuralReplayManifest, mutation string) {
			value.ExecutionReceiptRefs = mutateInventory(value.ExecutionReceiptRefs, mutation, func(item op.ExecutionReceiptRef) op.ExecutionReceiptRef {
				item.ExecutionReceiptSHA256 = fmt.Sprintf("%064x", 0)
				item.ExecutionReceiptID = zero(item.ExecutionReceiptID)
				return item
			})
		}},
		{"predecision_atom_refs", func(value *StructuralReplayManifest, mutation string) {
			value.PredecisionAtomRefs = mutateInventory(value.PredecisionAtomRefs, mutation, func(item kd.AtomRef) kd.AtomRef {
				item.AtomSHA256 = fmt.Sprintf("%064x", 0)
				item.AtomID = zero(item.AtomID)
				return item
			})
		}},
		{"postdecision_atom_refs", func(value *StructuralReplayManifest, mutation string) {
			value.PostdecisionAtomRefs = mutateInventory(value.PostdecisionAtomRefs, mutation, func(item kd.AtomRef) kd.AtomRef {
				item.AtomSHA256 = fmt.Sprintf("%064x", 0)
				item.AtomID = zero(item.AtomID)
				return item
			})
		}},
	}
}

func TestManifestRejectsEveryInventoryOmissionDuplicateForeignAndReorder(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	mutations := []string{"omit", "duplicate", "foreign", "reorder"}
	for _, inventory := range manifestInventoryCases() {
		for _, mutation := range mutations {
			candidate, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
			candidate.ManifestID, candidate.ManifestSHA256 = "", ""
			inventory.apply(candidate, mutation)
			if _, err := SealStructuralReplayManifest(candidate, &capsule.DecisionClosure); err == nil {
				t.Errorf("%s %s accepted", inventory.name, mutation)
			}
		}
	}
}

func TestManifestProjectionKeepsEveryAttemptAndAtomPartition(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	operational := &capsule.DecisionClosure.OperationalClosure
	manifest := &capsule.ReplayManifest
	counts := []int{len(manifest.ArtifactRefs), len(manifest.ArtifactReceiptRefs),
		len(manifest.CapabilityInvocationRefs), len(manifest.InteractionEventRefs),
		len(manifest.ExecutionReceiptRefs)}
	expected := []int{len(operational.Artifacts), len(operational.ArtifactReceipts),
		len(operational.CapabilityInvocations), len(operational.InteractionEvents),
		len(operational.ExecutionReceipts)}
	if !reflect.DeepEqual(counts, expected) {
		t.Fatalf("manifest inventory counts %v != embedded counts %v", counts, expected)
	}
	if operational.ExecutionReceipts[0].Outcome != "failed" ||
		operational.ExecutionReceipts[1].Outcome != "succeeded" || len(manifest.ExecutionReceiptRefs) != 2 {
		t.Fatal("failed and successful attempts were not both retained")
	}
	candidate, _ := cloneValue(manifest, maxManifestBytes)
	candidate.ManifestID, candidate.ManifestSHA256 = "", ""
	moved := candidate.PredecisionAtomRefs[len(candidate.PredecisionAtomRefs)-1]
	candidate.PredecisionAtomRefs = candidate.PredecisionAtomRefs[:len(candidate.PredecisionAtomRefs)-1]
	candidate.PostdecisionAtomRefs = append(candidate.PostdecisionAtomRefs, moved)
	if _, err := SealStructuralReplayManifest(candidate, &capsule.DecisionClosure); err == nil {
		t.Fatal("cross-phase atom movement accepted")
	}
}

type rawDecoderCase struct {
	name   string
	raw    []byte
	decode func([]byte) error
}

func replayDecoderCases(t *testing.T) []rawDecoderCase {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifestRaw, _ := CanonicalJSON(&capsule.ReplayManifest)
	capsuleRaw, _ := CanonicalJSON(capsule)
	branchRaw, _ := CanonicalJSON(&outer.EvaluationBranch)
	outerRaw, _ := CanonicalJSON(outer)
	return []rawDecoderCase{
		{"manifest", manifestRaw, func(raw []byte) error {
			_, err := DecodeStructuralReplayManifest(raw, &capsule.DecisionClosure)
			return err
		}},
		{"capsule", capsuleRaw, func(raw []byte) error { _, err := DecodeDecisionCapsule(raw); return err }},
		{"branch", branchRaw, func(raw []byte) error { _, err := DecodeEvaluationBranch(raw, capsule); return err }},
		{"outer", outerRaw, func(raw []byte) error { _, err := DecodeStructuralReplayClosure(raw); return err }},
	}
}

func TestEveryReplayAttestationRejectsTrueAndNumericZero(t *testing.T) {
	for _, document := range replayDecoderCases(t) {
		for _, field := range exactReplayAttestationFields {
			needle := []byte(fmt.Sprintf(`"%s":false`, field))
			if !bytes.Contains(document.raw, needle) {
				t.Fatalf("%s lacks %s", document.name, field)
			}
			for _, replacement := range []string{"true", "0"} {
				changed := bytes.Replace(document.raw, needle,
					[]byte(fmt.Sprintf(`"%s":%s`, field, replacement)), 1)
				if err := document.decode(changed); err == nil {
					t.Errorf("%s accepted %s=%s", document.name, field, replacement)
				}
			}
		}
	}
}

func mutateTopLevelWire(t *testing.T, raw []byte, mutation string) []byte {
	t.Helper()
	fields := make(map[string]json.RawMessage)
	if err := json.Unmarshal(raw, &fields); err != nil {
		t.Fatal(err)
	}
	switch mutation {
	case "missing":
		delete(fields, "api_version")
	case "unknown":
		fields["unknown"] = json.RawMessage("null")
	case "bad_kind":
		fields["kind"] = json.RawMessage("[]")
	}
	changed, err := CanonicalJSON(fields)
	if err != nil {
		t.Fatal(err)
	}
	return changed
}

func TestEveryDecoderRejectsMissingUnknownAndWrongTypedFields(t *testing.T) {
	for _, document := range replayDecoderCases(t) {
		for _, mutation := range []string{"missing", "unknown", "bad_kind"} {
			changed := mutateTopLevelWire(t, document.raw, mutation)
			if err := document.decode(changed); err == nil {
				t.Errorf("%s accepted %s top-level field mutation", document.name, mutation)
			}
		}
	}
}

func reflectionRefs(count int) []op.ArtifactRef {
	values := make([]op.ArtifactRef, count)
	for index := range values {
		values[index] = op.ArtifactRef{ArtifactKind: "reflection_report",
			ArtifactRef:    fmt.Sprintf("fixture/reflection/%02d", index),
			ArtifactSHA256: fmt.Sprintf("%064x", index+1)}
	}
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := op.CanonicalJSON(&values[left])
		rightRaw, _ := op.CanonicalJSON(&values[right])
		return bytes.Compare(leftRaw, rightRaw) < 0
	})
	return values
}

func TestReflectionRefsAcceptZeroAndThirtyTwoAndRejectDrift(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	for _, count := range []int{0, 32} {
		if _, err := DeriveStructuralReplayClosure(capsule, reflectionRefs(count)); err != nil {
			t.Errorf("reflection count %d: %v", count, err)
		}
	}
	invalid := [][]op.ArtifactRef{reflectionRefs(33), reflectionRefs(1),
		append(reflectionRefs(1), reflectionRefs(1)[0]), reflectionRefs(2)}
	invalid[1][0].ArtifactKind = "other"
	invalid[3][0], invalid[3][1] = invalid[3][1], invalid[3][0]
	for index, refs := range invalid {
		if _, err := DeriveStructuralReplayClosure(capsule, refs); err == nil {
			t.Errorf("invalid reflection case %d accepted", index)
		}
	}
}

func TestBranchAndOuterAreExactDependencyBindings(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	for _, mutate := range []func(*EvaluationBranch){
		func(value *EvaluationBranch) {
			value.CapsuleRef = CapsuleRef{CapsuleID: capsulePrefix + fmt.Sprintf("%064x", 0), CapsuleSHA256: fmt.Sprintf("%064x", 0)}
		},
		func(value *EvaluationBranch) {
			value.DecisionClosureRef = ClosureRef{ClosureID: decisionClosurePrefix + fmt.Sprintf("%064x", 0), ClosureSHA256: fmt.Sprintf("%064x", 0)}
		},
		func(value *EvaluationBranch) {
			value.ManifestRef = ManifestRef{ManifestID: manifestPrefix + fmt.Sprintf("%064x", 0), ManifestSHA256: fmt.Sprintf("%064x", 0)}
		},
	} {
		candidate, _ := cloneValue(&outer.EvaluationBranch, maxBranchBytes)
		candidate.BranchID, candidate.BranchSHA256 = "", ""
		mutate(candidate)
		if _, err := SealEvaluationBranch(candidate, capsule); err == nil {
			t.Fatal("foreign branch binding accepted")
		}
	}
	changed, _ := cloneValue(outer, maxClosureBytes)
	changed.ClosureID, changed.ClosureSHA256 = "", ""
	changed.EvaluationBranch.ManifestRef.ManifestSHA256 = fmt.Sprintf("%064x", 0)
	if _, err := SealStructuralReplayClosure(changed); err == nil {
		t.Fatal("outer accepted a branch not bound to its embedded capsule")
	}
}
