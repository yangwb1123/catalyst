//go:build linux && !android

package store

import (
	"context"
	"errors"
	"testing"

	"forgeos/forge-core/internal/controlstore"
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/workspace/application"
)

func TestWorkspaceReadRejectsCanonicalSemanticHistoryDrift(t *testing.T) {
	value := openIntegrationStore(t)
	spaceID := integrationID("spc", 90)
	command, event := driftedWorkspaceCommit(t, spaceID)
	if _, err := value.control.Commit(context.Background(), controlstore.CommitRequest{
		Command: command, Events: [][]byte{event}, Result: []byte(`{"stored":true}`),
		CommittedAtUnixMS: integrationUnixMS,
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := value.service.Space(context.Background(), spaceID); !errors.Is(err, application.ErrInvalidHistory) {
		t.Fatalf("semantic history drift error = %v", err)
	}
}

func TestWorkspaceListRejectsAggregateWithUnknownLaterHistory(t *testing.T) {
	value := openIntegrationStore(t)
	request := integrationSpace(91)
	if _, err := value.service.CreateSpace(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	command, event := unknownSpaceUpdate(t, request.SpaceID)
	if _, err := value.control.Commit(context.Background(), controlstore.CommitRequest{
		Command: command, Events: [][]byte{event}, Result: []byte("stored"),
		CommittedAtUnixMS: integrationUnixMS + 92,
	}); err != nil {
		t.Fatal(err)
	}
	page, err := value.service.ListSpaces(context.Background(), 0, 1)
	if !errors.Is(err, application.ErrInvalidHistory) || len(page.Items) != 0 ||
		page.More || page.NextAfterGlobalSequence != 0 {
		t.Fatalf("invalid-history list page = %+v, %v", page, err)
	}
}

func TestWorkspaceReplayRejectsCommandEventActorMismatch(t *testing.T) {
	value := openIntegrationStore(t)
	spaceID := integrationID("spc", 93)
	command, event := actorMismatchedWorkspaceCommit(t, spaceID)
	if _, err := value.control.Commit(context.Background(), controlstore.CommitRequest{
		Command: command, Events: [][]byte{event}, Result: []byte("stored"),
		CommittedAtUnixMS: integrationUnixMS + 93,
	}); err != nil {
		t.Fatal(err)
	}
	path := value.path
	value.close()
	reopened := openIntegrationPath(t, path)
	defer reopened.close()
	if _, err := reopened.service.Space(
		context.Background(), spaceID,
	); !errors.Is(err, application.ErrInvalidHistory) {
		t.Fatalf("actor-mismatched Space error = %v", err)
	}
	page, err := reopened.service.ListSpaces(context.Background(), 0, 1)
	if !errors.Is(err, application.ErrInvalidHistory) || len(page.Items) != 0 ||
		page.More || page.NextAfterGlobalSequence != 0 {
		t.Fatalf("actor-mismatched list page = %+v, %v", page, err)
	}
}

func driftedWorkspaceCommit(t *testing.T, spaceID string) ([]byte, []byte) {
	t.Helper()
	version := int64(0)
	command := core.CommandEnvelope{
		ActorRef:         core.ActorRef{ActorID: integrationID("acr", 1), ActorType: "human"},
		Canonicalization: core.CanonicalizationV1, CommandID: integrationID("cmd", 90),
		CorrelationID: integrationID("cor", 90), EnvelopeVersion: 1,
		ExpectedVersion: &version, Extensions: map[string]any{},
		IdempotencyKey: "workspace-drift-0090", IssuedAtUnixMS: integrationUnixMS,
		MessageID: integrationID("msg", 90), Payload: map[string]any{
			"name": "Drift", "operation": "create_space", "space_id": spaceID, "unexpected": true,
		},
		SchemaName: "forge.workspace.create_space", SchemaVersion: 1,
		ScopeRef:  core.ScopeRef{SpaceID: spaceID},
		TargetRef: core.EntityRef{EntityID: spaceID, EntityType: "space"},
	}
	commandBody, err := core.CanonicalCommandEnvelopeJSON(&command)
	if err != nil {
		t.Fatal(err)
	}
	cause := command.MessageID
	event := core.EventEnvelope{
		ActorRef: command.ActorRef, AggregateRef: command.TargetRef, AggregateVersion: 1,
		Canonicalization: core.CanonicalizationV1, CausationID: &cause,
		CorrelationID: command.CorrelationID, EnvelopeVersion: 1,
		EventID: integrationID("evt", 190), Extensions: map[string]any{},
		MessageID: integrationID("msg", 190), OccurredAtUnixMS: integrationUnixMS,
		Payload: command.Payload, SchemaName: "forge.workspace.space_created", SchemaVersion: 1,
		ScopeRef: command.ScopeRef, Sequence: 1, SourceComponent: "control_plane",
	}
	eventBody, err := core.CanonicalEventEnvelopeJSON(&event)
	if err != nil {
		t.Fatal(err)
	}
	return commandBody, eventBody
}

func unknownSpaceUpdate(t *testing.T, spaceID string) ([]byte, []byte) {
	t.Helper()
	version := int64(1)
	payload := map[string]any{"name": "Renamed", "operation": "rename_space", "space_id": spaceID}
	command := core.CommandEnvelope{
		ActorRef:         core.ActorRef{ActorID: integrationID("acr", 1), ActorType: "human"},
		Canonicalization: core.CanonicalizationV1, CommandID: integrationID("cmd", 92),
		CorrelationID: integrationID("cor", 92), EnvelopeVersion: 1,
		ExpectedVersion: &version, Extensions: map[string]any{},
		IdempotencyKey: "workspace-unknown-update-0092", IssuedAtUnixMS: integrationUnixMS + 92,
		MessageID: integrationID("msg", 92), Payload: payload,
		SchemaName: "forge.workspace.rename_space", SchemaVersion: 1,
		ScopeRef:  core.ScopeRef{SpaceID: spaceID},
		TargetRef: core.EntityRef{EntityID: spaceID, EntityType: "space"},
	}
	commandBody, err := core.CanonicalCommandEnvelopeJSON(&command)
	if err != nil {
		t.Fatal(err)
	}
	cause := command.MessageID
	event := core.EventEnvelope{
		ActorRef: command.ActorRef, AggregateRef: command.TargetRef, AggregateVersion: 2,
		Canonicalization: core.CanonicalizationV1, CausationID: &cause,
		CorrelationID: command.CorrelationID, EnvelopeVersion: 1,
		EventID: integrationID("evt", 192), Extensions: map[string]any{},
		MessageID: integrationID("msg", 192), OccurredAtUnixMS: integrationUnixMS + 92,
		Payload: payload, SchemaName: "forge.workspace.space_renamed", SchemaVersion: 1,
		ScopeRef: command.ScopeRef, Sequence: 2, SourceComponent: "control_plane",
	}
	eventBody, err := core.CanonicalEventEnvelopeJSON(&event)
	if err != nil {
		t.Fatal(err)
	}
	return commandBody, eventBody
}

func actorMismatchedWorkspaceCommit(t *testing.T, spaceID string) ([]byte, []byte) {
	t.Helper()
	commandBody, eventBody := driftedWorkspaceCommit(t, spaceID)
	command, err := core.DecodeCanonicalCommandEnvelope(commandBody)
	if err != nil {
		t.Fatal(err)
	}
	delete(command.Payload, "unexpected")
	command.CommandID = integrationID("cmd", 93)
	command.CorrelationID = integrationID("cor", 93)
	command.IdempotencyKey = "workspace-actor-mismatch-0093"
	command.MessageID = integrationID("msg", 93)
	commandBody, err = core.CanonicalCommandEnvelopeJSON(command)
	if err != nil {
		t.Fatal(err)
	}
	event, err := core.DecodeCanonicalEventEnvelope(eventBody)
	if err != nil {
		t.Fatal(err)
	}
	delete(event.Payload, "unexpected")
	event.ActorRef.ActorID = integrationID("acr", 2)
	event.CausationID = &command.MessageID
	event.CorrelationID = command.CorrelationID
	event.EventID = integrationID("evt", 193)
	event.MessageID = integrationID("msg", 193)
	eventBody, err = core.CanonicalEventEnvelopeJSON(event)
	if err != nil {
		t.Fatal(err)
	}
	return commandBody, eventBody
}
