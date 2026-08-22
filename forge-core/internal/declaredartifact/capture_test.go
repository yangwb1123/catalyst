package declaredartifact

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/productsource"
)

const emptyManifestJSON = `{"api_version":"forgeos.agent-output-artifact-manifest/v1","canonicalization":"forgeos.canonical-json/v1","items":[],"manifest_sha256":"34a0e966dfbeea1846f4863a373cefd79daa13dad00ce7b3a3f391807923fbff"}`

func TestCaptureCanonicalEmptyManifest(t *testing.T) {
	fixture := newDeclaredFixture(t)
	var prior outputbinding.ArtifactManifest
	for _, paths := range [][]string{nil, {}} {
		manifest, err := Capture(context.Background(), fixture.snapshot, paths)
		if err != nil {
			t.Fatal(err)
		}
		if manifest.Items == nil || len(manifest.Items) != 0 {
			t.Fatalf("empty items = %#v", manifest.Items)
		}
		if err := outputbinding.ValidateManifest(manifest); err != nil {
			t.Fatal(err)
		}
		encoded, err := outputbinding.CanonicalManifestJSON(manifest)
		if err != nil || string(encoded) != emptyManifestJSON {
			t.Fatalf("canonical empty manifest = %s, %v", encoded, err)
		}
		if prior.ManifestSHA256 != "" && manifest.ManifestSHA256 != prior.ManifestSHA256 {
			t.Fatalf("empty digest = %q, prior %q", manifest.ManifestSHA256, prior.ManifestSHA256)
		}
		prior = manifest
	}
}

func TestCaptureBindsExactBytesInDeclaredOrder(t *testing.T) {
	fixture := newDeclaredFixture(t)
	manifest, err := Capture(
		context.Background(), fixture.snapshot, []string{"a.bin", "z.txt"},
	)
	if err != nil {
		t.Fatal(err)
	}
	want := []outputbinding.ManifestItem{
		{Bytes: 4, Path: "a.bin", SHA256: "46a1fda4b60882b250ae85357e3b3e51fa7fe529c67f246b36eb16a702c7dfd6"},
		{Bytes: 4, Path: "z.txt", SHA256: "f71a59e61939400f3556358063bb57fc445c7165d6062754950867406462ca93"},
	}
	if len(manifest.Items) != len(want) {
		t.Fatalf("items = %#v", manifest.Items)
	}
	for index := range want {
		if manifest.Items[index] != want[index] {
			t.Fatalf("item %d = %#v, want %#v", index, manifest.Items[index], want[index])
		}
	}
}

func TestCaptureBindsMixedProductAndReleaseArtifacts(t *testing.T) {
	fixture := newDeclaredFixture(t)
	manifest, err := Capture(context.Background(), fixture.snapshot,
		[]string{"a.bin", "docs/release/plan.md", "z.txt"})
	if err != nil {
		t.Fatal(err)
	}
	wantContent := [][]byte{{0, 0xff, 'A', '\n'}, []byte("excluded"), []byte("zulu")}
	for index, path := range []string{"a.bin", "docs/release/plan.md", "z.txt"} {
		item := manifest.Items[index]
		if item.Path != path || item.Bytes != int64(len(wantContent[index])) ||
			item.SHA256 != outputbinding.SHA256(wantContent[index]) {
			t.Fatalf("mixed item %d = %#v", index, item)
		}
	}
}

func TestCaptureRejectsNonCanonicalOrNonSetDeclarations(t *testing.T) {
	fixture := newDeclaredFixture(t)
	tests := []struct {
		name  string
		paths []string
	}{
		{name: "unsorted", paths: []string{"z.txt", "a.bin"}},
		{name: "duplicate", paths: []string{"a.bin", "a.bin"}},
		{name: "invalid UTF-8", paths: []string{string([]byte{0xff})}},
		{name: "backslash", paths: []string{`dir\file`}},
		{name: "dot alias", paths: []string{"./a.bin"}},
		{name: "parent alias", paths: []string{"dir/../a.bin"}},
		{name: "parent prefix", paths: []string{"../a.bin"}},
		{name: "drive alias", paths: []string{"C:/a.bin"}},
		{name: "absolute", paths: []string{"/a.bin"}},
		{name: "empty path", paths: []string{""}},
		{name: "release case alias", paths: []string{"docs/Release/plan.md"}},
		{name: "release parent alias", paths: []string{"docs/release/../a.bin"}},
		{name: "control", paths: []string{".forge/state.json"}},
		{name: "control case alias", paths: []string{".Forge/state.json"}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := Capture(context.Background(), fixture.snapshot, test.paths); err == nil {
				t.Fatalf("Capture accepted %#v", test.paths)
			}
		})
	}
}

