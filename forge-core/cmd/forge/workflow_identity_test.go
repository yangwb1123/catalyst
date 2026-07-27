package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLoadWorkflowRejectsStageIdentityDrift(t *testing.T) {
	tests := []struct {
		name string
		body string
		want string
	}{
		{"empty", "phases:\n  - name: p\n    agent: planner\n", "empty stage"},
		{"mismatch", "stage: delivery\nphases:\n  - name: p\n    agent: planner\n    readonly: true\n", "must match"},
		{"unknown", "stage: custom\nphases:\n  - name: p\n    agent: planner\n", "unknown stage"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			name := "deploy"
			if tc.name == "unknown" {
				name = "custom"
			}
			writeWorkflowFixture(t, root, name, tc.body)
			if _, err := loadWorkflow(root, name); err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("load error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestShippedReleaseWorkflowsSatisfyRuntimeContract(t *testing.T) {
	root := filepath.Clean("../../..")
	for _, stage := range []string{"deploy", "rollback"} {
		if _, err := loadWorkflow(root, stage); err != nil {
			t.Fatalf("load shipped %s: %v", stage, err)
		}
	}
}

func TestReleaseWorkflowTamperingFailsClosed(t *testing.T) {
	deploy := shippedWorkflowYAML(t, "deploy")
	tests := []struct {
		name   string
		stage  string
		source string
		want   string
	}{
		{"top readonly", "deploy", replaceOnce(t, deploy, "readonly: true", "readonly: false"), "immutable release contract"},
		{"workflow id", "deploy", replaceOnce(t, deploy, "id: deploy", "id: build"), "immutable release contract"},
		{"phase name or order", "deploy", replaceOnce(t, deploy, "name: release-planning", "name: release-plan-validation"), "duplicates phase name"},
		{"agent", "deploy", replaceOnce(t, deploy, "agent: release-engineer", "agent: implementer"), "immutable release contract"},
		{"phase readonly", "deploy", replaceOnce(t, deploy, "    readonly: true", "    readonly: false"), "immutable release contract"},
		{"model", "deploy", replaceOnce(t, deploy, "model_tier: sonnet", "model_tier: opus"), "immutable release contract"},
		{"emit set", "deploy", replaceOnce(t, deploy, "      - docs/release/release-manifest.yml", "      - docs/release/release-manifest.yml\n      - docs/release/unapproved-extra.md"), "immutable release contract"},
		{"human gate", "deploy", replaceOnce(t, deploy, "type: human_gate", "type: external"), "immutable release contract"},
		{"approval", "deploy", replaceOnce(t, deploy, "human_approval: required", "human_approval: optional"), "immutable release contract"},
		{"durable wait", "deploy", replaceOnce(t, deploy, "durable_wait: true", "durable_wait: false"), "immutable release contract"},
		{"expression", "deploy", replaceOnce(t, deploy, "external_apply_evidence_verified_by_human == true", "plan_exists == true"), "immutable release contract"},
		{"on fail", "deploy", replaceOnce(t, deploy, "action: loop_back", "action: continue"), "immutable release contract"},
		{"on rejected", "deploy", replaceLast(t, deploy, "target_phase: release-planning", "target_phase: release-plan-validation"), "immutable release contract"},
		{"on approved", "deploy", replaceOnce(t, deploy, "next_stage: evolve", "next_stage: build"), "immutable release contract"},
		{"writes adr", "deploy", replaceOnce(t, deploy, "    model_tier: sonnet", "    model_tier: sonnet\n    writes_adr:\n      target: docs/adr"), "immutable release contract"},
		{"requires tools", "deploy", replaceOnce(t, deploy, "    model_tier: sonnet", "    model_tier: sonnet\n    requires_tools: [kubectl]"), "immutable release contract"},
		{"required gates", "deploy", replaceOnce(t, deploy, "required_gates: []", "required_gates: [security]"), "immutable release contract"},
		{"rollback transition", "rollback", replaceOnce(t, shippedWorkflowYAML(t, "rollback"), "on_approved: {}", "on_approved:\n    next_stage: evolve"), "immutable release contract"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			writeWorkflowFixture(t, root, tc.stage, tc.source)
			if _, err := loadWorkflow(root, tc.stage); err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("tampered load error = %v, want %q", err, tc.want)
			}
		})
	}
}

func shippedWorkflowYAML(t *testing.T, stage string) string {
	t.Helper()
	path := filepath.Join("..", "..", "..", ".agent", "workflows", stage+".yml")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func replaceOnce(t *testing.T, source, old, replacement string) string {
	t.Helper()
	if !strings.Contains(source, old) {
		t.Fatalf("fixture lacks mutation target %q", old)
	}
	return strings.Replace(source, old, replacement, 1)
}

func replaceLast(t *testing.T, source, old, replacement string) string {
	t.Helper()
	index := strings.LastIndex(source, old)
	if index < 0 {
		t.Fatalf("fixture lacks mutation target %q", old)
	}
	return source[:index] + replacement + source[index+len(old):]
}

func writeWorkflowFixture(t *testing.T, root, name, body string) {
	t.Helper()
	dir := filepath.Join(root, ".agent", "workflows")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, name+".yml"), []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
}
