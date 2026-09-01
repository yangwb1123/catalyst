package domain

import core "forgeos/forge-core/internal/platformcorecontract"

const spaceCreatedSchema = "forge.workspace.space_created"

// FoldSpace reconstructs one immutable Space v1 from exact canonical events.
func FoldSpace(events []*core.EventEnvelope) (Space, error) {
	event, err := creationEvent(events, "space", spaceCreatedSchema)
	if err != nil {
		return Space{}, err
	}
	if !noOptionalScope(event.ScopeRef) || event.ScopeRef.SpaceID != event.AggregateRef.EntityID ||
		event.SourceSnapshotRef != nil {
		return Space{}, invalidHistory("Space scope differs from aggregate")
	}
	if err := exactPayload(event.Payload, "name", "operation", "space_id"); err != nil {
		return Space{}, err
	}
	spaceID, err := payloadString(event.Payload, "space_id")
	if err != nil || spaceID != event.AggregateRef.EntityID {
		return Space{}, invalidHistory("Space payload identity differs")
	}
	operation, err := payloadString(event.Payload, "operation")
	if err != nil || operation != "create_space" {
		return Space{}, invalidHistory("Space operation differs")
	}
	name, err := payloadString(event.Payload, "name")
	if err != nil || validateName(name) != nil {
		return Space{}, invalidHistory("Space name is invalid")
	}
	return Space{
		SpaceID: spaceID, Name: name, CreatedBy: event.ActorRef,
		CreatedAtUnixMS: event.OccurredAtUnixMS, Version: 1,
	}, nil
}
