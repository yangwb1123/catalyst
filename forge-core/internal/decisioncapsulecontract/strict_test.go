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

func TestStrictWireRejectsFramingDuplicateUTF8FloatAndDepth(t *testing.T) {
	_, physical := golden(t)
	raw := physical[:len(physical)-1]
	invalidUTF8 := append([]byte(nil), raw...)
	invalidUTF8[20] = 0xff
	duplicate := bytes.Replace(raw, []byte(`{"api_version":`),
		[]byte(`{"api_version":"x","api_version":`), 1)
	float := bytes.Replace(raw, []byte(`"effect_replay_allowed":false`),
		[]byte(`"effect_replay_allowed":0.0`), 1)
	depth := bytes.Replace(raw,
		[]byte(`"comparison_result":"EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY"`),
		[]byte(`"comparison_result":[[[[[[[[[[[[[[[[["x"]]]]]]]]]]]]]]]]]`), 1)
	cases := [][]byte{physical, append([]byte(" "), raw...), append(raw, ' '),
		duplicate, invalidUTF8, float, depth}
	for index, changed := range cases {
		if _, err := DecodeStructuralReplayClosure(changed); err == nil {
			t.Errorf("wire mutation %d accepted", index)
		}
	}
}

func TestPublicCanonicalJSONHasExactTwentyEightMiBBoundary(t *testing.T) {
	maximum := strings.Repeat("x", maxStringBytes)
	value := make([][]string, 7)
	for row := range value {
		value[row] = make([]string, maxArrayItems)
		for column := range value[row] {
			value[row][column] = maximum
		}
	}
	value[len(value)-1][maxArrayItems-1] = strings.Repeat("x", 10_993)
	raw, err := CanonicalJSON(value)
	if err != nil || len(raw) != maxClosureBytes {
		t.Fatalf("exact 28 MiB canonical value: size=%d err=%v", len(raw), err)
	}
	overflow := append(value, append([]string{}, value[0]...))
	if _, err := measureTypedJSON(overflow, maxClosureBytes); err == nil {
		t.Fatal("array comma overflow was lost before the next child measurement")
	}
	value[len(value)-1][maxArrayItems-1] += "x"
	if _, err := CanonicalJSON(value); err == nil {
		t.Fatal("28 MiB plus one canonical value accepted")
	}
	if _, err := CanonicalJSON(overflow); err == nil {
		t.Fatal("large legal-shape aggregate bypassed bounded encoder")
	}
}

type cyclicJSONNode struct {
	Next *cyclicJSONNode `json:"next"`
}

type digestIdentityCase struct {
	name    string
	maximum int
	digest  func(string) error
}

func digestIdentityCases(t *testing.T) []digestIdentityCase {
	t.Helper()
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	return []digestIdentityCase{
		{"manifest", maxManifestBytes, func(id string) error {
			value := capsule.ReplayManifest
			value.ManifestID = id
			_, err := StructuralReplayManifestDigest(&value, &capsule.DecisionClosure)
			return err
		}},
		{"capsule", maxCapsuleBytes, func(id string) error {
			value := *capsule
			value.CapsuleID = id
			_, err := DecisionCapsuleDigest(&value)
			return err
		}},
		{"branch", maxBranchBytes, func(id string) error {
			value := outer.EvaluationBranch
			value.BranchID = id
			_, err := EvaluationBranchDigest(&value, capsule)
			return err
		}},
		{"outer", maxClosureBytes, func(id string) error {
			value := *outer
			value.ClosureID = id
			_, err := StructuralReplayClosureDigest(&value)
			return err
		}},
	}
}

func TestDigestsPreflightOriginalOwnIdentityStringBoundary(t *testing.T) {
	oversized := strings.Repeat("x", maxStringBytes+1)
	for _, test := range digestIdentityCases(t) {
		if err := test.digest(oversized); err == nil {
			t.Errorf("%s digest blanked a 16 KiB plus one own ID before preflight", test.name)
		}
	}
}

