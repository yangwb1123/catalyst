package domain

import pcstate "forgeos/forge-core/internal/platformcorecontract/state"

var objectiveStates = stringSet("draft", "active", "satisfied", "cancelled")
var changeStates = stringSet(
	"proposed", "awaiting_approval", "approved", "active", "paused", "cancelled",
)
var changeObservations = stringSet(
	"not_started", "in_progress", "blocked", "verifying", "completed", "failed",
	"uncertain", "cancelled",
)
var workGraphStates = stringSet("draft", "proposed", "accepted", "superseded")
var workItemRisks = stringSet("low", "medium", "high", "critical")

var objectiveEdges = edgeSet(map[string][]string{
	"draft": {"active", "cancelled"}, "active": {"satisfied", "cancelled"},
})
var changeEdges = edgeSet(map[string][]string{
	"proposed":          {"awaiting_approval", "cancelled"},
	"awaiting_approval": {"approved", "cancelled"},
	"approved":          {"active", "cancelled"},
	"active":            {"paused", "cancelled"}, "paused": {"active", "cancelled"},
})
var changeObservationEdges = edgeSet(map[string][]string{
	"not_started": {"in_progress", "blocked", "cancelled"},
	"in_progress": {"blocked", "verifying", "failed", "uncertain", "cancelled"},
	"blocked":     {"in_progress", "failed", "uncertain", "cancelled"},
	"verifying":   {"in_progress", "blocked", "completed", "failed", "uncertain", "cancelled"},
	"uncertain":   {"blocked", "failed", "cancelled"},
})
var workGraphEdges = edgeSet(map[string][]string{
	"draft": {"proposed"}, "proposed": {"accepted", "superseded"},
	"accepted": {"superseded"},
})

// ValidateObjectiveTransition checks one declared edge without advancing state.
func ValidateObjectiveTransition(from, to ObjectiveState) error {
	return validateEdge(string(from), string(to), objectiveStates, objectiveEdges)
}

// ValidateChangeTransition checks one desired-state edge without advancing state.
func ValidateChangeTransition(from, to ChangeState) error {
	return validateEdge(string(from), string(to), changeStates, changeEdges)
}

// ValidateChangeObservationTransition checks one observed-state edge only.
func ValidateChangeObservationTransition(from, to ChangeObservation) error {
	return validateEdge(string(from), string(to), changeObservations, changeObservationEdges)
}

// ValidateWorkGraphTransition checks one graph-state edge without advancing state.
func ValidateWorkGraphTransition(from, to WorkGraphState) error {
	return validateEdge(string(from), string(to), workGraphStates, workGraphEdges)
}

// ValidateWorkItemTransition reuses the pure Platform Core edge vocabulary.
func ValidateWorkItemTransition(from, to pcstate.WorkItemState) error {
	if err := pcstate.ValidateWorkItemTransition(from, to); err != nil {
		return invalidTransition("WorkItem edge is not declared", err)
	}
	return nil
}

func validateEdge(from, to string, states map[string]bool, edges map[string]bool) error {
	if !isBoundedClosedValue(from) || !isBoundedClosedValue(to) {
		return invalidTransition("state is not bounded", nil)
	}
	if !states[from] || !states[to] {
		return invalidTransition("state is unsupported", nil)
	}
	if !edges[from+"\x00"+to] {
		return invalidTransition("edge is not declared", nil)
	}
	return nil
}

func stringSet(values ...string) map[string]bool {
	result := make(map[string]bool, len(values))
	for _, value := range values {
		result[value] = true
	}
	return result
}

func edgeSet(values map[string][]string) map[string]bool {
	result := make(map[string]bool)
	for from, targets := range values {
		for _, to := range targets {
			result[from+"\x00"+to] = true
		}
	}
	return result
}
