package receipt

import (
	"fmt"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/platformcorecontract/state"
)

type evidenceRefCase struct {
	Arrangement  string  `json:"arrangement"`
	CaseID       string  `json:"case_id"`
	Count        int     `json:"count"`
	ExpectedCode *string `json:"expected_code"`
}

type stateMachineCase struct {
	AllowedTargets map[string][]string `json:"allowed_targets"`
	Machine        string              `json:"machine"`
	States         []string            `json:"states"`
}

func assertEvidenceRefCorpus(t *testing.T, cases []evidenceRefCase, source VerificationReceipt) {
	t.Helper()
	if len(cases) != 4 {
		t.Fatalf("evidence-ref case count = %d, want 4", len(cases))
	}
	resultIndex, template := evidenceTemplate(t, source.Results)
	for _, testCase := range cases {
		t.Run(testCase.CaseID, func(t *testing.T) {
			receipt := source
			receipt.Results = append([]VerificationCheckResult(nil), source.Results...)
			receipt.Results[resultIndex].EvidenceRefs = corpusEvidenceRefs(template, testCase)
			err := validateVerificationReceipt(&receipt)
			if testCase.ExpectedCode == nil {
				if err != nil {
					t.Fatalf("valid evidence boundary rejected: %v", err)
				}
				return
			}
			if code := mustRejectionCode(t, err); string(code) != *testCase.ExpectedCode {
				t.Fatalf("code = %s, want %s", code, *testCase.ExpectedCode)
			}
		})
	}
}

func evidenceTemplate(t *testing.T, results []VerificationCheckResult) (int, core.RecordRef) {
	t.Helper()
	for index, result := range results {
		if len(result.EvidenceRefs) > 0 {
			return index, result.EvidenceRefs[0]
		}
	}
	t.Fatal("receipt golden has no evidence reference template")
	return 0, core.RecordRef{}
}

func corpusEvidenceRefs(template core.RecordRef, testCase evidenceRefCase) []core.RecordRef {
	values := make([]core.RecordRef, testCase.Count)
	for index := range values {
		values[index] = template
		values[index].RecordID = fmt.Sprintf("evidence-%02d", index)
	}
	switch testCase.Arrangement {
	case "ascending":
	case "duplicate":
		for index := range values {
			values[index].RecordID = "evidence-00"
		}
	case "descending":
		for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
			values[left], values[right] = values[right], values[left]
		}
	}
	return values
}

func assertStateMachineCorpus(t *testing.T, cases []stateMachineCase) {
	t.Helper()
	expectedStates := map[string]int{"work_item": 12, "attempt": 8, "action": 9}
	if len(cases) != len(expectedStates) {
		t.Fatalf("state-machine case count = %d", len(cases))
	}
	totalEdges := 0
	seen := make(map[string]bool, len(cases))
	for _, testCase := range cases {
		if seen[testCase.Machine] || len(testCase.States) != expectedStates[testCase.Machine] {
			t.Fatalf("invalid state-machine inventory for %q", testCase.Machine)
		}
		seen[testCase.Machine] = true
		edges := stateMachineEdges(t, testCase)
		totalEdges += len(edges)
		assertAllStatePairs(t, testCase, edges)
	}
	if totalEdges != 61 {
		t.Fatalf("state-machine edge count = %d, want 61", totalEdges)
	}
}

func stateMachineEdges(t *testing.T, testCase stateMachineCase) map[string]bool {
	t.Helper()
	states := make(map[string]bool, len(testCase.States))
	for _, value := range testCase.States {
		if states[value] {
			t.Fatalf("duplicate %s state %q", testCase.Machine, value)
		}
		states[value] = true
	}
	if len(testCase.AllowedTargets) != len(states) {
		t.Fatalf("%s adjacency rows = %d, states = %d", testCase.Machine, len(testCase.AllowedTargets), len(states))
	}
	edges := make(map[string]bool)
	for source, targets := range testCase.AllowedTargets {
		if !states[source] {
			t.Fatalf("%s has unknown source %q", testCase.Machine, source)
		}
		for _, target := range targets {
			key := source + "\x00" + target
			if !states[target] || edges[key] {
				t.Fatalf("%s has invalid or duplicate edge %s -> %s", testCase.Machine, source, target)
			}
			edges[key] = true
		}
	}
	return edges
}

func assertAllStatePairs(t *testing.T, testCase stateMachineCase, allowed map[string]bool) {
	t.Helper()
	for _, source := range testCase.States {
		for _, target := range testCase.States {
			err := stateTransition(testCase.Machine, source, target)
			if allowed[source+"\x00"+target] {
				if err != nil {
					t.Fatalf("declared %s edge %s -> %s rejected: %v", testCase.Machine, source, target, err)
				}
				continue
			}
			if code := mustRejectionCode(t, err); code != core.RejectionCode("pc_transition_invalid") {
				t.Fatalf("undeclared %s edge %s -> %s code = %s", testCase.Machine, source, target, code)
			}
		}
	}
}

func stateTransition(machine, source, target string) error {
	switch machine {
	case "work_item":
		return state.ValidateWorkItemTransition(state.WorkItemState(source), state.WorkItemState(target))
	case "attempt":
		return state.ValidateAttemptTransition(state.AttemptState(source), state.AttemptState(target))
	case "action":
		return state.ValidateActionTransition(state.ActionState(source), state.ActionState(target))
	default:
		return rejectf(rejectionValueInvalid, "unknown state machine %q", machine)
	}
}
