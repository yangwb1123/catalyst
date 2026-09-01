package application

import (
	"errors"
	"fmt"
	"reflect"
	"strings"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestDecideRejectsMixedOrMalformedControlSnapshots(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
	}{
		{"duplicate assessment", duplicateAssessment},
		{"unknown assessment item", func(v *ControlSnapshot) { v.Assessments[0].WorkItemID = testID("wki", 9) }},
		{"change version", func(v *ControlSnapshot) { v.Assessments[0].ChangeVersion++ }},
		{"graph version", func(v *ControlSnapshot) { v.Assessments[0].WorkGraphVersion++ }},
		{"policy profile", func(v *ControlSnapshot) { v.Assessments[0].PolicyProfile = "other.a1" }},
		{"project snapshot", func(v *ControlSnapshot) { v.Assessments[0].ProjectSnapshotID = testID("psn", 9) }},
		{"risk", func(v *ControlSnapshot) { v.Assessments[0].Risk = "high" }},
		{"effect", func(v *ControlSnapshot) { v.Assessments[0].RequestedEffects = []string{"repo.read"} }},
		{"status", func(v *ControlSnapshot) { v.Assessments[0].Budget = "available" }},
		{"evidence", func(v *ControlSnapshot) { v.Assessments[0].EvidenceRefs[0].RecordSHA256 = "bad" }},
		{"progress", progressBeforePredecessor},
		{"invalid graph", func(v *ControlSnapshot) { v.WorkGraph.Items[1].Dependencies[0] = testID("wki", 9) }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value)
			got, err := Decide(value)
			if !errors.Is(err, ErrInvalidControlSnapshot) || !reflect.DeepEqual(got, Decision{}) {
				t.Fatalf("invalid snapshot returned decision=%#v error=%v", got, err)
			}
		})
	}
}

func duplicateAssessment(value *ControlSnapshot) {
	value.Assessments = append(value.Assessments, value.Assessments[0])
}

func progressBeforePredecessor(value *ControlSnapshot) {
	value.WorkGraph.Items[1].State = stateReady
}

func TestDecideAllowsMissingAssessmentButFailsClosedAtTheFrontier(t *testing.T) {
	value := validSnapshot()
	value.Assessments = nil
	got, err := Decide(value)
	if err != nil {
		t.Fatal(err)
	}
	assertDecision(t, value, got, DecisionAwaitApproval, reasonAssessmentMissing, testID("wki", 1))
}

func TestDecideRejectsAssessmentAndEvidenceOneOverBounds(t *testing.T) {
	value := validSnapshot()
	value.Assessments = make([]WorkItemAssessment, maxAssessments+1)
	if _, err := Decide(value); !errors.Is(err, ErrInvalidControlSnapshot) {
		t.Fatalf("assessment over-bound error = %v", err)
	}
	value = validSnapshot()
	value.Assessments[0].EvidenceRefs = nil
	if _, err := Decide(value); !errors.Is(err, ErrInvalidControlSnapshot) {
		t.Fatalf("empty evidence error = %v", err)
	}
	value = validSnapshot()
	value.Assessments[0].EvidenceRefs = evidenceValues(maxAssessmentEvidenceRefs)
	if _, err := Decide(value); err != nil {
		t.Fatalf("exact evidence bound: %v", err)
	}
	value.Assessments[0].EvidenceRefs = evidenceValues(maxAssessmentEvidenceRefs + 1)
	if _, err := Decide(value); !errors.Is(err, ErrInvalidControlSnapshot) {
		t.Fatalf("evidence over-bound error = %v", err)
	}
}

func TestDecideBoundsAssessmentIdentityAndEffectsBeforeLookupOrSorting(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
	}{
		{"oversized work item ID", func(v *ControlSnapshot) {
			v.Assessments[0].WorkItemID = strings.Repeat("x", 64*1024)
		}},
		{"effect count", func(v *ControlSnapshot) {
			v.Assessments[0].RequestedEffects = assessmentEffects(maxAssessmentRequestedEffects + 1)
		}},
		{"oversized effect", func(v *ControlSnapshot) {
			v.Assessments[0].RequestedEffects = []string{strings.Repeat("a", 64*1024)}
		}},
		{"malformed effect", func(v *ControlSnapshot) {
			v.Assessments[0].RequestedEffects = []string{"repo write"}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value)
			got, err := Decide(value)
			if !errors.Is(err, ErrInvalidControlSnapshot) || !reflect.DeepEqual(got, Decision{}) {
				t.Fatalf("unbounded assessment returned decision=%#v error=%v", got, err)
			}
		})
	}
}

func TestDecideAcceptsAssessmentEffectBoundsExactly(t *testing.T) {
	value := validSnapshot()
	effects := assessmentEffects(maxAssessmentRequestedEffects)
	effects[len(effects)-1] = strings.Repeat("a", maxAssessmentEffectBytes)
	value.WorkGraph.Items[0].RequestedEffects = append([]string(nil), effects...)
	value.Assessments[0].RequestedEffects = append([]string(nil), effects...)
	got, err := Decide(value)
	if err != nil || got.Kind != DecisionReadyWorkItem {
		t.Fatalf("exact assessment effect bounds returned decision=%#v error=%v", got, err)
	}
}

func assessmentEffects(count int) []string {
	result := make([]string, count)
	for index := range result {
		result[index] = fmt.Sprintf("effect.%02d", index)
	}
	return result
}

func evidenceValues(count int) []core.RecordRef {
	result := make([]core.RecordRef, count)
	for index := range result {
		result[index] = testEvidence(index + 1)
	}
	return result
}

func TestDecideRejectsDuplicateEvidenceIdentity(t *testing.T) {
	value := validSnapshot()
	value.Assessments[0].EvidenceRefs = append(
		value.Assessments[0].EvidenceRefs, value.Assessments[0].EvidenceRefs[0],
	)
	if _, err := Decide(value); !errors.Is(err, ErrInvalidControlSnapshot) {
		t.Fatalf("duplicate evidence error = %v", err)
	}
}

func TestDecideRejectsConflictingEvidenceAcrossAssessments(t *testing.T) {
	value := multiFrontierSnapshot()
	value.Assessments[1].EvidenceRefs[0] = value.Assessments[0].EvidenceRefs[0]
	value.Assessments[1].EvidenceRefs[0].RecordSHA256 = testEvidence(1).RecordSHA256
	if _, err := Decide(value); !errors.Is(err, ErrInvalidControlSnapshot) {
		t.Fatalf("conflicting evidence error = %v", err)
	}
}
