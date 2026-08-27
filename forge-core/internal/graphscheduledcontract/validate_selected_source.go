package graphscheduledcontract

import (
	"reflect"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/scheduledterminal"
)

// ValidateSelectedCandidateSource reconstructs an initial or selected
// successor candidate from its exact source snapshot and ordered direct-
// predecessor evidence. It grants no release or execution authority.
func ValidateSelectedCandidateSource(
	value ScheduledNodeContractCandidate,
	snapshot graphdispatch.ControlSnapshot,
	orderedReceipts []scheduledterminal.Receipt,
	contentArtifact *scheduledterminal.Artifact,
) error {
	if orderedReceipts == nil || validateCandidate(value) != nil {
		return errInvalidCandidate
	}
	prompt, err := decodeExact[userPrompt]([]byte(value.Request.UserPrompt))
	if err != nil {
		return errInvalidCandidate
	}
	switch value.ContractScope {
	case contractScope:
		return validateSelectedInitial(value, snapshot, orderedReceipts, contentArtifact, prompt)
	case successorContractScope:
		return validateSelectedSuccessor(value, snapshot, orderedReceipts, contentArtifact, prompt)
	default:
		return errInvalidCandidate
	}
}

func validateSelectedInitial(
	value ScheduledNodeContractCandidate,
	snapshot graphdispatch.ControlSnapshot,
	receipts []scheduledterminal.Receipt,
	artifact *scheduledterminal.Artifact,
	prompt userPrompt,
) error {
	if value.Node.ExecutionOrdinal != 0 || len(receipts) != 0 || artifact != nil ||
		prompt.PredecessorOutput != "" {
		return errInvalidCandidate
	}
	expected, err := BuildInitial(snapshot, value.ScheduleSHA256, optionsFrom(value))
	if err != nil || !reflect.DeepEqual(expected, value) {
		return errInvalidCandidate
	}
	return nil
}

func validateSelectedSuccessor(
	value ScheduledNodeContractCandidate,
	snapshot graphdispatch.ControlSnapshot,
	receipts []scheduledterminal.Receipt,
	artifact *scheduledterminal.Artifact,
	prompt userPrompt,
) error {
	if value.Node.ExecutionOrdinal == 0 || !orderedProjectionMatches(value.Request, receipts) ||
		!contentEvidenceMatches(prompt.PredecessorOutput, receipts, artifact) {
		return errInvalidCandidate
	}
	expected, err := BuildSuccessor(
		snapshot, value.ScheduleSHA256, optionsFrom(value), receipts,
		prompt.PredecessorOutput, value.Node.NodeID,
	)
	if err != nil || !reflect.DeepEqual(expected, value) {
		return errInvalidCandidate
	}
	return nil
}

func orderedProjectionMatches(
	request ScheduledNodeRequest,
	receipts []scheduledterminal.Receipt,
) bool {
	if len(receipts) != len(request.RequiredPredecessorNodeIDs) ||
		len(receipts) != len(request.PredecessorTerminalReceipts) {
		return false
	}
	for index, receipt := range receipts {
		if receipt.NodeID != request.RequiredPredecessorNodeIDs[index] ||
			projectTerminalReceipt(receipt) != request.PredecessorTerminalReceipts[index] {
			return false
		}
	}
	return true
}

func projectTerminalReceipt(receipt scheduledterminal.Receipt) PredecessorTerminalReceipt {
	return PredecessorTerminalReceipt{
		PredecessorNodeID: receipt.NodeID, PredecessorAttempt: receipt.Attempt,
		TerminalEventSeq: 0, TerminalEventSHA256: "",
		TerminalReceiptID: receipt.ReceiptID, TerminalReceiptSHA256: receipt.ReceiptSHA256,
		NodeOutcome: receipt.NodeOutcome, ProviderRequestID: receipt.ProviderRequestID,
		DispatchID: receipt.DispatchID,
	}
}

func contentEvidenceMatches(
	content string,
	receipts []scheduledterminal.Receipt,
	artifact *scheduledterminal.Artifact,
) bool {
	if content == "" {
		return artifact == nil
	}
	return artifact != nil && len(receipts) > 0 &&
		scheduledterminal.ValidatePredecessorContent(receipts[0], *artifact, content) == nil
}
