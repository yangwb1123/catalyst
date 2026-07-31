package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/persist"
)

func TestEvolveRejectsNonEvolveStageBeforeRunState(t *testing.T) {
	requirePython(t)
	build := strings.Replace(externalAgentWorkflow, `"stage": "evolve"`, `"stage": "build"`, 1)
	root := fakeRepo(t, "build", build)
	if code := cmdEvolve([]string{
		"build", "--root", root, "--mode", "explorer", "--lifecycle", "idea",
	}); code != 1 {
		t.Fatalf("forge evolve build exit=%d, want fail-closed stage rejection", code)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("non-Evolve workflow created run state before rejection: %v", err)
	}
}

func TestEvolveProposalOnlyDoesNotExecuteRepositoryAcceptance(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	sentinel := filepath.Join(root, "probe-ran")
	script := "require('node:fs').writeFileSync('probe-ran','ran'); console.log('{}');\n"
	writeFile(t, filepath.Join(root, "harness", "acceptance.mjs"), script)
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "explorer",
		"--lifecycle", "idea", "--max-iter", "1",
	}); code != 0 {
		t.Fatalf("proposal-only evolve exit=%d", code)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("proposal-only convergence executed repository acceptance: %v", err)
	}
}

func TestRunProposalOnlyDoesNotExecuteRepositoryAcceptance(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	sentinel := filepath.Join(root, "run-probe-ran")
	script := "require('node:fs').writeFileSync('run-probe-ran','ran'); console.log('{}');\n"
	writeFile(t, filepath.Join(root, "harness", "acceptance.mjs"), script)
	if code := cmdRun([]string{
		"evolve", "--root", root, "--mode", "explorer", "--lifecycle", "idea",
	}); code != 0 {
		t.Fatalf("proposal-only forge run evolve exit=%d", code)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("proposal-only forge run executed repository acceptance: %v", err)
	}
}

func TestRunChainProposalOnlyDoesNotExecuteRepositoryAcceptance(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	sentinel := filepath.Join(root, "chain-probe-ran")
	script := "require('node:fs').writeFileSync('chain-probe-ran','ran'); console.log('{}');\n"
	writeFile(t, filepath.Join(root, "harness", "acceptance.mjs"), script)
	if code := cmdRun([]string{
		"evolve", "--root", root, "--mode", "explorer", "--lifecycle", "idea", "--chain",
	}); code != 0 {
		t.Fatalf("proposal-only chained forge run evolve exit=%d", code)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("proposal-only chained forge run executed repository acceptance: %v", err)
	}
}

func TestProposalOnlyLoopHasNoRepositoryGateRunner(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	sentinel := filepath.Join(root, "loop-gate-ran")
	script := "require('node:fs').writeFileSync('loop-gate-ran','ran'); console.log('{}');\n"
	writeFile(t, filepath.Join(root, "harness", "acceptance.mjs"), script)
	wf, err := loadWorkflowNativeOnly(root, "evolve")
	if err != nil {
		t.Fatal(err)
	}
	budget, err := newRunBudget("")
	if err != nil {
		t.Fatal(err)
	}
	loop, _, _, _ := buildLoop(wf, runOpts{
		root: root, mode: "explorer", lifecycle: "idea", executor: "dry",
	}, 1, func(string) {}, nil, budget, "", nil)
	if result := loop.Engine.RunGate("test"); result.Status != "FAIL" {
		t.Fatalf("proposal-only gate status=%q, want FAIL", result.Status)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("proposal-only loop gate runner executed repository acceptance: %v", err)
	}
}

func TestRestrictedEvolveLoadDoesNotExecuteRepositoryShim(t *testing.T) {
	requirePython(t)
	for _, tc := range []struct {
		name string
		run  func(string) int
	}{
		{"evolve command", func(root string) int {
			return cmdEvolve([]string{
				"evolve", "--root", root, "--mode", "explorer",
				"--lifecycle", "idea", "--max-iter", "1",
			})
		}},
		{"run command", func(root string) int {
			return cmdRun([]string{
				"evolve", "--root", root, "--mode", "explorer",
				"--lifecycle", "idea",
			})
		}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			sentinel := filepath.Join(root, "workflow-shim-ran")
			writeFile(t, filepath.Join(root, ".agent", "workflows", "evolve.yml"), "stub: true\n")
			shim := "from pathlib import Path\n" +
				"Path(" + pyQuote(sentinel) + ").write_text('ran')\n" +
				"print(" + pyQuote(externalAgentWorkflow) + ")\n"
			writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
			if code := tc.run(root); code != 1 {
				t.Fatalf("restricted malformed workflow exit=%d, want fail-closed", code)
			}
			if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
				t.Fatalf("restricted config load executed repository shim: %v", err)
			}
			if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
				t.Fatalf("restricted config rejection created run state: %v", err)
			}
		})
	}
}

