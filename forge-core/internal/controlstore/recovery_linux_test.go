//go:build linux && !android

package controlstore

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

const (
	crashHelperMode = "FORGE_CONTROLSTORE_CRASH_MODE"
	crashHelperPath = "FORGE_CONTROLSTORE_CRASH_PATH"
)

func TestCrashRecoveryIncludesInterruptedSchemaInitialization(t *testing.T) {
	for _, mode := range []string{"committed", "uncommitted", "initializing"} {
		t.Run(mode, func(t *testing.T) {
			state := newPrivateStateDirectory(t)
			command := exec.Command(os.Args[0], "-test.run=^TestControlStoreCrashHelper$")
			command.Env = append(os.Environ(), crashHelperMode+"="+mode, crashHelperPath+"="+state)
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("crash helper: %v\n%s", err, output)
			}
			assertRecoveredState(t, state, mode == "committed")
		})
	}
}

func TestControlStoreCrashHelper(t *testing.T) {
	mode, state := os.Getenv(crashHelperMode), os.Getenv(crashHelperPath)
	if mode == "" {
		return
	}
	if mode == "initializing" {
		crashAfterWALProfile(t, state)
	}
	root, err := os.OpenRoot(state)
	if err != nil {
		t.Fatal(err)
	}
	store, err := OpenBound(context.Background(), root)
	if err != nil {
		t.Fatal(err)
	}
	if mode == "committed" {
		if _, err := store.Commit(context.Background(), commitRequest(t, 0, 1, 990)); err != nil {
			t.Fatal(err)
		}
	} else {
		tx, err := store.db.BeginTx(context.Background(), nil)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := tx.Exec(`INSERT INTO aggregate_heads VALUES ('space', ?, 1)`, testID("spc", 1)); err != nil {
			t.Fatal(err)
		}
	}
	os.Exit(0)
}

func crashAfterWALProfile(t *testing.T, state string) {
	t.Helper()
	root, err := os.OpenRoot(state)
	if err != nil {
		t.Fatal(err)
	}
	if err := createDatabaseFile(root); err != nil {
		t.Fatal(err)
	}
	directory, err := root.Open(".")
	if err != nil {
		t.Fatal(err)
	}
	db, err := sqlOpen(descriptorDSN(directory.Fd()))
	if err != nil {
		t.Fatal(err)
	}
	if err := db.PingContext(context.Background()); err != nil {
		t.Fatal(err)
	}
	var mode string
	if err := db.QueryRow(`PRAGMA journal_mode`).Scan(&mode); err != nil || mode != "wal" {
		t.Fatalf("initialization journal_mode = %q, %v", mode, err)
	}
	os.Exit(0)
}

func assertRecoveredState(t *testing.T, state string, committed bool) {
	t.Helper()
	store, root := reopenTestStore(t, state)
	defer func() { _ = store.Close(); _ = root.Close() }()
	version, err := store.AggregateVersion(context.Background(), "space", testID("spc", 1))
	if err != nil {
		t.Fatal(err)
	}
	events, err := store.Events(context.Background(), 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	want := 0
	if committed {
		want = 1
	}
	if version != int64(want) || len(events) != want {
		t.Fatalf("recovered state = version %d, events %d; want %d", version, len(events), want)
	}
	if _, err := os.Stat(filepath.Join(state, databaseName)); err != nil {
		t.Fatal(err)
	}
}
