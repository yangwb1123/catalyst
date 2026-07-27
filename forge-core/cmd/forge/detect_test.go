// detect_test.go — tests for cmdDetect, cmdDetectJSON, autoSelectWorkflow.
// Parser and integration tests live in detect_parsers_test.go.
package main

import (
	"flag"
	"os"
	"path/filepath"
	"testing"
)

// writeFileAt creates a file with optional content at path, creating parent dirs.
func writeFileAt(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestDetectProject_GoLang(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	p := detectProject(root)
	if p.Language != "go" {
		t.Errorf("language = %q, want go", p.Language)
	}
}

func TestDetectProject_NodeLang(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"), "{}")
	p := detectProject(root)
	if p.Language != "node" {
		t.Errorf("language = %q, want node", p.Language)
	}
}

func TestDetectProject_PythonLang(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "requirements.txt"), "flask\n")
	p := detectProject(root)
	if p.Language != "python" {
		t.Errorf("language = %q, want python", p.Language)
	}
}

func TestDetectProject_UnknownLang(t *testing.T) {
	root := t.TempDir() // empty dir
	p := detectProject(root)
	if p.Language != "unknown" {
		t.Errorf("language = %q, want unknown (no manifest)", p.Language)
	}
}

func TestDetectProject_HasTests_Go(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	writeFileAt(t, filepath.Join(root, "foo_test.go"), "package foo\n")
	p := detectProject(root)
	if !p.HasTests {
		t.Error("HasTests must be true when *_test.go exists")
	}
}

func TestDetectProject_NoTests(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	writeFileAt(t, filepath.Join(root, "main.go"), "package main\n")
	p := detectProject(root)
	if p.HasTests {
		t.Error("HasTests must be false when no *_test.go exists")
	}
}

func TestDetectProject_HasCI_GitHub(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	if err := os.MkdirAll(filepath.Join(root, ".github", "workflows"), 0o755); err != nil {
		t.Fatal(err)
	}
	p := detectProject(root)
	if !p.HasCI {
		t.Error("HasCI must be true when .github/workflows exists")
	}
}

func TestDetectProject_ProjectYML_OverridesDefaults(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, ".agent", "project.yml"),
		"mode: engineering\nlifecycle: production\n")
	p := detectProject(root)
	if p.Mode != "engineering" {
		t.Errorf("Mode = %q, want engineering (from project.yml)", p.Mode)
	}
	if p.Lifecycle != "production" {
		t.Errorf("Lifecycle = %q, want production (from project.yml)", p.Lifecycle)
	}
}

func TestSuggestWorkflow_UnknownNoTests_ReturnsDiscover(t *testing.T) {
	p := projectProfile{Language: "unknown", HasTests: false, Lifecycle: "mvp", Mode: "balanced"}
	s := suggestWorkflow(p)
	if s.Workflow != "discover" {
		t.Errorf("workflow = %q, want discover (greenfield hint)", s.Workflow)
	}
}

func TestSuggestWorkflow_GoWithTests_ReturnsEvolve(t *testing.T) {
	p := projectProfile{Language: "go", HasTests: true, Lifecycle: "mvp", Mode: "balanced"}
	s := suggestWorkflow(p)
	if s.Workflow != "evolve" {
		t.Errorf("workflow = %q, want evolve", s.Workflow)
	}
}

func TestSuggestWorkflow_PreservesMode(t *testing.T) {
	p := projectProfile{Language: "go", HasTests: true, Lifecycle: "production", Mode: "engineering"}
	s := suggestWorkflow(p)
	if s.Mode != "engineering" || s.Lifecycle != "production" {
		t.Errorf("mode=%q lifecycle=%q, want engineering/production (preserved from project.yml)", s.Mode, s.Lifecycle)
	}
}

func TestAutoSelectWorkflow_GoProject_ReturnsEvolve(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	writeFileAt(t, filepath.Join(root, "main_test.go"), "package main\n")
	fs := flag.NewFlagSet("evolve", flag.ContinueOnError)
	var o runOpts
	bindRunOpts(fs, &o)
	_ = fs.Parse(nil) // no flags set — auto should fill mode/lifecycle

	name := autoSelectWorkflow(root, fs, &o)
	if name != "evolve" {
		t.Errorf("autoSelectWorkflow = %q, want evolve for go project with tests", name)
	}
	if o.mode == "" {
		t.Error("autoSelectWorkflow must set o.mode when --mode not explicitly given")
	}
}

