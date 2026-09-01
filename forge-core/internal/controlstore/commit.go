package controlstore

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type preparedEvent struct {
	value  *core.EventEnvelope
	body   []byte
	digest string
}

type preparedOutbox struct {
	OutboxMessage
	digest string
}

type preparedCommit struct {
	command       *core.CommandEnvelope
	commandBody   []byte
	commandDigest string
	events        []preparedEvent
	outbox        []preparedOutbox
	result        []byte
	resultDigest  string
	committedAt   int64
}

// Commit appends one optimistic command outcome in a single SQLite transaction.
func (store *Store) Commit(ctx context.Context, request CommitRequest) (CommitReceipt, error) {
	if ctx == nil || store == nil || store.db == nil {
		return CommitReceipt{}, fmt.Errorf("control store commit requires an open store and context")
	}
	prepared, err := prepareCommit(request)
	if err != nil {
		return CommitReceipt{}, err
	}
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return CommitReceipt{}, fmt.Errorf("begin control commit: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	receipt, err := commitTransaction(ctx, tx, prepared)
	if err != nil {
		return CommitReceipt{}, err
	}
	if receipt.Replayed {
		return receipt, nil
	}
	if err := tx.Commit(); err != nil {
		return CommitReceipt{}, fmt.Errorf("commit control transaction: %w", err)
	}
	return receipt, nil
}

func prepareCommit(request CommitRequest) (preparedCommit, error) {
	if err := validateBody(request.Command, "command", true); err != nil {
		return preparedCommit{}, err
	}
	if err := validateBody(request.Result, "result", false); err != nil {
		return preparedCommit{}, err
	}
	if err := validateUnixMS(request.CommittedAtUnixMS, "committed_at_unix_ms"); err != nil {
		return preparedCommit{}, err
	}
	command, err := core.DecodeCanonicalCommandEnvelope(request.Command)
	if err != nil {
		return preparedCommit{}, fmt.Errorf("decode canonical command: %w", err)
	}
	if command.ExpectedVersion == nil {
		return preparedCommit{}, fmt.Errorf("durable control command requires expected_version")
	}
	events, err := prepareEvents(command, request.Events)
	if err != nil {
		return preparedCommit{}, err
	}
	outbox, err := prepareOutbox(request.Outbox, command, events)
	if err != nil {
		return preparedCommit{}, err
	}
	return preparedCommit{
		command: command, commandBody: cloneBytes(request.Command), commandDigest: digestBytes(request.Command), events: events,
		outbox: outbox, result: cloneBytes(request.Result), resultDigest: digestBytes(request.Result),
		committedAt: request.CommittedAtUnixMS,
	}, nil
}

func prepareEvents(command *core.CommandEnvelope, values [][]byte) ([]preparedEvent, error) {
	if len(values) < 1 || len(values) > maxBatchItems {
		return nil, fmt.Errorf("control commit requires 1..%d events", maxBatchItems)
	}
	events := make([]preparedEvent, 0, len(values))
	seen := make(map[string]struct{}, len(values))
	seenMessages := map[string]struct{}{command.MessageID: {}}
	var source core.SourceComponent
	for index, body := range values {
		event, err := prepareEvent(command, body, int64(index+1))
		if err != nil {
			return nil, err
		}
		if _, exists := seen[event.value.EventID]; exists {
			return nil, fmt.Errorf("duplicate event_id %s in commit", event.value.EventID)
		}
		if _, exists := seenMessages[event.value.MessageID]; exists {
			return nil, fmt.Errorf("duplicate message_id %s in commit", event.value.MessageID)
		}
		if index == 0 {
			source = event.value.SourceComponent
		} else if event.value.SourceComponent != source {
			return nil, fmt.Errorf("one commit cannot mix Control source components")
		}
		seen[event.value.EventID] = struct{}{}
		seenMessages[event.value.MessageID] = struct{}{}
		events = append(events, event)
	}
	return events, nil
}

func prepareEvent(command *core.CommandEnvelope, body []byte, ordinal int64) (preparedEvent, error) {
	if err := validateBody(body, "event", true); err != nil {
		return preparedEvent{}, err
	}
	event, err := core.DecodeCanonicalEventEnvelope(body)
	if err != nil {
		return preparedEvent{}, fmt.Errorf("decode canonical event %d: %w", ordinal, err)
	}
	if event.AggregateRef != command.TargetRef {
		return preparedEvent{}, fmt.Errorf("event %d aggregate does not match command target", ordinal)
	}
	if !isControlAggregate(event.AggregateRef.EntityType) {
		return preparedEvent{}, fmt.Errorf("aggregate type %q is not owned by Go Control", event.AggregateRef.EntityType)
	}
	wantVersion := *command.ExpectedVersion + ordinal
	if event.AggregateVersion != wantVersion {
		return preparedEvent{}, fmt.Errorf("event %d aggregate_version=%d want=%d", ordinal, event.AggregateVersion, wantVersion)
	}
	if !isControlSource(event.SourceComponent) {
		return preparedEvent{}, fmt.Errorf("control event source_component %q is not owned by Go Control", event.SourceComponent)
	}
	return preparedEvent{value: event, body: cloneBytes(body), digest: digestBytes(body)}, nil
}

func prepareOutbox(
	values []OutboxMessage, command *core.CommandEnvelope, events []preparedEvent,
) ([]preparedOutbox, error) {
	if len(values) > maxBatchItems {
		return nil, fmt.Errorf("control commit supports at most %d outbox messages", maxBatchItems)
	}
	eventIDs := make(map[string]struct{}, len(events))
	for _, event := range events {
		eventIDs[event.value.EventID] = struct{}{}
	}
	seen := map[string]struct{}{command.MessageID: {}}
	for _, event := range events {
		seen[event.value.MessageID] = struct{}{}
	}
	result := make([]preparedOutbox, 0, len(values))
	for _, value := range values {
		prepared, err := prepareOutboxMessage(value, eventIDs)
		if err != nil {
			return nil, err
		}
		if _, exists := seen[value.MessageID]; exists {
			return nil, fmt.Errorf("duplicate message_id %s in commit", value.MessageID)
		}
		seen[value.MessageID] = struct{}{}
		result = append(result, prepared)
	}
	return result, nil
}

func prepareOutboxMessage(value OutboxMessage, eventIDs map[string]struct{}) (preparedOutbox, error) {
	if err := validatePlatformID(value.MessageID, "msg", "outbox message_id"); err != nil {
		return preparedOutbox{}, err
	}
	if _, exists := eventIDs[value.SourceEvent]; !exists {
		return preparedOutbox{}, fmt.Errorf("outbox source_event must belong to the same commit")
	}
	if err := validateToken(value.Destination, "outbox destination", 64); err != nil {
		return preparedOutbox{}, err
	}
	if err := validateBody(value.Body, "outbox body", true); err != nil {
		return preparedOutbox{}, err
	}
	value.Body = cloneBytes(value.Body)
	return preparedOutbox{OutboxMessage: value, digest: digestBytes(value.Body)}, nil
}

func commitTransaction(ctx context.Context, tx *sql.Tx, value preparedCommit) (CommitReceipt, error) {
	receipt, found, err := findIdempotentReceipt(ctx, tx, value.command.IdempotencyKey)
	if err != nil {
		return CommitReceipt{}, err
	}
	if found {
		if receipt.requestDigest != value.commandDigest || !bytes.Equal(receipt.requestBody, value.commandBody) {
			return CommitReceipt{}, ErrIdempotencyConflict
		}
		return receipt.public(true)
	}
	if err := rejectIdentifierConflicts(ctx, tx, value); err != nil {
		return CommitReceipt{}, err
	}
	if err := validateCommitCausation(ctx, tx, value); err != nil {
		return CommitReceipt{}, err
	}
	current, err := loadAggregateVersion(ctx, tx, string(value.command.TargetRef.EntityType), value.command.TargetRef.EntityID)
	if err != nil {
		return CommitReceipt{}, err
	}
	if current != *value.command.ExpectedVersion {
		return CommitReceipt{}, fmt.Errorf("%w: current=%d expected=%d", ErrVersionConflict, current, *value.command.ExpectedVersion)
	}
	first, last, err := appendEvents(ctx, tx, value, current)
	if err != nil {
		return CommitReceipt{}, err
	}
	if err := appendOutbox(ctx, tx, value, first); err != nil {
		return CommitReceipt{}, err
	}
	return saveCommandReceipt(ctx, tx, value, first, last)
}

type storedReceipt struct {
	requestDigest string
	requestBody   []byte
	commandID     string
	messageID     string
	correlationID string
	causationID   sql.NullString
	aggregateType string
	aggregateID   string
	expected      int64
	version       int64
	first, last   int64
	resultDigest  string
	result        []byte
	issuedAt      int64
}

func findIdempotentReceipt(ctx context.Context, tx *sql.Tx, key string) (storedReceipt, bool, error) {
	var value storedReceipt
	err := tx.QueryRowContext(ctx, `SELECT request_sha256, request_bytes, command_id, message_id, correlation_id, causation_id, aggregate_type, aggregate_id, expected_version, aggregate_version, first_global_sequence, last_global_sequence, result_sha256, result_bytes, issued_at_unix_ms FROM command_receipts WHERE idempotency_key = ?`, key).
		Scan(&value.requestDigest, &value.requestBody, &value.commandID, &value.messageID,
			&value.correlationID, &value.causationID, &value.aggregateType,
			&value.aggregateID, &value.expected, &value.version, &value.first, &value.last,
			&value.resultDigest, &value.result, &value.issuedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return storedReceipt{}, false, nil
	}
	if err != nil {
		return storedReceipt{}, false, fmt.Errorf("read idempotency receipt: %w", err)
	}
	if err := validateStoredReceipt(key, value); err != nil {
		return storedReceipt{}, false, err
	}
	return value, true, nil
}

func validateStoredReceipt(key string, value storedReceipt) error {
	if err := verifyDigest(value.requestBody, value.requestDigest, "command request"); err != nil {
		return err
	}
	command, err := core.DecodeCanonicalCommandEnvelope(value.requestBody)
	if err != nil {
		return fmt.Errorf("%w: stored command is not canonical: %v", ErrCorruptStore, err)
	}
	if command.ExpectedVersion == nil || command.IdempotencyKey != key ||
		command.CommandID != value.commandID || command.MessageID != value.messageID ||
		command.CorrelationID != value.correlationID || !sameNullableString(command.CausationID, value.causationID) ||
		command.IssuedAtUnixMS != value.issuedAt || string(command.TargetRef.EntityType) != value.aggregateType ||
		command.TargetRef.EntityID != value.aggregateID || *command.ExpectedVersion != value.expected {
		return fmt.Errorf("%w: command receipt metadata differs from canonical bytes", ErrCorruptStore)
	}
	return verifyDigest(value.result, value.resultDigest, "command result")
}

func (value storedReceipt) public(replayed bool) (CommitReceipt, error) {
	return CommitReceipt{
		Replayed: replayed, AggregateVersion: value.version, FirstGlobalSequence: value.first,
		LastGlobalSequence: value.last, Result: cloneBytes(value.result), ResultSHA256: value.resultDigest,
	}, nil
}

func rejectIdentifierConflicts(ctx context.Context, tx *sql.Tx, value preparedCommit) error {
	if exists, err := rowExists(ctx, tx, `SELECT 1 FROM command_receipts WHERE command_id = ?`, value.command.CommandID); err != nil {
		return err
	} else if exists {
		return identifierConflict("command_id")
	}
	if exists, err := durableMessageIDExists(ctx, tx, value.command.MessageID); err != nil {
		return err
	} else if exists {
		return identifierConflict("command message_id")
	}
	for _, event := range value.events {
		if exists, err := rowExists(ctx, tx, `SELECT 1 FROM control_events WHERE event_id = ?`, event.value.EventID); err != nil {
			return err
		} else if exists {
			return identifierConflict("event_id")
		}
		if exists, err := durableMessageIDExists(ctx, tx, event.value.MessageID); err != nil {
			return err
		} else if exists {
			return identifierConflict("event message_id")
		}
	}
	for _, message := range value.outbox {
		if exists, err := durableMessageIDExists(ctx, tx, message.MessageID); err != nil {
			return err
		} else if exists {
			return identifierConflict("outbox message_id")
		}
	}
	return nil
}

func identifierConflict(label string) error {
	return fmt.Errorf("%w: %s", ErrIdentifierConflict, label)
}

func durableMessageIDExists(ctx context.Context, tx *sql.Tx, messageID string) (bool, error) {
	return rowExists(ctx, tx,
		`SELECT 1 FROM message_index WHERE message_id = ? UNION ALL SELECT 1 FROM outbox_messages WHERE message_id = ? LIMIT 1`,
		messageID, messageID,
	)
}

func rowExists(ctx context.Context, tx *sql.Tx, query string, arguments ...any) (bool, error) {
	var marker int
	err := tx.QueryRowContext(ctx, query, arguments...).Scan(&marker)
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("check durable identifier: %w", err)
	}
	return true, nil
}

