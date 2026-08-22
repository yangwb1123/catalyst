package transitionreceiptcontract

import "testing"

func TestFrozenStateGraphCoversEveryStatePair(t *testing.T) {
	if len(states) != 23 {
		t.Fatalf("state count = %d", len(states))
	}
	for _, from := range states {
		for _, to := range states {
			want := containsString(allowedEdges[from], to)
			if listedEdge(from, to) != want {
				t.Fatalf("edge %s -> %s drifted", from, to)
			}
		}
	}
	for _, terminal := range terminalStates {
		if len(allowedEdges[terminal]) != 0 {
			t.Fatalf("terminal %s has outgoing edges", terminal)
		}
	}
}

func TestEvaluatorClassifiesEveryStatePair(t *testing.T) {
	fixture := loadGolden(t)
	base := fixtureNode(t, fixture, "transition_receipt")
	for _, from := range states {
		for _, to := range states {
			current, previous := receiptForDeclaredEdge(t, base, from, to)
			got := assessRelation(t, requestFor(t, current, previous), "edge")
			wantListed := listedEdge(from, to) || declaredResumeEdge(previous, from, to)
			want := relation(wantListed, "listed_declared_edge", "unlisted_declared_edge")
			if got != want {
				t.Fatalf("edge %s -> %s = %s, want %s", from, to, got, want)
			}
		}
	}
}

func receiptForDeclaredEdge(t *testing.T, base map[string]any, from,
	to string) (map[string]any, map[string]any) {
	t.Helper()
	if from == "DRAFT" {
		current := cloneNode(base)
		setDeclaredTransition(current, from, to)
		resealReceipt(t, current)
		return current, nil
	}
	previous := cloneNode(base)
	setDeclaredTransition(previous, "DRAFT", from)
	resealReceipt(t, previous)
	return successor(t, previous, from, to), previous
}

func setDeclaredTransition(receipt map[string]any, from, to string) {
	transition := receipt["transition"].(map[string]any)
	transition["from_state"], transition["to_state"] = from, to
	transition["rework_target"], transition["resume_state"] = nil, nil
	if to == "CHANGES_REQUESTED" {
		transition["rework_target"] = "VERIFYING"
	}
	if isSuspendedState(to) {
		transition["resume_state"] = from
	}
	receipt["applicability"].(map[string]any)["stage_id"] = to
}

func declaredResumeEdge(previous map[string]any, from, to string) bool {
	if previous == nil || !isSuspendedState(from) {
		return false
	}
	transition := previous["transition"].(map[string]any)
	return transition["to_state"] == from && transition["resume_state"] == to
}

func TestVocabularyIsExactAndSelfBound(t *testing.T) {
	vocabulary, err := TransitionVocabulary()
	if err != nil {
		t.Fatal(err)
	}
	if err := validateVocabulary(vocabulary); err != nil {
		t.Fatal(err)
	}
	mutated := cloneNode(vocabulary)
	mutated["states"].([]any)[0] = "draft"
	if err := validateVocabulary(mutated); err == nil {
		t.Fatal("vocabulary alias unexpectedly passed")
	}
	mutated = cloneNode(vocabulary)
	edges := mutated["edges"].([]any)
	first := edges[0].(map[string]any)["allowed_to_states"].([]any)
	first[0], first[1] = first[1], first[0]
	if err := validateVocabulary(mutated); err == nil {
		t.Fatal("authored edge reorder unexpectedly passed")
	}
}
