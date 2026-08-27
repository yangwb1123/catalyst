package graphscheduledrelease

func validateReadyReleaseControl(value ReadyReleaseControl) error {
	if !validReadyControlHeader(value) {
		return errInvalidControl
	}
	legacy := readyLegacyProjection(value)
	if validateGraphSource(legacy) != nil || validateScheduleSource(legacy) != nil {
		return errInvalidControl
	}
	selected, err := validateReadyProgress(value)
	if err != nil || validateReadyContractSource(value, selected) != nil ||
		validateReadyProviderSource(value) != nil {
		return errInvalidControl
	}
	digest, err := domainDigest(readyReleaseControlDigestDomain, readyReleasePayload(value))
	if err != nil || digest != value.SnapshotSHA256 {
		return errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxReadyReleaseControlBytes {
		return errInvalidControl
	}
	return nil
}

func validReadyControlHeader(value ReadyReleaseControl) bool {
	return value.V == ReadyReleaseControlVersion && value.SchedulerProtocolVersion == 1 &&
		value.ReleaseControlProtocolVersion == ReadyReleaseControlProtocol &&
		value.JournalEvents != nil && len(value.JournalEvents) == 1 &&
		value.DirectPredecessorReceipts != nil && isLowerHexDigest(value.SnapshotSHA256)
}

func readyLegacyProjection(value ReadyReleaseControl) ReleaseControl {
	return ReleaseControl{
		V: ReleaseControlVersion, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: ReleaseControlProtocol,
		GraphRun:                      value.GraphRun, JournalEvents: value.JournalEvents,
		ControlSnapshot: value.ControlSnapshot, ScheduleRecord: value.ScheduleRecord,
		Schedule: value.Schedule, ScheduledContractRecord: value.ScheduledContractRecord,
		ScheduledContract: value.ScheduledContract, ProviderRequest: value.ProviderRequest,
		ProviderRequestJSON: value.ProviderRequestJSON,
	}
}
