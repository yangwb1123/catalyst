package graphterminal

func payloadFromReceipt(value Receipt) receiptPayload {
	return receiptPayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		TerminalReceiptProtocolVersion: value.TerminalReceiptProtocolVersion,
		TerminalControlSHA256:          value.TerminalControlSHA256,
		ExpectedLastEventSeq:           value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256:        value.ExpectedLastEventSHA256,
		GraphRunID:                     value.GraphRunID, GraphID: value.GraphID, NodeID: value.NodeID,
		Attempt: value.Attempt, DispatchID: value.DispatchID,
		LaneOwnershipID: value.LaneOwnershipID, ProjectLaneSHA256: value.ProjectLaneSHA256,
		ArtifactKind: value.ArtifactKind, ArtifactID: value.ArtifactID,
		ArtifactSHA256: value.ArtifactSHA256, NodeOutcome: value.NodeOutcome,
		WaveIndex: value.WaveIndex, WaveOutcome: value.WaveOutcome,
		GraphStatus: value.GraphStatus, RetryAuthorized: value.RetryAuthorized,
		LaneReleaseAuthorized: value.LaneReleaseAuthorized,
	}
}

// BuildReceipt fully rebuilds the claimed terminal control and returns Core's
// passive terminal decision. It cannot release a lane or mutate scheduler state.
func BuildReceipt(control TerminalControl) (Receipt, error) {
	if validateControl(control) != nil {
		return Receipt{}, errInvalidControl
	}
	outcome, err := receiptOutcome(control.Artifact)
	if err != nil {
		return Receipt{}, errInvalidControl
	}
	value := receiptFrom(control, outcome)
	digest, err := domainDigest(receiptDigestDomain, payloadFromReceipt(value))
	if err != nil {
		return Receipt{}, errInvalidControl
	}
	value.ReceiptID = "graph-node-terminal-receipt-" + digest
	value.ReceiptSHA256 = digest
	if validateReceipt(value) != nil {
		return Receipt{}, errInvalidControl
	}
	return value, nil
}

func receiptFrom(control TerminalControl, outcome string) Receipt {
	artifact, claim := control.Artifact, control.Claim
	return Receipt{
		V: TerminalReceiptVersion, SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		TerminalReceiptProtocolVersion: TerminalReceiptProtocol,
		TerminalControlSHA256:          control.SnapshotSHA256,
		ExpectedLastEventSeq:           4, ExpectedLastEventSHA256: claim.ClaimEventSHA256,
		GraphRunID: claim.GraphRunID, GraphID: control.GraphRun.GraphID,
		NodeID: claim.NodeID, Attempt: claim.Attempt, DispatchID: claim.DispatchID,
		LaneOwnershipID: claim.LaneOwnershipID, ProjectLaneSHA256: claim.ProjectLaneSHA256,
		ArtifactKind: artifact.ArtifactKind, ArtifactID: artifact.ArtifactID,
		ArtifactSHA256: artifact.ArtifactSHA256, NodeOutcome: outcome,
		WaveIndex: 0, WaveOutcome: outcome, GraphStatus: outcome,
		RetryAuthorized: false, LaneReleaseAuthorized: true,
	}
}

func receiptOutcome(artifact TerminalArtifact) (string, error) {
	if artifact.ArtifactKind == "result" && artifact.Classification == "completed" {
		return "completed", nil
	}
	if artifact.ArtifactKind == "result" && artifact.Classification == "length" {
		return "failed", nil
	}
	if artifact.ArtifactKind == "uncertainty" {
		return "failed_uncertain", nil
	}
	return "", errInvalidControl
}

func validateReceipt(value Receipt) error {
	if !validReceiptHeader(value) || !validReceiptOutcome(value) {
		return errInvalidControl
	}
	digest, err := domainDigest(receiptDigestDomain, payloadFromReceipt(value))
	if err != nil || digest != value.ReceiptSHA256 ||
		value.ReceiptID != "graph-node-terminal-receipt-"+digest {
		return errInvalidControl
	}
	return nil
}

func validReceiptHeader(value Receipt) bool {
	return value.V == TerminalReceiptVersion && value.SchedulerProtocolVersion == 1 &&
		value.TerminalReceiptProtocolVersion == TerminalReceiptProtocol &&
		value.ExpectedLastEventSeq == 4 && value.Attempt == 1 && value.WaveIndex == 0 &&
		validIdentifier(value.GraphRunID) && validIdentifier(value.GraphID) &&
		validIdentifier(value.NodeID) && validIdentifier(value.DispatchID) &&
		validIdentifier(value.LaneOwnershipID) && validIdentifier(value.ArtifactID) &&
		isLowerHexDigest(value.TerminalControlSHA256) &&
		isLowerHexDigest(value.ExpectedLastEventSHA256) &&
		isLowerHexDigest(value.ProjectLaneSHA256) &&
		isLowerHexDigest(value.ArtifactSHA256) &&
		value.ArtifactID == "graph-node-terminal-artifact-"+value.ArtifactSHA256 &&
		isLowerHexDigest(value.ReceiptSHA256) &&
		!value.RetryAuthorized && value.LaneReleaseAuthorized
}

func validReceiptOutcome(value Receipt) bool {
	if value.ArtifactKind != "result" && value.ArtifactKind != "uncertainty" {
		return false
	}
	valid := value.NodeOutcome == "completed" || value.NodeOutcome == "failed" ||
		value.NodeOutcome == "failed_uncertain"
	kindValid := value.ArtifactKind == "uncertainty" && value.NodeOutcome == "failed_uncertain" ||
		value.ArtifactKind == "result" && value.NodeOutcome != "failed_uncertain"
	return valid && kindValid && value.WaveOutcome == value.NodeOutcome && value.GraphStatus == value.NodeOutcome
}

// MarshalReceipt returns exact compact canonical JSON without a trailing LF.
func MarshalReceipt(value Receipt) ([]byte, error) {
	if validateReceipt(value) != nil {
		return nil, errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > maxReceiptBytes {
		return nil, errInvalidControl
	}
	return encoded, nil
}
