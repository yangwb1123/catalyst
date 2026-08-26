package graphscheduledreconcile

// Reconcile classifies one validated serial snapshot without performing an
// effect or authorizing the classified next step.
func Reconcile(snapshot ProgressSnapshot) (Decision, error) {
	if validateSnapshot(snapshot) != nil {
		return Decision{}, errInvalidSnapshot
	}
	index := firstIncomplete(snapshot.Nodes)
	disposition := "completed"
	var nextOrdinal *uint16
	var nextNodeID *string
	if index < len(snapshot.Nodes) {
		disposition = classifyIncomplete(snapshot.Nodes, index)
		if disposition == "ready" {
			ordinal, nodeID := uint16(index), snapshot.Nodes[index].NodeID
			nextOrdinal, nextNodeID = &ordinal, &nodeID
		}
	}
	decision := Decision{
		V: 1, ProgressProtocolVersion: ProgressProtocolVersion,
		GraphRunID: snapshot.GraphRunID, ScheduleID: snapshot.ScheduleID,
		ScheduleSHA256: snapshot.ScheduleSHA256, SnapshotSHA256: snapshot.SnapshotSHA256,
		Disposition: disposition, NextExecutionOrdinal: nextOrdinal, NextNodeID: nextNodeID,
	}
	digest, err := domainDigest(decisionDigestDomain, payloadFromDecision(decision))
	if err != nil {
		return Decision{}, errInvalidSnapshot
	}
	decision.DecisionSHA256 = digest
	return decision, nil
}

func firstIncomplete(nodes []ProgressNode) int {
	for index, node := range nodes {
		if node.LifecycleStatus == nil || *node.LifecycleStatus != "terminalized" ||
			node.TerminalOutcome == nil || *node.TerminalOutcome != "completed" {
			return index
		}
	}
	return len(nodes)
}

func classifyIncomplete(nodes []ProgressNode, index int) string {
	for _, node := range nodes[index+1:] {
		if hasProgressEvidence(node) {
			return "incompatible_progress"
		}
	}
	status := nodes[index].LifecycleStatus
	if status == nil {
		return "ready"
	}
	switch *status {
	case "claimed":
		return "claimed_unknown"
	case "quarantined", "adjudicated":
		return "manual_recovery_required"
	case "terminalized":
		return *nodes[index].TerminalOutcome
	default:
		return "incompatible_progress"
	}
}

func hasProgressEvidence(node ProgressNode) bool {
	return node.CandidateID != nil || node.CandidateSHA256 != nil ||
		node.ProviderRequestID != nil || node.PreparedRequestSHA256 != nil ||
		node.LifecycleStatus != nil || node.TerminalOutcome != nil ||
		node.TerminalReceiptSHA256 != nil
}

// MarshalDecision returns exact compact canonical JSON without a trailing LF.
func MarshalDecision(value Decision) ([]byte, error) {
	if validateDecision(value) != nil {
		return nil, errInvalidSnapshot
	}
	return canonicalBytes(value)
}

func validateDecision(value Decision) error {
	valid := value.V == 1 && value.ProgressProtocolVersion == ProgressProtocolVersion &&
		validIdentifier(value.GraphRunID) && isLowerHexDigest(value.ScheduleSHA256) &&
		value.ScheduleID == scheduleIDPrefix+value.ScheduleSHA256 &&
		isLowerHexDigest(value.SnapshotSHA256) && validDecisionShape(value)
	if !valid {
		return errInvalidSnapshot
	}
	digest, err := domainDigest(decisionDigestDomain, payloadFromDecision(value))
	if err != nil || digest != value.DecisionSHA256 {
		return errInvalidSnapshot
	}
	return nil
}

func validDecisionShape(value Decision) bool {
	switch value.Disposition {
	case "ready":
		return value.NextExecutionOrdinal != nil && value.NextNodeID != nil &&
			validIdentifier(*value.NextNodeID)
	case "claimed_unknown", "manual_recovery_required", "failed", "failed_uncertain",
		"completed", "incompatible_progress":
		return value.NextExecutionOrdinal == nil && value.NextNodeID == nil
	default:
		return false
	}
}
