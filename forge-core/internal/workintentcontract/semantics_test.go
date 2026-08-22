package workintentcontract

import (
	"fmt"
	"sort"
	"strings"
	"testing"
)

func attestationPointers(value *Attestations) []*bool {
	return []*bool{&value.Approval, &value.Authentication, &value.Authority,
		&value.Completion, &value.Effect, &value.Execution, &value.Freshness,
		&value.Materiality, &value.Ownership, &value.Permission, &value.Persistence,
		&value.ReferenceResolution, &value.Scope, &value.Truth}
}

func TestEveryAttestationMustBeExactlyFalse(t *testing.T) {
	for index := 0; index < 14; index++ {
		candidate := blankGolden(t)
		*attestationPointers(&candidate.Attestations)[index] = true
		if _, err := SealWorkIntent(candidate); err == nil {
			t.Fatalf("attestation %d true was accepted", index)
		}
	}
}

func TestExactADR0056PrincipalAndNullableOwner(t *testing.T) {
	expectSealError(t, func(value *WorkIntent) { value.Requester.PrincipalType = "tool" })
	expectSealError(t, func(value *WorkIntent) { value.Requester.AuthorityDomain = "" })
	candidate := blankGolden(t)
	candidate.DeclaredOwner = nil
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatal(err)
	}
}

func TestAllNullableDeclarationsMayBeNull(t *testing.T) {
	candidate := blankGolden(t)
	candidate.DeclaredOwner = nil
	candidate.Binding.RunID = nil
	candidate.Intent.DeadlineUnixMS = nil
	candidate.Origin.OriginRef = nil
	candidate.References.LocalSourceSnapshotDeclaration = nil
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatal(err)
	}
}

func TestConstantsAndEnumsRejectDrift(t *testing.T) {
	expectSealError(t, func(value *WorkIntent) { value.APIVersion = "forgeos.work-intent/v2" })
	expectSealError(t, func(value *WorkIntent) { value.Intent.WorkType = "execute" })
	expectSealError(t, func(value *WorkIntent) { value.Materiality.Basis = "verified" })
	expectSealError(t, func(value *WorkIntent) { value.Origin.OriginKind = "runtime_alert" })
	expectSealError(t, func(value *WorkIntent) { value.DeclaredAtUnixMS = -1 })
	expectSealError(t, func(value *WorkIntent) { negative := int64(-1); value.Intent.DeadlineUnixMS = &negative })
}

func narrativeValues(prefix string, count int) []string {
	values := make([]string, count)
	for index := range values {
		values[index] = fmt.Sprintf("%s-%03d", prefix, index)
	}
	return values
}

func TestNarrativeAuthoredOrderAndUniqueness(t *testing.T) {
	candidate := blankGolden(t)
	candidate.Intent.Scope = []string{"z-last-lexically", "a-first-lexically"}
	sealed := mustSeal(t, candidate)
	if sealed.Intent.Scope[0] != "z-last-lexically" {
		t.Fatal("authored narrative order changed")
	}
	expectSealError(t, func(value *WorkIntent) {
		value.Intent.SuccessSignals = []string{"same", "same"}
	})
	expectSealError(t, func(value *WorkIntent) { value.Intent.Scope = []string{} })
}

func TestNarrativePerArrayAndAggregateLimits(t *testing.T) {
	candidate := blankGolden(t)
	candidate.Intent.ExternalConstraints = narrativeValues("external", 64)
	candidate.Intent.NonGoals = narrativeValues("non-goal", 64)
	candidate.Intent.OpenQuestions = narrativeValues("question", 64)
	candidate.Intent.Scope = narrativeValues("scope", 63)
	candidate.Intent.SuccessSignals = narrativeValues("signal", 1)
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatalf("256 narrative entries: %v", err)
	}
	candidate.Intent.Scope = narrativeValues("scope", 64)
	if _, err := SealWorkIntent(candidate); err == nil {
		t.Fatal("257 narrative entries accepted")
	}
	expectSealError(t, func(value *WorkIntent) {
		value.Intent.ExternalConstraints = narrativeValues("external", 65)
	})
}

func recordRefs(prefix string, count int) []RecordRef {
	values := make([]RecordRef, count)
	for index := range values {
		values[index] = RecordRef{RecordID: fmt.Sprintf("%s-%03d", prefix, index),
			CanonicalSHA256: strings.Repeat("a", 64)}
	}
	return values
}

func TestRecordRefGrammarOrderDisjointnessAndLimits(t *testing.T) {
	candidate := blankGolden(t)
	candidate.References.ClaimRecordRefs = recordRefs("claim", 64)
	candidate.References.EvidenceRecordRefs = recordRefs("evidence", 64)
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatalf("64 plus 64 refs: %v", err)
	}
	expectSealError(t, func(value *WorkIntent) {
		value.References.ClaimRecordRefs = recordRefs("claim", 65)
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.ClaimRecordRefs[0].RecordID = "UPPER"
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.ClaimRecordRefs[0].RecordID = "claim\nrecord"
	})
	expectSealError(t, func(value *WorkIntent) {
		hash := strings.Repeat("a", 64)
		value.References.ClaimRecordRefs = []RecordRef{
			{CanonicalSHA256: hash, RecordID: "same-record"},
			{CanonicalSHA256: hash, RecordID: "same-record"},
		}
	})
}

