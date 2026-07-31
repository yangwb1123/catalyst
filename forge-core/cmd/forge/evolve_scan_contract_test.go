package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/persist"
)

func TestAppendEvolveScanPromptUsesEffectiveLifecycleDepth(t *testing.T) {
	phase := contractedScanPhase()
	depth := mode.Effective("explorer", "production").EvolveDepth
	got := appendEvolveScanPrompt("base prompt", phase, depth)
	for _, want := range []string{
		"base prompt",
		"Effective mode×lifecycle scan depth: standard",
		`"depth":"standard"`,
		evolvescan.MarkerPrefix,
	} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt lacks %q:\n%s", want, got)
		}
	}
	if strings.Contains(got, `"depth":"opportunistic"`) {
		t.Fatalf("transient mode ignored production lifecycle floor:\n%s", got)
	}
}

func TestAppendEvolveScanPromptIsIsolatedToDeclaredContract(t *testing.T) {
	plain := asset.Phase{Name: "scan", Agent: "explorer"}
	if got := appendEvolveScanPrompt("unchanged", plain, evolvescan.DepthThorough); got != "unchanged" {
		t.Fatalf("non-contracted prompt changed: %q", got)
	}
}

func TestPhaseOutputContractValidatesEffectiveScanDepth(t *testing.T) {
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	thorough := scanReport(t, evolvescan.DepthThorough, true)
	validate := phaseOutputContractWithPolicy(root, wf, mode.Effective("engineering", "mvp"))
	if err := validate("inventory", thorough); err != nil {
		t.Fatalf("valid thorough output: %v", err)
	}

	validate = phaseOutputContractWithPolicy(root, wf, mode.Effective("explorer", "production"))
	standard := scanReport(t, evolvescan.DepthStandard, false)
	if err := validate("inventory", standard); err != nil {
		t.Fatalf("production-raised standard output: %v", err)
	}
	if err := validate("inventory", scanReport(t, evolvescan.DepthOpportunistic, false)); err == nil ||
		!strings.Contains(err.Error(), "want effective depth") {
		t.Fatalf("depth-drift error = %v", err)
	}
}

func TestPhaseOutputContractWithoutEffectivePolicyFailsClosed(t *testing.T) {
	root := scanEvidenceRepo(t)
	validate := phaseOutputContract(root, evolveScanWorkflow())
	err := validate("inventory", scanReport(t, evolvescan.DepthStandard, false))
	if err == nil || !strings.Contains(err.Error(), "effective scan depth") {
		t.Fatalf("missing-policy error = %v", err)
	}
}

func TestEvolveScanRawOutputRequiresSuccessfulClaudeEnvelope(t *testing.T) {
	phase := contractedScanPhase()
	wf := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{phase}}
	validate := evolveScanRawOutputContract(wf, true)
	report := scanReport(t, evolvescan.DepthThorough, true)
	valid, err := json.Marshal(map[string]any{
		"type": "result", "subtype": "success", "is_error": false, "result": report,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := validate(phase.Name, string(valid)); err != nil {
		t.Fatalf("valid Claude envelope rejected: %v", err)
	}

	cases := []string{
		report,
		`{"type":"result","subtype":"success","is_error":true,"result":"` + evolvescan.MarkerPrefix + `"}`,
		`{"type":"result","subtype":"error_during_execution","is_error":false,"result":"` + evolvescan.MarkerPrefix + `"}`,
		`{"result":"` + evolvescan.MarkerPrefix + `","is_error":false}`,
		`{"type":"result"`,
	}
	for _, raw := range cases {
		if err := validate(phase.Name, raw); err == nil {
			t.Errorf("unsafe Claude envelope accepted: %s", raw)
		}
	}

	if err := evolveScanRawOutputContract(wf, false)(phase.Name, report); err != nil {
		t.Fatalf("plain custom executor output rejected: %v", err)
	}
	if err := validate("uncontracted", report); err != nil {
		t.Fatalf("uncontracted phase gained a raw envelope requirement: %v", err)
	}
}

func TestInvalidScanStopsBeforeGapAnalysis(t *testing.T) {
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skip("printf unavailable")
	}
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	policy := mode.Effective("engineering", "mvp")
	ledger := newPhaseOutputLedger()
	var built []string
	command := orchestrator.CommandExecutor{
		Build: func(phase asset.Phase, _ string) []string {
			built = append(built, phase.Name)
			return []string{printf, "not a scan report"}
		},
		ValidateOutput: phaseOutputContractWithPolicy(root, wf, policy),
		Observe: observeFor(
			false, nil, nil, ledger, feedsForwardOf(wf), nil, nil, nil,
			verdictContractOf(wf), scanContractOf(wf),
		),
	}
	engine := orchestrator.Engine{Exec: command, ModePolicy: policy}
	err = engine.Run(wf, "engineering")
	if err == nil || !strings.Contains(err.Error(), "evolve_scan_v1") {
		t.Fatalf("invalid scan error = %v", err)
	}
	if strings.Join(built, ",") != "inventory" {
		t.Fatalf("phases built after invalid scan: %v", built)
	}
	if output, ok := ledger.output("inventory"); ok {
		t.Fatalf("invalid contracted scan entered feed-forward ledger: %q", output)
	}
}

