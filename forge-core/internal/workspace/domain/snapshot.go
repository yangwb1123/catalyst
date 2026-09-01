package domain

import core "forgeos/forge-core/internal/platformcorecontract"

const snapshotRecordedSchema = "forge.workspace.project_snapshot_recorded"

// FoldProjectSnapshot reconstructs one immutable Snapshot reference v1.
func FoldProjectSnapshot(events []*core.EventEnvelope) (ProjectSnapshot, error) {
	event, err := creationEvent(events, "project_snapshot", snapshotRecordedSchema)
	if err != nil {
		return ProjectSnapshot{}, err
	}
	if !snapshotScopeMatches(event) {
		return ProjectSnapshot{}, invalidHistory("Snapshot scope differs from aggregate")
	}
	fields := []string{
		"captured_at_unix_ms", "observation_ref", "observation_status", "operation",
		"project_id", "project_snapshot_id", "space_id",
	}
	if err := exactPayload(event.Payload, fields...); err != nil {
		return ProjectSnapshot{}, err
	}
	result, err := snapshotPayload(event)
	if err != nil {
		return ProjectSnapshot{}, err
	}
	result.RecordedBy = event.ActorRef
	result.RecordedAtUnixMS = event.OccurredAtUnixMS
	result.Version = 1
	return result, nil
}

func snapshotScopeMatches(event *core.EventEnvelope) bool {
	scope := event.ScopeRef
	return scope.SpaceID != "" && scope.ProjectID != nil &&
		pointerEquals(scope.ProjectSnapshotID, event.AggregateRef.EntityID) &&
		event.SourceSnapshotRef != nil && *event.SourceSnapshotRef == event.AggregateRef &&
		scope.ActionID == nil && scope.AttemptID == nil && scope.ChangeID == nil &&
		scope.ObjectiveID == nil && scope.SessionID == nil && scope.TurnID == nil &&
		scope.WorkGraphID == nil && scope.WorkItemID == nil
}

func snapshotPayload(event *core.EventEnvelope) (ProjectSnapshot, error) {
	snapshotID, _ := payloadString(event.Payload, "project_snapshot_id")
	projectID, _ := payloadString(event.Payload, "project_id")
	spaceID, _ := payloadString(event.Payload, "space_id")
	operation, _ := payloadString(event.Payload, "operation")
	status, _ := payloadString(event.Payload, "observation_status")
	if snapshotID != event.AggregateRef.EntityID ||
		!pointerEquals(event.ScopeRef.ProjectID, projectID) || spaceID != event.ScopeRef.SpaceID ||
		operation != "record_project_snapshot" || status != ObservationDeclaredUnresolved {
		return ProjectSnapshot{}, invalidHistory("Snapshot payload identities differ")
	}
	capturedAt, err := payloadInt64(event.Payload, "captured_at_unix_ms")
	if err != nil || validateUnixMS(capturedAt, "captured_at_unix_ms") != nil ||
		capturedAt > event.OccurredAtUnixMS {
		return ProjectSnapshot{}, invalidHistory("Snapshot capture time is invalid")
	}
	observation, err := observationPayload(event.Payload)
	if err != nil {
		return ProjectSnapshot{}, err
	}
	return ProjectSnapshot{
		ProjectSnapshotID: snapshotID, ProjectID: projectID, SpaceID: spaceID,
		ObservationRef: observation, ObservationStatus: status, CapturedAtUnixMS: capturedAt,
	}, nil
}

func observationPayload(payload map[string]any) (core.RecordRef, error) {
	value, err := payloadObject(payload, "observation_ref")
	if err != nil {
		return core.RecordRef{}, err
	}
	if err := exactPayload(value, "record_id", "record_sha256", "record_type"); err != nil {
		return core.RecordRef{}, err
	}
	result := core.RecordRef{}
	result.RecordID, _ = payloadString(value, "record_id")
	result.RecordSHA256, _ = payloadString(value, "record_sha256")
	result.RecordType, _ = payloadString(value, "record_type")
	if err := validateObservation(result); err != nil {
		return core.RecordRef{}, err
	}
	return result, nil
}
