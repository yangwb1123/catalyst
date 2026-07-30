package migrationtxn

import (
	"bytes"
	"os"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestLifecycleMigrationRejectsPreexistingManualReceiptDrift(t *testing.T) {
	source := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Source\n"),
	)
	if _, err := ApplyManual(source); err != nil {
		t.Fatal(err)
	}
	manualPath, err := receiptPathForOperation(source, manualModeOperationID)
	if err != nil {
		t.Fatal(err)
	}
	manualReceipt := readTestFile(t, manualPath)

	target := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Target\n"),
	)
	seedTerminalReceipt(t, target, manualModeOperationID, manualReceipt)
	assertCrossReceiptBlocks(t, target, func() error {
		_, err := Preview(target, productionRequest())
		return err
	}, func() error {
		_, err := Apply(target, productionRequest())
		return err
	})
}

func TestManualMigrationRejectsPreexistingLifecycleReceiptDrift(t *testing.T) {
	source := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Source\n"),
	)
	if _, err := Apply(source, productionRequest()); err != nil {
		t.Fatal(err)
	}
	lifecycleReceipt := readTestFile(t, receiptPath(source))

	target := newRepository(
		t, migrate.ModeExplorer, migrate.LifecycleMVP, []byte("# Target\n"),
	)
	seedTerminalReceipt(t, target, promotionOperationID, lifecycleReceipt)
	assertCrossReceiptBlocks(t, target, func() error {
		_, err := PreviewManual(target)
		return err
	}, func() error {
		_, err := ApplyManual(target)
		return err
	})
}

func seedTerminalReceipt(
	t *testing.T,
	root, operation string,
	data []byte,
) {
	t.Helper()
	ops := realFileOps{}
	if err := ensureMigrationStateDir(root, ops); err != nil {
		t.Fatal(err)
	}
	path, err := receiptPathForOperation(root, operation)
	if err != nil {
		t.Fatal(err)
	}
	if err := ops.writeState(path, data); err != nil {
		t.Fatal(err)
	}
}

func assertCrossReceiptBlocks(
	t *testing.T,
	root string,
	preview, apply func() error,
) {
	t.Helper()
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	stateBefore := migrationStateBytes(t, root)
	if err := preview(); err == nil {
		t.Fatal("preview accepted conflicting terminal receipt")
	}
	if err := apply(); err == nil {
		t.Fatal("apply accepted conflicting terminal receipt")
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertBytesEqual(t, "migration state", migrationStateBytes(t, root), stateBefore)
	assertPending(t, root, false)
}

func migrationStateBytes(t *testing.T, root string) []byte {
	t.Helper()
	entries, err := os.ReadDir(migrationStateDir(root))
	if err != nil {
		t.Fatal(err)
	}
	var state []byte
	for _, entry := range entries {
		state = append(state, []byte(entry.Name())...)
		state = append(state, 0)
		state = append(state, readTestFile(t, migrationStateDir(root)+"/"+entry.Name())...)
		state = append(state, 0)
	}
	return bytes.Clone(state)
}
