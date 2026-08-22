package projectsnapshot

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestCaptureProducesCanonicalValidatedTwoPassEnvelope(t *testing.T) {
	root, environment := snapshotFixture(t)
	production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	if err := Validate(production); err != nil {
		t.Fatal(err)
	}
	if !json.Valid(production.JSON()) || bytes.HasSuffix(production.JSON(), []byte("\n")) {
		t.Fatal("capture output is not exact canonical JSON without framing")
	}
	value := production.Envelope()
	if value.Snapshot.Consistency != consistencyValue || value.Snapshot.Atomic ||
		value.Snapshot.TruthAttested || value.Snapshot.AuthorityAttested {
		t.Fatalf("snapshot semantics drifted: %+v", value.Snapshot)
	}
	if len(value.Snapshot.SourceManifest.Entries) != 3 ||
		len(value.Snapshot.SourceManifest.Excluded) != 3 {
		t.Fatalf("manifest counts = %d/%d", len(value.Snapshot.SourceManifest.Entries),
			len(value.Snapshot.SourceManifest.Excluded))
	}
}

func TestProtectedPathsAreClassifiedBeforeLeafLstat(t *testing.T) {
	root, environment := snapshotFixture(t)
	var lock sync.Mutex
	stages := map[string][]string{}
	_, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			lock.Lock()
			stages[path] = append(stages[path], stage)
			lock.Unlock()
		})
	if err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{".env", ".forge/state", "secrets/token"} {
		got := strings.Join(stages[path], ",")
		if got != observeBeforeClassification+","+observeBeforeClassification {
			t.Fatalf("protected %s filesystem stages = %q", path, got)
		}
	}
}

func TestSensitiveSymlinkIsNeverReadlinkAndEmitsNoTarget(t *testing.T) {
	root, environment := snapshotFixture(t)
	if err := os.Remove(filepath.Join(root, ".env")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("do-not-leak-target", filepath.Join(root, ".env")); err != nil {
		t.Fatal(err)
	}
	production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	encoded := string(production.JSON())
	if strings.Contains(encoded, "do-not-leak-target") || strings.Contains(encoded, `"path":".env"`) {
		t.Fatalf("sensitive symlink metadata leaked: %s", encoded)
	}
	assertExcludedReason(t, production, "sensitive_path", false)
}

func TestNonSensitiveSymlinkExcludesWithoutTarget(t *testing.T) {
	root, environment := snapshotFixture(t)
	if err := os.Symlink("public.txt", filepath.Join(root, "public-link")); err != nil {
		t.Fatal(err)
	}
	production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	assertExcludedReason(t, production, "symlink_leaf", true)
	if strings.Contains(string(production.JSON()), "public-link") {
		t.Fatal("symlink raw path or target leaked")
	}
}

func TestCaptureRejectsRegularHardlink(t *testing.T) {
	root, environment := snapshotFixture(t)
	if err := os.Link(filepath.Join(root, "public.txt"), filepath.Join(root, "alias.txt")); err != nil {
		t.Fatal(err)
	}
	if production, err := Capture(context.Background(), root, environment, "project-1", "run-1"); err == nil || production != nil || !strings.Contains(err.Error(), "exactly one link") {
		t.Fatalf("hardlink capture = %#v, %v", production, err)
	}
}

func TestCaptureRejectsContentDriftBetweenFullPasses(t *testing.T) {
	root, environment := snapshotFixture(t)
	mutated := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			if !mutated && stage == observeAfterFullPass {
				mutated = true
				_ = os.WriteFile(filepath.Join(root, "untracked.txt"), []byte("drift"), 0o644)
			}
		})
	if !mutated || err == nil || production != nil {
		t.Fatalf("two-pass drift capture = %#v, %v, mutated=%v", production, err, mutated)
	}
}

func TestCaptureRejectsRootReplacementBetweenPasses(t *testing.T) {
	root, environment := snapshotFixture(t)
	replaced := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, _ string) {
			if !replaced && stage == observeAfterFullPass {
				replaced = true
				if renameErr := os.Rename(root, root+"-old"); renameErr != nil {
					t.Fatal(renameErr)
				}
				if mkdirErr := os.Mkdir(root, 0o755); mkdirErr != nil {
					t.Fatal(mkdirErr)
				}
			}
		})
	if !replaced || err == nil || production != nil {
		t.Fatalf("root replacement capture = %#v, %v, replaced=%v", production, err, replaced)
	}
}

func TestCaptureRejectsTrackedGitlinkAndNonordinaryIndexFlag(t *testing.T) {
	root, environment := snapshotFixture(t)
	runSnapshotGit(t, root, "update-index", "--skip-worktree", "public.txt")
	if production, err := Capture(context.Background(), root, environment, "p", "r"); err == nil || production != nil {
		t.Fatalf("skip-worktree capture = %#v, %v", production, err)
	}
	runSnapshotGit(t, root, "update-index", "--no-skip-worktree", "public.txt")
	runSnapshotGit(t, root, "update-index", "--add", "--cacheinfo", "160000,"+
		strings.Repeat("1", 40)+",module")
	if production, err := Capture(context.Background(), root, environment, "p", "r"); err == nil || production != nil {
		t.Fatalf("gitlink capture = %#v, %v", production, err)
	}
}

