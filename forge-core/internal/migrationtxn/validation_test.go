package migrationtxn

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/migrate"
)

func TestMalformedProjectSelectorsFailClosedWithoutMutation(t *testing.T) {
	cases := []struct {
		name    string
		project string
		wantErr string
	}{
		{"missing-lifecycle", "mode: explorer\n", "missing top-level lifecycle"},
		{"missing-mode", "lifecycle: mvp\n", "missing top-level mode"},
		{"duplicate-mode", "mode: explorer\nmode: balanced\nlifecycle: mvp\n", "duplicate top-level mode"},
		{"duplicate-lifecycle", "mode: explorer\nlifecycle: mvp\nlifecycle: growth\n", "duplicate top-level lifecycle"},
		{"unknown-mode", "mode: prototype\nlifecycle: mvp\n", "unknown persistent mode"},
		{"unknown-lifecycle", "mode: explorer\nlifecycle: beta\n", "unknown persistent lifecycle"},
		{"unterminated-quote", "mode: \"explorer\nlifecycle: mvp\n", "unterminated quoted scalar"},
		{"mismatched-quote", "mode: \"explorer'\nlifecycle: mvp\n", "unterminated quoted scalar"},
		{"nested-only", "  mode: explorer\n  lifecycle: mvp\n", "missing top-level mode"},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			testMalformedProjectSelectors(t, []byte(testCase.project), testCase.wantErr)
		})
	}
}

func testMalformedProjectSelectors(t *testing.T, project []byte, wantErr string) {
	t.Helper()
	roadmap := []byte("# Roadmap\n")
	root := newRawRepository(t, project, roadmap)
	for _, operation := range []struct {
		name string
		run  func() error
	}{
		{"preview", func() error {
			_, err := Preview(root, productionRequest())
			return err
		}},
		{"apply", func() error {
			_, err := Apply(root, productionRequest())
			return err
		}},
	} {
		t.Run(operation.name, func(t *testing.T) {
			err := operation.run()
			if err == nil || !strings.Contains(err.Error(), wantErr) {
				t.Fatalf("error = %v, want substring %q", err, wantErr)
			}
		})
	}
	assertBytesEqual(t, "malformed project", readTestFile(t, projectPath(root)), project)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmap)
	assertPathMissing(t, filepath.Join(root, ".forge"))
}

func TestMalformedOrPreexistingPromotionMarkersFailClosed(t *testing.T) {
	tasks := migrate.ExplorerToEngineering().Tasks
	canonical := promotionRoadmapBlock(tasks, "\n")
	missingTask := bytes.Replace(
		canonical,
		[]byte("forge:migration-task:"+expectedPromotionTaskIDs[2]),
		[]byte("forge:migration-task:missing"),
		1,
	)
	cases := []struct {
		name    string
		roadmap []byte
		wantErr string
	}{
		{"start-only", []byte(promotionBlockStart + "\n"), "markers are incomplete or duplicated"},
		{"task-only", []byte("<!-- forge:migration-task:backfill-tests -->\n"), "markers are incomplete or duplicated"},
		{"canonical-without-receipt", canonical, "already exists without a matching terminal receipt"},
		{"missing-task", missingTask, "promotion task marker"},
		{"duplicated-block", append(append([]byte(nil), canonical...), canonical...), "markers are incomplete or duplicated"},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			root := newRepository(t, "explorer", "mvp", testCase.roadmap)
			projectBefore := readTestFile(t, projectPath(root))
			_, err := Preview(root, productionRequest())
			if err == nil || !strings.Contains(err.Error(), testCase.wantErr) {
				t.Fatalf("Preview error = %v, want substring %q", err, testCase.wantErr)
			}
			assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
			assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), testCase.roadmap)
			assertPathMissing(t, filepath.Join(root, ".forge"))
		})
	}
}

type malformedIntentCase struct {
	name      string
	transform func([]byte, promotionIntent) []byte
	wantErr   string
}