func TestValidScanRunsGapAnalysis(t *testing.T) {
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skip("printf unavailable")
	}
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	policy := mode.Effective("engineering", "mvp")
	var built []string
	command := orchestrator.CommandExecutor{
		Build: func(phase asset.Phase, _ string) []string {
			built = append(built, phase.Name)
			output := "done"
			if phase.ScanContract != "" {
				output = scanReport(t, evolvescan.DepthThorough, true)
			}
			return []string{printf, output}
		},
		ValidateOutput: phaseOutputContractWithPolicy(root, wf, policy),
	}
	engine := orchestrator.Engine{Exec: command, ModePolicy: policy}
	if err := engine.Run(wf, "engineering"); err != nil {
		t.Fatalf("valid scan run: %v", err)
	}
	if strings.Join(built, ",") != "inventory,gap-analysis,implement" {
		t.Fatalf("phases = %v", built)
	}
}

func TestScanFeedForwardKeepsCompleteCanonicalReport(t *testing.T) {
	root := scanEvidenceRepo(t)
	output := scanReport(t, evolvescan.DepthThorough, true)
	ledger := newPhaseOutputLedger()
	wf := evolveScanWorkflow()
	observe := observeFor(
		false, nil, nil, ledger, feedsForwardOf(wf), nil, nil, nil,
		verdictContractOf(wf), scanContractOf(wf),
	)
	observe("inventory", output, 0)
	context := ledger.context()
	if strings.Contains(context, "已截断") {
		t.Fatalf("contracted scan was truncated:\n%s", context)
	}
	canonical, err := evolvescan.Canonicalize(output)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(context, canonical) {
		t.Fatalf("ledger lacks complete canonical report:\n%s", context)
	}
	prompt := buildPrompt(root, wf.Phases[1], "engineering",
		func(asset.Phase) string { return "opus" }, nil, nil, ledger, nil)
	if !strings.Contains(prompt, canonical) {
		t.Fatalf("gap-analysis prompt lacks complete scan handoff:\n%s", prompt)
	}
}

func TestContractedScanCanonicalFailureNeverFallsBackToSummary(t *testing.T) {
	ledger := newPhaseOutputLedger()
	lookups := []func(string) string{
		nil, func(string) string { return asset.ScanContractEvolveV1 },
	}
	err := recordForwardedPhaseOutput(ledger, "inventory", "not a report", lookups)
	if err == nil || !strings.Contains(err.Error(), "canonical feed-forward") {
		t.Fatalf("canonical feed-forward error = %v", err)
	}
	if output, ok := ledger.output("inventory"); ok {
		t.Fatalf("contracted failure fell back to summary: %q", output)
	}
}