func TestCaptureRejectsIntentToAddIndexFlag(t *testing.T) {
	root, environment := snapshotFixture(t)
	writeSnapshotFile(t, root, "intent.txt", "intent")
	runSnapshotGit(t, root, "add", "--intent-to-add", "intent.txt")
	production, err := Capture(context.Background(), root, environment, "p", "r")
	if err == nil || production != nil {
		t.Fatalf("intent-to-add capture returned production=%v, err=%v", production != nil, err)
	}
}

func TestCaptureRejectsRootSymlinkAndInvalidIdentifiers(t *testing.T) {
	root, environment := snapshotFixture(t)
	alias := filepath.Join(t.TempDir(), "repo-link")
	if err := os.Symlink(root, alias); err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct{ root, project, run string }{
		{alias, "p", "r"}, {root, "Upper", "r"}, {root, "p", "bad space"},
	} {
		if production, err := Capture(context.Background(), test.root, environment, test.project, test.run); err == nil || production != nil {
			t.Fatalf("unsafe request accepted: %#v, %v", production, err)
		}
	}
}

func TestTrackedPathWithInitiallyMissingParentIsAbsent(t *testing.T) {
	root, environment := snapshotFixture(t)
	oid := strings.TrimSpace(snapshotGitOutput(t, root, "rev-parse", "HEAD:public.txt"))
	runSnapshotGit(t, root, "update-index", "--add", "--cacheinfo", "100644,"+oid+",absent/child.txt")
	production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range production.Envelope().Snapshot.SourceManifest.Entries {
		if entry.Path == "absent/child.txt" && entry.Kind == "tracked_absent" {
			return
		}
	}
	t.Fatal("index-only path with missing parent was not recorded as tracked_absent")
}

func TestTrackedParentDisappearanceRaceIsRejected(t *testing.T) {
	root, environment := snapshotFixture(t)
	writeSnapshotFile(t, root, "directory/item.txt", "item")
	runSnapshotGit(t, root, "add", "directory/item.txt")
	runSnapshotGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=f@example.invalid", "commit", "-qm", "parent")
	mutated := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			if !mutated && stage == observeBeforeLeafLstat && path == "directory/item.txt" {
				mutated = true
				_ = os.Rename(filepath.Join(root, "directory"), filepath.Join(root, "directory-old"))
			}
		})
	if !mutated || err == nil || production != nil {
		t.Fatalf("parent disappearance capture = %#v, %v, mutated=%v", production, err, mutated)
	}
}

func TestCaptureRejectsBOMInTrackedAndUntrackedPaths(t *testing.T) {
	for _, tracked := range []bool{false, true} {
		t.Run(map[bool]string{false: "untracked", true: "tracked"}[tracked], func(t *testing.T) {
			root, environment := snapshotFixture(t)
			path := "bom\ufeff.txt"
			writeSnapshotFile(t, root, path, "bom")
			if tracked {
				runSnapshotGit(t, root, "add", path)
			}
			production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
			if err == nil || production != nil {
				t.Fatalf("BOM path capture = %#v, %v", production, err)
			}
		})
	}
}

func assertExcludedReason(t *testing.T, production *Production, reason string, observed bool) {
	t.Helper()
	for _, item := range production.Envelope().Snapshot.SourceManifest.Excluded {
		if item.Reason == reason && item.LeafFilesystemObserved == observed {
			return
		}
	}
	t.Fatalf("missing exclusion %s observed=%v", reason, observed)
}

func snapshotFixture(t *testing.T) (string, []string) {
	t.Helper()
	root := t.TempDir()
	writeSnapshotFile(t, root, "public.txt", "public")
	writeSnapshotFile(t, root, "gone.txt", "gone")
	writeSnapshotFile(t, root, ".env", "secret-value")
	writeSnapshotFile(t, root, ".forge/state", "control-value")
	writeSnapshotFile(t, root, "secrets/token", "token-value")
	runSnapshotGit(t, root, "init", "-q")
	runSnapshotGit(t, root, "add", "public.txt", "gone.txt", ".env", ".forge/state", "secrets/token")
	runSnapshotGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=f@example.invalid", "commit", "-qm", "fixture")
	if err := os.Remove(filepath.Join(root, "gone.txt")); err != nil {
		t.Fatal(err)
	}
	writeSnapshotFile(t, root, "untracked.txt", "untracked")
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	return root, []string{"PATH=" + filepath.Dir(git)}
}

func writeSnapshotFile(t *testing.T, root, path, content string) {
	t.Helper()
	absolute := filepath.Join(root, filepath.FromSlash(path))
	if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(absolute, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func runSnapshotGit(t *testing.T, root string, args ...string) {
	t.Helper()
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	command := exec.Command(git, append([]string{"-C", root}, args...)...)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
}

func snapshotGitOutput(t *testing.T, root string, args ...string) string {
	t.Helper()
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	command := exec.Command(git, append([]string{"-C", root}, args...)...)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
	return string(output)
}
