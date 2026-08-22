package decisioncapsulecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"reflect"
	"strings"
	"testing"
)

const goldenPath = "../../../docs/contracts/fixtures/decision-capsule-structural-replay-v1.json"

func golden(t *testing.T) (*StructuralReplayClosure, []byte) {
	t.Helper()
	physical, err := os.ReadFile(goldenPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(physical) < 2 || physical[len(physical)-1] != '\n' ||
		physical[len(physical)-2] == '\n' {
		t.Fatal("golden must have exactly one physical trailing LF")
	}
	value, err := DecodeStructuralReplayClosure(physical[:len(physical)-1])
	if err != nil {
		t.Fatal(err)
	}
	return value, physical
}

func TestNestedLocalInvalidityPreemptsEveryCompositePublicRoute(t *testing.T) {
	outer, _ := golden(t)
	assertError := func(label, contains string, err error) {
		t.Helper()
		if err == nil || !strings.Contains(err.Error(), contains) {
			t.Fatalf("%s: error=%v, want local %q failure", label, err, contains)
		}
	}
	large := strings.Repeat("x", maxCapsuleBytes+1)

	invalidCapsule := outer.DecisionCapsule
	invalidCapsule.ReplayManifest.ReplayMode += "_drift"
	invalidCapsule.DecisionClosure.ClosureID = large
	_, err := DecisionCapsuleDigest(&invalidCapsule)
	assertError("capsule digest", "StructuralReplayManifest constants differ", err)
	assertError("capsule validate", "StructuralReplayManifest constants differ",
		ValidateDecisionCapsule(&invalidCapsule))
	blankCapsule := invalidCapsule
	blankCapsule.CapsuleID, blankCapsule.CapsuleSHA256 = "", ""
	_, err = SealDecisionCapsule(&blankCapsule)
	assertError("capsule seal", "StructuralReplayManifest constants differ", err)

	_, err = EvaluationBranchDigest(&outer.EvaluationBranch, &invalidCapsule)
	assertError("branch digest capsule preflight", "StructuralReplayManifest constants differ", err)
	assertError("branch validate capsule preflight", "StructuralReplayManifest constants differ",
		ValidateEvaluationBranch(&outer.EvaluationBranch, &invalidCapsule))
	blankBranch := outer.EvaluationBranch
	blankBranch.BranchID, blankBranch.BranchSHA256 = "", ""
	_, err = SealEvaluationBranch(&blankBranch, &invalidCapsule)
	assertError("branch seal capsule preflight", "StructuralReplayManifest constants differ", err)
	_, err = DeriveEvaluationBranch(&invalidCapsule)
	assertError("branch derive capsule preflight", "StructuralReplayManifest constants differ", err)

	invalidOuter := *outer
	invalidOuter.EvaluationBranch.BranchMode += "_drift"
	invalidOuter.DecisionCapsule.ReplayManifest.ReplayMode += "_drift"
	invalidOuter.DecisionCapsule.DecisionClosure.ClosureID = large
	_, err = StructuralReplayClosureDigest(&invalidOuter)
	assertError("outer digest branch preflight", "constants or comparison result differ", err)
	assertError("outer validate branch preflight", "constants or comparison result differ",
		ValidateStructuralReplayClosure(&invalidOuter))
	invalidOuter.ClosureID, invalidOuter.ClosureSHA256 = "", ""
	_, err = SealStructuralReplayClosure(&invalidOuter)
	assertError("outer seal branch preflight", "constants or comparison result differ", err)
	_, err = DeriveStructuralReplayClosure(
		&invalidCapsule, outer.ReflectionReportArtifactRefs[:0])
	assertError("outer derive capsule preflight", "StructuralReplayManifest constants differ", err)
}

func TestRelationsPreemptDeepDependencyValidation(t *testing.T) {
	outer, _ := golden(t)
	capsule := outer.DecisionCapsule
	manifest := capsule.ReplayManifest
	invalidClosure := capsule.DecisionClosure
	invalidClosure.ClosureID = decisionClosurePrefix + strings.Repeat("0", 64)
	want := "exact ordered projection"
	assertRelation := func(label string, err error) {
		t.Helper()
		if err == nil || !strings.Contains(err.Error(), want) {
			t.Fatalf("%s: error=%v, want relation failure %q", label, err, want)
		}
	}
	_, err := StructuralReplayManifestDigest(&manifest, &invalidClosure)
	assertRelation("manifest digest", err)
	assertRelation("manifest validate", ValidateStructuralReplayManifest(&manifest, &invalidClosure))
	manifest.ManifestID, manifest.ManifestSHA256 = "", ""
	_, err = SealStructuralReplayManifest(&manifest, &invalidClosure)
	assertRelation("manifest seal", err)

	capsule.DecisionClosure = invalidClosure
	_, err = DecisionCapsuleDigest(&capsule)
	assertRelation("capsule digest", err)
	assertRelation("capsule validate", ValidateDecisionCapsule(&capsule))
	blankCapsule := capsule
	blankCapsule.CapsuleID, blankCapsule.CapsuleSHA256 = "", ""
	_, err = SealDecisionCapsule(&blankCapsule)
	assertRelation("capsule seal", err)
	branch := outer.EvaluationBranch
	want = "unique structural comparison"
	_, err = EvaluationBranchDigest(&branch, &capsule)
	assertRelation("branch digest", err)
	assertRelation("branch validate", ValidateEvaluationBranch(&branch, &capsule))
	branch.BranchID, branch.BranchSHA256 = "", ""
	_, err = SealEvaluationBranch(&branch, &capsule)
	assertRelation("branch seal", err)
	changedOuter := *outer
	changedOuter.DecisionCapsule = capsule
	_, err = StructuralReplayClosureDigest(&changedOuter)
	assertRelation("outer digest", err)
	assertRelation("outer validate", ValidateStructuralReplayClosure(&changedOuter))
	changedOuter.ClosureID, changedOuter.ClosureSHA256 = "", ""
	_, err = SealStructuralReplayClosure(&changedOuter)
	assertRelation("outer seal", err)
}

func TestGoldenPhysicalSemanticAndObjectPins(t *testing.T) {
	value, physical := golden(t)
	physicalHash := sha256.Sum256(physical)
	if got := hex.EncodeToString(physicalHash[:]); got !=
		"d54494f49851cc4146905bbd64c0815fe7d79704476c0aeb1113f270d5cbb2d0" {
		t.Fatalf("physical golden SHA-256 = %s", got)
	}
	manifest := &value.DecisionCapsule.ReplayManifest
	checks := map[string][2]string{
		"manifest": {manifest.ManifestSHA256,
			"40d1fa34a2fc9b31856d3f16edd1cc346f47d0b447040539b667279f0f67365c"},
		"capsule": {value.DecisionCapsule.CapsuleSHA256,
			"f02c172fb5d65a36841361a9969dd8ad79eae08c548d1c6d0bbea5a564276b59"},
		"branch": {value.EvaluationBranch.BranchSHA256,
			"4442cf99caa21eda32a1c4062cfe66b333dff5188f4b818a9c69bf5cb829949a"},
		"outer": {value.ClosureSHA256,
			"38f14574e9a9531371d55800f1f77bbdb79648a121c0f774a2a9c0083cf13497"},
	}
	for label, check := range checks {
		if check[0] != check[1] {
			t.Errorf("%s digest = %s", label, check[0])
		}
	}
	raw, err := CanonicalJSON(value)
	if err != nil || !bytes.Equal(raw, physical[:len(physical)-1]) {
		t.Fatalf("canonical outer differs from physical golden: %v", err)
	}
}

func TestGoldenDerivesAndResealsEveryNewObject(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest, err := DeriveStructuralReplayManifest(&capsule.DecisionClosure)
	if err != nil || !reflect.DeepEqual(manifest, &capsule.ReplayManifest) {
		t.Fatalf("manifest derive mismatch: %v", err)
	}
	derivedCapsule, err := DeriveDecisionCapsule(&capsule.DecisionClosure)
	if err != nil || !reflect.DeepEqual(derivedCapsule, capsule) {
		t.Fatalf("capsule derive mismatch: %v", err)
	}
	branch, err := DeriveEvaluationBranch(capsule)
	if err != nil || !reflect.DeepEqual(branch, &outer.EvaluationBranch) {
		t.Fatalf("branch derive mismatch: %v", err)
	}
	derivedOuter, err := DeriveStructuralReplayClosure(
		capsule, outer.ReflectionReportArtifactRefs)
	if err != nil || !reflect.DeepEqual(derivedOuter, outer) {
		t.Fatalf("outer derive mismatch: %v", err)
	}
}

func TestDecisionClosureValidationCallsUpstreamValidateOnceWithoutSeal(t *testing.T) {
	source, err := os.ReadFile("primitives.go")
	if err != nil {
		t.Fatal(err)
	}
	if count := bytes.Count(source, []byte("kd.ValidateClosure(")); count != 1 {
		t.Fatalf("kd.ValidateClosure call sites = %d", count)
	}
	if bytes.Contains(source, []byte("kd.SealClosure(")) {
		t.Fatal("decision closure validation must not call upstream SealClosure")
	}
}

func TestManifestAndCapsulePublicDigestDecodeSealRoundtrips(t *testing.T) {
	outer, _ := golden(t)
	capsule := &outer.DecisionCapsule
	manifest := &capsule.ReplayManifest
	manifestDigest, err := StructuralReplayManifestDigest(manifest, &capsule.DecisionClosure)
	if err != nil || manifestDigest != manifest.ManifestSHA256 {
		t.Fatalf("manifest digest = %s, %v", manifestDigest, err)
	}
	manifestRaw, _ := CanonicalJSON(manifest)
	decodedManifest, err := DecodeStructuralReplayManifest(manifestRaw, &capsule.DecisionClosure)
	if err != nil || !reflect.DeepEqual(decodedManifest, manifest) {
		t.Fatalf("manifest decode: %v", err)
	}
	blankManifest, _ := cloneValue(manifest, maxManifestBytes)
	blankManifest.ManifestID, blankManifest.ManifestSHA256 = "", ""
	sealedManifest, err := SealStructuralReplayManifest(blankManifest, &capsule.DecisionClosure)
	if err != nil || !reflect.DeepEqual(sealedManifest, manifest) {
		t.Fatalf("manifest seal: %v", err)
	}
	capsuleDigest, err := DecisionCapsuleDigest(capsule)
	if err != nil || capsuleDigest != capsule.CapsuleSHA256 {
		t.Fatalf("capsule digest = %s, %v", capsuleDigest, err)
	}
	capsuleRaw, _ := CanonicalJSON(capsule)
	decodedCapsule, err := DecodeDecisionCapsule(capsuleRaw)
	if err != nil || !reflect.DeepEqual(decodedCapsule, capsule) {
		t.Fatalf("capsule decode: %v", err)
	}
	blankCapsule, _ := cloneValue(capsule, maxCapsuleBytes)
	blankCapsule.CapsuleID, blankCapsule.CapsuleSHA256 = "", ""
	sealedCapsule, err := SealDecisionCapsule(blankCapsule)
	if err != nil || !reflect.DeepEqual(sealedCapsule, capsule) {
		t.Fatalf("capsule seal: %v", err)
	}
}

func TestBranchAndOuterPublicDigestDecodeSealRoundtrips(t *testing.T) {
	outer, _ := golden(t)
	capsule, branch := &outer.DecisionCapsule, &outer.EvaluationBranch
	branchDigest, err := EvaluationBranchDigest(branch, capsule)
	if err != nil || branchDigest != branch.BranchSHA256 {
		t.Fatalf("branch digest = %s, %v", branchDigest, err)
	}
	branchRaw, _ := CanonicalJSON(branch)
	decodedBranch, err := DecodeEvaluationBranch(branchRaw, capsule)
	if err != nil || !reflect.DeepEqual(decodedBranch, branch) {
		t.Fatalf("branch decode: %v", err)
	}
	blankBranch, _ := cloneValue(branch, maxBranchBytes)
	blankBranch.BranchID, blankBranch.BranchSHA256 = "", ""
	sealedBranch, err := SealEvaluationBranch(blankBranch, capsule)
	if err != nil || !reflect.DeepEqual(sealedBranch, branch) {
		t.Fatalf("branch seal: %v", err)
	}
	outerDigest, err := StructuralReplayClosureDigest(outer)
	if err != nil || outerDigest != outer.ClosureSHA256 {
		t.Fatalf("outer digest = %s, %v", outerDigest, err)
	}
	outerRaw, _ := CanonicalJSON(outer)
	decodedOuter, err := DecodeStructuralReplayClosure(outerRaw)
	if err != nil || !reflect.DeepEqual(decodedOuter, outer) {
		t.Fatalf("outer decode: %v", err)
	}
	blankOuter, _ := cloneValue(outer, maxClosureBytes)
	blankOuter.ClosureID, blankOuter.ClosureSHA256 = "", ""
	sealedOuter, err := SealStructuralReplayClosure(blankOuter)
	if err != nil || !reflect.DeepEqual(sealedOuter, outer) {
		t.Fatalf("outer seal: %v", err)
	}
}