var malformedIntentCases = []malformedIntentCase{
	{
		name: "truncated-json",
		transform: func(_ []byte, _ promotionIntent) []byte {
			return []byte(`{"format":`)
		},
		wantErr: "decode state",
	},
	{
		name: "unknown-field",
		transform: func(valid []byte, _ promotionIntent) []byte {
			return bytes.Replace(valid, []byte("{\n"),
				[]byte("{\n  \"unknown\": true,\n"), 1)
		},
		wantErr: "unknown field",
	},
	{
		name: "noncanonical-encoding",
		transform: func(valid []byte, _ promotionIntent) []byte {
			return append(append([]byte(nil), valid...), ' ')
		},
		wantErr: "not in canonical encoding",
	},
	{
		name: "digest-mismatch",
		transform: func(_ []byte, intent promotionIntent) []byte {
			intent.ProjectBefore.SHA256 = strings.Repeat("0", 64)
			data, _ := marshalCanonical(intent)
			return data
		},
		wantErr: "project before image digest mismatch",
	},
	{
		name: "transition-mismatch",
		transform: func(_ []byte, intent promotionIntent) []byte {
			intent.Receipt.ToMode = migrate.ModeExplorer
			data, _ := marshalCanonical(intent)
			return data
		},
		wantErr: "transition disagrees",
	},
	{
		name: "present-image-null-data",
		transform: func(_ []byte, intent promotionIntent) []byte {
			image := newFileImage(nil, os.FileMode(intent.ProjectAfter.Mode), true)
			image.Data = nil
			intent.ProjectAfter = image
			data, _ := marshalCanonical(intent)
			return data
		},
		wantErr: "non-canonical null data",
	},
	{
		name: "absent-image-empty-data",
		transform: func(_ []byte, intent promotionIntent) []byte {
			intent.RoadmapBefore = fileImage{Data: []byte{}}
			data, _ := marshalCanonical(intent)
			return data
		},
		wantErr: "absent image carries content",
	},
}

func TestMalformedPendingIntentNeverMutatesTrackedFiles(t *testing.T) {
	for _, testCase := range malformedIntentCases {
		t.Run(testCase.name, func(t *testing.T) {
			testMalformedPendingIntent(t, testCase.transform, testCase.wantErr)
		})
	}
}

type terminalDriftCase struct {
	name    string
	drift   func(*testing.T, string)
	wantErr string
}

var terminalDriftCases = []terminalDriftCase{
	{
		name: "roadmap-marker-removed",
		drift: func(t *testing.T, root string) {
			path := roadmapPath(root)
			data := bytes.Replace(readTestFile(t, path),
				[]byte("forge:migration-task:add-ci"),
				[]byte("forge:migration-task:removed"), 1)
			writeTestFile(t, path, data, testFileMode(t, path))
		},
		wantErr: "promotion markers drifted",
	},
	{
		name: "roadmap-removed",
		drift: func(t *testing.T, root string) {
			if err := os.Remove(roadmapPath(root)); err != nil {
				t.Fatalf("remove roadmap: %v", err)
			}
		},
		wantErr: "promotion markers drifted",
	},
	{
		name: "mode-changed",
		drift: func(t *testing.T, root string) {
			path := projectPath(root)
			data := bytes.Replace(readTestFile(t, path),
				[]byte("mode: engineering"), []byte("mode: balanced"), 1)
			writeTestFile(t, path, data, testFileMode(t, path))
		},
		wantErr: "conflicts with current project selectors",
	},
	{
		name: "lifecycle-demoted",
		drift: func(t *testing.T, root string) {
			path := projectPath(root)
			data := bytes.Replace(readTestFile(t, path),
				[]byte("lifecycle: production"), []byte("lifecycle: growth"), 1)
			writeTestFile(t, path, data, testFileMode(t, path))
		},
		wantErr: "conflicts with non-production project state",
	},
}

func TestTerminalReceiptDetectsPromotedStateDrift(t *testing.T) {
	for _, testCase := range terminalDriftCases {
		t.Run(testCase.name, func(t *testing.T) {
			testTerminalDrift(t, testCase)
		})
	}
}

func testTerminalDrift(t *testing.T, testCase terminalDriftCase) {
	t.Helper()
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	if _, err := Apply(root, productionRequest()); err != nil {
		t.Fatalf("seed completed promotion: %v", err)
	}
	testCase.drift(t, root)
	_, err := Preview(root, productionRequest())
	if err == nil || !strings.Contains(err.Error(), testCase.wantErr) {
		t.Fatalf("Preview drift error = %v, want substring %q", err, testCase.wantErr)
	}
	assertPending(t, root, false)
}

