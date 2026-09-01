package application

import (
	"errors"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

var productionImportAllowlist = map[string]bool{
	"errors": true,
	"fmt":    true,
	"sort":   true,
	"forgeos/forge-core/internal/delivery/domain":            true,
	"forgeos/forge-core/internal/platformcorecontract":       true,
	"forgeos/forge-core/internal/platformcorecontract/state": true,
}

func TestProductionImportsStayPureAndAllowlisted(t *testing.T) {
	directory := reconcileSourceDirectory(t)
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	productionFiles := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") ||
			strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		productionFiles++
		assertSourceImports(t, filepath.Join(directory, entry.Name()))
	}
	if productionFiles == 0 {
		t.Fatal("Reconciler production source is missing")
	}
}

func reconcileSourceDirectory(t *testing.T) string {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("cannot locate Reconciler source")
	}
	return filepath.Dir(source)
}

func assertSourceImports(t *testing.T, path string) {
	t.Helper()
	parsed, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	for _, declaration := range parsed.Imports {
		imported, err := strconv.Unquote(declaration.Path.Value)
		if err != nil || !productionImportAllowlist[imported] {
			t.Fatalf("production source %s imports non-pure path %q", filepath.Base(path), imported)
		}
	}
}

func TestPublicContractsStayPassiveAndExact(t *testing.T) {
	assertFieldNames(t, reflect.TypeOf(ControlSnapshot{}), []string{
		"Assessments", "Change", "CurrentSnapshotBindings", "Objective", "WorkGraph",
	})
	assertFieldNames(t, reflect.TypeOf(WorkItemAssessment{}), []string{
		"Approval", "Budget", "ChangeVersion", "EvidenceRefs", "Policy", "PolicyProfile",
		"ProjectSnapshotID", "RequestedEffects", "Risk", "WorkGraphVersion", "WorkItemID",
	})
	assertFieldNames(t, reflect.TypeOf(Decision{}), []string{
		"ChangeID", "ChangeVersion", "EvidenceRefs", "Kind", "ObjectiveID", "ObjectiveVersion",
		"ReasonCode", "WorkGraphID", "WorkGraphVersion", "WorkItemID",
	})
}

func assertFieldNames(t *testing.T, value reflect.Type, want []string) {
	t.Helper()
	got := make([]string, value.NumField())
	for index := range got {
		got[index] = value.Field(index).Name
	}
	sort.Strings(got)
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("%s fields = %v, want %v", value.Name(), got, want)
	}
}

func TestDecisionVocabularyContainsNoEffectOrCompletionDecision(t *testing.T) {
	values := []DecisionKind{
		DecisionNoOp, DecisionAwaitApproval, DecisionReadyWorkItem,
		DecisionBlockWorkItem, DecisionReplanChange, DecisionEscalateUncertain,
	}
	for _, value := range values {
		text := string(value)
		if strings.Contains(text, "dispatch") || strings.Contains(text, "complete") ||
			strings.Contains(text, "verify") {
			t.Fatalf("effectful Decision kind entered v1: %q", value)
		}
	}
}

func TestReconcilerCoversTheExactPlatformCoreWorkItemVocabulary(t *testing.T) {
	if err := validateWorkItemVocabulary(); err != nil {
		t.Fatal(err)
	}
	values := pcstate.WorkItemStateValues()
	tests := []struct {
		name   string
		mutate func([]pcstate.WorkItemState) []pcstate.WorkItemState
	}{
		{"missing", func(v []pcstate.WorkItemState) []pcstate.WorkItemState { return v[:len(v)-1] }},
		{"unknown", func(v []pcstate.WorkItemState) []pcstate.WorkItemState {
			v[0] = pcstate.WorkItemState("future")
			return v
		}},
		{"duplicate", func(v []pcstate.WorkItemState) []pcstate.WorkItemState {
			v[len(v)-1] = v[0]
			return v
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			candidate := append([]pcstate.WorkItemState(nil), values...)
			if err := validateWorkItemVocabularyValues(test.mutate(candidate)); !errors.Is(err, ErrInvalidControlSnapshot) {
				t.Fatalf("vocabulary drift error = %v", err)
			}
		})
	}
}
