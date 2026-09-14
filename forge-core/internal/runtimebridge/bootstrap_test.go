package runtimebridge

import (
	"context"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"path/filepath"
	"testing"
)

func TestConversationBootstrapSupportsFrozenPhasesAndEmptyAdvancingPages(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "server-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	baselineResult := `{"snapshot_cursor":3,"conversations":[{"conversation":{"id":"c1","scope":{"kind":"project","id":"p1"},"title":"Build","created_at_ms":1,"updated_at_ms":2},"creation_cursor":0,"aggregate_version":2}],"scanned_through_cursor":0,"next_cursor":{"snapshot_cursor":3,"phase":"legacy_baseline","after_conversation_id":"c1"},"has_more":true}`
	writeFake(t, executable, responseScript(database, baselineResult))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.ConversationBootstrap(context.Background(), nil, 1)
	if err != nil {
		t.Fatal(err)
	}
	if page.SnapshotCursor != 3 || len(page.Conversations) != 1 ||
		page.Conversations[0].CreationCursor != 0 || page.NextCursor == nil ||
		page.NextCursor.Phase != model.ConversationBootstrapLegacyBaseline || !page.HasMore {
		t.Fatalf("baseline bootstrap page = %#v", page)
	}

	zero := uint64(0)
	changeCursor := &model.ConversationBootstrapCursor{
		SnapshotCursor: 3, Phase: model.ConversationBootstrapChangeLog, AfterChangeCursor: &zero,
	}
	promptOnlyResult := `{"snapshot_cursor":3,"conversations":[],"scanned_through_cursor":1,"next_cursor":{"snapshot_cursor":3,"phase":"change_log","after_change_cursor":1},"has_more":true}`
	writeFake(t, executable, responseScript(database, promptOnlyResult))
	page, err = client.ConversationBootstrap(context.Background(), changeCursor, 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Conversations) != 0 || page.NextCursor == nil ||
		page.NextCursor.AfterChangeCursor == nil || *page.NextCursor.AfterChangeCursor != 1 || !page.HasMore {
		t.Fatalf("empty advancing Prompt-only page = %#v", page)
	}
	if _, err := client.ConversationBootstrap(context.Background(), changeCursor, maxBootstrapPageLimit+1); err == nil {
		t.Fatal("over-limit bootstrap page was accepted")
	}
	invalid := *changeCursor
	invalid.AfterChangeCursor = nil
	if _, err := client.ConversationBootstrap(context.Background(), &invalid, 1); err == nil {
		t.Fatal("phase-incomplete bootstrap cursor was accepted")
	}
}

func TestConversationBootstrapValidationRejectsMalformedPages(t *testing.T) {
	valid := []byte(`{"snapshot_cursor":4,"conversations":[{"conversation":{"id":"c2","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":3},"creation_cursor":4,"aggregate_version":2}],"scanned_through_cursor":4,"has_more":false}`)
	var page model.ConversationBootstrapPage
	if err := decodeStrict(valid, &page); err != nil || !validConversationBootstrapPage(valid, page, &model.ConversationBootstrapCursor{
		SnapshotCursor: 4, Phase: model.ConversationBootstrapChangeLog, AfterChangeCursor: ptrUint64(3),
	}, 1) {
		t.Fatalf("valid change-log page rejected: %v / %#v", err, page)
	}
	for _, malformed := range [][]byte{
		[]byte(`{"snapshot_cursor":4,"conversations":[{"conversation":{"id":"c2","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":3,"path":"/private/work"},"creation_cursor":3,"aggregate_version":2}],"scanned_through_cursor":4,"has_more":false}`),
		[]byte(`{"snapshot_cursor":5,"conversations":[],"scanned_through_cursor":5,"has_more":false}`),
		[]byte(`{"snapshot_cursor":4,"conversations":[{"conversation":{"id":"c2","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":3},"creation_cursor":5,"aggregate_version":2}],"scanned_through_cursor":4,"has_more":false}`),
		[]byte(`{"snapshot_cursor":4,"conversations":[],"scanned_through_cursor":2,"next_cursor":{"snapshot_cursor":4,"phase":"change_log","after_change_cursor":4},"has_more":true}`),
		[]byte(`{"snapshot_cursor":4,"conversations":[],"scanned_through_cursor":2,"next_cursor":{"snapshot_cursor":4,"phase":"change_log","after_change_cursor":2,"after_conversation_id":null},"has_more":true}`),
		[]byte(`{"snapshot_cursor":4,"conversations":[{"conversation":{"id":"c2","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":3},"creation_cursor":3,"aggregate_version":2}],"scanned_through_cursor":3,"has_more":true}`),
		[]byte(`{"snapshot_cursor":4,"conversations":[],"scanned_through_cursor":3,"has_more":false}`),
		[]byte(`{"snapshot_cursor":1000,"conversations":[],"scanned_through_cursor":999,"next_cursor":{"snapshot_cursor":1000,"phase":"change_log","after_change_cursor":999},"has_more":true}`),
		[]byte(`{"snapshot_cursor":1000,"conversations":[],"scanned_through_cursor":1000,"has_more":false}`),
	} {
		var decoded model.ConversationBootstrapPage
		if err := decodeStrict(malformed, &decoded); err == nil && validConversationBootstrapPage(malformed, decoded, &model.ConversationBootstrapCursor{
			SnapshotCursor: 4, Phase: model.ConversationBootstrapChangeLog, AfterChangeCursor: ptrUint64(1),
		}, 1) {
			t.Fatalf("malformed bootstrap page accepted: %s", malformed)
		}
	}

	baseline := []byte(`{"snapshot_cursor":4,"conversations":[{"conversation":{"id":"b","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":1},"creation_cursor":0,"aggregate_version":1},{"conversation":{"id":"a","scope":{"kind":"global"},"title":"Session","created_at_ms":1,"updated_at_ms":1},"creation_cursor":0,"aggregate_version":1}],"scanned_through_cursor":0,"has_more":false}`)
	var unordered model.ConversationBootstrapPage
	if err := decodeStrict(baseline, &unordered); err != nil || validConversationBootstrapPage(baseline, unordered, nil, 2) {
		t.Fatal("unordered legacy baseline page was accepted")
	}
	if validConversationBootstrapCursor(model.ConversationBootstrapCursor{
		SnapshotCursor: 4, Phase: model.ConversationBootstrapLegacyBaseline, AfterConversationID: ptrString(" "),
	}) {
		t.Fatal("blank baseline boundary was accepted")
	}
}

func TestConversationBootstrapRejectsSkippedJournalCursors(t *testing.T) {
	cursor := &model.ConversationBootstrapCursor{
		SnapshotCursor: 1000, Phase: model.ConversationBootstrapChangeLog, AfterChangeCursor: ptrUint64(1),
	}
	for _, response := range []string{
		`{"snapshot_cursor":1000,"conversations":[],"scanned_through_cursor":999,"next_cursor":{"snapshot_cursor":1000,"phase":"change_log","after_change_cursor":999},"has_more":true}`,
		`{"snapshot_cursor":1000,"conversations":[],"scanned_through_cursor":1000,"has_more":false}`,
	} {
		data := []byte(response)
		var page model.ConversationBootstrapPage
		if err := decodeStrict(data, &page); err != nil {
			t.Fatalf("decode response: %v", err)
		}
		if validConversationBootstrapPage(data, page, cursor, 1) {
			t.Fatalf("bootstrap accepted a page that skipped journal rows: %s", response)
		}
	}
}

func ptrString(value string) *string { return &value }

func ptrUint64(value uint64) *uint64 { return &value }
