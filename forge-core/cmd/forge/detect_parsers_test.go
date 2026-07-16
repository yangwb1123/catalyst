// detect_parsers_test.go — tests for detect_parsers.go (semantic parsers,
// language/test/CI detection, suggestWorkflow, buildSuggestionReason).
package main

import (
	"path/filepath"
	"testing"
)

// ── Semantic parser tests (v1.5) ──────────────────────────────────────────

func TestParseGoMod_ModuleAndVersion(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"),
		"module github.com/example/my-project\n\ngo 1.22\n")
	modPath, goVer, ind := parseGoMod(root, nil)
	if modPath != "github.com/example/my-project" {
		t.Errorf("module path = %q, want github.com/example/my-project", modPath)
	}
	if goVer != "1.22" {
		t.Errorf("go version = %q, want 1.22", goVer)
	}
	if len(ind) != 2 {
		t.Errorf("indicators = %v, want 2 entries", ind)
	}
}

func TestParseGoMod_MissingFile(t *testing.T) {
	root := t.TempDir() // no go.mod
	modPath, goVer, ind := parseGoMod(root, nil)
	if modPath != "" || goVer != "" {
		t.Errorf("expected empty fields for missing go.mod, got mod=%q ver=%q", modPath, goVer)
	}
	if len(ind) != 0 {
		t.Errorf("expected empty indicators for missing go.mod, got %v", ind)
	}
}

func TestParseGoMod_NoModuleDirective(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"),
		"// just a comment, no module line\n")
	modPath, goVer, _ := parseGoMod(root, nil)
	if modPath != "" || goVer != "" {
		t.Errorf("expected empty fields when no directives, got mod=%q ver=%q", modPath, goVer)
	}
}

func TestParseGoMod_ModuleWithComment(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"),
		"module github.com/example/foo // this is the module\n")
	modPath, _, _ := parseGoMod(root, nil)
	if modPath != "github.com/example/foo" {
		t.Errorf("module path = %q, want github.com/example/foo (trailing comment stripped)", modPath)
	}
}

func TestParsePackageJSON_ScriptsAndDeps(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"),
		`{"scripts":{"build":"tsc","test":"vitest"},"dependencies":{"express":"4.0"},"devDependencies":{"typescript":"5.0"}}`)
	buildSc, testSc, deps, ind := parsePackageJSON(root, nil)
	if !buildSc {
		t.Error("hasBuildScript should be true")
	}
	if !testSc {
		t.Error("hasTestScript should be true")
	}
	if deps != 2 {
		t.Errorf("deps = %d, want 2", deps)
	}
	if len(ind) != 1 {
		t.Errorf("indicators = %v, want 1 entry", ind)
	}
}

func TestParsePackageJSON_MissingFile(t *testing.T) {
	root := t.TempDir()
	buildSc, testSc, deps, ind := parsePackageJSON(root, nil)
	if buildSc || testSc || deps != 0 {
		t.Errorf("expected zero values for missing package.json, got build=%v test=%v deps=%d", buildSc, testSc, deps)
	}
	if len(ind) != 0 {
		t.Errorf("expected empty indicators for missing package.json")
	}
}

func TestParsePackageJSON_Corrupt(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"), "this is not json")
	buildSc, testSc, deps, ind := parsePackageJSON(root, nil)
	if buildSc || testSc || deps != 0 {
		t.Errorf("expected zero values for corrupt package.json")
	}
	if len(ind) != 1 {
		t.Errorf("expected 1 indicator (parse error), got %v", ind)
	}
}

func TestParsePackageJSON_NoScripts(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"), `{"name":"foo"}`)
	buildSc, testSc, deps, ind := parsePackageJSON(root, nil)
	if buildSc || testSc || deps != 0 {
		t.Errorf("expected zero values for minimal package.json")
	}
	if len(ind) != 1 {
		t.Errorf("expected 1 indicator, got %v", ind)
	}
}

func TestParsePyprojectToml_BuildBackendAndPython(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "pyproject.toml"),
		`[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "foo"
requires-python = ">=3.11"
`)
	backend, pyVer, ind := parsePyprojectToml(root, nil)
	if backend != "hatchling.build" {
		t.Errorf("build-backend = %q, want hatchling.build", backend)
	}
	if pyVer != ">=3.11" {
		t.Errorf("requires-python = %q, want >=3.11", pyVer)
	}
	if len(ind) != 2 {
		t.Errorf("indicators = %v, want 2 entries", ind)
	}
}

func TestParsePyprojectToml_MissingFile(t *testing.T) {
	root := t.TempDir()
	backend, pyVer, ind := parsePyprojectToml(root, nil)
	if backend != "" || pyVer != "" {
		t.Errorf("expected empty for missing pyproject.toml")
	}
	if len(ind) != 0 {
		t.Errorf("expected 0 indicators for missing file")
	}
}

