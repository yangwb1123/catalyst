package transitionreceiptcontract

var allowedEdges = map[string][]string{
	"DRAFT":             {"NEEDS_EVIDENCE", "NEEDS_INFO", "REJECTED", "SUPERSEDED"},
	"NEEDS_EVIDENCE":    {"BASELINED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"BASELINED":         {"DESIGN_DRAFTED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"DESIGN_DRAFTED":    {"ASSESSED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"ASSESSED":          {"DESIGN_DRAFTED", "DESIGNED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"DESIGNED":          {"PLANNED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"PLANNED":           {"DESIGNED", "AUTHORIZED", "NEEDS_INFO", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"AUTHORIZED":        {"IMPLEMENTING", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"IMPLEMENTING":      {"VERIFYING", "CHANGES_REQUESTED", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"VERIFYING":         {"REVIEWING", "CHANGES_REQUESTED", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"REVIEWING":         {"RELEASE_READY", "CHANGES_REQUESTED", "BLOCKED", "REJECTED", "QUARANTINED", "SUPERSEDED"},
	"CHANGES_REQUESTED": {"DESIGN_DRAFTED", "ASSESSED", "DESIGNED", "PLANNED", "IMPLEMENTING", "VERIFYING", "BLOCKED", "REJECTED", "SUPERSEDED"},
	"RELEASE_READY":     {"RELEASING", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"RELEASING":         {"OBSERVING", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"OBSERVING":         {"REFLECTING", "CHANGES_REQUESTED", "BLOCKED", "QUARANTINED", "SUPERSEDED"},
	"REFLECTING":        {"LEARNING", "CHANGES_REQUESTED", "BLOCKED", "SUPERSEDED"},
	"LEARNING":          {"CLOSED", "BLOCKED", "SUPERSEDED"},
	"NEEDS_INFO":        {"BLOCKED", "REJECTED", "SUPERSEDED"},
	"BLOCKED":           {"REJECTED", "SUPERSEDED"},
	"QUARANTINED":       {"BLOCKED", "VERIFYING", "REJECTED", "SUPERSEDED"},
	"CLOSED":            {},
	"REJECTED":          {},
	"SUPERSEDED":        {},
}

func isState(value string) bool {
	return containsString(states, value)
}

func listedEdge(from, to string) bool {
	return containsString(allowedEdges[from], to)
}

func isTerminalState(value string) bool {
	return containsString(terminalStates, value)
}

func isSuspendedState(value string) bool {
	return value == "NEEDS_INFO" || value == "BLOCKED"
}

func containsString(values []string, value string) bool {
	for _, candidate := range values {
		if candidate == value {
			return true
		}
	}
	return false
}
