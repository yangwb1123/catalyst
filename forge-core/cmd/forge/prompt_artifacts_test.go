package main

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
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

func TestBuildPrompt_RejectsArtifactContextOutsideRepository(t *testing.T) {
	root := t.TempDir()
	outsideDir := t.TempDir()
	outside := filepath.Join(outsideDir, "operator-secret.txt")
	// secret-scan:ignore -- inert sentinel verifies that outside context is absent.
	const secret = "DO-NOT-SEND-OUTSIDE-REPOSITORY"
	if err := os.WriteFile(outside, []byte(secret), 0o600); err != nil {
		t.Fatal(err)
	}
	traversal, err := filepath.Rel(root, outside)
	if err != nil {
		t.Fatal(err)
	}
	candidates := []string{outside, traversal}
	if err := os.Symlink(outsideDir, filepath.Join(root, "outside-link")); err == nil {
		candidates = append(candidates, "outside-link/operator-secret.txt")
	}
	for _, candidate := range candidates {
		t.Run(candidate, func(t *testing.T) {
			p := asset.Phase{Name: "review", Agent: "reviewer", UsesTemplate: candidate}
			got := buildPromptWithEmits(
				root, p, "balanced", unbudgetedTier("balanced"),
				nil, nil, nil, nil, []string{candidate},
			)
			if strings.Contains(got, secret) {
				t.Fatalf("repository-external content leaked into agent prompt: %q", candidate)
			}
		})
	}
}