func TestParsePyprojectToml_NoBuildSystemSection(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "pyproject.toml"),
		`[tool.black]
line-length = 88
`)
	backend, pyVer, ind := parsePyprojectToml(root, nil)
	if backend != "" || pyVer != "" {
		t.Errorf("expected empty when no build-system/project sections")
	}
	if len(ind) != 0 {
		t.Errorf("expected 0 indicators for unrelated sections, got %v", ind)
	}
}

func TestParsePyprojectToml_SetuptoolsBackend(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "pyproject.toml"),
		`[build-system]
requires = ["setuptools", "wheel"]
build-backend = "setuptools.build_meta"
`)
	backend, _, _ := parsePyprojectToml(root, nil)
	if backend != "setuptools.build_meta" {
		t.Errorf("build-backend = %q, want setuptools.build_meta", backend)
	}
}

// ── Integration: enriched profile via detectProject ───────────────────────
//
// Cargo.toml parser tests (parseCargoToml)

func TestParseCargoToml_NameAndEdition(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"[package]\nname = \"my-crate\"\nedition = \"2021\"\n")
	crate, edition, ind := parseCargoToml(root, nil)
	if crate != "my-crate" {
		t.Errorf("crate name = %q, want my-crate", crate)
	}
	if edition != "2021" {
		t.Errorf("edition = %q, want 2021", edition)
	}
	if len(ind) != 2 {
		t.Errorf("indicators = %v, want 2 entries", ind)
	}
}

func TestParseCargoToml_MissingFile(t *testing.T) {
	root := t.TempDir()
	crate, edition, ind := parseCargoToml(root, nil)
	if crate != "" || edition != "" {
		t.Errorf("expected empty for missing Cargo.toml, got crate=%q edition=%q", crate, edition)
	}
	if len(ind) != 0 {
		t.Errorf("expected 0 indicators for missing file, got %v", ind)
	}
}

func TestParseCargoToml_MinimalPackage(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"[package]\nname = \"tiny\"\n")
	crate, edition, ind := parseCargoToml(root, nil)
	if crate != "tiny" {
		t.Errorf("crate name = %q, want tiny", crate)
	}
	if edition != "" {
		t.Errorf("edition should be empty for minimal config, got %q", edition)
	}
	if len(ind) != 1 {
		t.Errorf("indicators = %v, want 1 entry", ind)
	}
}

func TestParseCargoToml_UnknownSectionsIgnored(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"[package]\nname = \"foo\"\nedition = \"2021\"\n\n[dependencies]\nserde = \"1.0\"\n\n[workspace]\nmembers = [\"bar\"]\n")
	crate, edition, _ := parseCargoToml(root, nil)
	if crate != "foo" {
		t.Errorf("crate name = %q, want foo", crate)
	}
	if edition != "2021" {
		t.Errorf("edition = %q, want 2021", edition)
	}
}

func TestParseCargoToml_CommentAndBlankLineTolerance(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"# This is a comment\n\n[package]\nname = \"my-app\"\n# edition comment\nedition = \"2018\"\n")
	crate, edition, ind := parseCargoToml(root, nil)
	if crate != "my-app" {
		t.Errorf("crate name = %q, want my-app", crate)
	}
	if edition != "2018" {
		t.Errorf("edition = %q, want 2018", edition)
	}
	if len(ind) != 2 {
		t.Errorf("indicators = %v, want 2 entries", ind)
	}
}

// ── Integration: enriched profile via detectProject ───────────────────────

func TestDetectProject_GoSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"),
		"module github.com/myorg/myrepo\n\ngo 1.23\n")
	writeFileAt(t, filepath.Join(root, "main_test.go"), "package main\n")
	p := detectProject(root)
	if p.Language != "go" {
		t.Errorf("language = %q, want go", p.Language)
	}
	if p.GoModulePath != "github.com/myorg/myrepo" {
		t.Errorf("GoModulePath = %q, want github.com/myorg/myrepo", p.GoModulePath)
	}
	if p.GoVersion != "1.23" {
		t.Errorf("GoVersion = %q, want 1.23", p.GoVersion)
	}
	if !p.HasTests {
		t.Error("HasTests should be true")
	}
}

func TestDetectProject_NodeSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"),
		`{"scripts":{"test":"jest"},"dependencies":{"express":"4"}}`)
	writeFileAt(t, filepath.Join(root, "index.js"), "")
	p := detectProject(root)
	if p.Language != "node" {
		t.Errorf("language = %q, want node", p.Language)
	}
	if !p.HasTestScript {
		t.Error("HasTestScript should be true")
	}
	if p.HasBuildScript {
		t.Error("HasBuildScript should be false (only test script)")
	}
	if p.DepsCount != 1 {
		t.Errorf("DepsCount = %d, want 1", p.DepsCount)
	}
}

