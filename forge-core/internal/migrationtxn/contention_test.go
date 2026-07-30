package migrationtxn

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/migrate"
	"forgeos/forge-core/internal/runlock"
)

func TestMigrationAPIsRespectSharedRepositoryLockWithoutMutation(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP,
		[]byte("# Roadmap\n"))
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	lock, err := runlock.Acquire(root)
	if err != nil {
		t.Fatalf("hold repository lock: %v", err)
	}
	defer lock.Release()
	lockBefore := readTestFile(t, lock.Path)

	calls := []struct {
		name string
		run  func() error
	}{
		{"lifecycle-preview", func() error {
			_, err := Preview(root, productionRequest())
			return err
		}},
		{"lifecycle-apply", func() error {
			_, err := Apply(root, productionRequest())
			return err
		}},
		{"manual-preview", func() error {
			_, err := PreviewManual(root)
			return err
		}},
		{"manual-apply", func() error {
			_, err := ApplyManual(root)
			return err
		}},
	}
	for _, call := range calls {
		t.Run(call.name, func(t *testing.T) {
			if err := call.run(); err == nil {
				t.Fatal("migration API ignored held repository lock")
			}
		})
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	if got := readTestFile(t, lock.Path); !bytes.Equal(got, lockBefore) {
		t.Fatalf("contention probe changed run.lock\nbefore=%q\nafter=%q", lockBefore, got)
	}
	if _, err := os.Lstat(filepath.Join(root, ".forge", "migrations")); !os.IsNotExist(err) {
		t.Fatalf("contention created migration state: %v", err)
	}
}