// buildPrompt must inject a [context:writes_adr] block when the phase declares
// writes_adr with a target directory (design.yml's solution-architect). The
// block must mention the target directory and the next ADR sequence number.
func TestBuildPrompt_WritesAdrInjectsContext(t *testing.T) {
	root := t.TempDir()
	adrDir := filepath.Join(root, "docs", "adr")
	if err := os.MkdirAll(adrDir, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	// Create one existing ADR to test sequence number detection.
	if err := os.WriteFile(filepath.Join(adrDir, "0001-test-decision.md"), []byte("# ADR 1"), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	p := asset.Phase{
		Name:  "solution-architect",
		Agent: "architect",
		WritesADR: &asset.WritesADR{
			Condition: "mode in [engineering, cto]",
			Target:    "docs/adr/",
		},
	}
	got := buildPromptWithEmits(root, p, "engineering", unbudgetedTier("engineering"), nil, nil, nil, nil, nil)
	if !strings.Contains(got, "[context:writes_adr]") {
		t.Errorf("buildPrompt with writes_adr must inject a [context:writes_adr] block, got:\n%s", got)
	}
	if !strings.Contains(got, "docs/adr/") {
		t.Errorf("writes_adr context must mention the target directory, got:\n%s", got)
	}
	if !strings.Contains(got, "ADR-0002") {
		t.Errorf("writes_adr context must suggest the next sequence number (0002), got:\n%s", got)
	}
	// Without writes_adr, the block must be absent.
	plain := buildPromptWithEmits(root, asset.Phase{Name: "solution-architect", Agent: "architect"}, "engineering", unbudgetedTier("engineering"), nil, nil, nil, nil, nil)
	if strings.Contains(plain, "[context:writes_adr]") {
		t.Errorf("buildPrompt without writes_adr must not inject a writes_adr block, got:\n%s", plain)
	}
}

// Prompt construction must be side-effect free: a missing ADR directory is
// described to the agent but is not created merely by inspecting/building a prompt.
func TestBuildPrompt_WritesAdrDoesNotCreateTargetDir(t *testing.T) {
	root := t.TempDir()
	adrDir := filepath.Join(root, "docs", "adr")
	p := asset.Phase{
		Name:  "solution-architect",
		Agent: "architect",
		WritesADR: &asset.WritesADR{
			Condition: "mode in [engineering, cto]",
			Target:    "docs/adr/",
		},
	}
	got := buildPromptWithEmits(root, p, "engineering", unbudgetedTier("engineering"), nil, nil, nil, nil, nil)
	if !strings.Contains(got, "[context:writes_adr]") {
		t.Fatalf("missing target should still produce ADR guidance, got:\n%s", got)
	}
	if _, err := os.Stat(adrDir); !os.IsNotExist(err) {
		t.Errorf("prompt construction must not create writes_adr target; stat error = %v", err)
	}
}

// nextADRSequence must return the correct next sequence number based on
// existing ADR files. It must handle both "ADR-XXXX-*.md" and "XXXX-*.md"
// naming conventions, skip non-ADR files, and return 1 for an empty dir.
func TestNextADRSequence(t *testing.T) {
	dir := t.TempDir()
	// Test 1: empty directory.
	if got := nextADRSequence(dir); got != 1 {
		t.Errorf("nextADRSequence(empty) = %d, want 1", got)
	}
	// Test 2: mixed naming conventions.
	os.WriteFile(filepath.Join(dir, "0001-first-decision.md"), []byte("#1"), 0o644)
	os.WriteFile(filepath.Join(dir, "ADR-0003-third-adr.md"), []byte("#3"), 0o644)
	os.WriteFile(filepath.Join(dir, "0002-second-decision.md"), []byte("#2"), 0o644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("not an ADR"), 0o644)
	if got := nextADRSequence(dir); got != 4 {
		t.Errorf("nextADRSequence(mixed) = %d, want 4 (max 3 + 1)", got)
	}
	// Test 3: non-ADR files (no .md extension) must be ignored.
	dir2 := t.TempDir()
	os.WriteFile(filepath.Join(dir2, "notes.txt"), []byte("text"), 0o644)
	os.WriteFile(filepath.Join(dir2, "ADR-0005-decision.md"), []byte("#5"), 0o644)
	if got := nextADRSequence(dir2); got != 6 {
		t.Errorf("nextADRSequence(no-md-ignored) = %d, want 6", got)
	}
}

// writedAdrContext must return "" for nil WritesADR or empty target, so
// phases without writes_adr never inject an ADR context block.
func TestWritesAdrContextNilOrEmpty(t *testing.T) {
	root := t.TempDir()
	if got := writesAdrContext(root, nil); got != "" {
		t.Errorf("writesAdrContext(nil) = %q, want empty", got)
	}
	if got := writesAdrContext(root, &asset.WritesADR{Condition: "always", Target: ""}); got != "" {
		t.Errorf("writesAdrContext(empty target) = %q, want empty", got)
	}
}

func TestParseWritesADRCondition(t *testing.T) {
	valid, err := parseWritesADRCondition(" mode in [engineering, cto] ")
	if err != nil {
		t.Fatalf("parse valid condition: %v", err)
	}
	if !valid.matches("engineering") || !valid.matches("cto") || valid.matches("balanced") {
		t.Errorf("parsed mode set has wrong membership: %+v", valid.modes)
	}
	for _, invalid := range []string{
		"",
		"always",
		"mode == engineering",
		"modein [engineering]",
		"mode inside [engineering]",
		"mode in []",
		"mode in [engineering, typo]",
		"lifecycle in [production]",
	} {
		t.Run(invalid, func(t *testing.T) {
			if _, err := parseWritesADRCondition(invalid); err == nil {
				t.Errorf("parseWritesADRCondition(%q) unexpectedly succeeded", invalid)
			}
		})
	}
}

func TestEffectiveWritesADRCombinesConditionAndPolicy(t *testing.T) {
	root := t.TempDir()
	declared := &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"}
	tests := []struct {
		name, runMode, lifecycle string
		wa                       *asset.WritesADR
		wantEnabled              bool
	}{
		{"engineering declaration and policy allow", "engineering", "mvp", declared, true},
		{"balanced policy denies", "balanced", "mvp", declared, false},
		{"condition narrows otherwise-enabled mode", "engineering", "mvp", &asset.WritesADR{Condition: "mode in [cto]", Target: "docs/adr/"}, false},
		{"production safety veto forces ADR", "explorer", "production", declared, true},
		{"malformed condition fails closed", "engineering", "mvp", &asset.WritesADR{Condition: "always", Target: "docs/adr/"}, false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got, _ := effectiveWritesADR(root, tc.runMode, tc.lifecycle, tc.wa)
			if (got != nil) != tc.wantEnabled {
				t.Errorf("effectiveWritesADR(%s, %s) enabled=%v, want %v", tc.runMode, tc.lifecycle, got != nil, tc.wantEnabled)
			}
			if got != nil && got.Target != "docs/adr/" {
				t.Errorf("normalized target = %q, want docs/adr/", got.Target)
			}
		})
	}
}

func TestWritesADRTargetMustStayWithinRepo(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	for _, target := range []string{
		"../outside",
		outside,
		"docs/../../outside",
		"docs/**",
		"docs/adr) Write(/**",
	} {
		t.Run(strings.ReplaceAll(target, "/", "_"), func(t *testing.T) {
			wa := &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: target}
			if got := writesAdrContext(root, wa); got != "" {
				t.Errorf("escaping target %q produced ADR context:\n%s", target, got)
			}
			if authorized, _ := effectiveWritesADR(root, "engineering", "mvp", wa); authorized != nil {
				t.Errorf("escaping target %q received authorization: %+v", target, authorized)
			}
		})
	}
}