func TestContractedScanResumeRestoresLedgerWithoutReplayingPhases(t *testing.T) {
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	report := scanReport(t, evolvescan.DepthThorough, true)
	canonical, err := evolvescan.Canonicalize(report)
	if err != nil {
		t.Fatal(err)
	}
	ledger := newPhaseOutputLedger()
	executed := false
	command := orchestrator.CommandExecutor{
		Build: func(asset.Phase, string) []string {
			executed = true
			return nil
		},
		ValidateOutput: phaseOutputContractWithPolicy(
			root, wf, mode.Effective("engineering", "mvp"),
		),
		Observe: observeFor(
			false, nil, nil, ledger, feedsForwardOf(wf), nil, nil, nil,
			verdictContractOf(wf), scanContractOf(wf),
		),
	}
	if err := restoreContractedScanOutput(command, wf, 2, canonical); err != nil {
		t.Fatalf("restore scan output: %v", err)
	}
	if executed {
		t.Fatal("resume restoration spawned/replayed an Agent phase")
	}
	if got, ok := ledger.output("inventory"); !ok || got != canonical {
		t.Fatalf("restored ledger = %q, %v", got, ok)
	}
	if got, required := checkpointScanReport(wf, ledger, 2); !required || got != canonical {
		t.Fatalf("checkpoint scan report = %q, required=%v", got, required)
	}
	if got, required := checkpointScanReport(wf, newPhaseOutputLedger(), 2); !required || got != "" {
		t.Fatalf("missing checkpoint scan report = %q, required=%v; must fail closed", got, required)
	}
	legacy := wf
	legacy.Phases = append([]asset.Phase(nil), wf.Phases...)
	legacy.Phases[0].ScanContract = ""
	if err := restoreContractedScanOutput(orchestrator.DryRunExecutor{}, legacy, 2, ""); err != nil {
		t.Fatalf("legacy resume changed: %v", err)
	}
}

func TestContractedScanResumeRejectsBeforeObserve(t *testing.T) {
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	ledger := newPhaseOutputLedger()
	command := orchestrator.CommandExecutor{
		ValidateOutput: phaseOutputContractWithPolicy(
			root, wf, mode.Effective("engineering", "mvp"),
		),
		Observe: observeFor(
			false, nil, nil, ledger, feedsForwardOf(wf), nil, nil, nil,
			verdictContractOf(wf), scanContractOf(wf),
		),
	}
	if err := restoreContractedScanOutput(command, wf, 2, "not a report"); err == nil {
		t.Fatal("invalid restored scan report accepted")
	}
	if output, ok := ledger.output("inventory"); ok {
		t.Fatalf("restore observed before validation: %q", output)
	}
}