func TestDetectProject_PythonSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "pyproject.toml"),
		`[build-system]
build-backend = "pdm.build"
[project]
requires-python = ">=3.12"
`)
	writeFileAt(t, filepath.Join(root, "test_foo.py"), "")
	p := detectProject(root)
	if p.Language != "python" {
		t.Errorf("language = %q, want python", p.Language)
	}
	if p.BuildBackend != "pdm.build" {
		t.Errorf("BuildBackend = %q, want pdm.build", p.BuildBackend)
	}
	if p.PythonVersion != ">=3.12" {
		t.Errorf("PythonVersion = %q, want >=3.12", p.PythonVersion)
	}
	if !p.HasTests {
		t.Error("HasTests should be true")
	}
}

func TestDetectProject_RustWithSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"[package]\nname = \"my-crate\"\nedition = \"2021\"\n")
	// Add a test file to exercise test detection for rust
	writeFileAt(t, filepath.Join(root, "tests", "integration_test.rs"), "")
	p := detectProject(root)
	if p.Language != "rust" {
		t.Errorf("language = %q, want rust", p.Language)
	}
	if p.CrateName != "my-crate" {
		t.Errorf("CrateName = %q, want my-crate", p.CrateName)
	}
	if p.RustEdition != "2021" {
		t.Errorf("RustEdition = %q, want 2021", p.RustEdition)
	}
	if !p.HasTests {
		t.Error("HasTests should be true (tests/integration_test.rs)")
	}
}

// TestDetectProject_RustTestsDirWithoutTestSuffix regression-tests dirHasGlob's
// "tests/*.rs" pattern: idiomatic cargo tests have no "_test" suffix (unlike
// the sibling test above, whose name also happens to satisfy *_test.rs and so
// masked this bug) — basename-only matching can never satisfy a pattern with
// a "/" segment, so this must match against the path relative to root.
func TestDetectProject_RustTestsDirWithoutTestSuffix(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "Cargo.toml"),
		"[package]\nname = \"my-crate\"\nedition = \"2021\"\n")
	writeFileAt(t, filepath.Join(root, "tests", "cli.rs"), "#[test]\nfn it_works() {}\n")
	p := detectProject(root)
	if !p.HasTests {
		t.Error("HasTests should be true (tests/cli.rs matches the tests/*.rs glob)")
	}
}

func TestDetectProject_CorruptGoModDoesNotCrash(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), string([]byte{0xff, 0xfe})) // invalid UTF-8
	p := detectProject(root)
	if p.Language != "go" {
		t.Errorf("language = %q, want go (detected from existence)", p.Language)
	}
	if p.GoModulePath != "" {
		t.Errorf("expected empty GoModulePath for corrupt go.mod, got %q", p.GoModulePath)
	}
}

// ── buildSuggestionReason tests ───────────────────────────────────────────

func TestBuildSuggestionReason_GoProject(t *testing.T) {
	p := projectProfile{
		Language: "go", GoModulePath: "github.com/foo/bar",
		GoVersion: "1.22", HasTests: true, HasCI: true,
	}
	r := buildSuggestionReason(p)
	if r != "iterative improvement loop (go | mod=github.com/foo/bar | tests-found | ci)" {
		t.Errorf("reason = %q", r)
	}
}

func TestBuildSuggestionReason_NodeWithScripts(t *testing.T) {
	p := projectProfile{
		Language: "node", HasBuildScript: true, HasTestScript: true,
		DepsCount: 12, HasTests: true, HasCI: false,
	}
	r := buildSuggestionReason(p)
	if r != "iterative improvement loop (node | test-script | 12-deps | tests-found | no-ci)" {
		t.Errorf("reason = %q", r)
	}
}

func TestBuildSuggestionReason_PythonWithBackend(t *testing.T) {
	p := projectProfile{
		Language: "python", BuildBackend: "hatchling.build",
		HasTests: false, HasCI: false,
	}
	r := buildSuggestionReason(p)
	if r != "iterative improvement loop (python | backend=hatchling.build | no-tests | no-ci)" {
		t.Errorf("reason = %q", r)
	}
}

func TestBuildSuggestionReason_UnknownProject(t *testing.T) {
	p := projectProfile{}
	r := buildSuggestionReason(p)
	if r != "iterative improvement loop (unknown | no-tests | no-ci)" {
		t.Errorf("reason = %q", r)
	}
}

func TestBuildSuggestionReason_HasBuildButNotTest(t *testing.T) {
	p := projectProfile{
		Language: "node", HasBuildScript: true, HasTests: false,
	}
	r := buildSuggestionReason(p)
	if r != "iterative improvement loop (node | build-script | no-tests | no-ci)" {
		t.Errorf("reason = %q", r)
	}
}

func TestShortenModule(t *testing.T) {
	tests := []struct{ in, want string }{
		{"github.com/foo/bar", "github.com/foo/bar"},
		{"github.com/foo/bar/v2", "github.com/foo/bar"},
		{"github.com/a/b/c/d", "github.com/a/b"},
		{"example.com/foo", "example.com/foo"},
	}
	for _, tc := range tests {
		got := shortenModule(tc.in)
		if got != tc.want {
			t.Errorf("shortenModule(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}