func TestWritesADRTargetRejectsSymlinkEscape(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "docs")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	wa := &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"}
	if got := writesAdrContext(root, wa); got != "" {
		t.Errorf("symlink escape produced ADR context:\n%s", got)
	}
	if authorized, _ := effectiveWritesADR(root, "engineering", "mvp", wa); authorized != nil {
		t.Errorf("symlink escape received authorization: %+v", authorized)
	}
}

func TestAgentExecutorAppliesWritesADRConditionAndPolicy(t *testing.T) {
	declared := &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"}
	tests := []struct {
		name, runMode, lifecycle string
		wa                       *asset.WritesADR
		wantADR                  bool
	}{
		{"engineering enabled", "engineering", "mvp", declared, true},
		{"balanced disabled", "balanced", "mvp", declared, false},
		{"production forces explorer", "explorer", "production", declared, true},
		{"malformed condition disabled", "engineering", "mvp", &asset.WritesADR{Condition: "always", Target: "docs/adr/"}, false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			o := runOpts{executor: "command", agentCmd: "claude", root: root, lifecycle: tc.lifecycle}
			ex := agentExecutor(o, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
			ce, ok := ex.(orchestrator.CommandExecutor)
			if !ok {
				t.Fatalf("executor=command yielded %T", ex)
			}
			p := asset.Phase{
				Name:      "solution-architect",
				Agent:     "architect",
				Readonly:  true,
				WritesADR: tc.wa,
			}
			argv := strings.Join(ce.Build(p, tc.runMode), " ")
			hasPrompt := strings.Contains(argv, "[context:writes_adr]")
			hasScope := strings.Contains(argv, "Edit(/docs/adr/**)")
			if hasPrompt != tc.wantADR || hasScope != tc.wantADR {
				t.Errorf("want ADR=%v, prompt=%v scope=%v; argv:\n%s", tc.wantADR, hasPrompt, hasScope, argv)
			}
		})
	}
}

func TestReadonlyToolScopeRejectsEscapingADRTarget(t *testing.T) {
	root := t.TempDir()
	p := asset.Phase{
		Name:      "solution-architect",
		Agent:     "architect",
		Readonly:  true,
		WritesADR: &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "../outside/"},
	}
	deny, allow := readonlyToolScopeForRoot(root, p)
	if deny != "" {
		t.Errorf("deny = %q, want empty with dontAsk", deny)
	}
	if strings.Contains(allow, "outside") || strings.Contains(allow, "adr") {
		t.Errorf("escaping ADR target leaked into readonly allowance: %q", allow)
	}
	if allow != "" {
		t.Errorf("readonly phase without emits must receive no ordinary write grant: %q", allow)
	}
}

func TestReadonlyToolScopeRejectsSymlinkEscapingADRTarget(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "docs-link")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	p := asset.Phase{
		Name:      "solution-architect",
		Agent:     "architect",
		Readonly:  true,
		WritesADR: &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs-link/adr/"},
	}
	_, allow := readonlyToolScopeForRoot(root, p)
	if strings.Contains(allow, "docs-link") || strings.Contains(allow, "/adr/") {
		t.Errorf("symlink-escaping ADR target leaked into readonly allowance: %q", allow)
	}
}

func TestDryRunWithWritesADRHasNoFilesystemSideEffects(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "docs", "adr")
	ex := agentExecutor(runOpts{executor: "dry", root: root}, nil, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	err := ex.Execute(context.Background(), asset.Phase{
		Name:      "solution-architect",
		Agent:     "architect",
		WritesADR: &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"},
	}, "engineering")
	if err != nil {
		t.Fatalf("dry-run execute: %v", err)
	}
	if _, err := os.Stat(target); !os.IsNotExist(err) {
		t.Errorf("dry-run created ADR target; stat error = %v", err)
	}
}
