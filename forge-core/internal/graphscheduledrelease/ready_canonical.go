package graphscheduledrelease

func readyReleasePayload(value ReadyReleaseControl) readyReleaseControlPayload {
	return readyReleaseControlPayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: value.ReleaseControlProtocolVersion,
		GraphRun:                      value.GraphRun, JournalEvents: value.JournalEvents,
		ControlSnapshot: value.ControlSnapshot, ScheduleRecord: value.ScheduleRecord,
		Schedule: value.Schedule, ProgressSnapshot: value.ProgressSnapshot,
		ReconcileDecision:          value.ReconcileDecision,
		ScheduledContractRecord:    value.ScheduledContractRecord,
		ScheduledContract:          value.ScheduledContract,
		DirectPredecessorReceipts:  value.DirectPredecessorReceipts,
		PredecessorContentArtifact: value.PredecessorContentArtifact,
		ProviderRequest:            value.ProviderRequest, ProviderRequestJSON: value.ProviderRequestJSON,
	}
}
