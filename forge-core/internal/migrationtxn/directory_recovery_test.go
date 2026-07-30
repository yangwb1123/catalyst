package migrationtxn

import (
	"os"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestLifecycleRecoverySecuresPreexistingMigrationDirectory(t *testing.T) {
	root := newRepository(t, migrate.ModeExplorer, migrate.LifecycleMVP,
		[]byte("# Recovery roadmap\n"))
	ensureTestForgeDir(t, root)
	seedPendingBeforeRoadmap(t, root)
	dir := migrationStateDir(root)
	if err := os.Chmod(dir, 0o777); err != nil {
		t.Fatalf("chmod migration directory: %v", err)
	}
	if mode := testFileMode(t, dir); mode != 0o777 {
		t.Fatalf("migration directory precondition mode = %#o, want 0777", mode)
	}

	recovered, err := Apply(root, productionRequest())
	if err != nil {
		t.Fatalf("Apply recovery: %v", err)
	}
	assertResult(t, recovered, StatusRecovered,
		"explorer", "engineering", "mvp", true, len(expectedPromotionTaskIDs))
	if mode := testFileMode(t, dir); mode != 0o700 {
		t.Fatalf("migration directory mode after recovery = %#o, want 0700", mode)
	}
	assertPromotedExplorer(t, root, migrate.LifecycleMVP)
}