func TestDigestsPreflightOriginalDocumentCeiling(t *testing.T) {
	for _, test := range digestIdentityCases(t) {
		oversized := strings.Repeat("x", test.maximum+1)
		if err := test.digest(oversized); err == nil {
			t.Errorf("%s digest blanked an original document above its ceiling", test.name)
		}
	}
}

func TestCanonicalJSONAcceptsArraysAndFiniteAliasesAndRejectsCycles(t *testing.T) {
	if raw, err := CanonicalJSON([2]int{1, 2}); err != nil || string(raw) != "[1,2]" {
		t.Fatalf("array canonical JSON = %q, %v", raw, err)
	}
	finite := make([]any, 1)
	finite[0] = finite[:0]
	if raw, err := CanonicalJSON(finite); err != nil || string(raw) != "[[]]" {
		t.Fatalf("finite slice alias = %q, %v", raw, err)
	}
	cyclicMap := make(map[string]any)
	cyclicMap["x"] = cyclicMap
	cyclicSlice := make([]any, 1)
	cyclicSlice[0] = cyclicSlice
	cyclicPointer := &cyclicJSONNode{}
	cyclicPointer.Next = cyclicPointer
	for index, value := range []any{cyclicMap, cyclicSlice, cyclicPointer} {
		if _, err := CanonicalJSON(value); err == nil {
			t.Errorf("cyclic typed value %d accepted", index)
		}
	}
}

func TestProgrammaticSealUsesManifestAndBranchSpecificCeilings(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
	manifest.ManifestID, manifest.ManifestSHA256 = "", ""
	large := strings.Repeat("f", maxStringBytes)
	manifest.PredecisionAtomRefs = make([]kd.AtomRef, maxAtoms)
	for index := range manifest.PredecisionAtomRefs {
		manifest.PredecisionAtomRefs[index] = kd.AtomRef{AtomID: large, AtomSHA256: large}
	}
	if raw, err := canonicalBytes(manifest, maxClosureBytes); err != nil || len(raw) <= maxManifestBytes {
		t.Fatalf("manifest oversize construction: bytes=%d err=%v", len(raw), err)
	}
	if _, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure); err == nil {
		t.Fatal("programmatic manifest above 4 MiB accepted")
	}
	branch, _ := cloneValue(&outer.EvaluationBranch, maxBranchBytes)
	branch.BranchID, branch.BranchSHA256 = "", ""
	branch.CapsuleRef, branch.ManifestRef = CapsuleRef{large, large}, ManifestRef{large, large}
	branch.DecisionClosureRef = ClosureRef{large, large}
	if raw, err := canonicalBytes(branch, maxClosureBytes); err != nil || len(raw) <= maxBranchBytes {
		t.Fatalf("branch oversize construction: bytes=%d err=%v", len(raw), err)
	}
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("programmatic branch above 64 KiB accepted")
	}
}

func TestLocalInvalidityPreemptsLargeDependencyValidation(t *testing.T) {
	outer, _ := golden(t)
	assertError := func(label, contains string, err error) {
		t.Helper()
		if err == nil || !strings.Contains(err.Error(), contains) {
			t.Fatalf("%s: error=%v, want local %q failure", label, err, contains)
		}
	}

	invalidManifest := outer.DecisionCapsule.ReplayManifest
	invalidManifest.ReplayMode += "_drift"
	_, err := StructuralReplayManifestDigest(&invalidManifest, nil)
	assertError("manifest digest", "constants differ", err)
	assertError("manifest validate", "constants differ",
		ValidateStructuralReplayManifest(&invalidManifest, nil))
	invalidManifest.ManifestID, invalidManifest.ManifestSHA256 = "", ""
	_, err = SealStructuralReplayManifest(&invalidManifest, nil)
	assertError("manifest seal", "constants differ", err)

	invalidBranch := outer.EvaluationBranch
	invalidBranch.BranchMode += "_drift"
	_, err = EvaluationBranchDigest(&invalidBranch, nil)
	assertError("branch digest", "constants or comparison result differ", err)
	assertError("branch validate", "constants or comparison result differ",
		ValidateEvaluationBranch(&invalidBranch, nil))
	invalidBranch.BranchID, invalidBranch.BranchSHA256 = "", ""
	_, err = SealEvaluationBranch(&invalidBranch, nil)
	assertError("branch seal", "constants or comparison result differ", err)

	invalidOuter := *outer
	invalidOuter.ReflectionReportArtifactRefs = make([]op.ArtifactRef, 33)
	_, err = StructuralReplayClosureDigest(&invalidOuter)
	assertError("outer digest", "cardinality", err)
	assertError("outer validate", "cardinality", ValidateStructuralReplayClosure(&invalidOuter))
	invalidOuter.ClosureID, invalidOuter.ClosureSHA256 = "", ""
	_, err = SealStructuralReplayClosure(&invalidOuter)
	assertError("outer seal", "cardinality", err)
	_, err = DeriveStructuralReplayClosure(nil, invalidOuter.ReflectionReportArtifactRefs)
	assertError("outer derive", "cardinality", err)
}

