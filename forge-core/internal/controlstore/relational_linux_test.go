//go:build linux && !android

package controlstore

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestReopenRejectsInterleavedCommandRanges(t *testing.T) {
	value := openTestStore(t)
	first := twoEventCommit(t, 1200)
	if _, err := value.store.Commit(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	second := retargetCommit(t, commitRequest(t, 0, 1, 1300), "project")
	second.Events[0] = mutateCanonicalEvent(t, second.Events[0], func(event *core.EventEnvelope) {
		event.SourceComponent = "app_server"
	})
	if _, err := value.store.Commit(context.Background(), second); err != nil {
		t.Fatal(err)
	}
	disableUpdateTriggers(t, value.store.db, "control_events", "command_receipts", "message_index")
	swapJournalRows(t, value.store.db)
	if _, err := value.store.db.Exec(`UPDATE command_receipts SET last_global_sequence = 3 WHERE command_id = ?`, testID("cmd", 1200)); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE command_receipts SET first_global_sequence = 2, last_global_sequence = 2 WHERE command_id = ?`, testID("cmd", 1300)); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE message_index SET journal_sequence = 2 WHERE journal_kind = 'command' AND message_id = ?`, testID("msg", 1300)); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "control_events", "command_receipts", "message_index")
	expectCorruptReopen(t, value)
}

func twoEventCommit(t *testing.T, serial int) CommitRequest {
	t.Helper()
	correlation := testID("cor", serial)
	commandCause := testID("msg", serial)
	eventCause := testID("msg", serial+1000)
	return CommitRequest{
		Command: canonicalCommand(t, 0, serial, "two-event-command", "advance"),
		Events: [][]byte{
			canonicalEventWithLinks(t, 1, 1, serial, "control_plane", "first", correlation, &commandCause),
			canonicalEventWithLinks(t, 2, 2, serial+1, "control_plane", "second", correlation, &eventCause),
		},
		Result: []byte(`{"accepted":true}`), CommittedAtUnixMS: testUnixMS,
	}
}

func swapJournalRows(t *testing.T, db *sql.DB) {
	t.Helper()
	statements := []string{
		`UPDATE control_events SET global_sequence = 100 WHERE global_sequence = 2`,
		`UPDATE control_events SET global_sequence = 2 WHERE global_sequence = 3`,
		`UPDATE control_events SET global_sequence = 3 WHERE global_sequence = 100`,
		`UPDATE message_index SET journal_sequence = 100 WHERE journal_kind = 'control_event' AND journal_sequence = 2`,
		`UPDATE message_index SET journal_sequence = 2 WHERE journal_kind = 'control_event' AND journal_sequence = 3`,
		`UPDATE message_index SET journal_sequence = 3 WHERE journal_kind = 'control_event' AND journal_sequence = 100`,
	}
	for _, statement := range statements {
		if _, err := db.Exec(statement); err != nil {
			t.Fatal(err)
		}
	}
}

func TestReopenRejectsControlSourceOrderDrift(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), twoEventCommit(t, 1400)); err != nil {
		t.Fatal(err)
	}
	disableUpdateTriggers(t, value.store.db, "control_events")
	swapIntegerColumn(t, value.store.db, "source_sequence", "global_sequence")
	enableUpdateTriggers(t, value.store.db, "control_events")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsAggregateVersionOrderDrift(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), twoEventCommit(t, 1500)); err != nil {
		t.Fatal(err)
	}
	disableUpdateTriggers(t, value.store.db, "control_events")
	swapIntegerColumn(t, value.store.db, "aggregate_version", "global_sequence")
	enableUpdateTriggers(t, value.store.db, "control_events")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsInboxSourceOrderDrift(t *testing.T) {
	value := openTestStore(t)
	correlation := testID("cor", 1600)
	first := canonicalEventWithLinks(t, 1, 1, 1600, "runtime", "first", correlation, nil)
	firstMessage := testID("msg", 2600)
	second := canonicalEventWithLinks(t, 2, 2, 1601, "runtime", "second", correlation, &firstMessage)
	for _, event := range [][]byte{first, second} {
		if _, err := value.store.RecordInbox(context.Background(), "runtime:ordered", event, testUnixMS); err != nil {
			t.Fatal(err)
		}
	}
	disableUpdateTriggers(t, value.store.db, "inbox_messages")
	swapIntegerColumn(t, value.store.db, "source_sequence", "inbox_sequence")
	enableUpdateTriggers(t, value.store.db, "inbox_messages")
	expectCorruptReopen(t, value)
}

func swapIntegerColumn(t *testing.T, db *sql.DB, column, key string) {
	t.Helper()
	queries := []string{
		`UPDATE ` + tableForKey(key) + ` SET ` + column + ` = 100 WHERE ` + key + ` = 1`,
		`UPDATE ` + tableForKey(key) + ` SET ` + column + ` = 1 WHERE ` + key + ` = 2`,
		`UPDATE ` + tableForKey(key) + ` SET ` + column + ` = 2 WHERE ` + column + ` = 100`,
	}
	for _, query := range queries {
		if _, err := db.Exec(query); err != nil {
			t.Fatal(err)
		}
	}
}

func tableForKey(key string) string {
	if key == "inbox_sequence" {
		return "inbox_messages"
	}
	return "control_events"
}

