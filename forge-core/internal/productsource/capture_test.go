package productsource

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestCaptureExcludesControlAndReleaseState(t *testing.T) {
	root, environment := productSourceFixture(t)
	before, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if err := Validate(before.Manifest, before.SHA256); err != nil {
		t.Fatal(err)
	}
	assertProductPaths(t, before, []string{"product.txt", "untracked.txt"})
	writeProductFile(t, root, "docs/release/plan.md", "new release")
	writeProductFile(t, root, ".forge/state.json", "new control")
	afterExcluded, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if afterExcluded.SHA256 != before.SHA256 {
		t.Fatalf("excluded state changed product digest: %s != %s", afterExcluded.SHA256, before.SHA256)
	}
	writeProductFile(t, root, "product.txt", "changed product")
	afterProduct, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	if afterProduct.SHA256 == before.SHA256 {
		t.Fatal("product byte change did not change source digest")
	}
	if !SameCapturedRoot(before, afterProduct) {
		t.Fatal("stable repository root identity was lost")
	}
}

func TestCaptureRejectsTrackedForgeControlState(t *testing.T) {
	root, environment := productSourceFixture(t)
	runProductGit(t, root, "add", "-f", ".forge/state.json")
	runProductGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "track control")
	if _, err := Capture(context.Background(), root, environment); err == nil ||
		(!strings.Contains(err.Error(), "tracked Forge control path") &&
			!strings.Contains(err.Error(), "tracked or control source path")) {
		t.Fatalf("tracked control state error = %v", err)
	}
}

func TestCaptureRejectsPortableReleaseAlias(t *testing.T) {
	root, environment := productSourceFixture(t)
	writeProductFile(t, root, "docs/Release/alias.md", "alias")
	canonical, canonicalErr := os.Stat(filepath.Join(root, "docs/release"))
	alias, aliasErr := os.Stat(filepath.Join(root, "docs/Release"))
	if canonicalErr == nil && aliasErr == nil && os.SameFile(canonical, alias) {
		t.Skip("filesystem does not distinguish the portable case alias")
	}
	if _, err := Capture(context.Background(), root, environment); err == nil ||
		!strings.Contains(err.Error(), "portable alias") {
		t.Fatalf("release alias error = %v", err)
	}
}

func TestProductManifestRejectsExcludedOrTamperedProjection(t *testing.T) {
	root, environment := productSourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	tampered := CloneManifest(snapshot.Manifest)
	tampered.Entries[0].Path = "docs/release/forged.md"
	if _, err := CanonicalJSON(tampered); err == nil || !strings.Contains(err.Error(), "excluded path") {
		t.Fatalf("excluded manifest entry error = %v", err)
	}
	tampered = CloneManifest(snapshot.Manifest)
	tampered.Entries[0].Path = "docs/Release/forged.md"
	if _, err := CanonicalJSON(tampered); err == nil || !strings.Contains(err.Error(), "portable alias") {
		t.Fatalf("aliased manifest entry error = %v", err)
	}
	if err := Validate(snapshot.Manifest, strings.Repeat("0", 64)); err == nil {
		t.Fatal("wrong product source digest accepted")
	}
}

func TestReadRegularFilesUsesCapturedProductSnapshot(t *testing.T) {
	root, environment := productSourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	files, err := ReadRegularFiles(context.Background(), snapshot, []string{"product.txt"}, RegularReadLimits{
		MaxFiles: 4, MaxFileBytes: 1 << 20, MaxTotalBytes: 2 << 20, MaxPathDepth: 16,
	})
	if err != nil || len(files) != 1 || string(files[0].Content) != "product" {
		t.Fatalf("ReadRegularFiles = %#v, %v", files, err)
	}
	writeProductFile(t, root, "product.txt", "changed")
	if _, err := ReadRegularFiles(context.Background(), snapshot, []string{"product.txt"}, RegularReadLimits{
		MaxFiles: 4, MaxFileBytes: 1 << 20, MaxTotalBytes: 2 << 20, MaxPathDepth: 16,
	}); err == nil {
		t.Fatal("stale product snapshot read accepted changed bytes")
	}
}

func TestReadDeclaredFilesBindsMixedProductAndReleaseSet(t *testing.T) {
	root, environment := productSourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	limits := RegularReadLimits{
		MaxFiles: 4, MaxFileBytes: 1 << 20, MaxTotalBytes: 2 << 20, MaxPathDepth: 16,
	}
	paths := []string{"docs/release/plan.md", "product.txt"}
	files, err := ReadSingleLinkDeclaredFiles(context.Background(), snapshot, paths, limits)
	if err != nil {
		t.Fatal(err)
	}
	if len(files) != 2 || string(files[0].Content) != "release" ||
		string(files[1].Content) != "product" {
		t.Fatalf("declared files = %#v", files)
	}
	if _, err := ReadRegularFiles(
		context.Background(), snapshot, []string{"docs/release/plan.md"}, limits,
	); err == nil || !strings.Contains(err.Error(), "product source manifest") {
		t.Fatalf("product-only reader accepted release file: %v", err)
	}
	writeProductFile(t, root, "docs/release/plan.md", "changed release")
	if _, err := ReadSingleLinkDeclaredFiles(context.Background(), snapshot, paths, limits); err == nil {
		t.Fatal("declared reader accepted stale release bytes")
	}
}

func TestDeclaredFileSelectionRejectsControlAliasesAndInvalidSets(t *testing.T) {
	root, environment := productSourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	tests := [][]string{
		nil,
		{".forge/state.json"},
		{".Forge/state.json"},
		{"docs/Release/plan.md"},
		{"Docs/release/plan.md"},
		{"docs/release"},
		{"docs/release/../product.txt"},
		{"missing.txt"},
		{"product.txt", "docs/release/plan.md"},
	}
	for _, paths := range tests {
		if err := ValidateDeclaredRegularFileSet(snapshot, paths); err == nil {
			t.Fatalf("declared selection accepted %#v", paths)
		}
	}
}

func productSourceFixture(t *testing.T) (string, []string) {
	t.Helper()
	root := t.TempDir()
	writeProductFile(t, root, "product.txt", "product")
	writeProductFile(t, root, "docs/release/plan.md", "release")
	writeProductFile(t, root, ".forge/state.json", "control")
	writeProductFile(t, root, "untracked.txt", "untracked")
	runProductGit(t, root, "init", "-q")
	runProductGit(t, root, "add", "product.txt", "docs/release/plan.md")
	runProductGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	return root, []string{"PATH=" + filepath.Dir(git)}
}

func assertProductPaths(t *testing.T, snapshot Snapshot, want []string) {
	t.Helper()
	got := make([]string, len(snapshot.Manifest.Entries))
	for index, entry := range snapshot.Manifest.Entries {
		got[index] = entry.Path
	}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("product paths = %v, want %v", got, want)
	}
}

func writeProductFile(t *testing.T, root, path, value string) {
	t.Helper()
	absolute := filepath.Join(root, filepath.FromSlash(path))
	if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(absolute, []byte(value), 0o644); err != nil {
		t.Fatal(err)
	}
}

func runProductGit(t *testing.T, root string, args ...string) {
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
