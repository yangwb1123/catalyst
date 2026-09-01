package application

import (
	"bytes"
	"encoding/json"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type preparedCreation struct {
	plan         creationPlan
	command      []byte
	eventID      string
	eventMessage string
	result       []byte
}

type commandResult struct {
	AggregateID      string `json:"aggregate_id"`
	AggregateType    string `json:"aggregate_type"`
	AggregateVersion int64  `json:"aggregate_version"`
	SchemaName       string `json:"schema_name"`
	SchemaVersion    int64  `json:"schema_version"`
}

func (service *Service) prepareCreation(plan creationPlan) (preparedCreation, error) {
	if plan.meta.ExpectedVersion != 0 {
		return preparedCreation{}, invalidInput("creation expected_version must be zero", nil)
	}
	eventID, err := service.identities("evt")
	if err != nil {
		return preparedCreation{}, err
	}
	eventMessage, err := messageForEvent(eventID)
	if err != nil {
		return preparedCreation{}, err
	}
	command, err := canonicalCommand(plan)
	if err != nil {
		return preparedCreation{}, invalidInput("canonical command", err)
	}
	result, err := canonicalResult(plan.target)
	if err != nil {
		return preparedCreation{}, err
	}
	prepared := preparedCreation{
		plan: plan, command: command, eventID: eventID,
		eventMessage: eventMessage, result: result,
	}
	event, _, err := prepared.canonicalEvent(1)
	if err != nil {
		return preparedCreation{}, invalidInput("canonical event", err)
	}
	if err := plan.validate(event); err != nil {
		return preparedCreation{}, invalidInput("Workspace creation semantics", err)
	}
	return prepared, nil
}

func canonicalCommand(plan creationPlan) ([]byte, error) {
	expected := plan.meta.ExpectedVersion
	command := core.CommandEnvelope{
		ActorRef: plan.meta.ActorRef, Canonicalization: core.CanonicalizationV1,
		CommandID: plan.meta.CommandID, CorrelationID: plan.meta.CorrelationID,
		EnvelopeVersion: core.EnvelopeVersionV1, ExpectedVersion: &expected,
		Extensions: map[string]any{}, IdempotencyKey: plan.meta.IdempotencyKey,
		IssuedAtUnixMS: plan.meta.IssuedAtUnixMS, MessageID: plan.meta.MessageID,
		Payload: plan.payload, SchemaName: plan.commandSchema, SchemaVersion: 1,
		ScopeRef: plan.scope, TargetRef: plan.target,
	}
	return core.CanonicalCommandEnvelopeJSON(&command)
}

func (value preparedCreation) canonicalEvent(sequence int64) (*core.EventEnvelope, []byte, error) {
	cause := value.plan.meta.MessageID
	event := core.EventEnvelope{
		ActorRef: value.plan.meta.ActorRef, AggregateRef: value.plan.target,
		AggregateVersion: 1, Canonicalization: core.CanonicalizationV1,
		CausationID: &cause, CorrelationID: value.plan.meta.CorrelationID,
		EnvelopeVersion: core.EnvelopeVersionV1, EventID: value.eventID,
		Extensions: map[string]any{}, MessageID: value.eventMessage,
		OccurredAtUnixMS: value.plan.meta.IssuedAtUnixMS, Payload: value.plan.payload,
		SchemaName: value.plan.eventSchema, SchemaVersion: 1, ScopeRef: value.plan.scope,
		Sequence: sequence, SourceComponent: "control_plane",
		SourceSnapshotRef: value.plan.sourceSnapshot,
	}
	body, err := core.CanonicalEventEnvelopeJSON(&event)
	return &event, body, err
}

func canonicalResult(target core.EntityRef) ([]byte, error) {
	value := commandResult{
		AggregateID: target.EntityID, AggregateType: string(target.EntityType),
		AggregateVersion: 1, SchemaName: "forge.workspace.command_result", SchemaVersion: 1,
	}
	return json.Marshal(value)
}

func validateReceipt(receipt JournalReceipt, expected []byte) error {
	if receipt.AggregateVersion != 1 || !bytes.Equal(receipt.Result, expected) {
		return fmt.Errorf("%w: command result differs", ErrInvalidHistory)
	}
	return nil
}

func invalidInput(message string, cause error) error {
	if cause == nil {
		return fmt.Errorf("%w: %s", ErrInvalidInput, message)
	}
	return fmt.Errorf("%w: %s: %v", ErrInvalidInput, message, cause)
}