func TestReopenRejectsCommandCausationDrift(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 1700)); err != nil {
		t.Fatal(err)
	}
	second := retargetCommit(t, commitRequest(t, 0, 2, 1701), "project")
	if _, err := value.store.Commit(context.Background(), second); err != nil {
		t.Fatal(err)
	}
	invalid := mutateCanonicalCommand(t, second.Command, func(command *core.CommandEnvelope) {
		cause := testID("msg", 1700)
		command.CausationID = &cause
	})
	disableUpdateTriggers(t, value.store.db, "command_receipts")
	if _, err := value.store.db.Exec(`UPDATE command_receipts SET causation_id = ?, request_sha256 = ?, request_bytes = ? WHERE command_id = ?`, testID("msg", 1700), digestBytes(invalid), invalid, testID("cmd", 1701)); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "command_receipts")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsFutureCommandCausation(t *testing.T) {
	value := openTestStore(t)
	first := commitRequest(t, 0, 1, 1750)
	if _, err := value.store.Commit(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	second := retargetCommit(t, commitRequest(t, 0, 2, 1751), "project")
	correlation := testID("cor", 1750)
	second.Command = mutateCanonicalCommand(t, second.Command, func(command *core.CommandEnvelope) {
		command.CorrelationID = correlation
	})
	second.Events[0] = mutateCanonicalEvent(t, second.Events[0], func(event *core.EventEnvelope) {
		event.CorrelationID = correlation
	})
	if _, err := value.store.Commit(context.Background(), second); err != nil {
		t.Fatal(err)
	}
	invalid := mutateCanonicalCommand(t, first.Command, func(command *core.CommandEnvelope) {
		cause := testID("msg", 1751)
		command.CausationID = &cause
	})
	disableUpdateTriggers(t, value.store.db, "command_receipts")
	if _, err := value.store.db.Exec(`UPDATE command_receipts SET causation_id = ?, request_sha256 = ?, request_bytes = ? WHERE command_id = ?`, testID("msg", 1751), digestBytes(invalid), invalid, testID("cmd", 1750)); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "command_receipts")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsControlEventCauseFromAnotherCommand(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 1800)); err != nil {
		t.Fatal(err)
	}
	second := retargetCommit(t, commitRequest(t, 0, 2, 1801), "project")
	correlation := testID("cor", 1800)
	second.Command = mutateCanonicalCommand(t, second.Command, func(command *core.CommandEnvelope) {
		command.CorrelationID = correlation
	})
	second.Events[0] = mutateCanonicalEvent(t, second.Events[0], func(event *core.EventEnvelope) {
		event.CorrelationID = correlation
	})
	if _, err := value.store.Commit(context.Background(), second); err != nil {
		t.Fatal(err)
	}
	invalid := mutateCanonicalEvent(t, second.Events[0], func(event *core.EventEnvelope) {
		cause := testID("msg", 1800)
		event.CausationID = &cause
	})
	disableUpdateTriggers(t, value.store.db, "control_events")
	if _, err := value.store.db.Exec(`UPDATE control_events SET causation_id = ?, event_sha256 = ?, event_bytes = ? WHERE command_id = ?`, testID("msg", 1800), digestBytes(invalid), invalid, testID("cmd", 1801)); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "control_events")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsInboxCausationDrift(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 1900)); err != nil {
		t.Fatal(err)
	}
	second := retargetCommit(t, commitRequest(t, 0, 2, 1901), "project")
	if _, err := value.store.Commit(context.Background(), second); err != nil {
		t.Fatal(err)
	}
	cause := testID("msg", 1900)
	event := canonicalEventWithLinks(t, 1, 1, 1950, "runtime", "external", testID("cor", 1900), &cause)
	if _, err := value.store.RecordInbox(context.Background(), "runtime:causal-drift", event, testUnixMS); err != nil {
		t.Fatal(err)
	}
	invalid := mutateCanonicalEvent(t, event, func(event *core.EventEnvelope) {
		cause := testID("msg", 1901)
		event.CausationID = &cause
	})
	disableUpdateTriggers(t, value.store.db, "inbox_messages")
	if _, err := value.store.db.Exec(`UPDATE inbox_messages SET causation_id = ?, event_sha256 = ?, event_bytes = ?`, testID("msg", 1901), digestBytes(invalid), invalid); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "inbox_messages")
	expectCorruptReopen(t, value)
}

func TestReopenRejectsMessageIdentitySharedByJournalAndOutbox(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 1980)
	request.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 9980), SourceEvent: testID("evt", 2980),
		Destination: "runtime", Body: []byte(`{"collision":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	disableUpdateTriggers(t, value.store.db, "outbox_messages")
	if _, err := value.store.db.Exec(
		`UPDATE outbox_messages SET message_id = ? WHERE message_id = ?`,
		testID("msg", 1980), testID("msg", 9980),
	); err != nil {
		t.Fatal(err)
	}
	enableUpdateTriggers(t, value.store.db, "outbox_messages")
	expectCorruptReopen(t, value)
}

func disableUpdateTriggers(t *testing.T, db *sql.DB, tables ...string) {
	t.Helper()
	for _, table := range tables {
		if _, err := db.Exec(`DROP TRIGGER ` + table + `_no_update`); err != nil {
			t.Fatal(err)
		}
	}
}

func enableUpdateTriggers(t *testing.T, db *sql.DB, tables ...string) {
	t.Helper()
	for _, table := range tables {
		if _, err := db.Exec(schemaDDL(t, table+"_no_update")); err != nil {
			t.Fatal(err)
		}
	}
}

func expectCorruptReopen(t *testing.T, value testStore) {
	t.Helper()
	value.close()
	root, err := os.OpenRoot(value.path)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = root.Close() }()
	if _, err := OpenBound(context.Background(), root); !errors.Is(err, ErrCorruptStore) {
		t.Fatalf("relational drift error = %v", err)
	}
}