func TestBuildHaltedRunLoadDoesNotExecuteRepositoryShim(t *testing.T) {
	build := strings.Replace(externalAgentWorkflow, `"stage": "evolve"`, `"stage": "build"`, 1)
	root := fakeRepo(t, "build", build)
	sentinel := filepath.Join(root, "halted-build-shim-ran")
	writeFile(t, filepath.Join(root, ".agent", "workflows", "build.yml"), "stub: true\n")
	shim := "from pathlib import Path\n" +
		"Path(" + pyQuote(sentinel) + ").write_text('ran')\n" +
		"print(" + pyQuote(build) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	if code := cmdRun([]string{
		"build", "--root", root, "--mode", "cto", "--lifecycle", "idea",
	}); code != 1 {
		t.Fatalf("halted Build malformed workflow exit=%d, want fail-closed", code)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("halted Build load executed repository shim: %v", err)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("halted Build config rejection created run state: %v", err)
	}
}

func TestEvolveAutoChainEntryDoesNotExecuteRepositoryShim(t *testing.T) {
	discover := strings.Replace(externalAgentWorkflow, `"stage": "evolve"`, `"stage": "discover"`, 1)
	root := fakeRepo(t, "discover", discover)
	sentinel := filepath.Join(root, "auto-chain-shim-ran")
	writeFile(t, filepath.Join(root, ".agent", "workflows", "discover.yml"), "stub: true\n")
	shim := "from pathlib import Path\n" +
		"Path(" + pyQuote(sentinel) + ").write_text('ran')\n" +
		"print(" + pyQuote(discover) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	if code := cmdEvolve([]string{
		"auto", "--root", root, "--chain", "--mode", "balanced", "--lifecycle", "mvp",
	}); code != 1 {
		t.Fatalf("auto chain malformed entry exit=%d, want fail-closed", code)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("auto chain entry executed repository shim: %v", err)
	}
}

func TestEvolveResumeRejectsDiagnosticOnlyFormatBeforeTrace(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	wf, err := loadWorkflow(root, "evolve")
	if err != nil {
		t.Fatal(err)
	}
	cp := persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: "balanced", Lifecycle: "mvp", Iteration: 1,
	}
	data, err := json.Marshal(cp) // FormatVersion empty -> no _format field.
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(checkpointPath(root)), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(checkpointPath(root), data, 0o600); err != nil {
		t.Fatal(err)
	}
	before, _ := os.ReadFile(checkpointPath(root))
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "balanced",
		"--lifecycle", "mvp", "--max-iter", "2", "--resume",
	}); code != 1 {
		t.Fatalf("diagnostic-only resume exit=%d, want 1", code)
	}
	after, _ := os.ReadFile(checkpointPath(root))
	if string(after) != string(before) {
		t.Fatal("diagnostic-only resume modified checkpoint")
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("diagnostic-only resume created trace: %v", err)
	}
}

func TestEvolveResumeRejectsV2DiagnosticCheckpointBeforeTrace(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	wf, err := loadWorkflow(root, "evolve")
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(map[string]any{
		"_format":          "forgeos.checkpoint.v2",
		"workflow":         "evolve",
		"workflow_digest":  checkpointWorkflowDigest(wf),
		"mode":             "balanced",
		"lifecycle":        "mvp",
		"iteration":        1,
		"phase_index":      1,
		"spent_usd_micros": 1000,
	})
	if err != nil {
		t.Fatal(err)
	}
	mkdir(t, filepath.Join(root, ".forge"))
	if err := os.WriteFile(checkpointPath(root), data, 0o600); err != nil {
		t.Fatal(err)
	}
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "balanced",
		"--lifecycle", "mvp", "--max-iter", "2", "--resume",
	}); code != 1 {
		t.Fatalf("v2 diagnostic resume exit=%d, want 1", code)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("v2 diagnostic resume created trace: %v", err)
	}
}

