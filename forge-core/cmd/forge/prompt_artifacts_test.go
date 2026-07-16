package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

// buildPrompt must inject the AI-SDLC template content when the phase declares
// secondary_template — mirroring TestBuildPrompt_UsesTemplateInjectsContent
// exactly, but for the OPTIONAL second template review.yml's
// performance-reliability-review phase pairs alongside uses_template
// (05-performance-review.md + 06-production-readiness.md). The template
// content appears as its own [context:template:...] block in the prompt.
func TestBuildPrompt_SecondaryTemplateInjectsContent(t *testing.T) {
	root := t.TempDir()
	tmplDir := filepath.Join(root, ".ai", "prompts")
	if err := os.MkdirAll(tmplDir, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	tmplPath := filepath.Join(tmplDir, "06-production-readiness.md")
	tmplContent := "# Production Readiness Template\n\n## Rollback Plan\n- Feature flag\n"
	if err := os.WriteFile(tmplPath, []byte(tmplContent), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	p := asset.Phase{Name: "performance-reliability-review", Agent: "performance-engineer", SecondaryTemplate: ".ai/prompts/06-production-readiness.md"}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	if !strings.Contains(got, "[context:template:") {
		t.Errorf("buildPrompt with secondary_template must inject a [context:template:...] block, got:\n%s", got)
	}
	if !strings.Contains(got, "Rollback Plan") {
		t.Errorf("buildPrompt must inject secondary_template content, got:\n%s", got)
	}
	// Without secondary_template, the template block must be absent.
	plain := buildPromptWithEmits(root, asset.Phase{Name: "performance-reliability-review", Agent: "performance-engineer"}, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	if strings.Contains(plain, "[context:template:") {
		t.Errorf("buildPrompt without secondary_template must not inject a template block, got:\n%s", plain)
	}
}

// buildPrompt must inject BOTH uses_template and secondary_template as
// separate [context:template:...] blocks when a phase declares both — the
// exact review.yml performance-reliability-review shape (05-performance-review.md
// paired with 06-production-readiness.md, one phase, two review dimensions).
func TestBuildPrompt_UsesTemplateAndSecondaryTemplateBothInjected(t *testing.T) {
	root := t.TempDir()
	tmplDir := filepath.Join(root, ".ai", "prompts")
	if err := os.MkdirAll(tmplDir, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	primary := "# Performance Review Template\nLatency budget.\n"
	secondary := "# Production Readiness Template\nRollback plan.\n"
	if err := os.WriteFile(filepath.Join(tmplDir, "05-performance-review.md"), []byte(primary), 0o644); err != nil {
		t.Fatalf("WriteFile primary: %v", err)
	}
	if err := os.WriteFile(filepath.Join(tmplDir, "06-production-readiness.md"), []byte(secondary), 0o644); err != nil {
		t.Fatalf("WriteFile secondary: %v", err)
	}
	p := asset.Phase{
		Name:              "performance-reliability-review",
		Agent:             "performance-engineer",
		UsesTemplate:      ".ai/prompts/05-performance-review.md",
		SecondaryTemplate: ".ai/prompts/06-production-readiness.md",
	}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	if !strings.Contains(got, "Latency budget.") {
		t.Errorf("buildPrompt must inject uses_template content, got:\n%s", got)
	}
	if !strings.Contains(got, "Rollback plan.") {
		t.Errorf("buildPrompt must ALSO inject secondary_template content, got:\n%s", got)
	}
}

// buildPrompt must WARN (via stderr) but NOT fail when secondary_template
// references a missing file — mirroring TestBuildPrompt_UsesTemplateMissingFileWarns,
// and the WARNING text must name "secondary_template" (not "uses_template")
// so the two fields' diagnostics stay distinguishable.
func TestBuildPrompt_SecondaryTemplateMissingFileWarns(t *testing.T) {
	root := t.TempDir()
	oldStderr := os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	os.Stderr = w

	p := asset.Phase{Name: "performance-reliability-review", Agent: "performance-engineer", SecondaryTemplate: ".ai/prompts/nonexistent2.md"}
	got := buildPromptWithEmits(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil, nil)
	w.Close()
	os.Stderr = oldStderr
	var stderrBuf bytes.Buffer
	if _, err := stderrBuf.ReadFrom(r); err != nil {
		t.Fatalf("read stderr pipe: %v", err)
	}
	if !strings.Contains(stderrBuf.String(), "WARNING secondary_template") {
		t.Errorf("missing secondary_template file must produce a WARNING naming secondary_template on stderr, got: %q", stderrBuf.String())
	}
	if strings.Contains(got, "[context:template:") {
		t.Errorf("missing secondary_template must not inject a template block, got:\n%s", got)
	}
}