func loadAggregateVersion(ctx context.Context, tx *sql.Tx, aggregateType, aggregateID string) (int64, error) {
	var version int64
	err := tx.QueryRowContext(ctx, `SELECT current_version FROM aggregate_heads WHERE aggregate_type = ? AND aggregate_id = ?`, aggregateType, aggregateID).Scan(&version)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("read aggregate head: %w", err)
	}
	return version, nil
}

func appendEvents(ctx context.Context, tx *sql.Tx, value preparedCommit, current int64) (int64, int64, error) {
	var globalHead int64
	if err := tx.QueryRowContext(ctx, `SELECT coalesce(max(global_sequence), 0) FROM control_events`).Scan(&globalHead); err != nil {
		return 0, 0, fmt.Errorf("read control journal head: %w", err)
	}
	source := string(value.events[0].value.SourceComponent)
	sourceHead, err := loadControlSourceHead(ctx, tx, source)
	if err != nil {
		return 0, 0, err
	}
	for index, event := range value.events {
		want := sourceHead + int64(index) + 1
		if event.value.Sequence != want {
			return 0, 0, fmt.Errorf("%w: source=%s event=%d want=%d", ErrSequenceConflict, source, event.value.Sequence, want)
		}
	}
	first := globalHead + 1
	if err := insertMessageIndex(ctx, tx, commandIdentity(value.command), "command", first); err != nil {
		return 0, 0, err
	}
	for index, event := range value.events {
		globalSequence := globalHead + int64(index) + 1
		if err := insertMessageIndex(ctx, tx, eventIdentity(event.value), "control_event", globalSequence); err != nil {
			return 0, 0, err
		}
		if err := insertEvent(ctx, tx, event, value.command.CommandID, globalSequence); err != nil {
			return 0, 0, err
		}
	}
	last := globalHead + int64(len(value.events))
	nextVersion := current + int64(len(value.events))
	if err := saveAggregateHead(ctx, tx, value.command, current, nextVersion); err != nil {
		return 0, 0, err
	}
	return first, last, nil
}