func TestEvolveResumeRejectsIncompleteV3BeforeTrace(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	wf, err := loadWorkflow(root, "evolve")
	if err != nil {
		t.Fatal(err)
	}
	cp := persist.Checkpoint{
		FormatVersion: persist.CheckpointFormatCurrent,
		Workflow:      "evolve", WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: "balanced", Lifecycle: "mvp",
		RoadmapCompletion: 0.25, GatesGreen: false,
		Reason: "iteration complete", UpdatedAtUnix: 1_750_000_000,
	}
	data, err := json.Marshal(cp)
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	delete(fields, "iteration")
	data, err = json.Marshal(fields)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(checkpointPath(root)), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(checkpointPath(root), data, 0o600); err != nil {
		t.Fatal(err)
	}
	before := append([]byte(nil), data...)
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "balanced",
		"--lifecycle", "mvp", "--max-iter", "2", "--resume",
	}); code != 1 {
		t.Fatalf("incomplete v2 resume exit=%d, want 1", code)
	}
	after, _ := os.ReadFile(checkpointPath(root))
	if string(after) != string(before) {
		t.Fatal("incomplete v2 resume modified checkpoint")
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("incomplete v2 resume created trace: %v", err)
	}
}

func TestEvolveResumeRejectsMaxIterationBeforeTrace(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	wf, err := loadWorkflow(root, "evolve")
	if err != nil {
		t.Fatal(err)
	}
	cp := persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: "balanced", Lifecycle: "mvp", Iteration: int(^uint(0) >> 1),
		RoadmapCompletion: 0.5, Reason: "iteration complete",
		UpdatedAtUnix: 1_750_000_000,
	}
	if err := persist.Save(checkpointPath(root), cp, 0); err != nil {
		t.Fatal(err)
	}
	before, _ := os.ReadFile(checkpointPath(root))
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "balanced",
		"--lifecycle", "mvp", "--max-iter", "2", "--resume",
	}); code != 1 {
		t.Fatalf("MaxInt resume exit=%d, want 1", code)
	}
	after, _ := os.ReadFile(checkpointPath(root))
	if string(after) != string(before) {
		t.Fatal("MaxInt resume modified checkpoint")
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("MaxInt resume created trace: %v", err)
	}
}

func TestEvolveResumeRejectsWorkflowStructureDrift(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	args := []string{
		"evolve", "--root", root, "--mode", "engineering",
		"--lifecycle", "growth", "--max-iter", "1",
	}
	if code := cmdEvolve(args); code != 0 {
		t.Fatalf("first evolve exit=%d", code)
	}
	checkpointBefore, _ := os.ReadFile(checkpointPath(root))
	tracePath := filepath.Join(root, ".forge", "trace.jsonl")
	traceBefore, _ := os.ReadFile(tracePath)

	changed := strings.Replace(
		externalAgentWorkflow,
		`"phases": [`,
		`"phases": [{"name":"new-scan","agent":"explorer","readonly":true,"effect":"observe","required_gates":[]},`,
		1,
	)
	shim := "import sys\nsys.stdout.write(" + pyQuote(changed) + ")\n"
	writeFile(t, filepath.Join(root, ".agent", "workflows", "evolve.yml"), "stub: true\n")
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	if code := cmdEvolve(append(args, "--resume")); code != 1 {
		t.Fatalf("workflow-drift resume exit=%d, want 1", code)
	}
	checkpointAfter, _ := os.ReadFile(checkpointPath(root))
	traceAfter, _ := os.ReadFile(tracePath)
	if string(checkpointAfter) != string(checkpointBefore) ||
		string(traceAfter) != string(traceBefore) {
		t.Fatal("workflow-drift resume mutated checkpoint or trace before rejection")
	}
}

func TestEvolveResumeBindsProjectResolvedLifecycle(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	project := filepath.Join(root, ".agent", "project.yml")
	writeFile(t, project, "lifecycle: growth\n")
	args := []string{
		"evolve", "--root", root, "--mode", "engineering", "--max-iter", "1",
	}
	if code := cmdEvolve(args); code != 0 {
		t.Fatalf("first project-lifecycle evolve exit=%d", code)
	}
	before, _ := os.ReadFile(checkpointPath(root))
	tracePath := filepath.Join(root, ".forge", "trace.jsonl")
	traceBefore, _ := os.ReadFile(tracePath)
	writeFile(t, project, "lifecycle: production\n")
	if code := cmdEvolve(append(args, "--resume")); code != 1 {
		t.Fatalf("resolved-lifecycle drift resume exit=%d, want 1", code)
	}
	after, _ := os.ReadFile(checkpointPath(root))
	traceAfter, _ := os.ReadFile(tracePath)
	if string(after) != string(before) || string(traceAfter) != string(traceBefore) {
		t.Fatal("resolved-lifecycle mismatch mutated checkpoint or trace")
	}
}
