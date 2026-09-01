package controlstore

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const storedEventColumns = `e.global_sequence, e.command_id, e.event_id, e.message_id, e.correlation_id, e.causation_id, e.source_component, e.source_sequence, e.aggregate_type, e.aggregate_id, e.aggregate_version, e.occurred_at_unix_ms, e.event_sha256, e.event_bytes, r.request_sha256, r.request_bytes`

const storedEventTables = `control_events e LEFT JOIN command_receipts r ON r.command_id = e.command_id`

const pendingOutboxPageQuery = `SELECT o.outbox_sequence, o.message_id, o.source_event_id,
o.destination, o.created_at_unix_ms, o.message_sha256, o.message_bytes, a.message_id,
(SELECT coalesce(max(head.outbox_sequence), 0) FROM outbox_messages head)
FROM outbox_messages o LEFT JOIN outbox_acknowledgements a USING (message_id)
WHERE o.outbox_sequence > ? ORDER BY o.outbox_sequence LIMIT ?`

// ControlSourceHead returns one Go-Control component stream's durable sequence.
func (store *Store) ControlSourceHead(ctx context.Context, source string) (int64, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return 0, err
	}
	if !isControlSource(core.SourceComponent(source)) {
		return 0, fmt.Errorf("source_component is not owned by Go Control")
	}
	var head int64
	if err := store.db.QueryRowContext(ctx, `SELECT coalesce(max(source_sequence), 0) FROM control_events WHERE source_component = ?`, source).Scan(&head); err != nil {
		return 0, fmt.Errorf("read Control source head: %w", err)
	}
	if head < 0 {
		return 0, fmt.Errorf("%w: Control source head is negative", ErrCorruptStore)
	}
	return head, nil
}

// AggregateVersion returns the durable current version, or zero for a new aggregate.
func (store *Store) AggregateVersion(ctx context.Context, aggregateType, aggregateID string) (int64, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return 0, err
	}
	if err := validateToken(aggregateType, "aggregate_type", 32); err != nil {
		return 0, err
	}
	if err := core.ValidatePlatformID(aggregateID); err != nil {
		return 0, fmt.Errorf("aggregate_id must be a Platform ID")
	}
	var version int64
	err := store.db.QueryRowContext(ctx, `SELECT current_version FROM aggregate_heads WHERE aggregate_type = ? AND aggregate_id = ?`, aggregateType, aggregateID).Scan(&version)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("read aggregate version: %w", err)
	}
	return version, nil
}

// InboxCursor returns source, sequence and found for one durable source stream.
func (store *Store) InboxCursor(ctx context.Context, streamID string) (string, int64, bool, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return "", 0, false, err
	}
	if err := validateToken(streamID, "stream_id", 128); err != nil {
		return "", 0, false, err
	}
	var source string
	var sequence int64
	err := store.db.QueryRowContext(ctx, `SELECT source_component, current_sequence FROM inbox_sources WHERE stream_id = ?`, streamID).
		Scan(&source, &sequence)
	if errors.Is(err, sql.ErrNoRows) {
		return "", 0, false, nil
	}
	if err != nil {
		return "", 0, false, fmt.Errorf("read inbox cursor: %w", err)
	}
	if sequence < 1 || isControlSource(core.SourceComponent(source)) ||
		(source != "runtime" && source != "harness") {
		return "", 0, false, fmt.Errorf("%w: inbox cursor metadata is invalid", ErrCorruptStore)
	}
	return source, sequence, true, nil
}

// Events returns an integrity-checked bounded page ordered by global sequence.
func (store *Store) Events(ctx context.Context, after int64, limit int) ([]StoredEvent, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return nil, err
	}
	if err := validatePage(after, limit); err != nil {
		return nil, err
	}
	rows, err := store.db.QueryContext(ctx, `SELECT `+storedEventColumns+` FROM `+storedEventTables+`
WHERE e.global_sequence > ? ORDER BY e.global_sequence LIMIT ?`, after, limit)
	if err != nil {
		return nil, fmt.Errorf("read control events: %w", err)
	}
	defer func() { _ = rows.Close() }()
	return scanEvents(rows)
}