func TestAutoSelectWorkflow_ExplicitModePreserved(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"), "module example.com/foo\n")
	fs := flag.NewFlagSet("evolve", flag.ContinueOnError)
	var o runOpts
	bindRunOpts(fs, &o)
	_ = fs.Parse([]string{"--mode", "engineering"}) // explicit mode

	_ = autoSelectWorkflow(root, fs, &o)
	if o.mode != "engineering" {
		t.Errorf("explicit --mode must not be overridden; got %q", o.mode)
	}
}

func TestEvolveAutoRoutesGreenfieldThroughRunSemantics(t *testing.T) {
	root := greenfieldDiscoverRepo(t)
	if code := cmdEvolve([]string{"auto", "--root", root}); code != 0 {
		t.Fatalf("forge evolve auto greenfield exit=%d, want one-shot success", code)
	}
	if _, err := os.Stat(checkpointPath(root)); !os.IsNotExist(err) {
		t.Fatalf("one-shot auto route must not create evolve checkpoint: %v", err)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); err != nil {
		t.Fatalf("one-shot auto route must use forge run resources: %v", err)
	}
}

func TestEvolveAutoRejectsExplicitLoopFlagsForGreenfield(t *testing.T) {
	root := greenfieldDiscoverRepo(t)
	if code := cmdEvolve([]string{
		"auto", "--root", root, "--max-iter", "2",
	}); code != 2 {
		t.Fatalf("greenfield auto + explicit --max-iter exit=%d, want usage transfer", code)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("rejected auto transfer created run state: %v", err)
	}
}

// ── JSON output tests ─────────────────────────────────────────────────────

func TestCmdDetectJSON_GoProject(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "go.mod"),
		"module github.com/myorg/myproject\n\ngo 1.23\n")
	writeFileAt(t, filepath.Join(root, "main_test.go"), "package main\n")
	writeFileAt(t, filepath.Join(root, ".github", "workflows", "ci.yml"), "")
	p := detectProject(root)
	s := suggestWorkflow(p)

	exitCode := cmdDetectJSON(p, s)
	if exitCode != 0 {
		t.Fatalf("cmdDetectJSON returned exit code %d, want 0", exitCode)
	}
	if p.Language != "go" {
		t.Errorf("Language = %q, want go", p.Language)
	}
	if p.GoModulePath != "github.com/myorg/myproject" {
		t.Errorf("GoModulePath = %q, want github.com/myorg/myproject", p.GoModulePath)
	}
	if p.GoVersion != "1.23" {
		t.Errorf("GoVersion = %q, want 1.23", p.GoVersion)
	}
	if !p.HasTests {
		t.Error("HasTests should be true")
	}
	if !p.HasCI {
		t.Error("HasCI should be true")
	}
	if s.Workflow != "evolve" {
		t.Errorf("workflow = %q, want evolve", s.Workflow)
	}
}

func TestCmdDetectJSON_EmptyProject(t *testing.T) {
	root := t.TempDir()
	p := detectProject(root)
	s := suggestWorkflow(p)

	exitCode := cmdDetectJSON(p, s)
	if exitCode != 0 {
		t.Fatalf("cmdDetectJSON returned exit code %d, want 0", exitCode)
	}
	if p.Language != "unknown" {
		t.Errorf("Language = %q, want unknown", p.Language)
	}
	if s.Workflow != "discover" {
		t.Errorf("workflow = %q, want discover (no manifest, no tests)", s.Workflow)
	}
	if p.GoModulePath != "" || p.GoVersion != "" {
		t.Error("expected empty semantic fields for unknown project")
	}
	if p.HasBuildScript || p.HasTestScript || p.DepsCount != 0 {
		t.Error("expected zero-valued Node fields for unknown project")
	}
	if p.BuildBackend != "" || p.PythonVersion != "" {
		t.Error("expected empty Python fields for unknown project")
	}
}
