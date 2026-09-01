//go:build linux && !android

package controlstore

import (
	"context"
	"errors"
	"testing"
)

func TestInboxEnforcesReplayConflictGapAndReorder(t *testing.T) {
	value := openTestStore(t)
	correlation := testID("cor", 100)
	firstMessage := testID("msg", 1100)
	first := canonicalEventWithLinks(t, 1, 1, 100, "runtime", "accepted", correlation, nil)
	receipt, err := value.store.RecordInbox(context.Background(), "runtime/local", first, testUnixMS)
	if err != nil || receipt.Replayed || receipt.Sequence != 1 {
		t.Fatalf("first inbox receipt = %+v, %v", receipt, err)
	}
	replayed, err := value.store.RecordInbox(context.Background(), "runtime/local", first, testUnixMS+1)
	if err != nil || !replayed.Replayed {
		t.Fatalf("inbox replay = %+v, %v", replayed, err)
	}
	changed := canonicalEventWithLinks(t, 1, 1, 100, "runtime", "changed", correlation, nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", changed, testUnixMS+1); !errors.Is(err, ErrInboxConflict) {
		t.Fatalf("changed replay error = %v", err)
	}
	third := canonicalEventWithLinks(t, 3, 3, 102, "runtime", "third", correlation, &firstMessage)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", third, testUnixMS+3); !errors.Is(err, ErrInboxGap) {
		t.Fatalf("inbox gap error = %v", err)
	}
	second := canonicalEventWithLinks(t, 2, 2, 101, "runtime", "second", correlation, &firstMessage)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", second, testUnixMS+2); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", first, testUnixMS+4); err != nil {
		t.Fatalf("exact reordered replay: %v", err)
	}
	conflict := canonicalEventWithLinks(t, 1, 1, 103, "runtime", "other", correlation, nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", conflict, testUnixMS+4); !errors.Is(err, ErrInboxConflict) {
		t.Fatalf("reordered conflict error = %v", err)
	}
	events, err := value.store.InboxEvents(context.Background(), 0, 10)
	if err != nil || len(events) != 2 || events[0].SourceSequence != 1 || events[1].SourceSequence != 2 {
		t.Fatalf("inbox events = %+v, %v", events, err)
	}
}

func TestInboxResolvesDurableCauseAndCorrelation(t *testing.T) {
	value := openTestStore(t)
	correlation := testID("cor", 150)
	firstMessage := testID("msg", 1150)
	first := canonicalEventWithLinks(t, 1, 1, 150, "runtime", "first", correlation, nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/causal", first, testUnixMS); err != nil {
		t.Fatal(err)
	}
	unknown := testID("msg", 9999)
	missing := canonicalEventWithLinks(t, 2, 2, 151, "runtime", "second", correlation, &unknown)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/causal", missing, testUnixMS+1); err == nil {
		t.Fatal("unresolved inbox cause succeeded")
	}
	wrong := canonicalEventWithLinks(t, 2, 2, 151, "runtime", "second", testID("cor", 9999), &firstMessage)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/causal", wrong, testUnixMS+1); err == nil {
		t.Fatal("cross-correlation inbox cause succeeded")
	}
	valid := canonicalEventWithLinks(t, 2, 2, 151, "runtime", "second", correlation, &firstMessage)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/causal", valid, testUnixMS+1); err != nil {
		t.Fatalf("durably caused inbox event: %v", err)
	}
}

func TestInboxRejectsOutboxMessageIdentity(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 1750)
	messageID := testID("msg", 9750)
	request.Outbox = []OutboxMessage{{
		MessageID: messageID, SourceEvent: testID("evt", 2750),
		Destination: "runtime", Body: []byte(`{"outbox":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	inbox := canonicalEventWithLinks(t, 1, 1, 8750, "runtime", "collision", testID("cor", 9750), nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/collision", inbox, testUnixMS+1); !errors.Is(err, ErrInboxConflict) {
		t.Fatalf("inbox/outbox durable collision error = %v", err)
	}
}

func TestInboxStreamsAdvanceIndependentlyAndRejectControlSources(t *testing.T) {
	value := openTestStore(t)
	for index, stream := range []string{"runtime/a", "runtime/b"} {
		serial := 200 + index
		event := canonicalEventWithLinks(t, 1, 1, serial, "runtime", "accepted", testID("cor", serial), nil)
		if _, err := value.store.RecordInbox(context.Background(), stream, event, testUnixMS); err != nil {
			t.Fatalf("stream %s: %v", stream, err)
		}
	}
	for index, source := range []string{"app_server", "control_plane", "legacy_importer"} {
		serial := 300 + index
		control := canonicalEventWithLinks(t, 1, 1, serial, source, "owned", testID("cor", serial), nil)
		if _, err := value.store.RecordInbox(context.Background(), "runtime/c", control, testUnixMS); err == nil {
			t.Fatalf("Control-owned source %s entered inbox", source)
		}
	}
	mixed := canonicalEventWithLinks(t, 2, 2, 301, "harness", "mixed", testID("cor", 200), nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/a", mixed, testUnixMS+1); !errors.Is(err, ErrInboxConflict) {
		t.Fatalf("mixed source component error = %v", err)
	}
	events, err := value.store.InboxEvents(context.Background(), 0, 10)
	if err != nil || len(events) != 2 || events[0].InboxSequence != 1 || events[1].InboxSequence != 2 {
		t.Fatalf("independent streams = %+v, %v", events, err)
	}
}

func TestInboxAppendFailureRollsBackSourceHead(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.db.Exec(`CREATE TEMP TRIGGER inject_inbox_failure BEFORE INSERT ON inbox_messages BEGIN SELECT RAISE(ABORT, 'injected'); END`); err != nil {
		t.Fatal(err)
	}
	event := canonicalEventWithLinks(t, 1, 1, 400, "runtime", "accepted", testID("cor", 400), nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", event, testUnixMS); err == nil {
		t.Fatal("injected inbox failure succeeded")
	}
	if _, err := value.store.db.Exec(`DROP TRIGGER temp.inject_inbox_failure`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", event, testUnixMS); err != nil {
		t.Fatalf("inbox retry after rollback: %v", err)
	}
}

func TestInboxReopenPreservesCursorAndCanonicalBytes(t *testing.T) {
	value := openTestStore(t)
	correlation := testID("cor", 500)
	firstMessage := testID("msg", 1500)
	first := canonicalEventWithLinks(t, 1, 1, 500, "runtime", "accepted", correlation, nil)
	if _, err := value.store.RecordInbox(context.Background(), "runtime/local", first, testUnixMS); err != nil {
		t.Fatal(err)
	}
	value.close()
	store, root := reopenTestStore(t, value.path)
	defer func() { _ = store.Close(); _ = root.Close() }()
	second := canonicalEventWithLinks(t, 2, 2, 501, "runtime", "running", correlation, &firstMessage)
	if _, err := store.RecordInbox(context.Background(), "runtime/local", second, testUnixMS+1); err != nil {
		t.Fatal(err)
	}
	source, sequence, found, err := store.InboxCursor(context.Background(), "runtime/local")
	if err != nil || !found || source != "runtime" || sequence != 2 {
		t.Fatalf("reopened inbox cursor = %s/%d, found=%t, %v", source, sequence, found, err)
	}
	events, err := store.InboxEvents(context.Background(), 0, 10)
	if err != nil || len(events) != 2 || string(events[0].Body) != string(first) {
		t.Fatalf("reopened inbox = %+v, %v", events, err)
	}
}