func scanEvents(rows *sql.Rows) ([]StoredEvent, error) {
	var result []StoredEvent
	for rows.Next() {
		var event StoredEvent
		var causation sql.NullString
		var commandDigest sql.NullString
		var commandBody []byte
		if err := rows.Scan(
			&event.GlobalSequence, &event.CommandID, &event.EventID, &event.MessageID,
			&event.CorrelationID, &causation, &event.SourceComponent, &event.SourceSequence,
			&event.AggregateType, &event.AggregateID,
			&event.AggregateVersion, &event.OccurredAtUnixMS, &event.BodySHA256, &event.Body,
			&commandDigest, &commandBody,
		); err != nil {
			return nil, fmt.Errorf("scan control event: %w", err)
		}
		if causation.Valid {
			event.CausationID = &causation.String
		}
		if err := validateStoredEvent(event); err != nil {
			return nil, err
		}
		if err := bindStoredCommand(&event, commandDigest, commandBody); err != nil {
			return nil, err
		}
		event.Body = cloneBytes(event.Body)
		result = append(result, event)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate control events: %w", err)
	}
	return result, nil
}

func bindStoredCommand(
	stored *StoredEvent, digest sql.NullString, body []byte,
) error {
	if !digest.Valid || len(body) == 0 {
		return fmt.Errorf("%w: stored event has no originating command", ErrCorruptStore)
	}
	if err := verifyDigest(body, digest.String, "originating command"); err != nil {
		return err
	}
	command, err := core.DecodeCanonicalCommandEnvelope(body)
	if err != nil {
		return fmt.Errorf("%w: originating command is not canonical: %v", ErrCorruptStore, err)
	}
	if command.CommandID != stored.CommandID || command.TargetRef.EntityID != stored.AggregateID ||
		string(command.TargetRef.EntityType) != stored.AggregateType ||
		command.CorrelationID != stored.CorrelationID || command.ExpectedVersion == nil ||
		*command.ExpectedVersion >= stored.AggregateVersion {
		return fmt.Errorf("%w: originating command metadata differs", ErrCorruptStore)
	}
	stored.CommandActorID = command.ActorRef.ActorID
	stored.CommandActorType = string(command.ActorRef.ActorType)
	return nil
}

func validateStoredEvent(stored StoredEvent) error {
	if err := verifyDigest(stored.Body, stored.BodySHA256, "control event"); err != nil {
		return err
	}
	event, err := core.DecodeCanonicalEventEnvelope(stored.Body)
	if err != nil {
		return fmt.Errorf("%w: stored event is not canonical: %v", ErrCorruptStore, err)
	}
	if event.Sequence != stored.SourceSequence || event.EventID != stored.EventID ||
		event.MessageID != stored.MessageID || event.CorrelationID != stored.CorrelationID ||
		!sameStringPointer(event.CausationID, stored.CausationID) ||
		string(event.SourceComponent) != stored.SourceComponent ||
		string(event.AggregateRef.EntityType) != stored.AggregateType ||
		event.AggregateRef.EntityID != stored.AggregateID ||
		event.AggregateVersion != stored.AggregateVersion ||
		event.OccurredAtUnixMS != stored.OccurredAtUnixMS {
		return fmt.Errorf("%w: stored event metadata differs from canonical bytes", ErrCorruptStore)
	}
	return nil
}

// PendingOutboxMessages filters one bounded raw outbox sequence window.
func (store *Store) PendingOutboxMessages(
	ctx context.Context, after int64, limit int,
) (PendingOutboxPage, error) {
	initial := PendingOutboxPage{NextAfterOutboxSequence: after}
	if err := validateReadContext(ctx, store); err != nil {
		return initial, err
	}
	if err := validatePage(after, limit); err != nil {
		return initial, err
	}
	rows, err := store.db.QueryContext(ctx, pendingOutboxPageQuery, after, limit)
	if err != nil {
		return initial, fmt.Errorf("read pending outbox: %w", err)
	}
	defer func() { _ = rows.Close() }()
	page, err := scanOutboxPage(rows, after)
	if err != nil {
		return initial, err
	}
	return page, nil
}

