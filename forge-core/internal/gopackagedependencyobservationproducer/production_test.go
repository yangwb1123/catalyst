package gopackagedependencyobservationproducer

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

func TestProduceCapturesLiveLexicalUnionWithoutWorkingTreeEffects(t *testing.T) {
	root, environment := producerFixture(t)
	before := producerGitOutput(t, root, "status", "--porcelain=v1", "-z")
	production, err := produceWith(
		context.Background(), root, ".", "run-live", environment,
		func() time.Time { return time.UnixMilli(1_700_000_000_123) },
		gitworktreesource.Capture, gitworktreesource.ReadRegularFiles,
	)
	if err != nil {
		t.Fatal(err)
	}
	after := producerGitOutput(t, root, "status", "--porcelain=v1", "-z")
	if string(before) != string(after) {
		t.Fatalf("working tree changed:\nbefore=%q\nafter=%q", before, after)
	}
	value := production.Package()
	assertLiveGraph(t, value)
	decoded, err := DecodeProduction(production.ProductionJSON())
	if err != nil || decoded.SHA256() != production.SHA256() {
		t.Fatalf("live decode = %v", err)
	}
	value.GraphObservation.Files[0].Imports = append(value.GraphObservation.Files[0].Imports, "mutated")
	if strings.Contains(string(production.GraphObservationJSON()), "mutated") {
		t.Fatal("Package returned shared graph slices")
	}
}

func assertLiveGraph(t *testing.T, value ProductionPackage) {
	t.Helper()
	graph := value.GraphObservation
	resolutions := make([]string, 0, len(graph.Dependencies))
	for _, edge := range graph.Dependencies {
		resolutions = append(resolutions, edge.Resolution)
	}
	sort.Strings(resolutions)
	for _, resolution := range []string{
		"ambiguous_local", "cgo_pseudo", "external_candidate", "local",
		"nested_module_boundary", "stdlib_candidate", "unresolved_local", "unsupported",
	} {
		if !containsString(resolutions, resolution) {
			t.Fatalf("resolution %q absent from %#v", resolution, resolutions)
		}
	}
	if len(graph.Module.NestedModules) != 2 || graph.Coverage.GoEntriesExcludedNonregular != 1 {
		t.Fatalf("nested/coverage = %#v / %#v", graph.Module.NestedModules, graph.Coverage)
	}
	pkg := findPackage(graph.Packages, "p", "p_test")
	if pkg == nil || pkg.ImportPath != nil {
		t.Fatalf("test-only package = %#v", pkg)
	}
	if !containsFile(graph.Files, "p/p_linux.go") || !containsDiagnostic(graph.Diagnostics, "broken.go") {
		t.Fatalf("build-tag union or diagnostic missing: %#v %#v", graph.Files, graph.Diagnostics)
	}
	if findPackage(graph.Packages, "deleted", "deleted") == nil {
		t.Fatal("deleted nested go.mod incorrectly excluded its Go package")
	}
}

func producerFixture(t *testing.T) (string, []string) {
	t.Helper()
	parent := t.TempDir()
	root := filepath.Join(parent, "repo")
	if err := os.Mkdir(root, 0o755); err != nil {
		t.Fatal(err)
	}
	files := producerFixtureFiles()
	for path, content := range files {
		writeProducerFile(t, root, path, content)
	}
	if err := os.Symlink("main.go", filepath.Join(root, "linked.go")); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "linkedmod"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("../go.mod", filepath.Join(root, "linkedmod", "go.mod")); err != nil {
		t.Fatal(err)
	}
	runProducerGit(t, root, "init", "-q")
	runProducerGit(t, root, "add", ".")
	runProducerGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	if err := os.Remove(filepath.Join(root, "deleted", "go.mod")); err != nil {
		t.Fatal(err)
	}
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	return root, []string{"PATH=" + filepath.Dir(gitPath)}
}

func producerFixtureFiles() map[string]string {
	return map[string]string{
		"go.mod":             "module example.com/live\n",
		"main.go":            "package main\nimport (\n\"./bad\"\n\"C\"\n\"example.com/live/missing\"\n\"example.com/live/multi\"\n\"example.com/live/p\"\n\"example.com/live/nested/x\"\n\"fmt\"\n\"github.com/lib/x\"\n)\n",
		"broken.go":          "package broken\nimport (\n\"fmt\"\n",
		"p/p.go":             "package p\n",
		"p/p_linux.go":       "//go:build never\n\npackage p\nimport \"os\"\n",
		"p/p_test.go":        "package p\nimport \"testing\"\n",
		"p/external_test.go": "package p_test\nimport \"example.com/live/p\"\n",
		"multi/a.go":         "package a\n", "multi/b.go": "package b\n",
		"nested/go.mod": "module example.com/nested\n", "nested/x/x.go": "package x\n",
		"linkedmod/x.go": "package linkedmod\n",
		"deleted/go.mod": "module example.com/deleted\n", "deleted/keep.go": "package deleted\n",
	}
}

func writeProducerFile(t *testing.T, root, path, content string) {
	t.Helper()
	absolute := filepath.Join(root, filepath.FromSlash(path))
	if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(absolute, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func runProducerGit(t *testing.T, root string, args ...string) {
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

func producerGitOutput(t *testing.T, root string, args ...string) []byte {
	t.Helper()
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	command := exec.Command(gitPath, append([]string{"-C", root}, args...)...)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	output, err := command.Output()
	if err != nil {
		t.Fatal(err)
	}
	return output
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func findPackage(values []gopackagegraph.Package, directory, name string) *gopackagegraph.Package {
	for index := range values {
		if values[index].Directory == directory && values[index].Name == name {
			return &values[index]
		}
	}
	return nil
}

func containsFile(values []gopackagegraph.File, path string) bool {
	for _, value := range values {
		if value.Path == path {
			return true
		}
	}
	return false
}

func containsDiagnostic(values []gopackagegraph.Diagnostic, path string) bool {
	for _, value := range values {
		if value.Path == path {
			return true
		}
	}
	return false
}