func TestApplyLoopResumeRevalidatesEvidenceWithoutSpawningAgent(t *testing.T) {
	root := scanEvidenceRepo(t)
	wf := evolveScanWorkflow()
	canonical, err := evolvescan.Canonicalize(
		scanReport(t, evolvescan.DepthThorough, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(filepath.Join(root, "evidence", "code.txt")); err != nil {
		t.Fatal(err)
	}
	spawned := false
	command := orchestrator.CommandExecutor{
		Build: func(asset.Phase, string) []string {
			spawned = true
			return nil
		},
		ValidateOutput: phaseOutputContractWithPolicy(
			root, wf, mode.Effective("engineering", "mvp"),
		),
	}
	loop := orchestrator.LoopEngine{Engine: orchestrator.Engine{Exec: command}}
	err = applyLoopResume(&loop, wf, loopResumeState{
		found: true, start: 2, phaseStart: 2, scanReport: canonical,
		agentCalls: 2, maxLoopBacks: maxLoopBack,
	})
	if err == nil || !strings.Contains(err.Error(), "unavailable") {
		t.Fatalf("deleted evidence restore error = %v", err)
	}
	if spawned {
		t.Fatal("failed recovery validation spawned an Agent")
	}
}

func TestApplyLoopResumeRejectsSerialPhaseCheckpointInParallelMode(t *testing.T) {
	tests := []struct {
		name    string
		resumed loopResumeState
	}{
		{
			name: "advanced phase",
			resumed: loopResumeState{
				start: 2, phaseStart: 2, scanReport: "durable report",
			},
		},
		{
			name: "phase zero with reserved agent call",
			resumed: loopResumeState{
				start: 2, phaseStart: 0, agentCalls: 1,
			},
		},
		{
			name: "phase zero after loop-back",
			resumed: loopResumeState{
				start: 2, phaseStart: 0, agentCalls: 1, loopBacks: 1,
			},
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			loop := orchestrator.LoopEngine{Parallel: true}
			err := applyLoopResume(&loop, evolveScanWorkflow(), tc.resumed)
			if err == nil || !strings.Contains(err.Error(), "without --parallel") {
				t.Fatalf("parallel mid-iteration resume error = %v", err)
			}
			if loop.StartIter != 0 || loop.StartPhase != 0 {
				t.Fatalf("rejected resume mutated loop start: iter=%d phase=%d",
					loop.StartIter, loop.StartPhase)
			}
		})
	}
}

func TestApplyLoopResumeAllowsParallelIterationBoundary(t *testing.T) {
	loop := orchestrator.LoopEngine{Parallel: true}
	resumed := loopResumeState{
		found: true, start: 2, phaseStart: 0, agentCalls: 0, loopBacks: 0,
	}
	if err := applyLoopResume(&loop, evolveScanWorkflow(), resumed); err != nil {
		t.Fatalf("parallel iteration-boundary resume: %v", err)
	}
	if loop.StartIter != 2 || loop.StartPhase != 0 {
		t.Fatalf("iteration-boundary resume = iter %d phase %d, want 2/0",
			loop.StartIter, loop.StartPhase)
	}
}

func TestPhaseCheckpointDoesNotAdvanceWithoutValidatedScanReport(t *testing.T) {
	root := t.TempDir()
	wf := evolveScanWorkflow()
	var warnings []string
	err := phaseCheckpointHook(
		runOpts{root: root, mode: "engineering", lifecycle: "mvp"},
		wf,
		&runBudget{},
		newPhaseOutputLedger(),
		func(message string) { warnings = append(warnings, message) },
	)(1, 1, 1, 0)
	if err == nil || !strings.Contains(err.Error(), "scan report is unavailable") {
		t.Fatalf("missing report checkpoint error = %v", err)
	}
	if _, found, err := persist.Load(checkpointPath(root)); err != nil || found {
		t.Fatalf("unsafe checkpoint written without scan report: found=%v err=%v", found, err)
	}
	if len(warnings) != 1 || !strings.Contains(warnings[0], "checkpoint refused") {
		t.Fatalf("missing fail-closed warning: %v", warnings)
	}
}

func contractedScanPhase() asset.Phase {
	return asset.Phase{
		Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: asset.ScanContractEvolveV1,
	}
}

func evolveScanWorkflow() asset.Workflow {
	return asset.Workflow{Stage: "evolve", Phases: []asset.Phase{
		contractedScanPhase(),
		{Name: "gap-analysis", Agent: "architect", Readonly: true, Effect: "propose"},
		{Name: "implement", Agent: "implementer", Effect: "mutate"},
	}}
}

func scanEvidenceRepo(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "evidence"), 0o755); err != nil {
		t.Fatal(err)
	}
	for _, name := range evolvescan.Dimensions() {
		data := strings.Repeat(name+" evidence ", 20)
		if err := os.WriteFile(filepath.Join(root, "evidence", name+".txt"), []byte(data), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func scanReport(t *testing.T, depth string, finding bool) string {
	t.Helper()
	names := evolvescan.Dimensions()
	if depth != evolvescan.DepthThorough {
		names = names[:1]
	}
	report := evolvescan.Report{
		Version: evolvescan.ContractV1, Depth: depth,
		Opportunities: []evolvescan.Opportunity{},
	}
	for _, name := range names {
		status := evolvescan.StatusClear
		if finding && name == "code" {
			status = evolvescan.StatusFinding
		}
		report.Dimensions = append(report.Dimensions, evolvescan.Dimension{
			Name: name, Status: status,
			Evidence: []evolvescan.Evidence{{
				Path: "evidence/" + name + ".txt", Line: 1,
				Detail: strings.Repeat("evidence for "+name+" ", 12),
			}},
		})
	}
	if finding {
		report.Opportunities = []evolvescan.Opportunity{{
			ID: "code-finding", Dimension: "code", Title: "address code finding",
			Evidence: []evolvescan.Evidence{{
				Path: "evidence/code.txt", Line: 1, Detail: "direct code evidence",
			}},
			Obvious: true, CandidateTask: "Implement the focused fix and regression coverage.",
		}}
	}
	data, err := json.Marshal(report)
	if err != nil {
		t.Fatal(err)
	}
	return "scan narrative\n" + evolvescan.MarkerPrefix + string(data)
}
