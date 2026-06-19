package main

import (
	"path/filepath"
	"testing"
)

// resolveLifecycle precedence: explicit --lifecycle wins; else project.yml's
// `lifecycle:`; else the "mvp" default. project.yml reading must strip a trailing
// comment, exactly the production project.yml shape (`lifecycle: mvp  # ...`).
// (mkdir/writeFile helpers are shared from main_test.go in this package.)
func TestResolveLifecycle_Precedence(t *testing.T) {
	// (1) Explicit flag wins even when project.yml says otherwise.
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "mode: engineering\nlifecycle: production  # comment\n")
	if got := resolveLifecycle(runOpts{root: root, lifecycle: "idea"}); got != "idea" {
		t.Errorf("explicit flag = %q, want idea (flag wins over project.yml)", got)
	}
	// (2) No flag: read project.yml, stripping the trailing comment.
	if got := resolveLifecycle(runOpts{root: root}); got != "production" {
		t.Errorf("project.yml lifecycle = %q, want production (comment stripped)", got)
	}
	// (3) No flag, no project.yml: the mvp default.
	if got := resolveLifecycle(runOpts{root: t.TempDir()}); got != "mvp" {
		t.Errorf("default = %q, want mvp", got)
	}
	// (4) project.yml present but no lifecycle key: still the mvp default.
	bare := t.TempDir()
	mkdir(t, filepath.Join(bare, ".agent"))
	writeFile(t, filepath.Join(bare, ".agent", "project.yml"), "mode: balanced\n")
	if got := resolveLifecycle(runOpts{root: bare}); got != "mvp" {
		t.Errorf("missing lifecycle key = %q, want mvp", got)
	}
}

// projectYAMLValue reads a flat scalar and returns "" for a missing file/key, so
// the caller can fall back rather than crash — project.yml is optional.
func TestProjectYAMLValue(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "mode: engineering   # full gates\nlifecycle: growth\n")
	if got := projectYAMLValue(root, "mode"); got != "engineering" {
		t.Errorf("mode = %q, want engineering", got)
	}
	if got := projectYAMLValue(root, "lifecycle"); got != "growth" {
		t.Errorf("lifecycle = %q, want growth", got)
	}
	if got := projectYAMLValue(root, "absent"); got != "" {
		t.Errorf("absent key = %q, want empty", got)
	}
	if got := projectYAMLValue(t.TempDir(), "mode"); got != "" {
		t.Errorf("missing file = %q, want empty", got)
	}
}
