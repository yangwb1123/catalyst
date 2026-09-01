//go:build linux && !android

package controlstore

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestOpenRejectsExactHeaderDriftWithoutChangingJournalMode(t *testing.T) {
	state := newPrivateStateDirectory(t)
	path := filepath.Join(state, databaseName)
	db, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`CREATE TABLE foreign_state(value TEXT)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`PRAGMA application_id = 1179603527`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`PRAGMA user_version = 1`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o600); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	root, err := os.OpenRoot(state)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = root.Close() }()
	if _, err := OpenBound(context.Background(), root); !errors.Is(err, ErrSchemaIncompatible) {
		t.Fatalf("incompatible exact-header database error = %v", err)
	}
	after, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("incompatible database changed: equal=%v err=%v", bytes.Equal(before, after), err)
	}
	assertDeleteJournalWithoutSidecars(t, path)
}

func TestOpenRejectsObjectFreeNonPristineDatabase(t *testing.T) {
	state := newPrivateStateDirectory(t)
	path := filepath.Join(state, databaseName)
	db, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`CREATE TABLE discarded(value BLOB); INSERT INTO discarded VALUES (zeroblob(100000)); DROP TABLE discarded`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o600); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	root, err := os.OpenRoot(state)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = root.Close() }()
	if _, err := OpenBound(context.Background(), root); !errors.Is(err, ErrSchemaIncompatible) {
		t.Fatalf("non-pristine shell error = %v", err)
	}
	after, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("non-pristine shell changed: equal=%v err=%v", bytes.Equal(before, after), err)
	}
	assertDeleteJournalWithoutSidecars(t, path)
}

func newPrivateStateDirectory(t *testing.T) string {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	state := filepath.Join(parent, "state")
	if err := os.Mkdir(state, 0o700); err != nil {
		t.Fatal(err)
	}
	return state
}

func assertDeleteJournalWithoutSidecars(t *testing.T, path string) {
	t.Helper()
	db, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = db.Close() }()
	var mode string
	if err := db.QueryRow(`PRAGMA journal_mode`).Scan(&mode); err != nil || mode != "delete" {
		t.Fatalf("journal_mode after rejected open = %q, %v", mode, err)
	}
	for _, suffix := range []string{"-wal", "-shm"} {
		if _, err := os.Stat(path + suffix); !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("rejected open left %s: %v", suffix, err)
		}
	}
}

func TestDescriptorConnectionAppliesPragmasAndDefensiveMode(t *testing.T) {
	value := openTestStore(t)
	checks := map[string]int{
		`PRAGMA foreign_keys`: 1, `PRAGMA trusted_schema`: 0,
		`PRAGMA recursive_triggers`: 0, `PRAGMA synchronous`: 2,
		`PRAGMA busy_timeout`: 5000,
	}
	for query, want := range checks {
		var got int
		if err := value.store.db.QueryRow(query).Scan(&got); err != nil || got != want {
			t.Fatalf("%s = %d, %v; want %d", query, got, err, want)
		}
	}
	var before, after int
	if err := value.store.db.QueryRow(`PRAGMA schema_version`).Scan(&before); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`PRAGMA schema_version = 99999`); err != nil {
		t.Fatal(err)
	}
	if err := value.store.db.QueryRow(`PRAGMA schema_version`).Scan(&after); err != nil || after != before {
		t.Fatalf("defensive schema_version = %d, %v; want %d", after, err, before)
	}
	if _, err := value.store.db.Exec(`PRAGMA writable_schema = ON`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE sqlite_schema SET sql = sql`); err == nil {
		t.Fatal("defensive connection allowed a direct sqlite_schema write")
	}
}

func TestDescriptorTransactionsAcquireImmediateWriteReservation(t *testing.T) {
	value := openTestStore(t)
	tx, err := value.store.db.BeginTx(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tx.Rollback() }()
	query := url.Values{}
	query.Set("_txlock", "immediate")
	query.Add("_pragma", "busy_timeout(1)")
	dsn := (&url.URL{Scheme: "file", Path: filepath.Join(value.path, databaseName), RawQuery: query.Encode()}).String()
	contender, err := sql.Open("sqlite", dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = contender.Close() }()
	if other, err := contender.BeginTx(context.Background(), nil); err == nil {
		_ = other.Rollback()
		t.Fatal("second immediate transaction acquired the reserved writer lock")
	}
}

func TestCommandCoverageCheckUsesOneGroupedEventScan(t *testing.T) {
	value := openTestStore(t)
	rows, err := value.store.db.Query(`EXPLAIN QUERY PLAN ` + commandCoverageCheck)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rows.Close() }()
	var details []string
	for rows.Next() {
		var id, parent, unused int
		var detail string
		if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
			t.Fatal(err)
		}
		details = append(details, detail)
	}
	plan := strings.ToUpper(strings.Join(details, "\n"))
	if err := rows.Err(); err != nil || strings.Contains(plan, "CORRELATED") {
		t.Fatalf("command coverage query plan is not a grouped scan: %q, %v", plan, err)
	}
	if !strings.Contains(plan, "MATERIALIZE E") || !strings.Contains(plan, "CONTROL_EVENTS_COMMAND_ID_IX") {
		t.Fatalf("command coverage query plan lost its grouped indexed scan: %q", plan)
	}
}

func TestStoreCloseIsConcurrentWithReadsAndIdempotent(t *testing.T) {
	value := openTestStore(t)
	start := make(chan struct{})
	var wait sync.WaitGroup
	for index := 0; index < 16; index++ {
		wait.Add(2)
		go func() {
			defer wait.Done()
			<-start
			_, _ = value.store.ControlSourceHead(context.Background(), "control_plane")
		}()
		go func() {
			defer wait.Done()
			<-start
			_ = value.store.Close()
		}()
	}
	close(start)
	wait.Wait()
	if err := value.store.Close(); err != nil && !strings.Contains(err.Error(), "closed") {
		t.Fatalf("idempotent close = %v", err)
	}
}
