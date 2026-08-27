package graphscheduledcontract

import (
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/graphschedule"
)

func validateCandidate(value ScheduledNodeContractCandidate) error {
	if !validHeader(value) || !validNode(value.Node, value.ContractScope) ||
		!validRequest(value.Request, value) || !validPolicies(value) {
		return errInvalidCandidate
	}
	digest, err := domainDigest(contractDigestDomain, candidatePayloadFrom(value))
	if err != nil || digest != value.ContractSHA256 ||
		value.ContractID != contractIDPrefix+digest {
		return errInvalidCandidate
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxCandidateBytes {
		return errInvalidCandidate
	}
	return nil
}

func validHeader(value ScheduledNodeContractCandidate) bool {
	scopeValid := value.ContractScope == contractScope ||
		value.ContractScope == successorContractScope
	return value.V == CandidateVersion && value.SchedulerProtocolVersion == 1 &&
		value.NodeExecutionProtocolVersion == NodeExecutionProtocolVersion &&
		value.ExecutionScheduleProtocolVersion == graphschedule.ExecutionScheduleProtocolVersion &&
		scopeValid && validIdentifier(value.GraphRunID, 128) &&
		validIdentifier(value.GraphID, 128) && isLowerHexDigest(value.SourceSnapshotSHA256) &&
		isLowerHexDigest(value.GraphManifestSHA256) && isLowerHexDigest(value.CorePlanSHA256) &&
		isLowerHexDigest(value.ControlSnapshotSHA256) && validScheduleIdentity(value) &&
		value.ExpectedLastEventSeq == 1 && isLowerHexDigest(value.ExpectedLastEventSHA256) &&
		!value.LifecycleContractAdmitted && !value.ProviderRequestPresent &&
		!value.ExecutionAuthorityReleased && !value.DispatchAuthorityReleased &&
		!value.ProgressObserved && !value.SuccessorAdvanceAuthorized &&
		isLowerHexDigest(value.ContractSHA256)
}

func validScheduleIdentity(value ScheduledNodeContractCandidate) bool {
	return isLowerHexDigest(value.ScheduleSHA256) &&
		value.ScheduleID == "graph-execution-schedule-"+value.ScheduleSHA256
}

func validNode(node CandidateNode, scope string) bool {
	ordinalValid, waveValid := node.ExecutionOrdinal == 0, node.TopologyWaveIndex == 0
	if scope == successorContractScope {
		ordinalValid = node.ExecutionOrdinal >= 1 &&
			node.ExecutionOrdinal <= maxSuccessorOrdinal
		waveValid = node.TopologyWaveIndex <= maxTopologyWaveIndex
	}
	return ordinalValid && node.AuthoredNodeIndex <= maxAuthoredNodeIndex && waveValid && node.Attempt == 1 &&
		validIdentifier(node.NodeID, 128) && validIdentifier(node.ProjectID, 128) &&
		validIdentifier(node.MemberRole, 64) && validIdentifier(node.AgentProfile, 128) &&
		node.ProjectLaneSHA256 == rawDomainDigest(projectLaneDomain, node.ProjectID) &&
		node.SameProjectPolicy == "exclusive_until_terminal"
}

// 边界常量与 docs/contracts/scheduled-successor-protocol.md 的 Bounds 表
// 同源(单一事实源,spec_contract_test.go 校验一致性)。
const (
	maxSuccessorOrdinal  = 31
	maxTopologyWaveIndex = 31
	maxAuthoredNodeIndex = 31
	maxPredecessorCount  = 31
)

func validRequest(
	request ScheduledNodeRequest,
	candidate ScheduledNodeContractCandidate,
) bool {
	bound := request.V == RequestVersion && request.GraphRunID == candidate.GraphRunID &&
		request.ScheduleID == candidate.ScheduleID && request.ScheduleSHA256 == candidate.ScheduleSHA256 &&
		request.ExecutionOrdinal == candidate.Node.ExecutionOrdinal && request.NodeID == candidate.Node.NodeID &&
		request.Attempt == candidate.Node.Attempt && request.RequiredPredecessorNodeIDs != nil &&
		request.PredecessorTerminalReceipts != nil &&
		request.Tools != nil && len(request.Tools) == 0
	if candidate.ContractScope == contractScope {
		bound = bound && len(request.RequiredPredecessorNodeIDs) == 0 &&
			len(request.PredecessorTerminalReceipts) == 0 && !request.PredecessorContentIncluded
	} else {
		// ADR-0035: the candidate carries exactly its direct predecessors'
		// receipts — a wave sibling with an empty direct-predecessor set
		// carries zero receipts, while every required predecessor must be
		// covered by the carried evidence. Disclosed predecessor plaintext
		// must have at least one carried receipt to authenticate its source.
		bound = bound &&
			predecessorsMatchReceipts(request.RequiredPredecessorNodeIDs,
				request.PredecessorTerminalReceipts) &&
			(!request.PredecessorContentIncluded || len(request.PredecessorTerminalReceipts) > 0)
	}
	if !bound || !validPrompts(request) || !isLowerHexDigest(request.RequestSHA256) {
		return false
	}
	digest, err := domainDigest(requestDigestDomain, requestPayloadFrom(request))
	return err == nil && digest == request.RequestSHA256 && request.RequestID == requestIDPrefix+digest
}

// predecessorsMatchReceipts requires a one-to-one ordered evidence sequence:
// every required direct predecessor has exactly one completed receipt at the
// same position, with no unrelated or duplicate IDs. A same-wave sibling may
// have two empty sequences.
func predecessorsMatchReceipts(
	required []string,
	receipts []PredecessorTerminalReceipt,
) bool {
	if len(required) != len(receipts) {
		return false
	}
	requiredSet := make(map[string]bool, len(required))
	for index, predecessorID := range required {
		if !validIdentifier(predecessorID, maxIdentifierBytes) || requiredSet[predecessorID] {
			return false
		}
		requiredSet[predecessorID] = true
		receipt := receipts[index]
		if receipt.PredecessorNodeID != predecessorID || !validPredecessorReceipt(receipt) {
			return false
		}
	}
	return true
}

func validPredecessorReceipt(receipt PredecessorTerminalReceipt) bool {
	return validIdentifier(receipt.PredecessorNodeID, maxIdentifierBytes) &&
		receipt.PredecessorAttempt == 1 && receipt.TerminalEventSeq == 0 &&
		receipt.TerminalEventSHA256 == "" && isLowerHexDigest(receipt.TerminalReceiptSHA256) &&
		receipt.TerminalReceiptID == terminalReceiptIDPrefix+receipt.TerminalReceiptSHA256 &&
		receipt.NodeOutcome == "completed" &&
		validIdentifier(receipt.ProviderRequestID, maxIdentifierBytes) &&
		validIdentifier(receipt.DispatchID, maxIdentifierBytes)
}

func validPrompts(request ScheduledNodeRequest) bool {
	systemValid := uint64(len(request.SystemPrompt)) == request.SystemPromptBytes &&
		byteDigest(request.SystemPrompt) == request.SystemPromptSHA256 &&
		validProse(request.SystemPrompt, maxProseBytes+1_024)
	userValid := uint64(len(request.UserPrompt)) == request.UserPromptBytes &&
		byteDigest(request.UserPrompt) == request.UserPromptSHA256 &&
		len(request.UserPrompt) <= MaxUserPromptBytes
	if !systemValid || !userValid {
		return false
	}
	decoded, err := decodeExact[userPrompt]([]byte(request.UserPrompt))
	contentPresent := decoded.PredecessorOutput != ""
	contentValid := !contentPresent || utf8.ValidString(decoded.PredecessorOutput) &&
		len(decoded.PredecessorOutput) <= MaxPredecessorOutputBytes
	return err == nil && decoded.V == RequestVersion && decoded.NodeID == request.NodeID &&
		validIdentifier(decoded.NodeID, maxIdentifierBytes) && validProse(decoded.Task, maxProseBytes) &&
		validProse(decoded.Acceptance, maxProseBytes) &&
		request.PredecessorContentIncluded == contentPresent && contentValid
}

func validPolicies(value ScheduledNodeContractCandidate) bool {
	workspace, provider := value.Workspace, value.Provider
	approval, result, failure := value.Approval, value.Result, value.Failure
	fixed := workspace.Mode == "none" && workspace.RootIdentity == nil && workspace.IsolationID == nil &&
		workspace.AllowedReadPaths != nil && len(workspace.AllowedReadPaths) == 0 &&
		provider.Kind == "openai_responses" && !provider.Store && provider.Stream &&
		value.Budgets.MaxTurns == 1 && value.Budgets.MaxToolCalls == 0 &&
		approval.ProviderDispatch == "fresh_off_machine_consent" && approval.Workspace == "forbidden" &&
		approval.Tools == "forbidden" && approval.Writeback == "forbidden" &&
		result.ArtifactKind == "local_graph_node_artifact" && result.PredecessorDataflow == "none" &&
		result.ConversationWriteback == "none" && result.PromptWriteback == "none" &&
		result.MemoryWriteback == "none" && !failure.AutomaticRetry && !failure.LeaseRetry &&
		failure.PostClaimUncertainty == "dispatch_unknown" && failure.FailurePropagationOwner == "forge_core"
	return fixed && validExecutionOptions(optionsFrom(value))
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func validIdentifier(value string, maximum int) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" && len(value) <= maximum &&
		!strings.ContainsFunc(value, unsupportedCharacter)
}

func validProse(value string, maximum int) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" && len(value) <= maximum &&
		!strings.ContainsFunc(value, unsupportedProseCharacter)
}

func unsupportedProseCharacter(value rune) bool {
	return unicode.IsControl(value) && value != '\n' && value != '\r' && value != '\t' ||
		unsupportedBidiCharacter(value)
}

func unsupportedCharacter(value rune) bool {
	return unicode.IsControl(value) || unsupportedBidiCharacter(value)
}

func unsupportedBidiCharacter(value rune) bool {
	return value == '\u061c' || value == '\u200e' || value == '\u200f' ||
		value >= '\u2028' && value <= '\u202e' || value >= '\u2066' && value <= '\u2069'
}
