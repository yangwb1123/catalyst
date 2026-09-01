//go:build linux && !android

package controlstore

import (
	"context"
	"fmt"
	"strings"
	"testing"
)

func TestPendingOutboxPageAdvancesAcrossBoundedAcknowledgedWindow(t *testing.T) {
	value := openTestStore(t)
	seed := commitRequest(t, 0, 1, 80)
	if _, err := value.store.Commit(context.Background(), seed); err != nil {
		t.Fatal(err)
	}
	insertOutboxWindow(t, value.store, testID("evt", 1080), maxPageItems+1)
	first, err := value.store.PendingOutboxMessages(context.Background(), 0, maxPageItems)
	if err != nil || len(first.Items) != 0 || !first.More ||
		first.NextAfterOutboxSequence != maxPageItems {
		t.Fatalf("first pending page = %+v, %v", first, err)
	}
	second, err := value.store.PendingOutboxMessages(
		context.Background(), first.NextAfterOutboxSequence, maxPageItems,
	)
	if err != nil || len(second.Items) != 1 || second.More ||
		second.NextAfterOutboxSequence != maxPageItems+1 {
		t.Fatalf("second pending page = %+v, %v", second, err)
	}
}

func TestPendingOutboxQueryUsesSequenceRangeWithoutTemporarySort(t *testing.T) {
	value := openTestStore(t)
	rows, err := value.store.db.Query(
		"EXPLAIN QUERY PLAN "+pendingOutboxPageQuery, 0, maxPageItems,
	)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rows.Close() }()
	var details strings.Builder
	for rows.Next() {
		var id, parent, unused int
		var detail string
		if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
			t.Fatal(err)
		}
		details.WriteString(detail)
		details.WriteByte('\n')
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	plan := details.String()
	if !strings.Contains(plan, "SEARCH o USING INTEGER PRIMARY KEY (rowid>?)") ||
		strings.Contains(plan, "USE TEMP B-TREE") {
		t.Fatalf("pending outbox query plan:\n%s", plan)
	}
}

func insertOutboxWindow(t *testing.T, store *Store, sourceEvent string, total int) {
	t.Helper()
	tx, err := store.db.Begin()
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	for sequence := 1; sequence <= total; sequence++ {
		messageID := testID("msg", 7000+sequence)
		body := []byte(fmt.Sprintf("outbox-%d", sequence))
		_, err = tx.Exec(`INSERT INTO outbox_messages
(outbox_sequence, message_id, source_event_id, destination, message_sha256, message_bytes, created_at_unix_ms)
VALUES (?, ?, ?, ?, ?, ?, ?)`, sequence, messageID, sourceEvent, "runtime.local",
			digestBytes(body), body, testUnixMS)
		if err != nil {
			t.Fatal(err)
		}
		if sequence <= maxPageItems {
			if _, err = tx.Exec(`INSERT INTO outbox_acknowledgements
(message_id, delivered_at_unix_ms) VALUES (?, ?)`, messageID, testUnixMS+1); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := tx.Commit(); err != nil {
		t.Fatal(err)
	}
}
