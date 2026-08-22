package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestHighMaterialityBuildLoadIsNativeOnlyAndRejectsDuplicateKeys(t *testing.T) {
	for _, tc := range []struct {
		name string
		body string
	}{
		{
			"duplicate YAML phase field",
			"stage: build\nphases:\n  - name: reviewer\n    agent: reviewer\n" +
				"    readonly: true\n    readonly: false\n",
		},
		{
			"duplicate inline YAML field",
			"stage: build\nphases:\n  - name: reviewer\n    agent: reviewer\n" +
				"    on_fail: {action: loop_back, action: continue}\n",
		},
		{
			"duplicate JSON field",
			`{"stage":"build","phases":[{"name":"reviewer","agent":"reviewer",` +
				`"readonly":true,"readonly":false}]}`,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			writeWorkflowFixture(t, root, "build", tc.body)
			sentinel := filepath.Join(root, "shim-ran")
			shim := "from pathlib import Path\nPath(" + pyQuote(sentinel) + ").write_text('ran')\n"
			if err := os.MkdirAll(filepath.Join(root, "harness"), 0o755); err != nil {
				t.Fatal(err)
			}
			writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
			_, err := loadWorkflowForExecution(root, "build", "balanced", "idea", "L3")
			if err == nil {
				t.Fatal("high-materiality duplicate workflow loaded")
			}
			if _, statErr := os.Stat(sentinel); !os.IsNotExist(statErr) {
				t.Fatalf("high-materiality loader executed repository shim: %v", statErr)
			}
		})
	}
}

func TestLowMaterialityRetainsTrustedHostWorkflowFallback(t *testing.T) {
	root := t.TempDir()
	writeWorkflowFixture(t, root, "custom", "unsupported: &anchor value\n")
	workflow := `{"stage":"custom","phases":[{"name":"p","agent":"reviewer"}]}`
	shim := "print(" + pyQuote(workflow) + ")\n"
	if err := os.MkdirAll(filepath.Join(root, "harness"), 0o755); err != nil {
		t.Fatal(err)
	}
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	_, err := loadWorkflowForExecution(root, "custom", "balanced", "idea", "L2")
	if err == nil || !strings.Contains(err.Error(), "unknown stage") {
		t.Fatalf("trusted-host fallback result = %v, want shim-produced workflow validation", err)
	}
}

func TestHighMaterialityBuildRejectsUnconsumedYAMLBeforeStateOrSpawn(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	path := filepath.Join(root, ".agent", "workflows", "build.yml")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	marker := "  # ── P5 QA / acceptance ──"
	mutated := strings.Replace(string(body), marker, "unconsumed scalar\n"+marker, 1)
	if mutated == string(body) {
		t.Fatal("build fixture lacks QA marker")
	}
	if err := os.WriteFile(path, []byte(mutated), 0o600); err != nil {
		t.Fatal(err)
	}
	args := append(shippedRunArgs("build", root, fake, false, false, "engineering"), "--materiality", "L3")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 1 || !strings.Contains(output, "fallback is disabled for restricted execution") {
		t.Fatalf("unconsumed high Build YAML exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"planner": 0, "reviewer": 0})
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("unconsumed YAML rejection created runtime state: %v", err)
	}
}
