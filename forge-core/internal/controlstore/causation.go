package controlstore

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type messageIdentity struct {
	messageID     string
	correlationID string
	emittedAt     int64
}

func validateCommitCausation(ctx context.Context, tx *sql.Tx, value preparedCommit) error {
	command := value.command
	if command.CausationID != nil {
		cause, found, err := findDurableMessage(ctx, tx, *command.CausationID)
		if err != nil {
			return err
		}
		if !found {
			return fmt.Errorf("command causation_id does not resolve to durable history")
		}
		if err := validateCause(cause, command.CorrelationID, command.IssuedAtUnixMS); err != nil {
			return fmt.Errorf("command causation: %w", err)
		}
	}
	known := map[string]messageIdentity{command.MessageID: commandIdentity(command)}
	for index, event := range value.events {
		if err := validateControlEventCause(event.value, known); err != nil {
			return fmt.Errorf("event %d causation: %w", index+1, err)
		}
		known[event.value.MessageID] = eventIdentity(event.value)
	}
	return nil
}

func validateControlEventCause(event *core.EventEnvelope, known map[string]messageIdentity) error {
	if event.CausationID == nil {
		return fmt.Errorf("Control event requires causation_id")
	}
	cause, found := known[*event.CausationID]
	if !found {
		return fmt.Errorf("cause must be the current command or an earlier event in its commit")
	}
	return validateCause(cause, event.CorrelationID, event.OccurredAtUnixMS)
}

func validateInboxCausation(ctx context.Context, tx *sql.Tx, event *core.EventEnvelope) error {
	if event.CausationID == nil {
		return nil
	}
	cause, found, err := findDurableMessage(ctx, tx, *event.CausationID)
	if err != nil {
		return err
	}
	if !found {
		return fmt.Errorf("inbox causation_id does not resolve to durable history")
	}
	return validateCause(cause, event.CorrelationID, event.OccurredAtUnixMS)
}

func validateCause(cause messageIdentity, correlationID string, emittedAt int64) error {
	if cause.correlationID != correlationID {
		return fmt.Errorf("cause correlation_id differs")
	}
	if cause.emittedAt > emittedAt {
		return fmt.Errorf("cause does not precede the message")
	}
	return nil
}

func findDurableMessage(ctx context.Context, tx *sql.Tx, messageID string) (messageIdentity, bool, error) {
	var value messageIdentity
	err := tx.QueryRowContext(ctx, `SELECT message_id, correlation_id, emitted_at_unix_ms FROM message_index WHERE message_id = ?`, messageID).
		Scan(&value.messageID, &value.correlationID, &value.emittedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return messageIdentity{}, false, nil
	}
	if err != nil {
		return messageIdentity{}, false, fmt.Errorf("resolve durable cause: %w", err)
	}
	return value, true, nil
}

func insertMessageIndex(
	ctx context.Context,
	tx *sql.Tx,
	value messageIdentity,
	kind string,
	sequence int64,
) error {
	_, err := tx.ExecContext(ctx, `INSERT INTO message_index VALUES (?, ?, ?, ?, ?)`,
		value.messageID, value.correlationID, value.emittedAt, kind, sequence)
	if err != nil {
		return fmt.Errorf("append %s message identity: %w", kind, err)
	}
	return nil
}

func commandIdentity(value *core.CommandEnvelope) messageIdentity {
	return messageIdentity{
		messageID: value.MessageID, correlationID: value.CorrelationID, emittedAt: value.IssuedAtUnixMS,
	}
}

func eventIdentity(value *core.EventEnvelope) messageIdentity {
	return messageIdentity{
		messageID: value.MessageID, correlationID: value.CorrelationID, emittedAt: value.OccurredAtUnixMS,
	}
}

func nullableString(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}
