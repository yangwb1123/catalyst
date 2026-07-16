// detect_output_test.go — end-to-end `forge detect` output tests: full
// project profiles (semantic + tests + CI) through to cmdDetectJSON's exit
// code. Split from detect_parsers_test.go to stay under the file-size gate.
package main

import (
	"path/filepath"
	"testing"
)

// ── Integration: full detection with semantic + CI ────────────────────────

func TestDetectOutput_NodeWithFullSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "package.json"),
		`{"scripts":{"build":"tsc","test":"vitest"},"dependencies":{"express":"4"},"devDependencies":{"typescript":"5"}}`)
	writeFileAt(t, filepath.Join(root, "src", "index.test.ts"), "")
	writeFileAt(t, filepath.Join(root, ".github", "workflows", "test.yml"), "")
	p := detectProject(root)
	s := suggestWorkflow(p)

	if p.Language != "node" {
		t.Errorf("Language = %q, want node", p.Language)
	}
	if !p.HasBuildScript {
		t.Error("HasBuildScript should be true")
	}
	if !p.HasTestScript {
		t.Error("HasTestScript should be true")
	}
	if p.DepsCount != 2 {
		t.Errorf("DepsCount = %d, want 2", p.DepsCount)
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
	exitCode := cmdDetectJSON(p, s)
	if exitCode != 0 {
		t.Fatalf("cmdDetectJSON returned exit code %d, want 0", exitCode)
	}
}

func TestDetectOutput_PythonFullSemantic(t *testing.T) {
	root := t.TempDir()
	writeFileAt(t, filepath.Join(root, "pyproject.toml"),
		`[build-system]
build-backend = "flit_core.buildapi"
[project]
requires-python = ">=3.10"
`)
	writeFileAt(t, filepath.Join(root, "test_foo.py"), "")
	writeFileAt(t, filepath.Join(root, ".github", "workflows", "ci.yml"), "")
	p := detectProject(root)
	s := suggestWorkflow(p)

	if p.Language != "python" {
		t.Errorf("Language = %q, want python", p.Language)
	}
	if p.BuildBackend != "flit_core.buildapi" {
		t.Errorf("BuildBackend = %q, want flit_core.buildapi", p.BuildBackend)
	}
	if p.PythonVersion != ">=3.10" {
		t.Errorf("PythonVersion = %q, want >=3.10", p.PythonVersion)
	}
	if !p.HasTests {
		t.Error("HasTests should be true")
	}
	if !p.HasCI {
		t.Error("HasCI should be true")
	}
	exitCode := cmdDetectJSON(p, s)
	if exitCode != 0 {
		t.Fatalf("cmdDetectJSON returned exit code %d, want 0", exitCode)
	}
}
