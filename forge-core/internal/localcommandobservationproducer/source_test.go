package localcommandobservationproducer

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestSourceSnapshotBindsTrackedUntrackedReleaseSymlinkAndDeletion(t *testing.T) {
	root, environment := sourceFixture(t)
	manifest, digest, err := sourceSnapshot(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	byPath := sourceEntriesByPath(manifest.Entries)
	for _, path := range []string{"docs/release/plan.md", "linked.txt", "tracked.txt", "untracked.txt"} {
		if byPath[path].Path == "" {
			t.Fatalf("source manifest omitted %q: %#v", path, manifest.Entries)
		}
	}
	if _, exists := byPath[".forge/state.json"]; exists {
		t.Fatal("untracked .forge control state must be excluded")
	}
	linked := byPath["linked.txt"]
	if linked.Kind != "symlink" || linked.Executable == nil || *linked.Executable ||
		byPath["tracked.txt"].Tracking != "tracked" ||
		byPath["untracked.txt"].Tracking != "untracked" || !strings.HasPrefix(manifest.SourceRevision, "git-sha1:") {
		t.Fatalf("source entry semantics drifted: %#v", manifest)
	}
	if err := os.Remove(filepath.Join(root, "tracked.txt")); err != nil {
		t.Fatal(err)
	}
	changed, changedDigest, err := sourceSnapshot(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if changedDigest == digest || sourceEntriesByPath(changed.Entries)["tracked.txt"].Kind != "deleted" {
		t.Fatalf("tracked deletion not identity-bearing: %s %s %#v", digest, changedDigest, changed.Entries)
	}
}

func TestSourceSnapshotRejectsTrackedForgeControlAndGitlinks(t *testing.T) {
	root, environment := sourceFixture(t)
	control := filepath.Join(root, ".forge", "tracked.json")
	if err := os.WriteFile(control, []byte("tracked-control"), 0o644); err != nil {
		t.Fatal(err)
	}
	runFixtureGit(t, root, "add", "-f", ".forge/tracked.json")
	if _, _, err := sourceSnapshot(context.Background(), root, environment); err == nil || !strings.Contains(err.Error(), "forbidden") {
		t.Fatalf("tracked .forge control state error = %v", err)
	}
}

func TestSourceDigestChangesWithContentAndExecutableMode(t *testing.T) {
	root, environment := sourceFixture(t)
	_, first, err := sourceSnapshot(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	tracked := filepath.Join(root, "tracked.txt")
	if err := os.WriteFile(tracked, []byte("changed"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(tracked, 0o755); err != nil {
		t.Fatal(err)
	}
	manifest, second, err := sourceSnapshot(context.Background(), root, environment)
	if err != nil || first == second {
		t.Fatalf("source content/mode drift first=%s second=%s err=%v", first, second, err)
	}
	entry := sourceEntriesByPath(manifest.Entries)["tracked.txt"]
	if entry.Executable == nil || !*entry.Executable || entry.ContentSHA256 == nil || *entry.ContentSHA256 != sha256Bytes([]byte("changed")) {
		t.Fatalf("changed regular entry = %#v", entry)
	}
}

func TestSourceSnapshotRejectsTrackedIndexAndWorkingTreeKindDrift(t *testing.T) {
	t.Run("index regular becomes symlink", func(t *testing.T) {
		root, environment := sourceFixture(t)
		tracked := filepath.Join(root, "tracked.txt")
		if err := os.Remove(tracked); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink("untracked.txt", tracked); err != nil {
			t.Fatal(err)
		}
		if _, _, err := sourceSnapshot(context.Background(), root, environment); err == nil ||
			!strings.Contains(err.Error(), "index regular file") {
			t.Fatalf("regular-to-symlink drift error = %v", err)
		}
	})

	t.Run("index symlink becomes regular", func(t *testing.T) {
		root, environment := sourceFixture(t)
		linked := filepath.Join(root, "linked.txt")
		if err := os.Remove(linked); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(linked, []byte("working regular"), 0o644); err != nil {
			t.Fatal(err)
		}
		if _, _, err := sourceSnapshot(context.Background(), root, environment); err == nil ||
			!strings.Contains(err.Error(), "index symlink") {
			t.Fatalf("symlink-to-regular drift error = %v", err)
		}
	})
}

func TestSourceSnapshotRejectsTrackedFileWithOutsideSymlinkParent(t *testing.T) {
	root, environment := sourceFixture(t)
	outside := t.TempDir()
	if err := os.WriteFile(filepath.Join(outside, "plan.md"), []byte("outside"), 0o644); err != nil {
		t.Fatal(err)
	}
	release := filepath.Join(root, "docs", "release")
	if err := os.RemoveAll(release); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, release); err != nil {
		t.Fatal(err)
	}
	if _, _, err := sourceSnapshot(context.Background(), root, environment); err == nil ||
		!strings.Contains(err.Error(), "forbidden symlink parent") ||
		!strings.Contains(err.Error(), "docs/release") {
		t.Fatalf("outside symlink parent error = %v", err)
	}
}

func TestSourceSnapshotAllowsLeafSymlinkWithOutsideTarget(t *testing.T) {
	root, environment := sourceFixture(t)
	outside := filepath.Join(t.TempDir(), "outside.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o644); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "docs", "release", "outside-link.txt")
	if err := os.Symlink(outside, path); err != nil {
		t.Fatal(err)
	}
	runFixtureGit(t, root, "add", "docs/release/outside-link.txt")
	manifest, _, err := sourceSnapshot(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	entry := sourceEntriesByPath(manifest.Entries)["docs/release/outside-link.txt"]
	if entry.Kind != "symlink" || entry.Tracking != "tracked" || entry.IndexMode == nil ||
		*entry.IndexMode != "120000" || entry.SymlinkTarget == nil || *entry.SymlinkTarget != outside ||
		entry.ContentSHA256 == nil || *entry.ContentSHA256 != sha256Bytes([]byte(outside)) {
		t.Fatalf("outside-target leaf symlink = %#v", entry)
	}
}

func sourceFixture(t *testing.T) (string, []string) {
	t.Helper()
	root := t.TempDir()
	for path, value := range map[string]string{
		"tracked.txt": "tracked", "docs/release/plan.md": "release", "untracked.txt": "untracked",
		".forge/state.json": "control",
	} {
		absolute := filepath.Join(root, filepath.FromSlash(path))
		if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(absolute, []byte(value), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Symlink("tracked.txt", filepath.Join(root, "linked.txt")); err != nil {
		t.Fatal(err)
	}
	runFixtureGit(t, root, "init", "-q")
	runFixtureGit(t, root, "add", "tracked.txt", "linked.txt", "docs/release/plan.md")
	runFixtureGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	pathValue := filepath.Dir(gitPath)
	return root, []string{"LANG=C", "LC_ALL=C", "PATH=" + pathValue}
}

func runFixtureGit(t *testing.T, root string, args ...string) {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", root}, args...)...)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
}

func sourceEntriesByPath(entries []SourceEntry) map[string]SourceEntry {
	result := make(map[string]SourceEntry, len(entries))
	for _, entry := range entries {
		result[entry.Path] = entry
	}
	return result
}