func scanOutboxPage(rows *sql.Rows, after int64) (PendingOutboxPage, error) {
	page := PendingOutboxPage{NextAfterOutboxSequence: after}
	for rows.Next() {
		message, acknowledged, head, err := scanOutboxRow(rows)
		if err != nil {
			return PendingOutboxPage{}, err
		}
		page.NextAfterOutboxSequence = message.OutboxSequence
		page.More = message.OutboxSequence < head
		if !acknowledged {
			page.Items = append(page.Items, message)
		}
	}
	if err := rows.Err(); err != nil {
		return PendingOutboxPage{}, fmt.Errorf("iterate pending outbox: %w", err)
	}
	return page, nil
}

func scanOutboxRow(rows *sql.Rows) (PendingOutbox, bool, int64, error) {
	var message PendingOutbox
	var acknowledgement sql.NullString
	var head int64
	if err := rows.Scan(
		&message.OutboxSequence, &message.MessageID, &message.SourceEvent,
		&message.Destination, &message.CreatedAtUnixMS, &message.BodySHA256, &message.Body,
		&acknowledgement, &head,
	); err != nil {
		return PendingOutbox{}, false, 0, fmt.Errorf("scan pending outbox: %w", err)
	}
	if head < message.OutboxSequence ||
		(acknowledgement.Valid && acknowledgement.String != message.MessageID) {
		return PendingOutbox{}, false, 0, fmt.Errorf("%w: outbox page metadata differs", ErrCorruptStore)
	}
	if err := verifyDigest(message.Body, message.BodySHA256, "outbox message"); err != nil {
		return PendingOutbox{}, false, 0, err
	}
	message.Body = cloneBytes(message.Body)
	return message, acknowledgement.Valid, head, nil
}

// AcknowledgeOutbox records an immutable delivery acknowledgement idempotently.
func (store *Store) AcknowledgeOutbox(ctx context.Context, messageID string, deliveredAtUnixMS int64) (bool, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return false, err
	}
	if err := validatePlatformID(messageID, "msg", "message_id"); err != nil {
		return false, err
	}
	if err := validateUnixMS(deliveredAtUnixMS, "delivered_at_unix_ms"); err != nil {
		return false, err
	}
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return false, fmt.Errorf("begin outbox acknowledgement: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	replayed, err := acknowledgeOutbox(ctx, tx, messageID, deliveredAtUnixMS)
	if err != nil {
		return false, err
	}
	if !replayed {
		err = tx.Commit()
	}
	return replayed, err
}

func acknowledgeOutbox(ctx context.Context, tx *sql.Tx, messageID string, deliveredAt int64) (bool, error) {
	var existing int64
	err := tx.QueryRowContext(ctx, `SELECT delivered_at_unix_ms FROM outbox_acknowledgements WHERE message_id = ?`, messageID).Scan(&existing)
	if err == nil {
		return true, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return false, fmt.Errorf("read outbox acknowledgement: %w", err)
	}
	if exists, err := rowExists(ctx, tx, `SELECT 1 FROM outbox_messages WHERE message_id = ?`, messageID); err != nil {
		return false, err
	} else if !exists {
		return false, fmt.Errorf("outbox message does not exist")
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO outbox_acknowledgements VALUES (?, ?)`, messageID, deliveredAt)
	if err != nil {
		return false, fmt.Errorf("append outbox acknowledgement: %w", err)
	}
	return false, nil
}

func validateReadContext(ctx context.Context, store *Store) error {
	if ctx == nil || store == nil || store.db == nil {
		return fmt.Errorf("control store operation requires an open store and context")
	}
	return nil
}
