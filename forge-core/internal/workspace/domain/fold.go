package domain

import (
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const workspaceSchemaVersion = int64(1)

func creationEvent(
	events []*core.EventEnvelope,
	aggregateType, schemaName string,
) (*core.EventEnvelope, error) {
	if len(events) != 1 || events[0] == nil {
		return nil, invalidHistory("v1 aggregate requires exactly one creation event")
	}
	event := events[0]
	if err := core.ValidateEventEnvelope(event); err != nil {
		return nil, invalidHistory("creation event violates Platform Core")
	}
	if string(event.AggregateRef.EntityType) != aggregateType ||
		event.AggregateVersion != 1 {
		return nil, invalidHistory("creation aggregate identity or version differs")
	}
	if event.SchemaName != schemaName || event.SchemaVersion != workspaceSchemaVersion {
		return nil, invalidHistory("creation event schema differs")
	}
	if string(event.SourceComponent) != "control_plane" || len(event.Extensions) != 0 {
		return nil, invalidHistory("creation source or extensions differ")
	}
	if event.CausationID == nil || event.Payload == nil || event.PayloadArtifactRef != nil {
		return nil, invalidHistory("creation causation or payload form differs")
	}
	return event, nil
}

func invalidHistory(message string) error {
	return fmt.Errorf("%w: %s", ErrInvalidHistory, message)
}

func pointerEquals(value *string, expected string) bool {
	return value != nil && *value == expected
}

func noOptionalScope(scope core.ScopeRef) bool {
	return scope.ActionID == nil && scope.AttemptID == nil &&
		scope.ChangeID == nil && scope.ObjectiveID == nil &&
		scope.ProjectID == nil && scope.ProjectSnapshotID == nil &&
		scope.SessionID == nil && scope.TurnID == nil &&
		scope.WorkGraphID == nil && scope.WorkItemID == nil
}
