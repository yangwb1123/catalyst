package gitworktreesource

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestCaptureBindsTrackedUntrackedReleaseSymlinkAndDeletion(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.Root != root || snapshot.Manifest.APIVersion != APIVersion ||
		snapshot.Manifest.Canonicalization != Canonicalization || snapshot.Manifest.ProfileID != ProfileID {
		t.Fatalf("snapshot fixed fields = %#v", snapshot)
	}
	byPath := entriesByPath(snapshot.Manifest.Entries)
	for _, path := range []string{"docs/release/plan.md", "linked.txt", "tracked.txt", "untracked.txt"} {
		if byPath[path].Path == "" {
			t.Fatalf("source manifest omitted %q: %#v", path, snapshot.Manifest.Entries)
		}
	}
	if _, exists := byPath[".forge/state.json"]; exists {
		t.Fatal("untracked .forge control state must be excluded")
	}
	if byPath["linked.txt"].Kind != "symlink" || byPath["tracked.txt"].Tracking != "tracked" ||
		byPath["untracked.txt"].Tracking != "untracked" ||
		!strings.HasPrefix(snapshot.Manifest.SourceRevision, "git-sha1:") {
		t.Fatalf("source entry semantics drifted: %#v", snapshot.Manifest)
	}
	if err := Validate(snapshot.Manifest, snapshot.SHA256); err != nil {
		t.Fatalf("captured source did not self-validate: %v", err)
	}
	if err := os.Remove(filepath.Join(root, "tracked.txt")); err != nil {
		t.Fatal(err)
	}
	changed, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if changed.SHA256 == snapshot.SHA256 || entriesByPath(changed.Manifest.Entries)["tracked.txt"].Kind != "deleted" {
		t.Fatalf("tracked deletion not identity-bearing: %#v", changed)
	}
}

func TestCaptureRejectsParentSymlinkAndAllowsLeafSymlinkTarget(t *testing.T) {
	t.Run("parent", func(t *testing.T) {
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
		if _, err := Capture(context.Background(), root, environment); err == nil ||
			!strings.Contains(err.Error(), "forbidden symlink parent") {
			t.Fatalf("outside symlink parent error = %v", err)
		}
	})
	t.Run("leaf", func(t *testing.T) {
		root, environment := sourceFixture(t)
		outside := filepath.Join(t.TempDir(), "outside.txt")
		if err := os.WriteFile(outside, []byte("outside"), 0o644); err != nil {
			t.Fatal(err)
		}
		link := filepath.Join(root, "docs", "release", "outside-link.txt")
		if err := os.Symlink(outside, link); err != nil {
			t.Fatal(err)
		}
		runFixtureGit(t, root, "add", "docs/release/outside-link.txt")
		snapshot, err := Capture(context.Background(), root, environment)
		if err != nil {
			t.Fatal(err)
		}
		entry := entriesByPath(snapshot.Manifest.Entries)["docs/release/outside-link.txt"]
		if entry.Kind != "symlink" || entry.SymlinkTarget == nil || *entry.SymlinkTarget != outside ||
			entry.ContentSHA256 == nil || *entry.ContentSHA256 != sha256Bytes([]byte(outside)) {
			t.Fatalf("outside-target leaf symlink = %#v", entry)
		}
	})
}

func TestCaptureEnvironmentRejectsAmbiguousPathAndDropsTMPDIR(t *testing.T) {
	root, environment := sourceFixture(t)
	pathValue, _ := exactPathValue(environment)
	for _, test := range []struct {
		name string
		env  []string
	}{
		{name: "missing", env: []string{"LANG=C"}},
		{name: "duplicate", env: []string{"PATH=" + pathValue, "PATH=" + pathValue}},
	} {
		t.Run(test.name, func(t *testing.T) {
			if _, err := Capture(context.Background(), root, test.env); err == nil ||
				!strings.Contains(err.Error(), "exactly one PATH") {
				t.Fatalf("ambiguous PATH error = %v", err)
			}
		})
	}
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	wrapper := filepath.Join(bin, "git")
	script := "#!/bin/sh\nif env | grep '^TMPDIR=' >/dev/null; then exit 97; fi\nexec " + realGit + " \"$@\"\n"
	if err := os.WriteFile(wrapper, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	environment = []string{
		"PATH=" + strings.Join([]string{bin, filepath.Dir(realGit), "/usr/bin", "/bin"}, string(filepath.ListSeparator)),
		"TMPDIR=relative/attacker-controlled", "UNUSED=value",
	}
	if _, err := Capture(context.Background(), root, environment); err != nil {
		t.Fatalf("unused TMPDIR reached hardened Git child: %v", err)
	}
}

func TestCanonicalDigestValidationAndCloneAreExact(t *testing.T) {
	target := "target-<>&"
	digest := sha256Bytes([]byte(target))
	indexMode := "120000"
	executable := false
	manifest := SourceManifest{
		APIVersion: APIVersion, Canonicalization: Canonicalization, ProfileID: ProfileID,
		SourceRevision: "git-sha1:" + strings.Repeat("a", 40),
		Entries: []SourceEntry{{
			Bytes: int64(len(target)), ContentSHA256: &digest, Executable: &executable,
			IndexMode: &indexMode, Kind: "symlink", Path: "linked.txt",
			SymlinkTarget: &target, Tracking: "tracked",
		}},
	}
	canonical, err := CanonicalManifestJSON(manifest)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(canonical), target) || strings.Contains(string(canonical), `\u003c`) {
		t.Fatalf("canonical source JSON escaped HTML-sensitive text: %s", canonical)
	}
	identity, err := Digest(manifest)
	if err != nil {
		t.Fatal(err)
	}
	if err := Validate(manifest, identity); err != nil {
		t.Fatal(err)
	}
	copyManifest := CloneManifest(manifest)
	*copyManifest.Entries[0].SymlinkTarget = "mutated"
	if *manifest.Entries[0].SymlinkTarget != target {
		t.Fatal("CloneManifest shared nested pointers")
	}
	manifest.Entries[0].Path = ".forge/state"
	if _, err := CanonicalManifestJSON(manifest); err == nil {
		t.Fatal("strict canonicalization accepted protected source path")
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
	return root, []string{"LANG=C", "LC_ALL=C", "PATH=" + filepath.Dir(gitPath)}
}

func runFixtureGit(t *testing.T, root string, args ...string) {
	t.Helper()
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	command := exec.Command(gitPath, append([]string{"-C", root}, args...)...)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
}

func entriesByPath(entries []SourceEntry) map[string]SourceEntry {
	result := make(map[string]SourceEntry, len(entries))
	for _, entry := range entries {
		result[entry.Path] = entry
	}
	return result
}
