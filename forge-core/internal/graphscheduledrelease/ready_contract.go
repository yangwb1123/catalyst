package graphscheduledrelease

import (
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

func validateReadyContractSource(
	value ReadyReleaseControl,
	selected graphscheduledreconcile.ProgressNode,
) error {
	contract := value.ScheduledContract
	if graphscheduledcontract.ValidateSelectedCandidateSource(
		contract, value.ControlSnapshot, value.DirectPredecessorReceipts,
		value.PredecessorContentArtifact,
	) != nil {
		return errInvalidControl
	}
	encoded, err := graphscheduledcontract.MarshalCandidate(contract)
	if err != nil || len(encoded) == 0 || !validReadyContractRecord(value, uint64(len(encoded))) ||
		!validReadyReceiptClosure(value, selected) {
		return errInvalidControl
	}
	return nil
}

func validReadyContractRecord(value ReadyReleaseControl, contractBytes uint64) bool {
	record, contract := value.ScheduledContractRecord, value.ScheduledContract
	return record.V == graphscheduledcontract.CandidateVersion && record.ContractID == contract.ContractID &&
		record.GraphRunID == contract.GraphRunID && record.ScheduleID == contract.ScheduleID &&
		record.NodeID == contract.Node.NodeID && record.ExecutionOrdinal == uint64(contract.Node.ExecutionOrdinal) &&
		record.Attempt == contract.Node.Attempt && record.ControlSnapshotSHA256 == contract.ControlSnapshotSHA256 &&
		record.ScheduleSHA256 == contract.ScheduleSHA256 && record.ContractSHA256 == contract.ContractSHA256 &&
		record.ContractBytes == contractBytes && record.RequestID == contract.Request.RequestID &&
		record.RequestSHA256 == contract.Request.RequestSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.ExpectedLastEventSeq == contract.ExpectedLastEventSeq &&
		record.ExpectedLastEventSHA256 == contract.ExpectedLastEventSHA256 &&
		record.PredecessorReceiptCount == uint64(len(value.DirectPredecessorReceipts)) &&
		validReadyContractRecordFlags(record) && validSignedTime(record.CreatedAtMS)
}

func validReadyContractRecordFlags(record ScheduledNodeContractRecord) bool {
	return !record.LifecycleContractAdmitted && !record.ProviderRequestPresent &&
		!record.ExecutionAuthorityReleased && !record.DispatchAuthorityReleased &&
		!record.ProgressObserved && !record.SuccessorAdvanceAuthorized
}

func validReadyReceiptClosure(
	value ReadyReleaseControl,
	selected graphscheduledreconcile.ProgressNode,
) bool {
	node, ok := readyScheduledNode(value.Schedule, selected.ExecutionOrdinal)
	if !ok || node.NodeID != selected.NodeID || node.NodeID != value.ScheduledContract.Node.NodeID ||
		len(node.DirectPredecessorNodeIDs) != len(value.DirectPredecessorReceipts) ||
		len(value.ScheduledContract.Request.PredecessorTerminalReceipts) != len(value.DirectPredecessorReceipts) {
		return false
	}
	for index, receipt := range value.DirectPredecessorReceipts {
		if !validReadyReceiptAt(value, node.DirectPredecessorNodeIDs[index], index, receipt) {
			return false
		}
	}
	return true
}

func validReadyReceiptAt(
	value ReadyReleaseControl,
	nodeID string,
	index int,
	receipt scheduledterminal.Receipt,
) bool {
	scheduled, ok := readyScheduledNodeByID(value.Schedule, nodeID)
	if !ok || receipt.NodeID != nodeID || receipt.GraphRunID != value.GraphRun.GraphRunID ||
		receipt.GraphID != value.GraphRun.GraphID || receipt.Attempt != scheduled.Attempt ||
		receipt.ProjectLaneSHA256 != scheduled.ProjectLaneSHA256 || !validatedReadyReceipt(receipt) {
		return false
	}
	progress := value.ProgressSnapshot.Nodes[scheduled.ExecutionOrdinal]
	projection := value.ScheduledContract.Request.PredecessorTerminalReceipts[index]
	return validCompletedPredecessorProgress(progress, receipt) &&
		validReadyReceiptProjection(projection, receipt)
}

func validatedReadyReceipt(receipt scheduledterminal.Receipt) bool {
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		return false
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	return err == nil && decoded == receipt && receipt.NodeOutcome == "completed" &&
		receipt.ArtifactKind == "result" && !receipt.RetryAuthorized &&
		receipt.LaneReleaseAuthorized && !receipt.SuccessorAdvanceAuthorized
}

func validCompletedPredecessorProgress(
	progress graphscheduledreconcile.ProgressNode,
	receipt scheduledterminal.Receipt,
) bool {
	return progress.NodeID == receipt.NodeID && progress.Attempt == receipt.Attempt &&
		progress.ProviderRequestID != nil && *progress.ProviderRequestID == receipt.ProviderRequestID &&
		progress.LifecycleStatus != nil && *progress.LifecycleStatus == "terminalized" &&
		progress.TerminalOutcome != nil && *progress.TerminalOutcome == "completed" &&
		progress.TerminalReceiptSHA256 != nil && *progress.TerminalReceiptSHA256 == receipt.ReceiptSHA256
}

func validReadyReceiptProjection(
	projection graphscheduledcontract.PredecessorTerminalReceipt,
	receipt scheduledterminal.Receipt,
) bool {
	return projection.PredecessorNodeID == receipt.NodeID &&
		projection.PredecessorAttempt == receipt.Attempt && projection.TerminalEventSeq == 0 &&
		projection.TerminalEventSHA256 == "" && projection.TerminalReceiptID == receipt.ReceiptID &&
		projection.TerminalReceiptSHA256 == receipt.ReceiptSHA256 &&
		projection.NodeOutcome == receipt.NodeOutcome &&
		projection.ProviderRequestID == receipt.ProviderRequestID && projection.DispatchID == receipt.DispatchID
}

func readyScheduledNode(
	schedule graphschedule.ExecutionSchedule,
	ordinal uint16,
) (graphschedule.ScheduledNode, bool) {
	if int(ordinal) >= len(schedule.Nodes) {
		return graphschedule.ScheduledNode{}, false
	}
	node := schedule.Nodes[ordinal]
	return node, node.ExecutionOrdinal == ordinal
}

func readyScheduledNodeByID(
	schedule graphschedule.ExecutionSchedule,
	nodeID string,
) (graphschedule.ScheduledNode, bool) {
	for _, node := range schedule.Nodes {
		if node.NodeID == nodeID {
			return node, true
		}
	}
	return graphschedule.ScheduledNode{}, false
}
