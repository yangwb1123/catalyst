//go:build linux && !android

package controlstore

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestOpenCreatesExactPrivateWALSchemaAndReopens(t *testing.T) {
	value := openTestStore(t)
	info, err := os.Stat(filepath.Join(value.path, databaseName))
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("control.db = %v, %v", info, err)
	}
	var mode string
	if err := value.store.db.QueryRow(`PRAGMA journal_mode`).Scan(&mode); err != nil || mode != "wal" {
		t.Fatalf("journal_mode = %q, %v", mode, err)
	}
	actual := catalogNames(t, value.store.db)
	if want := sortedSchemaObjectNames(); !reflect.DeepEqual(actual, want) {
		t.Fatalf("schema names = %v, want %v", actual, want)
	}
	value.close()
	store, root := reopenTestStore(t, value.path)
	defer func() { _ = store.Close(); _ = root.Close() }()
	if version, err := store.AggregateVersion(context.Background(), "space", testID("spc", 1)); err != nil || version != 0 {
		t.Fatalf("empty aggregate version = %d, %v", version, err)
	}
}

func catalogNames(t *testing.T, db *sql.DB) []string {
	t.Helper()
	rows, err := db.Query(`SELECT name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name`)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rows.Close() }()
	var names []string
	for rows.Next() {
		var name string
		if err := rows.Scan(&name); err != nil {
			t.Fatal(err)
		}
		names = append(names, name)
	}
	return names
}

func TestOpenStaysBoundToRenamedDirectoryDescriptor(t *testing.T) {
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	original, moved := filepath.Join(parent, "state"), filepath.Join(parent, "moved")
	if err := os.Mkdir(original, 0o700); err != nil {
		t.Fatal(err)
	}
	root, err := os.OpenRoot(original)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = root.Close() }()
	if err := os.Rename(original, moved); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(original, 0o700); err != nil {
		t.Fatal(err)
	}
	store, err := OpenBound(context.Background(), root)
	if err != nil {
		t.Fatal(err)
	}
	_ = store.Close()
	if _, err := os.Stat(filepath.Join(moved, databaseName)); err != nil {
		t.Fatalf("bound directory has no database: %v", err)
	}
	if _, err := os.Stat(filepath.Join(original, databaseName)); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("replacement directory was mutated: %v", err)
	}
}

func TestOpenRejectsForeignAndDriftedSchemas(t *testing.T) {
	for _, mutation := range []string{
		`CREATE TABLE attacker(value TEXT)`,
		`PRAGMA application_id = 7`,
		`PRAGMA user_version = 2`,
	} {
		t.Run(mutation, func(t *testing.T) {
			parent := t.TempDir()
			if err := os.Chmod(parent, 0o700); err != nil {
				t.Fatal(err)
			}
			state := filepath.Join(parent, "state")
			if err := os.Mkdir(state, 0o700); err != nil {
				t.Fatal(err)
			}
			path := filepath.Join(state, databaseName)
			db, err := sql.Open("sqlite", path)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := db.Exec(mutation); err != nil {
				t.Fatal(err)
			}
			_ = db.Close()
			if err := os.Chmod(path, 0o600); err != nil {
				t.Fatal(err)
			}
			root, err := os.OpenRoot(state)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = root.Close() }()
			if _, err := OpenBound(context.Background(), root); !errors.Is(err, ErrSchemaIncompatible) {
				t.Fatalf("drifted schema error = %v", err)
			}
		})
	}
}

func TestOpenRejectsSymlinkAndHardLinkedDatabase(t *testing.T) {
	for _, kind := range []string{"symlink", "hardlink"} {
		t.Run(kind, func(t *testing.T) {
			parent := t.TempDir()
			if err := os.Chmod(parent, 0o700); err != nil {
				t.Fatal(err)
			}
			state := filepath.Join(parent, "state")
			if err := os.Mkdir(state, 0o700); err != nil {
				t.Fatal(err)
			}
			external := filepath.Join(parent, "external")
			if err := os.WriteFile(external, nil, 0o600); err != nil {
				t.Fatal(err)
			}
			target := filepath.Join(state, databaseName)
			var err error
			if kind == "symlink" {
				err = os.Symlink(external, target)
			} else {
				err = os.Link(external, target)
			}
			if err != nil {
				t.Fatal(err)
			}
			root, err := os.OpenRoot(state)
			if err != nil {
				t.Fatal(err)
			}
			defer func() { _ = root.Close() }()
			if _, err := OpenBound(context.Background(), root); err == nil {
				t.Fatal("unsafe control.db succeeded")
			}
		})
	}
}

func TestReopenRejectsRelationalAggregateHeadDrift(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 700)); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE aggregate_heads SET current_version = 2`); err != nil {
		t.Fatal(err)
	}
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

func TestEventReadRejectsDigestDriftWithExactCatalogRestored(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 701)); err != nil {
		t.Fatal(err)
	}
	trigger := schemaDDL(t, "control_events_no_update")
	if _, err := value.store.db.Exec(`DROP TRIGGER control_events_no_update`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE control_events SET event_bytes = X'78'`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(trigger); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.Events(context.Background(), 0, 10); !errors.Is(err, ErrCorruptStore) {
		t.Fatalf("digest drift read error = %v", err)
	}
}

func schemaDDL(t *testing.T, name string) string {
	t.Helper()
	for _, object := range schemaObjects {
		if object.name == name {
			return object.ddl
		}
	}
	t.Fatalf("schema object %s not found", name)
	return ""
}
