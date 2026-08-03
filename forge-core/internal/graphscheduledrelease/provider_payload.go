package graphscheduledrelease

func preparedPayload(value ScheduledNodeProviderRequestRecord) preparedRequestPayload {
	return preparedRequestPayload{
		V: value.V, CodecProtocolVersion: value.CodecProtocolVersion,
		GraphRunID: value.GraphRunID, ScheduleID: value.ScheduleID,
		ScheduleSHA256: value.ScheduleSHA256, ScheduledContractID: value.ScheduledContractID,
		ScheduledContractSHA256: value.ScheduledContractSHA256,
		ExpectedLastEventSeq:    value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: value.ExpectedLastEventSHA256,
		ExecutionOrdinal:        value.ExecutionOrdinal, NodeID: value.NodeID, Attempt: value.Attempt,
		ProjectLaneSHA256: value.ProjectLaneSHA256, ProviderKind: value.Provider,
		Endpoint: value.Endpoint, Model: value.Model, DestinationSHA256: value.DestinationSHA256,
		LogicalRequestID:           value.LogicalRequestID,
		LogicalRequestSHA256:       value.LogicalRequestSHA256,
		PricingSnapshotSHA256:      value.PricingSnapshotSHA256,
		RequestBodyBytes:           value.ProviderRequestBytes,
		RequestBodySHA256:          value.ProviderRequestSHA256,
		ProviderRequestPrepared:    value.ProviderRequestPrepared,
		ProviderRequestSent:        value.ProviderRequestSent,
		LifecycleContractAdmitted:  value.LifecycleContractAdmitted,
		ExecutionAuthorityReleased: value.ExecutionAuthorityReleased,
		DispatchAuthorityReleased:  value.DispatchAuthorityReleased,
		ProjectLaneClaimed:         value.ProjectLaneClaimed, ProgressObserved: value.ProgressObserved,
		SuccessorAdvanceAuthorized: value.SuccessorAdvanceAuthorized,
	}
}