func testMalformedPendingIntent(
	t *testing.T,
	transform func([]byte, promotionIntent) []byte,
	wantErr string,
) {
	t.Helper()
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	ensureTestForgeDir(t, root)
	result, intent, err := preparePromotion(root, realFileOps{})
	if err != nil || intent == nil || result.Status != StatusPlanned {
		t.Fatalf("prepare valid intent: result=%+v intent=%v err=%v", result, intent != nil, err)
	}
	valid, err := encodeIntent(*intent)
	if err != nil {
		t.Fatalf("encode valid intent: %v", err)
	}
	if err := ensureMigrationStateDir(root, realFileOps{}); err != nil {
		t.Fatalf("ensure migration state dir: %v", err)
	}
	data := transform(valid, *intent)
	if err := (realFileOps{}).writeState(pendingPath(root), data); err != nil {
		t.Fatalf("seed malformed pending intent: %v", err)
	}

	_, err = applyLocked(root, realFileOps{})
	if err == nil || !strings.Contains(err.Error(), wantErr) {
		t.Fatalf("recovery error = %v, want substring %q", err, wantErr)
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertPending(t, root, true)
}

var malformedReceiptCases = []struct {
	name      string
	transform func([]byte, promotionReceipt) []byte
	wantErr   string
}{
	{
		name: "truncated-json",
		transform: func(_ []byte, _ promotionReceipt) []byte {
			return []byte(`{"format":`)
		},
		wantErr: "decode state",
	},
	{
		name: "unknown-field",
		transform: func(valid []byte, _ promotionReceipt) []byte {
			return bytes.Replace(valid, []byte("{\n"),
				[]byte("{\n  \"unknown\": true,\n"), 1)
		},
		wantErr: "unknown field",
	},
	{
		name: "noncanonical-encoding",
		transform: func(valid []byte, _ promotionReceipt) []byte {
			return append(append([]byte(nil), valid...), '\n')
		},
		wantErr: "not in canonical encoding",
	},
	{
		name: "task-manifest-mismatch",
		transform: func(_ []byte, receipt promotionReceipt) []byte {
			receipt.TaskIDs = append([]string(nil), receipt.TaskIDs[:len(receipt.TaskIDs)-1]...)
			data, _ := marshalCanonical(receipt)
			return data
		},
		wantErr: "task manifest disagrees",
	},
	{
		name: "null-task-manifest",
		transform: func(_ []byte, receipt promotionReceipt) []byte {
			receipt.TaskIDs = nil
			data, _ := marshalCanonical(receipt)
			return data
		},
		wantErr: "task manifest uses non-canonical null",
	},
}

func TestMalformedTerminalReceiptFailsClosed(t *testing.T) {
	for _, testCase := range malformedReceiptCases {
		t.Run(testCase.name, func(t *testing.T) {
			testMalformedTerminalReceipt(t, testCase.transform, testCase.wantErr)
		})
	}
}

func testMalformedTerminalReceipt(
	t *testing.T,
	transform func([]byte, promotionReceipt) []byte,
	wantErr string,
) {
	t.Helper()
	root := newRepository(t, "explorer", "mvp", []byte("# Roadmap\n"))
	if _, err := Apply(root, productionRequest()); err != nil {
		t.Fatalf("seed completed promotion: %v", err)
	}
	projectBefore := readTestFile(t, projectPath(root))
	roadmapBefore := readTestFile(t, roadmapPath(root))
	valid := readTestFile(t, receiptPath(root))
	receipt, err := decodeReceipt(valid)
	if err != nil {
		t.Fatalf("decode seed receipt: %v", err)
	}
	writeTestFile(t, receiptPath(root), transform(valid, receipt), 0o600)

	for _, operation := range []struct {
		name string
		run  func() error
	}{
		{"preview", func() error {
			_, err := Preview(root, productionRequest())
			return err
		}},
		{"apply", func() error {
			_, err := Apply(root, productionRequest())
			return err
		}},
	} {
		t.Run(operation.name, func(t *testing.T) {
			err := operation.run()
			if err == nil || !strings.Contains(err.Error(), wantErr) {
				t.Fatalf("error = %v, want substring %q", err, wantErr)
			}
		})
	}
	assertBytesEqual(t, "project", readTestFile(t, projectPath(root)), projectBefore)
	assertBytesEqual(t, "roadmap", readTestFile(t, roadmapPath(root)), roadmapBefore)
	assertPending(t, root, false)
}