func loadControlSourceHead(ctx context.Context, tx *sql.Tx, source string) (int64, error) {
	var head int64
	if err := tx.QueryRowContext(ctx, `SELECT coalesce(max(source_sequence), 0) FROM control_events WHERE source_component = ?`, source).Scan(&head); err != nil {
		return 0, fmt.Errorf("read Control source head: %w", err)
	}
	return head, nil
}

func insertEvent(ctx context.Context, tx *sql.Tx, event preparedEvent, commandID string, globalSequence int64) error {
	value := event.value
	_, err := tx.ExecContext(ctx, `INSERT INTO control_events (global_sequence, command_id, event_id, message_id, correlation_id, causation_id, source_component, source_sequence, aggregate_type, aggregate_id, aggregate_version, occurred_at_unix_ms, event_sha256, event_bytes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		globalSequence, commandID, value.EventID, value.MessageID, value.CorrelationID,
		nullableString(value.CausationID), string(value.SourceComponent), value.Sequence,
		string(value.AggregateRef.EntityType), value.AggregateRef.EntityID, value.AggregateVersion,
		value.OccurredAtUnixMS, event.digest, event.body)
	if err != nil {
		return fmt.Errorf("append control event: %w", err)
	}
	return nil
}

func saveAggregateHead(ctx context.Context, tx *sql.Tx, command *core.CommandEnvelope, current, next int64) error {
	aggregateType, aggregateID := string(command.TargetRef.EntityType), command.TargetRef.EntityID
	if current == 0 {
		_, err := tx.ExecContext(ctx, `INSERT INTO aggregate_heads VALUES (?, ?, ?)`, aggregateType, aggregateID, next)
		return err
	}
	result, err := tx.ExecContext(ctx, `UPDATE aggregate_heads SET current_version = ? WHERE aggregate_type = ? AND aggregate_id = ? AND current_version = ?`, next, aggregateType, aggregateID, current)
	if err != nil {
		return fmt.Errorf("advance aggregate head: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil || changed != 1 {
		return fmt.Errorf("%w: aggregate head changed during commit", ErrVersionConflict)
	}
	return nil
}

func appendOutbox(ctx context.Context, tx *sql.Tx, value preparedCommit, firstEventSequence int64) error {
	var head int64
	if err := tx.QueryRowContext(ctx, `SELECT coalesce(max(outbox_sequence), 0) FROM outbox_messages`).Scan(&head); err != nil {
		return fmt.Errorf("read outbox head: %w", err)
	}
	for index, message := range value.outbox {
		_, err := tx.ExecContext(ctx, `INSERT INTO outbox_messages (outbox_sequence, message_id, source_event_id, destination, message_sha256, message_bytes, created_at_unix_ms) VALUES (?, ?, ?, ?, ?, ?, ?)`,
			head+int64(index)+1, message.MessageID, message.SourceEvent, message.Destination,
			message.digest, message.Body, value.committedAt)
		if err != nil {
			return fmt.Errorf("append outbox message after event %d: %w", firstEventSequence, err)
		}
	}
	return nil
}

func saveCommandReceipt(ctx context.Context, tx *sql.Tx, value preparedCommit, first, last int64) (CommitReceipt, error) {
	command := value.command
	version := *command.ExpectedVersion + int64(len(value.events))
	_, err := tx.ExecContext(ctx, `INSERT INTO command_receipts (idempotency_key, command_id, message_id, correlation_id, causation_id, request_sha256, request_bytes, aggregate_type, aggregate_id, expected_version, aggregate_version, first_global_sequence, last_global_sequence, result_sha256, result_bytes, issued_at_unix_ms, committed_at_unix_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		command.IdempotencyKey, command.CommandID, command.MessageID, command.CorrelationID,
		nullableString(command.CausationID), value.commandDigest, value.commandBody, string(command.TargetRef.EntityType),
		command.TargetRef.EntityID, *command.ExpectedVersion, version, first, last,
		value.resultDigest, value.result, command.IssuedAtUnixMS, value.committedAt)
	if err != nil {
		return CommitReceipt{}, fmt.Errorf("save command receipt: %w", err)
	}
	return CommitReceipt{
		AggregateVersion: version, FirstGlobalSequence: first, LastGlobalSequence: last,
		Result: cloneBytes(value.result), ResultSHA256: value.resultDigest,
	}, nil
}
