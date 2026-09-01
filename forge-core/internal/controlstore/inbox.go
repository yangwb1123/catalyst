package controlstore

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type preparedInbox struct {
	streamID   string
	event      *core.EventEnvelope
	body       []byte
	digest     string
	receivedAt int64
}

// RecordInbox appends one exact canonical external event while enforcing a
// contiguous per-source stream sequence.
func (store *Store) RecordInbox(
	ctx context.Context,
	streamID string,
	eventBytes []byte,
	receivedAtUnixMS int64,
) (InboxReceipt, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return InboxReceipt{}, err
	}
	prepared, err := prepareInbox(streamID, eventBytes, receivedAtUnixMS)
	if err != nil {
		return InboxReceipt{}, err
	}
	tx, err := store.db.BeginTx(ctx, nil)
	if err != nil {
		return InboxReceipt{}, fmt.Errorf("begin inbox append: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	receipt, err := recordInbox(ctx, tx, prepared)
	if err != nil {
		return InboxReceipt{}, err
	}
	if receipt.Replayed {
		return receipt, nil
	}
	if err := tx.Commit(); err != nil {
		return InboxReceipt{}, fmt.Errorf("commit inbox append: %w", err)
	}
	return receipt, nil
}

func prepareInbox(streamID string, eventBytes []byte, receivedAt int64) (preparedInbox, error) {
	if err := validateToken(streamID, "stream_id", 128); err != nil {
		return preparedInbox{}, err
	}
	if err := validateBody(eventBytes, "inbox event", true); err != nil {
		return preparedInbox{}, err
	}
	if err := validateUnixMS(receivedAt, "received_at_unix_ms"); err != nil {
		return preparedInbox{}, err
	}
	event, err := core.DecodeCanonicalEventEnvelope(eventBytes)
	if err != nil {
		return preparedInbox{}, fmt.Errorf("decode canonical inbox event: %w", err)
	}
	if isControlSource(event.SourceComponent) {
		return preparedInbox{}, fmt.Errorf("inbox cannot ingest a Control-owned source_component")
	}
	return preparedInbox{
		streamID: streamID, event: event, body: cloneBytes(eventBytes),
		digest: digestBytes(eventBytes), receivedAt: receivedAt,
	}, nil
}

func recordInbox(ctx context.Context, tx *sql.Tx, value preparedInbox) (InboxReceipt, error) {
	existing, found, err := findInboxByMessage(ctx, tx, value.streamID, value.event.MessageID)
	if err != nil {
		return InboxReceipt{}, err
	}
	if found {
		return compareInboxReplay(existing, value)
	}
	if exists, err := durableMessageIDExists(ctx, tx, value.event.MessageID); err != nil {
		return InboxReceipt{}, err
	} else if exists {
		return InboxReceipt{}, ErrInboxConflict
	}
	current, err := loadInboxHead(ctx, tx, value.streamID, string(value.event.SourceComponent))
	if err != nil {
		return InboxReceipt{}, err
	}
	if value.event.Sequence <= current {
		return compareInboxSequence(ctx, tx, value)
	}
	if value.event.Sequence != current+1 {
		return InboxReceipt{}, fmt.Errorf("%w: current=%d received=%d", ErrInboxGap, current, value.event.Sequence)
	}
	if err := validateInboxCausation(ctx, tx, value.event); err != nil {
		return InboxReceipt{}, err
	}
	if err := advanceInboxHead(
		ctx, tx, value.streamID, string(value.event.SourceComponent), current, value.event.Sequence,
	); err != nil {
		return InboxReceipt{}, err
	}
	if err := insertInboxEvent(ctx, tx, value); err != nil {
		return InboxReceipt{}, err
	}
	return InboxReceipt{StreamID: value.streamID, Sequence: value.event.Sequence}, nil
}

type inboxIdentity struct {
	sequence int64
	eventID  string
	digest   string
	body     []byte
}

func findInboxByMessage(ctx context.Context, tx *sql.Tx, streamID, messageID string) (inboxIdentity, bool, error) {
	var value inboxIdentity
	err := tx.QueryRowContext(ctx, `SELECT source_sequence, event_id, event_sha256, event_bytes FROM inbox_messages WHERE stream_id = ? AND message_id = ?`, streamID, messageID).
		Scan(&value.sequence, &value.eventID, &value.digest, &value.body)
	if errors.Is(err, sql.ErrNoRows) {
		return inboxIdentity{}, false, nil
	}
	if err != nil {
		return inboxIdentity{}, false, fmt.Errorf("read inbox message: %w", err)
	}
	if err := verifyDigest(value.body, value.digest, "inbox event"); err != nil {
		return inboxIdentity{}, false, err
	}
	return value, true, nil
}

func compareInboxReplay(existing inboxIdentity, value preparedInbox) (InboxReceipt, error) {
	if existing.sequence != value.event.Sequence || existing.eventID != value.event.EventID ||
		existing.digest != value.digest || !bytes.Equal(existing.body, value.body) {
		return InboxReceipt{}, ErrInboxConflict
	}
	return InboxReceipt{Replayed: true, StreamID: value.streamID, Sequence: existing.sequence}, nil
}

func compareInboxSequence(ctx context.Context, tx *sql.Tx, value preparedInbox) (InboxReceipt, error) {
	var existing inboxIdentity
	var messageID string
	err := tx.QueryRowContext(ctx, `SELECT source_sequence, message_id, event_id, event_sha256, event_bytes FROM inbox_messages WHERE stream_id = ? AND source_sequence = ?`, value.streamID, value.event.Sequence).
		Scan(&existing.sequence, &messageID, &existing.eventID, &existing.digest, &existing.body)
	if errors.Is(err, sql.ErrNoRows) {
		return InboxReceipt{}, ErrInboxConflict
	}
	if err != nil {
		return InboxReceipt{}, fmt.Errorf("read inbox sequence: %w", err)
	}
	if messageID != value.event.MessageID {
		return InboxReceipt{}, ErrInboxConflict
	}
	return compareInboxReplay(existing, value)
}

func loadInboxHead(ctx context.Context, tx *sql.Tx, streamID, source string) (int64, error) {
	var current int64
	var storedSource string
	err := tx.QueryRowContext(ctx, `SELECT source_component, current_sequence FROM inbox_sources WHERE stream_id = ?`, streamID).
		Scan(&storedSource, &current)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("read inbox source head: %w", err)
	}
	if storedSource != source {
		return 0, fmt.Errorf("%w: source stream component changed", ErrInboxConflict)
	}
	return current, nil
}

