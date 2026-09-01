package domain

import (
	"errors"
	"strings"
	"testing"
)

type changeBoundCase struct {
	name   string
	valid  bool
	mutate func(*Change)
}

type graphBoundCase struct {
	name   string
	valid  bool
	mutate func(*Change, *WorkGraph)
}

func TestAcceptanceCriterionAndPolicyCallSiteByteBounds(t *testing.T) {
	tests := []changeBoundCase{
		{"criterion id empty", false, func(v *Change) { v.AcceptanceCriteria[0].CriterionID = "" }},
		{"criterion id maximum", true, func(v *Change) { v.AcceptanceCriteria[0].CriterionID = strings.Repeat("a", 64) }},
		{"criterion id over", false, func(v *Change) { v.AcceptanceCriteria[0].CriterionID = strings.Repeat("a", 65) }},
		{"description minimum", true, func(v *Change) { v.AcceptanceCriteria[0].Description = "a" }},
		{"description maximum", true, func(v *Change) { v.AcceptanceCriteria[0].Description = strings.Repeat("d", 2048) }},
		{"description over", false, func(v *Change) { v.AcceptanceCriteria[0].Description = strings.Repeat("d", 2049) }},
		{"criterion verification empty", false, func(v *Change) { v.AcceptanceCriteria[0].VerificationRequirements = []string{""} }},
		{"criterion verification maximum", true, func(v *Change) { v.AcceptanceCriteria[0].VerificationRequirements = []string{strings.Repeat("v", 64)} }},
		{"criterion verification over", false, func(v *Change) { v.AcceptanceCriteria[0].VerificationRequirements = []string{strings.Repeat("v", 65)} }},
		{"criterion verification optional", true, func(v *Change) { v.AcceptanceCriteria[0].VerificationRequirements = nil }},
		{"policy maximum", true, func(v *Change) { v.PolicyProfile = strings.Repeat("p", 64) }},
		{"policy over", false, func(v *Change) { v.PolicyProfile = strings.Repeat("p", 65) }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validChange()
			test.mutate(&value)
			assertDomainValidity(t, ValidateChange(validObjective(), value), test.valid)
		})
	}
}

func TestWorkItemTextAndTokenCallSiteByteBounds(t *testing.T) {
	tests := []graphBoundCase{
		{"purpose minimum", true, func(_ *Change, v *WorkGraph) { v.Items[0].Purpose = "a" }},
		{"purpose maximum", true, func(_ *Change, v *WorkGraph) { v.Items[0].Purpose = strings.Repeat("p", 2048) }},
		{"purpose over", false, func(_ *Change, v *WorkGraph) { v.Items[0].Purpose = strings.Repeat("p", 2049) }},
		{"criterion reference maximum", true, func(c *Change, v *WorkGraph) {
			criterionID := strings.Repeat("c", 64)
			c.AcceptanceCriteria[0].CriterionID = criterionID
			c.AcceptanceCriteria[0].VerificationRequirements = nil
			v.Items[0].AcceptanceCriterionIDs = []string{criterionID}
		}},
		{"effect empty", false, func(_ *Change, v *WorkGraph) { v.Items[0].RequestedEffects = []string{""} }},
		{"effect maximum", true, func(_ *Change, v *WorkGraph) { v.Items[0].RequestedEffects = []string{strings.Repeat("e", 64)} }},
		{"effect over", false, func(_ *Change, v *WorkGraph) { v.Items[0].RequestedEffects = []string{strings.Repeat("e", 65)} }},
		{"effect optional", true, func(_ *Change, v *WorkGraph) { v.Items[0].RequestedEffects = nil }},
		{"agent empty", false, func(_ *Change, v *WorkGraph) { v.Items[0].AgentRequirements = []string{""} }},
		{"agent maximum", true, func(_ *Change, v *WorkGraph) { v.Items[0].AgentRequirements = []string{strings.Repeat("a", 256)} }},
		{"agent over", false, func(_ *Change, v *WorkGraph) { v.Items[0].AgentRequirements = []string{strings.Repeat("a", 257)} }},
		{"agent optional", true, func(_ *Change, v *WorkGraph) { v.Items[0].AgentRequirements = nil }},
		{"verification empty", false, func(_ *Change, v *WorkGraph) { v.Items[0].VerificationRequirements = []string{""} }},
		{"verification maximum", true, func(c *Change, v *WorkGraph) {
			c.AcceptanceCriteria[0].VerificationRequirements = nil
			v.Items[0].VerificationRequirements = []string{strings.Repeat("v", 64)}
		}},
		{"verification over", false, func(_ *Change, v *WorkGraph) { v.Items[0].VerificationRequirements = []string{strings.Repeat("v", 65)} }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			change, graph := validChange(), validGraph()
			test.mutate(&change, &graph)
			assertDomainValidity(t, ValidateWorkGraph(validObjective(), change, graph), test.valid)
		})
	}
}

func TestDependencyCallSiteAcceptsExactMaximum(t *testing.T) {
	items := bareWorkItems(maxItemListEntries + 2)
	items[len(items)-1].Dependencies = numberedIDs("wki", maxItemListEntries)
	if _, err := TopologicalOrder(items); err != nil {
		t.Fatalf("maximum WorkItem dependency list: %v", err)
	}
}

func TestOversizedIDsFailAtScalarValidationBeforeMapLookup(t *testing.T) {
	hugeID := strings.Repeat("x", 8<<20)
	items := bareWorkItems(2)
	items[1].Dependencies = []string{hugeID}
	_, err := TopologicalOrder(items)
	if !errors.Is(err, ErrInvalidDomain) || !strings.Contains(err.Error(), "typed Platform ID") {
		t.Fatalf("oversized dependency lookup order = %v", err)
	}

	change, graph := validChange(), validGraph()
	graph.Items[0].ProjectID = hugeID
	err = ValidateWorkGraph(validObjective(), change, graph)
	if !errors.Is(err, ErrInvalidDomain) || !strings.Contains(err.Error(), "project_id") {
		t.Fatalf("oversized project lookup order = %v", err)
	}

	graph = validGraph()
	graph.Items[0].ProjectSnapshotID = hugeID
	err = ValidateWorkGraph(validObjective(), change, graph)
	if !errors.Is(err, ErrInvalidDomain) || !strings.Contains(err.Error(), "project_snapshot_id") {
		t.Fatalf("oversized snapshot equality order = %v", err)
	}
}

func assertDomainValidity(t *testing.T, err error, valid bool) {
	t.Helper()
	if valid && err != nil {
		t.Fatalf("maximum/minimum valid value: %v", err)
	}
	if !valid && !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("out-of-bound value error = %v", err)
	}
}