func worstArtifacts(count int) []op.ArtifactRef {
	values := make([]op.ArtifactRef, count)
	for index := range values {
		values[index] = op.ArtifactRef{
			ArtifactKind:   strings.Repeat(`"`, maxShortBytes),
			ArtifactRef:    strings.Repeat(`\`, maxReferenceBytes),
			ArtifactSHA256: fmt.Sprintf("%064x", index+1),
		}
	}
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := op.CanonicalJSON(&values[left])
		rightRaw, _ := op.CanonicalJSON(&values[right])
		return bytes.Compare(leftRaw, rightRaw) < 0
	})
	return values
}

func worstReflectionRefs() []op.ArtifactRef {
	values := make([]op.ArtifactRef, maxReflectionReportRefs)
	for index := range values {
		values[index] = op.ArtifactRef{
			ArtifactKind:   "reflection_report",
			ArtifactRef:    strings.Repeat(`\`, maxReferenceBytes),
			ArtifactSHA256: fmt.Sprintf("%064x", index+1),
		}
	}
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := op.CanonicalJSON(&values[left])
		rightRaw, _ := op.CanonicalJSON(&values[right])
		return bytes.Compare(leftRaw, rightRaw) < 0
	})
	return values
}

func referenceHash(index int) string {
	return fmt.Sprintf("%064x", index+1)
}

func manifestEnvelope(base *StructuralReplayManifest) *StructuralReplayManifest {
	value, _ := cloneValue(base, maxManifestBytes)
	value.ManifestID, value.ManifestSHA256 = "", ""
	value.ArtifactRefs = worstArtifacts(64)
	value.ArtifactReceiptRefs = make([]op.ArtifactReceiptRef, 64)
	value.CapabilityInvocationRefs = make([]op.CapabilityInvocationRef, 64)
	value.InteractionEventRefs = make([]op.InteractionEventRef, 256)
	value.ExecutionReceiptRefs = make([]op.ExecutionReceiptRef, 64)
	value.PredecisionAtomRefs = make([]kd.AtomRef, 256)
	value.PostdecisionAtomRefs = make([]kd.AtomRef, 0)
	for index := 0; index < 256; index++ {
		digest := referenceHash(index)
		value.InteractionEventRefs[index] = op.InteractionEventRef{
			EventID: eventPrefix + digest, EventSHA256: digest}
		atom := kd.AtomRef{AtomID: atomPrefix + digest, AtomSHA256: digest}
		value.PredecisionAtomRefs[index] = atom
	}
	populateEnvelopeAttemptRefs(value)
	return value
}

func populateEnvelopeAttemptRefs(value *StructuralReplayManifest) {
	for index := 0; index < 64; index++ {
		digest := referenceHash(index)
		value.ArtifactReceiptRefs[index] = op.ArtifactReceiptRef{
			ArtifactReceiptID:     artifactReceiptPrefix + digest,
			ArtifactReceiptSHA256: digest,
		}
		value.CapabilityInvocationRefs[index] = op.CapabilityInvocationRef{
			InvocationID: invocationPrefix + digest, InvocationSHA256: digest}
		value.ExecutionReceiptRefs[index] = op.ExecutionReceiptRef{
			ExecutionReceiptID:     executionReceiptPrefix + digest,
			ExecutionReceiptSHA256: digest,
		}
	}
}

func TestConservativeCapsuleAndOuterByteBoundsAreExact(t *testing.T) {
	outer, _ := golden(t)
	length := func(value any) int {
		t.Helper()
		raw, err := CanonicalJSON(value)
		if err != nil {
			t.Fatal(err)
		}
		return len(raw)
	}
	capsuleSize := length(&outer.DecisionCapsule)
	decisionSize := length(&outer.DecisionCapsule.DecisionClosure)
	manifestSize := length(&outer.DecisionCapsule.ReplayManifest)
	capsuleOverhead := capsuleSize - decisionSize - manifestSize
	if capsuleOverhead != 1_867 {
		t.Fatalf("capsule fixed wrapper overhead = %d", capsuleOverhead)
	}
	if bound := 20*1024*1024 + 684_440 + capsuleOverhead; bound != 21_657_827 {
		t.Fatalf("capsule conservative bound = %d", bound)
	}
	branchSize := length(&outer.EvaluationBranch)
	if branchSize != 2_305 {
		t.Fatalf("maximum branch bytes = %d", branchSize)
	}
	refsSize := length(worstReflectionRefs())
	if refsSize != 266_657 {
		t.Fatalf("maximum reflection refs bytes = %d", refsSize)
	}
	outerOverhead := length(outer) - capsuleSize - branchSize -
		length(outer.ReflectionReportArtifactRefs)
	if outerOverhead != 2_083 {
		t.Fatalf("outer fixed wrapper overhead = %d", outerOverhead)
	}
	if bound := 21_657_827 + branchSize + refsSize + outerOverhead; bound != 21_928_872 {
		t.Fatalf("outer conservative bound = %d", bound)
	}
}

func TestEveryDocumentDecoderHasExactConfiguredCeiling(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	cases := []struct {
		maximum int
		decode  func([]byte) error
	}{
		{maxManifestBytes, func(raw []byte) error {
			_, err := DecodeStructuralReplayManifest(raw, &capsule.DecisionClosure)
			return err
		}},
		{maxCapsuleBytes, func(raw []byte) error { _, err := DecodeDecisionCapsule(raw); return err }},
		{maxBranchBytes, func(raw []byte) error {
			_, err := DecodeEvaluationBranch(raw, capsule)
			return err
		}},
		{maxClosureBytes, func(raw []byte) error { _, err := DecodeStructuralReplayClosure(raw); return err }},
	}
	for _, test := range cases {
		if err := test.decode(bytes.Repeat([]byte(" "), test.maximum+1)); err == nil {
			t.Errorf("decoder accepted %d-byte ceiling plus one", test.maximum)
		}
	}
}

func TestSealsDeepCloneTheirInputs(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
	manifest.ManifestID, manifest.ManifestSHA256 = "", ""
	sealedManifest, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure)
	if err != nil {
		t.Fatal(err)
	}
	manifest.PredecisionAtomRefs[0].AtomID = "mutated"
	if sealedManifest.PredecisionAtomRefs[0].AtomID == "mutated" {
		t.Fatal("manifest seal aliases caller storage")
	}
	blankOuter, _ := cloneValue(outer, maxClosureBytes)
	blankOuter.ClosureID, blankOuter.ClosureSHA256 = "", ""
	sealedOuter, err := SealStructuralReplayClosure(blankOuter)
	if err != nil {
		t.Fatal(err)
	}
	blankOuter.ReflectionReportArtifactRefs[0].ArtifactRef = "mutated"
	if sealedOuter.ReflectionReportArtifactRefs[0].ArtifactRef == "mutated" {
		t.Fatal("outer seal aliases caller storage")
	}
}

func TestNegativeControlsAndExactResultMarkersFailClosed(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
	manifest.ManifestID, manifest.ManifestSHA256 = "", ""
	manifest.EffectReplayAllowed = true
	if _, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure); err == nil {
		t.Fatal("manifest effect replay control accepted true")
	}
	manifest.EffectReplayAllowed, manifest.HistoryRewriteAllowed = false, true
	if _, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure); err == nil {
		t.Fatal("manifest history rewrite control accepted true")
	}
	branch, _ := cloneValue(&outer.EvaluationBranch, maxBranchBytes)
	branch.BranchID, branch.BranchSHA256 = "", ""
	branch.HistoryRewriteAllowed = true
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("branch history rewrite control accepted true")
	}
	branch.HistoryRewriteAllowed, branch.EffectReplayAllowed = false, true
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("branch effect replay control accepted true")
	}
	branch.EffectReplayAllowed, branch.ComparisonResult = false, "SEMANTICALLY_EQUIVALENT"
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("branch comparison result drift accepted")
	}
	blankCapsule, _ := cloneValue(capsule, maxCapsuleBytes)
	blankCapsule.CapsuleID, blankCapsule.CapsuleSHA256 = "", ""
	blankCapsule.Result += " drift"
	if _, err := SealDecisionCapsule(blankCapsule); err == nil {
		t.Fatal("capsule result drift accepted")
	}
}

func TestExactModesAndOuterResultMarkerFailClosed(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
	manifest.ManifestID, manifest.ManifestSHA256 = "", ""
	manifest.ReplayMode += "_drift"
	if _, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure); err == nil {
		t.Fatal("manifest replay mode drift accepted")
	}
	branch, _ := cloneValue(&outer.EvaluationBranch, maxBranchBytes)
	branch.BranchID, branch.BranchSHA256 = "", ""
	branch.BranchMode += "_drift"
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("branch mode drift accepted")
	}
	blankOuter, _ := cloneValue(outer, maxClosureBytes)
	blankOuter.ClosureID, blankOuter.ClosureSHA256 = "", ""
	blankOuter.Result += " drift"
	if _, err := SealStructuralReplayClosure(blankOuter); err == nil {
		t.Fatal("outer result marker drift accepted")
	}
}

func TestSealsRequireBothOwnIdentityFieldsBlank(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, _ := cloneValue(&capsule.ReplayManifest, maxManifestBytes)
	manifest.ManifestID = ""
	if _, err := SealStructuralReplayManifest(manifest, &capsule.DecisionClosure); err == nil {
		t.Fatal("manifest with only ID blank accepted")
	}
	blankCapsule, _ := cloneValue(capsule, maxCapsuleBytes)
	blankCapsule.CapsuleSHA256 = ""
	if _, err := SealDecisionCapsule(blankCapsule); err == nil {
		t.Fatal("capsule with only hash blank accepted")
	}
	branch, _ := cloneValue(&outer.EvaluationBranch, maxBranchBytes)
	branch.BranchID = ""
	if _, err := SealEvaluationBranch(branch, capsule); err == nil {
		t.Fatal("branch with only ID blank accepted")
	}
	blankOuter, _ := cloneValue(outer, maxClosureBytes)
	blankOuter.ClosureSHA256 = ""
	if _, err := SealStructuralReplayClosure(blankOuter); err == nil {
		t.Fatal("outer with only hash blank accepted")
	}
}

func TestBlankOrStaleNestedSealsAreRejected(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	blankCapsule, _ := cloneValue(capsule, maxCapsuleBytes)
	blankCapsule.CapsuleID, blankCapsule.CapsuleSHA256 = "", ""
	blankCapsule.DecisionClosure.ClosureID = ""
	if _, err := SealDecisionCapsule(blankCapsule); err == nil {
		t.Fatal("blank nested decision closure accepted")
	}
	staleCapsule, _ := cloneValue(capsule, maxCapsuleBytes)
	staleCapsule.CapsuleID, staleCapsule.CapsuleSHA256 = "", ""
	staleCapsule.DecisionClosure.Result += " drift"
	if _, err := SealDecisionCapsule(staleCapsule); err == nil {
		t.Fatal("stale nested decision closure accepted")
	}
	blankOuter, _ := cloneValue(outer, maxClosureBytes)
	blankOuter.ClosureID, blankOuter.ClosureSHA256 = "", ""
	blankOuter.DecisionCapsule.CapsuleSHA256 = strings.Repeat("0", 64)
	if _, err := SealStructuralReplayClosure(blankOuter); err == nil {
		t.Fatal("stale nested capsule accepted")
	}
}