func advanceInboxHead(
	ctx context.Context,
	tx *sql.Tx,
	streamID, source string,
	current, next int64,
) error {
	if current == 0 {
		_, err := tx.ExecContext(ctx, `INSERT INTO inbox_sources VALUES (?, ?, ?)`, streamID, source, next)
		return err
	}
	result, err := tx.ExecContext(ctx, `UPDATE inbox_sources SET current_sequence = ? WHERE stream_id = ? AND current_sequence = ?`, next, streamID, current)
	if err != nil {
		return fmt.Errorf("advance inbox source: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil || changed != 1 {
		return ErrInboxConflict
	}
	return nil
}

func insertInboxEvent(ctx context.Context, tx *sql.Tx, value preparedInbox) error {
	var head int64
	if err := tx.QueryRowContext(ctx, `SELECT coalesce(max(inbox_sequence), 0) FROM inbox_messages`).Scan(&head); err != nil {
		return fmt.Errorf("read inbox journal head: %w", err)
	}
	event := value.event
	sequence := head + 1
	if err := insertMessageIndex(ctx, tx, eventIdentity(event), "inbox_event", sequence); err != nil {
		return err
	}
	_, err := tx.ExecContext(ctx, `INSERT INTO inbox_messages (inbox_sequence, stream_id, source_component, source_sequence, message_id, event_id, correlation_id, causation_id, occurred_at_unix_ms, event_sha256, event_bytes, received_at_unix_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		sequence, value.streamID, string(event.SourceComponent), event.Sequence,
		event.MessageID, event.EventID, event.CorrelationID, nullableString(event.CausationID),
		event.OccurredAtUnixMS, value.digest, value.body, value.receivedAt)
	if err != nil {
		return fmt.Errorf("append inbox event: %w", err)
	}
	return nil
}

// InboxEvents returns an integrity-checked global inbox page for projection replay.
func (store *Store) InboxEvents(ctx context.Context, after int64, limit int) ([]StoredInboxEvent, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return nil, err
	}
	if err := validatePage(after, limit); err != nil {
		return nil, err
	}
	rows, err := store.db.QueryContext(ctx, `SELECT inbox_sequence, stream_id, source_component, source_sequence, message_id, event_id, correlation_id, causation_id, occurred_at_unix_ms, received_at_unix_ms, event_sha256, event_bytes FROM inbox_messages WHERE inbox_sequence > ? ORDER BY inbox_sequence LIMIT ?`, after, limit)
	if err != nil {
		return nil, fmt.Errorf("read inbox events: %w", err)
	}
	defer func() { _ = rows.Close() }()
	return scanInboxEvents(rows)
}

func scanInboxEvents(rows *sql.Rows) ([]StoredInboxEvent, error) {
	var result []StoredInboxEvent
	for rows.Next() {
		var event StoredInboxEvent
		var causation sql.NullString
		if err := rows.Scan(
			&event.InboxSequence, &event.StreamID, &event.SourceComponent, &event.SourceSequence,
			&event.MessageID, &event.EventID, &event.CorrelationID, &causation,
			&event.OccurredAtUnixMS, &event.ReceivedAtUnixMS, &event.BodySHA256, &event.Body,
		); err != nil {
			return nil, fmt.Errorf("scan inbox event: %w", err)
		}
		if causation.Valid {
			event.CausationID = &causation.String
		}
		if err := validateStoredInboxEvent(event); err != nil {
			return nil, err
		}
		event.Body = cloneBytes(event.Body)
		result = append(result, event)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate inbox events: %w", err)
	}
	return result, nil
}

func validateStoredInboxEvent(stored StoredInboxEvent) error {
	if err := verifyDigest(stored.Body, stored.BodySHA256, "inbox event"); err != nil {
		return err
	}
	event, err := core.DecodeCanonicalEventEnvelope(stored.Body)
	if err != nil {
		return fmt.Errorf("%w: stored inbox event is not canonical: %v", ErrCorruptStore, err)
	}
	if event.Sequence != stored.SourceSequence || event.MessageID != stored.MessageID ||
		event.EventID != stored.EventID || event.CorrelationID != stored.CorrelationID ||
		!sameStringPointer(event.CausationID, stored.CausationID) ||
		event.OccurredAtUnixMS != stored.OccurredAtUnixMS ||
		string(event.SourceComponent) != stored.SourceComponent {
		return fmt.Errorf("%w: inbox metadata differs from canonical bytes", ErrCorruptStore)
	}
	return nil
}