func TestCaptureRejectsUnavailableOrExcludedDeclaredFiles(t *testing.T) {
	fixture := newDeclaredFixture(t)
	paths := []string{
		"deleted.txt", "docs/release/linked.md", "docs/release/missing.md", "ignored.txt",
		"linked.txt", "missing.txt",
	}
	for _, path := range paths {
		t.Run(path, func(t *testing.T) {
			if _, err := Capture(context.Background(), fixture.snapshot, []string{path}); err == nil {
				t.Fatalf("Capture accepted unavailable path %q", path)
			}
		})
	}
}

func TestCaptureRejectsStaleReleaseSnapshotBytes(t *testing.T) {
	fixture := newDeclaredFixture(t)
	writeDeclaredFile(t, fixture.root, "docs/release/plan.md", []byte("replaced"))
	if _, err := Capture(
		context.Background(), fixture.snapshot, []string{"docs/release/plan.md"},
	); err == nil || !strings.Contains(err.Error(), "does not match source manifest") {
		t.Fatalf("stale release snapshot error = %v", err)
	}
}

func TestCaptureRejectsStaleSnapshotBytes(t *testing.T) {
	fixture := newDeclaredFixture(t)
	writeDeclaredFile(t, fixture.root, "a.bin", []byte{0, 0xff, 'B', '\n'})
	_, err := Capture(context.Background(), fixture.snapshot, []string{"a.bin"})
	if err == nil || !strings.Contains(err.Error(), "does not match source manifest") {
		t.Fatalf("stale snapshot error = %v", err)
	}
}

func TestCaptureRejectsHardLinkedDeclaredFiles(t *testing.T) {
	for _, artifact := range []string{"a.bin", "docs/release/plan.md"} {
		for _, linkBeforeSnapshot := range []bool{true, false} {
			when := map[bool]string{true: "capture", false: "stable-reread"}[linkBeforeSnapshot]
			t.Run(strings.ReplaceAll(artifact, "/", "-")+"-"+when, func(t *testing.T) {
				fixture := newDeclaredFixture(t)
				outside := filepath.Join(t.TempDir(), "outside-link")
				if err := os.Link(filepath.Join(fixture.root, filepath.FromSlash(artifact)), outside); err != nil {
					t.Skipf("hard links unavailable: %v", err)
				}
				if linkBeforeSnapshot {
					snapshot, err := productsource.Capture(
						context.Background(), fixture.root, declaredGitEnvironment(t),
					)
					if err != nil {
						t.Fatal(err)
					}
					fixture.snapshot = snapshot
				}
				if _, err := Capture(context.Background(), fixture.snapshot, []string{artifact}); err == nil || !strings.Contains(err.Error(), "single-link") {
					t.Fatalf("hard-linked artifact error = %v", err)
				}
			})
		}
	}
}

func TestDeclaredBoundsAreExactAndInclusive(t *testing.T) {
	fixture := newDeclaredFixture(t)
	wantLimits := productsource.RegularReadLimits{
		MaxFiles: 4_096, MaxFileBytes: 64 << 20,
		MaxTotalBytes: 512 << 20, MaxPathDepth: 128,
	}
	reader := func(
		_ context.Context, _ productsource.Snapshot, paths []string,
		limits productsource.RegularReadLimits,
	) ([]productsource.RegularFile, error) {
		if limits != wantLimits || paths == nil {
			t.Fatalf("reader limits = %#v, paths = %#v", limits, paths)
		}
		return []productsource.RegularFile{}, nil
	}
	if _, err := captureWithReader(context.Background(), fixture.snapshot, nil, reader); err != nil {
		t.Fatal(err)
	}
	assertPathBounds(t)
	assertByteBounds(t)
}

