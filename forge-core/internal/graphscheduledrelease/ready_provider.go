package graphscheduledrelease

func validateReadyProviderSource(value ReadyReleaseControl) error {
	body := []byte(value.ProviderRequestJSON)
	bodySHA256, err := validateProviderBody(readyLegacyProjection(value), body)
	if err != nil || !validReadyProviderRecord(value, body, bodySHA256) ||
		validateProviderIdentities(value.ProviderRequest) != nil {
		return errInvalidControl
	}
	return nil
}

func validReadyProviderRecord(
	value ReadyReleaseControl,
	body []byte,
	bodySHA256 string,
) bool {
	record, contract := value.ProviderRequest, value.ScheduledContract
	return validReadyProviderRecordHeader(record, body, bodySHA256) &&
		record.GraphRunID == contract.GraphRunID && record.ScheduleID == contract.ScheduleID &&
		record.ScheduledContractID == contract.ContractID &&
		record.ExecutionOrdinal == uint64(contract.Node.ExecutionOrdinal) &&
		record.NodeID == contract.Node.NodeID && record.Attempt == contract.Node.Attempt &&
		record.ScheduledContractSHA256 == contract.ContractSHA256 &&
		record.LogicalRequestID == contract.Request.RequestID &&
		record.LogicalRequestSHA256 == contract.Request.RequestSHA256 &&
		record.ScheduleSHA256 == contract.ScheduleSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.Provider == contract.Provider.Kind && record.Endpoint == contract.Provider.Endpoint &&
		record.Model == contract.Provider.Model &&
		record.PricingSnapshotSHA256 == contract.Budgets.PricingSnapshotSHA256 &&
		record.ExpectedLastEventSeq == contract.ExpectedLastEventSeq &&
		record.ExpectedLastEventSHA256 == contract.ExpectedLastEventSHA256
}

func validReadyProviderRecordHeader(
	record ScheduledNodeProviderRequestRecord,
	body []byte,
	bodySHA256 string,
) bool {
	return record.V == 1 && record.ExecutionOrdinal <= 31 && record.Attempt == 1 &&
		record.Provider == "openai_responses" && record.CodecProtocolVersion == 1 &&
		record.ProviderRequestSHA256 == bodySHA256 &&
		record.ProviderRequestBytes == uint64(len(body)) && record.ExpectedLastEventSeq == 1 &&
		record.ProviderRequestPrepared && !record.ProviderRequestSent &&
		!record.LifecycleContractAdmitted && !record.ExecutionAuthorityReleased &&
		!record.DispatchAuthorityReleased && !record.ProjectLaneClaimed &&
		!record.ProgressObserved && !record.SuccessorAdvanceAuthorized &&
		validSignedTime(record.CreatedAtMS)
}
