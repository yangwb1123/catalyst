package graphscheduledrelease

import (
	"reflect"

	"forgeos/forge-core/internal/graphschedule"
)

func validateScheduleSource(value ReleaseControl) error {
	encoded, err := graphschedule.MarshalSchedule(value.Schedule)
	if err != nil || len(encoded) == 0 {
		return errInvalidControl
	}
	expected, err := graphschedule.Build(value.ControlSnapshot)
	if err != nil || !reflect.DeepEqual(expected, value.Schedule) {
		return errInvalidControl
	}
	if !validScheduleRecord(value, uint64(len(encoded))) {
		return errInvalidControl
	}
	return nil
}

func validScheduleRecord(value ReleaseControl, scheduleBytes uint64) bool {
	record, schedule := value.ScheduleRecord, value.Schedule
	return record.V == graphschedule.ExecutionScheduleVersion &&
		record.ScheduleID == schedule.ScheduleID && record.GraphRunID == schedule.GraphRunID &&
		record.GraphID == schedule.GraphID &&
		record.ControlSnapshotSHA256 == schedule.ControlSnapshotSHA256 &&
		record.ScheduleSHA256 == schedule.ScheduleSHA256 && record.ScheduleBytes == scheduleBytes &&
		record.NodeCount == uint64(schedule.NodeCount) && record.WaveCount == uint64(schedule.WaveCount) &&
		record.ExpectedLastEventSeq == schedule.ExpectedLastEventSeq &&
		record.ExpectedLastEventSHA256 == schedule.ExpectedLastEventSHA256 &&
		!record.ExecutionContractPresent && !record.DispatchAuthorityReleased &&
		validSignedTime(record.CreatedAtMS)
}
