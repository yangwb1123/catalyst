package graphscheduledrelease

import "forgeos/forge-core/internal/graphscheduledcontract"

func validateContractSource(value ReleaseControl) error {
	candidate := value.ScheduledContract
	if graphscheduledcontract.ValidateCandidateSource(candidate, value.ControlSnapshot) != nil {
		return errInvalidControl
	}
	encoded, err := graphscheduledcontract.MarshalCandidate(candidate)
	if err != nil || len(encoded) == 0 || !validContractRecord(value, uint64(len(encoded))) {
		return errInvalidControl
	}
	return nil
}

func validContractRecord(value ReleaseControl, contractBytes uint64) bool {
	record, contract := value.ScheduledContractRecord, value.ScheduledContract
	return record.V == graphscheduledcontract.CandidateVersion && record.ContractID == contract.ContractID &&
		record.GraphRunID == contract.GraphRunID && record.ScheduleID == contract.ScheduleID &&
		record.NodeID == contract.Node.NodeID && record.ExecutionOrdinal == uint64(contract.Node.ExecutionOrdinal) &&
		record.Attempt == contract.Node.Attempt &&
		record.ControlSnapshotSHA256 == contract.ControlSnapshotSHA256 &&
		record.ScheduleSHA256 == contract.ScheduleSHA256 &&
		record.ContractSHA256 == contract.ContractSHA256 && record.ContractBytes == contractBytes &&
		record.RequestID == contract.Request.RequestID &&
		record.RequestSHA256 == contract.Request.RequestSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.ExpectedLastEventSeq == contract.ExpectedLastEventSeq &&
		record.ExpectedLastEventSHA256 == contract.ExpectedLastEventSHA256 &&
		validContractRecordFlags(record) && validSignedTime(record.CreatedAtMS)
}

func validContractRecordFlags(record ScheduledNodeContractRecord) bool {
	return record.PredecessorReceiptCount == 0 && !record.LifecycleContractAdmitted &&
		!record.ProviderRequestPresent && !record.ExecutionAuthorityReleased &&
		!record.DispatchAuthorityReleased && !record.ProgressObserved &&
		!record.SuccessorAdvanceAuthorized
}
