package domain

import (
	"errors"
	"math"
	"strings"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

func TestTextAndTokenProfilesEnforceByteAndUnicodeBounds(t *testing.T) {
	if err := validateText(strings.Repeat("a", 8), "text", 1, 8); err != nil {
		t.Fatal(err)
	}
	if err := validateToken(strings.Repeat("a", 8), "token", 8); err != nil {
		t.Fatal(err)
	}
	invalidText := []string{
		"", " padded", "padded ", "line\nbreak", "bad\u202e", "line\u2028break",
		"paragraph\u2029break", "zero\u200bwidth", "byte\ufefforder", "\xff", "123456789",
	}
	for _, value := range invalidText {
		if err := validateText(value, "text", 1, 8); !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("invalid text %q error = %v", value, err)
		}
	}
	invalidTokens := []string{"", "Bad", "_edge", "edge_", "two words", "ümlaut", "123456789"}
	for _, value := range invalidTokens {
		if err := validateToken(value, "token", 8); !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("invalid token %q error = %v", value, err)
		}
	}
}

func TestBudgetAndScalarBoundsAreClosedAndOverflowSafe(t *testing.T) {
	valid := []BudgetLimit{
		{MaxAttempts: 1, MaxDurationMS: 1, MaxCostMicroUSD: 0},
		{MaxAttempts: maxAttempts, MaxDurationMS: maxDurationMS, MaxCostMicroUSD: maxCostMicroUSD},
	}
	for _, value := range valid {
		if err := validateBudget(value, "budget"); err != nil {
			t.Fatalf("valid budget %+v: %v", value, err)
		}
	}
	invalid := []BudgetLimit{
		{MaxAttempts: 0, MaxDurationMS: 1},
		{MaxAttempts: maxAttempts + 1, MaxDurationMS: 1},
		{MaxAttempts: 1, MaxDurationMS: 0},
		{MaxAttempts: 1, MaxDurationMS: maxDurationMS + 1},
		{MaxAttempts: 1, MaxDurationMS: 1, MaxCostMicroUSD: -1},
		{MaxAttempts: 1, MaxDurationMS: 1, MaxCostMicroUSD: maxCostMicroUSD + 1},
	}
	for _, value := range invalid {
		if err := validateBudget(value, "budget"); !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("invalid budget %+v error = %v", value, err)
		}
	}
	if total, ok := addWithin(math.MaxInt64-1, 1, math.MaxInt64); !ok || total != math.MaxInt64 {
		t.Fatalf("exact maximum sum = %d, %t", total, ok)
	}
	if _, ok := addWithin(math.MaxInt64-1, 2, math.MaxInt64); ok {
		t.Fatal("overflowing sum was accepted")
	}
	if _, ok := addWithin(2, 0, 1); ok {
		t.Fatal("already over-bound total was accepted")
	}
}

func TestTimestampAndVersionBounds(t *testing.T) {
	for _, timestamp := range []int64{0, maxUnixMilliseconds} {
		if err := validateUnixMS(timestamp, "timestamp"); err != nil {
			t.Fatalf("timestamp %d: %v", timestamp, err)
		}
	}
	for _, timestamp := range []int64{-1, maxUnixMilliseconds + 1} {
		if err := validateUnixMS(timestamp, "timestamp"); !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("timestamp %d error = %v", timestamp, err)
		}
	}
	if err := validateVersion(1, "version"); err != nil {
		t.Fatal(err)
	}
	if err := validateVersion(0, "version"); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("zero version error = %v", err)
	}
}

func TestOversizedClosedVocabularyValuesProduceBoundedErrors(t *testing.T) {
	huge := strings.Repeat("x", 1<<20)
	objective := validObjective()
	objective.CreatedBy.ActorType = core.ActorType(huge)
	graph := validGraph()
	graph.Items[0].State = pcstate.WorkItemState(huge)
	artifactGraph := validGraph()
	artifactGraph.Items[0].ContextArtifactRef.SourceSnapshotRef.EntityType = core.EntityType(huge)
	tests := []struct {
		name     string
		relation error
		call     func() error
	}{
		{"actor", ErrInvalidDomain, func() error { return ValidateObjective(objective) }},
		{"value state", ErrInvalidDomain, func() error {
			return ValidateWorkGraph(validObjective(), validChange(), graph)
		}},
		{"artifact entity type", ErrInvalidDomain, func() error {
			return ValidateWorkGraph(validObjective(), validChange(), artifactGraph)
		}},
		{"transition state", ErrInvalidTransition, func() error {
			return ValidateWorkItemTransition(pcstate.WorkItemState(huge), "planned")
		}},
	}
	for _, test := range tests {
		err := test.call()
		if !errors.Is(err, test.relation) || len(err.Error()) > 512 {
			t.Fatalf("%s error relation/length = %v/%d", test.name, err, len(err.Error()))
		}
	}
}
