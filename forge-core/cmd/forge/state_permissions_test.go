package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/trace"
)

func TestChainStateIsPrivate(t *testing.T) {
	root := t.TempDir()
	if err := saveChainState(root, chainState{Status: "running", CurrentStage: "build"}); err != nil {
		t.Fatal(err)
	}
	assertPrivateStateFile(t, chainStatePath(root))
}

func TestApprovalDecisionMarkerIsPrivate(t *testing.T) {
	root := t.TempDir()
	captureStdout(t, func() {
		if code := writeApproval(root, "design", true); code != 0 {
			t.Fatalf("writeApproval = %d", code)
		}
	})
	assertPrivateStateFile(t, filepath.Join(forgeDir(root), "design.approved"))
}

func TestTraceFileIsPrivate(t *testing.T) {
	root := t.TempDir()
	tracer, closeTrace, err := openTracer(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := tracer.Emit(trace.Event{Kind: "test", Name: "permissions"}); err != nil {
		closeTrace()
		t.Fatal(err)
	}
	closeTrace()
	assertPrivateStateFile(t, filepath.Join(forgeDir(root), "trace.jsonl"))
}

func TestOpenTracerRejectsTraceSymlinkWithoutClobber(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(forgeDir(root), 0o700); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(t.TempDir(), "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(sentinel, filepath.Join(forgeDir(root), "trace.jsonl")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if _, closeTrace, err := openTracer(root); err == nil {
		closeTrace()
		t.Fatal("trace symlink was accepted")
	}
	assertSentinelUnchanged(t, sentinel)
}

func TestChainStateRejectsLeafAndLegacyTempSymlinks(t *testing.T) {
	for _, suffix := range []string{"", ".tmp"} {
		t.Run("suffix="+suffix, func(t *testing.T) {
			root := t.TempDir()
			if err := os.Mkdir(forgeDir(root), 0o700); err != nil {
				t.Fatal(err)
			}
			sentinel := filepath.Join(t.TempDir(), "sentinel")
			if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(sentinel, chainStatePath(root)+suffix); err != nil {
				t.Skipf("symlink unavailable: %v", err)
			}
			if err := saveChainState(root, chainState{Status: "running"}); err == nil {
				t.Fatal("chain state alias was accepted")
			}
			assertSentinelUnchanged(t, sentinel)
		})
	}
}

func TestApprovalRejectsForgeDirectorySymlinkInsideRepository(t *testing.T) {
	root := t.TempDir()
	inside := filepath.Join(root, "docs")
	if err := os.Mkdir(inside, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(inside, forgeDir(root)); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if code := writeApproval(root, "design", true); code == 0 {
		t.Fatal(".forge directory symlink was accepted")
	}
	if _, err := os.Lstat(filepath.Join(inside, "design.approved")); !os.IsNotExist(err) {
		t.Fatalf("approval marker escaped through .forge symlink: %v", err)
	}
}

func TestReleaseApprovalRequiresStructuredPersistentMarker(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	if humanApproved(root, "deploy", true) {
		t.Fatal("--approved must not bypass a delivery-stage evidence boundary")
	}
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(approvalPath(root, "deploy"), nil, 0o600); err != nil {
		t.Fatal(err)
	}
	if humanApproved(root, "deploy", false) {
		t.Fatal("an empty/legacy marker must not approve a delivery stage")
	}
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("writeApproval(deploy) = %d", code)
		}
	})
	if !humanApproved(root, "deploy", false) {
		t.Fatal("forge approve deploy must create a valid persistent decision")
	}
	data, err := os.ReadFile(approvalPath(root, "deploy"))
	if err != nil {
		t.Fatal(err)
	}
	var marker decisionMarker
	if json.Unmarshal(data, &marker) != nil || marker.Stage != "deploy" ||
		marker.Decision != "approved" || marker.SourceRevision == "" || marker.ArtifactDigest == "" {
		t.Fatalf("approval marker metadata = %s", data)
	}
}

func TestReleaseEngineerReadonlyScopeIsDocsOnly(t *testing.T) {
	phase := asset.Phase{
		Agent: "release-engineer", Readonly: true,
		Emits: []string{
			"docs/release/deployment-plan.md",
			"docs/release/deployment-runbook.md",
		},
	}
	deny, allow := readonlyToolScope(phase)
	if deny != "Bash WebFetch WebSearch" ||
		allow != "Edit(/docs/release/deployment-plan.md) Edit(/docs/release/deployment-runbook.md)" ||
		strings.Contains(allow, "/docs/release/**") ||
		strings.Contains(allow, "Write(") {
		t.Fatalf("release-engineer scope = deny %q allow %q", deny, allow)
	}
}

func assertPrivateStateFile(t *testing.T, path string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("%s permissions = %04o, want 0600", path, got)
	}
}

func assertSentinelUnchanged(t *testing.T, path string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}
