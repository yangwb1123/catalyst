//go:build linux && !android

package appserver

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRunInitializesPrivateControlStoreBeforeAnnouncement(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	ctx, cancel := context.WithCancel(context.Background())
	err := Run(ctx, serverConfig(stateDir), func(Ready) error {
		info, statErr := os.Stat(filepath.Join(stateDir, controlDBName))
		if statErr != nil || !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
			t.Fatalf("control store before announcement = %v, %v", info, statErr)
		}
		cancel()
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	lock, err := acquireInstanceLock(stateDir)
	if err != nil {
		t.Fatalf("reopen state with control store: %v", err)
	}
	_ = lock.Close()
}

func TestRunRejectsIncompatibleControlSchemaBeforeAnnouncement(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	ctx, cancel := context.WithCancel(context.Background())
	if err := Run(ctx, serverConfig(stateDir), func(Ready) error {
		cancel()
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	db, err := sql.Open("sqlite", filepath.Join(stateDir, controlDBName))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`PRAGMA user_version = 2`); err != nil {
		_ = db.Close()
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	announced := false
	err = Run(context.Background(), serverConfig(stateDir), func(Ready) error {
		announced = true
		return nil
	})
	if err == nil || !strings.Contains(err.Error(), "schema is incompatible") {
		t.Fatalf("incompatible schema error = %v", err)
	}
	if announced {
		t.Fatal("incompatible schema announced readiness")
	}
}

func TestInstanceLockRejectsControlSidecarWithoutDatabase(t *testing.T) {
	stateDir := initializedStateDir(t)
	if err := os.WriteFile(filepath.Join(stateDir, "control.db-wal"), []byte("orphan"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("orphan control sidecar succeeded")
	}
}

func TestInstanceLockRejectsHardLinkedControlFile(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	ctx, cancel := context.WithCancel(context.Background())
	if err := Run(ctx, serverConfig(stateDir), func(Ready) error {
		cancel()
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	database := filepath.Join(stateDir, controlDBName)
	external := filepath.Join(filepath.Dir(stateDir), "control-copy")
	if err := os.Link(database, external); err != nil {
		t.Fatal(err)
	}
	if _, err := acquireInstanceLock(stateDir); err == nil {
		t.Fatal("hard-linked control database succeeded")
	}
	if info, err := os.Stat(database); err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("rejected database was changed: %v, %v", info, err)
	}
}

func TestRunRejectsCancelledSchemaOpenWithoutReadiness(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	announced := false
	err := Run(ctx, serverConfig(stateDir), func(Ready) error {
		announced = true
		return nil
	})
	if !errors.Is(err, context.Canceled) || announced {
		t.Fatalf("cancelled startup = %v, announced=%t", err, announced)
	}
}