func TestRecordRefOrderingAndCrossSetIdentity(t *testing.T) {
	expectSealError(t, func(value *WorkIntent) {
		value.References.ClaimRecordRefs = []RecordRef{
			{RecordID: "z-record", CanonicalSHA256: strings.Repeat("a", 64)},
			{RecordID: "a-record", CanonicalSHA256: strings.Repeat("b", 64)},
		}
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.EvidenceRecordRefs = append([]RecordRef{},
			value.References.ClaimRecordRefs...)
	})
}

func artifactDeclarations(count int) []ArtifactDeclaration {
	values := make([]ArtifactDeclaration, count)
	for index := range values {
		values[index] = ArtifactDeclaration{ArtifactKind: fmt.Sprintf("kind-%03d", index),
			ArtifactRef: fmt.Sprintf("artifact/%03d", index), ArtifactSHA256: strings.Repeat("b", 64)}
	}
	sort.Slice(values, func(left, right int) bool {
		leftBytes, _ := canonicalArtifact(values[left])
		rightBytes, _ := canonicalArtifact(values[right])
		return string(leftBytes) < string(rightBytes)
	})
	return values
}

func TestArtifactCanonicalOrderPairIdentityAndLimit(t *testing.T) {
	candidate := blankGolden(t)
	candidate.References.LocalArtifactDeclarations = artifactDeclarations(32)
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatalf("32 artifacts: %v", err)
	}
	expectSealError(t, func(value *WorkIntent) {
		value.References.LocalArtifactDeclarations = artifactDeclarations(33)
	})
	expectSealError(t, func(value *WorkIntent) {
		items := artifactDeclarations(2)
		items[0], items[1] = items[1], items[0]
		value.References.LocalArtifactDeclarations = items
	})
}

func TestArtifactSamePairDifferentHashAndSnapshotReject(t *testing.T) {
	expectSealError(t, func(value *WorkIntent) {
		first := value.References.LocalArtifactDeclarations[0]
		second := first
		second.ArtifactSHA256 = strings.Repeat("9", 64)
		value.References.LocalArtifactDeclarations = []ArtifactDeclaration{first, second}
		sort.Slice(value.References.LocalArtifactDeclarations, func(i, j int) bool {
			left, _ := canonicalArtifact(value.References.LocalArtifactDeclarations[i])
			right, _ := canonicalArtifact(value.References.LocalArtifactDeclarations[j])
			return string(left) < string(right)
		})
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.LocalSourceSnapshotDeclaration.SnapshotID = "UPPER"
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.LocalSourceSnapshotDeclaration.SnapshotID = "snapshot\nidentifier"
	})
	expectSealError(t, func(value *WorkIntent) {
		value.References.LocalSourceSnapshotDeclaration.SnapshotType = "git"
	})
}

func TestNilArraysAndSelfIdentityMutationsReject(t *testing.T) {
	expectSealError(t, func(value *WorkIntent) { value.Intent.NonGoals = nil })
	expectSealError(t, func(value *WorkIntent) { value.References.ClaimRecordRefs = nil })
	document, _ := loadGolden(t)
	mutated := cloneWorkIntent(document)
	mutated.WorkIntentSHA256 = strings.Repeat("0", 64)
	mutated.WorkIntentID = workIntentIDPrefix + mutated.WorkIntentSHA256
	if err := ValidateWorkIntent(mutated); err == nil {
		t.Fatal("mutated digest was accepted")
	}
	mutated = cloneWorkIntent(document)
	mutated.WorkIntentID = workIntentIDPrefix + strings.Repeat("0", 64)
	if err := ValidateWorkIntent(mutated); err == nil {
		t.Fatal("mutated ID was accepted")
	}
}

func TestSealRejectsNonblankIdentity(t *testing.T) {
	document, _ := loadGolden(t)
	if _, err := SealWorkIntent(document); err == nil {
		t.Fatal("SealWorkIntent accepted an already sealed value")
	}
}

func TestDigestIgnoresBlankSealedAndArbitraryIdentity(t *testing.T) {
	blank := blankGolden(t)
	sealed, _ := loadGolden(t)
	arbitrary := cloneWorkIntent(sealed)
	arbitrary.WorkIntentID = "not-an-id"
	arbitrary.WorkIntentSHA256 = "not-a-digest"
	if err := ValidateWorkIntent(arbitrary); err == nil {
		t.Fatal("ValidateWorkIntent accepted arbitrary identity")
	}
	root, err := workIntentNode(arbitrary)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalWorkIntent(raw); err == nil {
		t.Fatal("DecodeCanonicalWorkIntent accepted arbitrary identity")
	}
	for index, value := range []*WorkIntent{blank, sealed, arbitrary} {
		digest, err := WorkIntentSHA256(value)
		if err != nil || digest != GoldenRecordSHA256 {
			t.Fatalf("identity form %d digest = %q, %v", index, digest, err)
		}
	}
}

func TestDigestStillRejectsNonIdentityDrift(t *testing.T) {
	value := blankGolden(t)
	value.APIVersion = "forgeos.work-intent/v2"
	if _, err := WorkIntentSHA256(value); err == nil {
		t.Fatal("digest accepted non-identity semantic drift")
	}
	value = blankGolden(t)
	value.Intent.NonGoals = nil
	if _, err := WorkIntentSHA256(value); err == nil {
		t.Fatal("digest repaired a null narrative array")
	}
	value = blankGolden(t)
	value.References.ClaimRecordRefs = nil
	if _, err := WorkIntentSHA256(value); err == nil {
		t.Fatal("digest repaired a null reference array")
	}
}