func TestCaptureFailsClosedOnReaderContractDrift(t *testing.T) {
	fixture := newDeclaredFixture(t)
	content := []byte{0, 0xff, 'A', '\n'}
	digest := outputbinding.SHA256(content)
	tests := []struct {
		name  string
		files []productsource.RegularFile
	}{
		{name: "missing result", files: []productsource.RegularFile{}},
		{name: "wrong path", files: []productsource.RegularFile{{Path: "b", Content: content, SHA256: digest}}},
		{name: "wrong digest", files: []productsource.RegularFile{{Path: "a.bin", Content: content, SHA256: strings.Repeat("0", 64)}}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			reader := func(context.Context, productsource.Snapshot, []string,
				productsource.RegularReadLimits) ([]productsource.RegularFile, error) {
				return test.files, nil
			}
			if _, err := captureWithReader(
				context.Background(), fixture.snapshot, []string{"a.bin"}, reader,
			); err == nil {
				t.Fatal("reader contract drift was accepted")
			}
		})
	}
}

func assertPathBounds(t *testing.T) {
	t.Helper()
	paths := make([]string, maxFiles+1)
	for index := range paths {
		paths[index] = fmt.Sprintf("f%04d", index)
	}
	if err := validateDeclaredPaths(paths[:maxFiles]); err != nil {
		t.Fatalf("maximum file count: %v", err)
	}
	if err := validateDeclaredPaths(paths); err == nil {
		t.Fatal("over-maximum file count was accepted")
	}
	depth128 := strings.Repeat("d/", maxPathDepth-1) + "f"
	if err := validateDeclaredPaths([]string{depth128}); err != nil {
		t.Fatalf("maximum path depth: %v", err)
	}
	depth129 := strings.Repeat("d/", maxPathDepth) + "f"
	if err := validateDeclaredPaths([]string{depth129}); err == nil {
		t.Fatal("over-maximum path depth was accepted")
	}
}

func assertByteBounds(t *testing.T) {
	t.Helper()
	total, err := addFileBytes(maxTotalBytes-maxFileBytes, maxFileBytes)
	if err != nil || total != maxTotalBytes {
		t.Fatalf("inclusive byte bounds = %d, %v", total, err)
	}
	if _, err := addFileBytes(0, maxFileBytes+1); err == nil {
		t.Fatal("over-maximum file bytes were accepted")
	}
	if _, err := addFileBytes(maxTotalBytes, 1); err == nil {
		t.Fatal("over-maximum total bytes were accepted")
	}
}

type declaredFixture struct {
	root     string
	snapshot productsource.Snapshot
}

func newDeclaredFixture(t *testing.T) declaredFixture {
	t.Helper()
	root := t.TempDir()
	writeDeclaredFile(t, root, ".gitignore", []byte("ignored.txt\n"))
	writeDeclaredFile(t, root, "a.bin", []byte{0, 0xff, 'A', '\n'})
	writeDeclaredFile(t, root, "deleted.txt", []byte("deleted"))
	writeDeclaredFile(t, root, "docs/release/plan.md", []byte("excluded"))
	writeDeclaredFile(t, root, "z.txt", []byte("zulu"))
	if err := os.Symlink("z.txt", filepath.Join(root, "linked.txt")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("plan.md", filepath.Join(root, "docs/release/linked.md")); err != nil {
		t.Fatal(err)
	}
	runDeclaredGit(t, root, "init", "-q")
	runDeclaredGit(t, root, "add", ".gitignore", "a.bin", "deleted.txt",
		"docs/release/linked.md", "docs/release/plan.md", "linked.txt", "z.txt")
	runDeclaredGit(t, root, "-c", "user.name=Fixture", "-c",
		"user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	if err := os.Remove(filepath.Join(root, "deleted.txt")); err != nil {
		t.Fatal(err)
	}
	writeDeclaredFile(t, root, "ignored.txt", []byte("ignored"))
	snapshot, err := productsource.Capture(context.Background(), root, declaredGitEnvironment(t))
	if err != nil {
		t.Fatal(err)
	}
	return declaredFixture{root: root, snapshot: snapshot}
}

func writeDeclaredFile(t *testing.T, root, path string, content []byte) {
	t.Helper()
	absolute := filepath.Join(root, filepath.FromSlash(path))
	if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(absolute, content, 0o644); err != nil {
		t.Fatal(err)
	}
}

func runDeclaredGit(t *testing.T, root string, args ...string) {
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

func declaredGitEnvironment(t *testing.T) []string {
	t.Helper()
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	return []string{"PATH=" + filepath.Dir(git)}
}
