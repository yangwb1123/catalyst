package main

import (
	"flag"
	"os"
	"path/filepath"
	"testing"
)

// writeFile creates a file with optional content at path, creating parent dirs.
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
