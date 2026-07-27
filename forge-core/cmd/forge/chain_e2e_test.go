package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/persist"
)

func humanChainWorkflow(stage, next string, agents ...string) asset.Workflow {
	phases := make([]asset.Phase, len(agents))
	for i, agent := range agents {
		phases[i] = asset.Phase{Name: agent, Agent: agent}
	}
	return asset.Workflow{
		Stage: stage, Phases: phases,
		Stop: asset.StopCondition{
			Type: "human_gate", HumanApproval: "required",
			OnApproved: asset.OnApproved{NextStage: next},
		},
	}
}

func externalChainWorkflow() asset.Workflow {
	return asset.Workflow{
		Stage: "evolve",
		Phases: []asset.Phase{
			{Name: "scan", Agent: "explorer", Readonly: true, Effect: "observe"},
			{Name: "implementation-boundary", Agent: "custom-writer", Effect: "mutate"},
		},
		Stop: asset.StopCondition{Type: "external"},
	}
}

func writeChainAsset(t *testing.T, root, name string, wf asset.Workflow) {
	t.Helper()
	var body strings.Builder
	fmt.Fprintf(&body, "stage: %s\n", wf.Stage)
	if len(wf.Phases) == 0 {
		body.WriteString("phases:\n  - name: observer\n    agent: explorer\n    required_gates: []\n")
	} else {
		body.WriteString("phases:\n")
		for _, phase := range wf.Phases {
			fmt.Fprintf(&body, "  - name: %s\n    agent: %s\n    readonly: %t\n    effect: %s\n    required_gates: []\n",
				phase.Name, phase.Agent, phase.Readonly, phase.Effect)
		}
	}
	fmt.Fprintf(&body, "stop_condition:\n  type: %s\n", wf.Stop.Type)
	if wf.Stop.HumanApproval != "" {
		fmt.Fprintf(&body, "  human_approval: %s\n", wf.Stop.HumanApproval)
	}
	if wf.Stop.OnApproved.NextStage != "" {
		fmt.Fprintf(&body, "  on_approved:\n    next_stage: %s\n", wf.Stop.OnApproved.NextStage)
	}
	dir := filepath.Join(root, ".agent", "workflows")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir workflows: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, name+".yml"), []byte(body.String()), 0o644); err != nil {
		t.Fatalf("write workflow %s: %v", name, err)
	}
}

func approveChainStage(t *testing.T, root, stage string) {
	t.Helper()
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatalf("mkdir .forge: %v", err)
	}
	if err := os.WriteFile(filepath.Join(forgeDir(root), stage+".approved"), nil, 0o644); err != nil {
		t.Fatalf("approve %s: %v", stage, err)
	}
}

func chainOpts(root string) runOpts {
	return runOpts{
		root: root, mode: "balanced", lifecycle: "idea",
		executor: "dry", chain: true, maxChainStages: defaultMaxChainStages,
	}
}

func captureChainOutput(t *testing.T, fn func() int) (int, string) {
	t.Helper()
	oldOut, oldErr := os.Stdout, os.Stderr
	outR, outW, _ := os.Pipe()
	errR, errW, _ := os.Pipe()
	os.Stdout, os.Stderr = outW, errW
	code := fn()
	outW.Close()
	errW.Close()
	os.Stdout, os.Stderr = oldOut, oldErr
	stdout, _ := io.ReadAll(outR)
	stderr, _ := io.ReadAll(errR)
	outR.Close()
	errR.Close()
	return code, string(stdout) + string(stderr)
}

func mustChainState(t *testing.T, root string) chainState {
	t.Helper()
	state, found, err := loadChainState(root)
	if err != nil || !found {
		t.Fatalf("chain state: found=%v err=%v", found, err)
	}
	return state
}

func TestChainApprovedFlagIsScopedAndHeldGateReturnsNonZero(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("design", "review")
	writeChainAsset(t, root, "review", humanChainWorkflow("review", "build"))
	o := chainOpts(root)
	o.approved = true

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != exitChainIncomplete {
		t.Fatalf("chain exit = %d, want %d for held approval; output:\n%s", code, exitChainIncomplete, out)
	}
	for _, want := range []string{"awaiting human approval", "stage=review", "forge approve review"} {
		if !strings.Contains(out, want) {
			t.Errorf("held-gate output missing %q:\n%s", want, out)
		}
	}
	state := mustChainState(t, root)
	if state.Status != "waiting_approval" || state.CurrentStage != "review" {
		t.Errorf("state = %+v, want waiting_approval at review", state)
	}
	if strings.Join(state.CompletedStages, ",") != "design" {
		t.Errorf("completed = %v, want only design", state.CompletedStages)
	}
}

func TestChainGuardFailuresPersistCycleAndLimitDiagnostics(t *testing.T) {
	tests := []struct {
		name       string
		max        int
		secondNext string
		want       string
	}{
		{name: "cycle", max: 5, secondNext: "discover", want: "cycle detected"},
		{name: "limit", max: 2, secondNext: "review", want: "stage limit 2"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			first := humanChainWorkflow("discover", "design")
			writeChainAsset(t, root, "discover", first)
			writeChainAsset(t, root, "design", humanChainWorkflow("design", tc.secondNext))
			writeChainAsset(t, root, "review", humanChainWorkflow("review", ""))
			approveChainStage(t, root, "discover")
			approveChainStage(t, root, "design")
			o := chainOpts(root)
			o.maxChainStages = tc.max

			code, out := captureChainOutput(t, func() int {
				return execEngine(context.Background(), first, o)
			})
			if code != 1 || !strings.Contains(out, tc.want) {
				t.Fatalf("exit=%d output=%q, want failure containing %q", code, out, tc.want)
			}
			state := mustChainState(t, root)
			if state.Status != "failed" || !strings.Contains(state.Reason, tc.want) {
				t.Errorf("state = %+v, want persisted %q failure", state, tc.want)
			}
			if len(state.CompletedStages) != 2 {
				t.Errorf("completed = %v, want two stages before guard failure", state.CompletedStages)
			}
		})
	}
}

func TestChainCTOBuildHaltIsPersistedEndToEnd(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("review", "build")
	approveChainStage(t, root, "review")
	o := chainOpts(root)
	o.mode = "cto"

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != 0 || !strings.Contains(out, "build=halt") {
		t.Fatalf("cto chain exit=%d output=%q, want clean policy halt", code, out)
	}
	state := mustChainState(t, root)
	if state.Status != "halted" || state.CurrentStage != "build" {
		t.Errorf("state = %+v, want halted before build", state)
	}
}

func TestDirectCTOBuildHaltDoesNotExecuteFirstStage(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("build", "")
	o := chainOpts(root)
	o.chain = false
	o.mode = "cto"

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != 0 || !strings.Contains(out, "halted before execution") {
		t.Fatalf("direct cto build exit=%d output=%q, want clean pre-execution halt", code, out)
	}
	if strings.Contains(out, "dry-run agent") || strings.Contains(out, "workflow completed") {
		t.Fatalf("direct cto build executed despite build=halt: %s", out)
	}
	state := mustChainState(t, root)
	if state.Status != "halted" || state.CurrentStage != "build" {
		t.Errorf("state = %+v, want halted before build", state)
	}
}

func TestChainBuildHandsOffToRealEvolveLoopEngine(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("build", "evolve")
	writeChainAsset(t, root, "evolve", externalChainWorkflow())
	approveChainStage(t, root, "build")
	o := chainOpts(root)
	o.mode = "explorer" // idea+explorer resolves evolve max-iter to 2

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != 0 {
		t.Fatalf("chain exit=%d, want clean evolve bound; output:\n%s", code, out)
	}
	for _, want := range []string{"entering evolve LoopEngine", "iteration 2/2", "ran to safety bound"} {
		if !strings.Contains(out, want) {
			t.Errorf("evolve handoff output missing %q:\n%s", want, out)
		}
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found || cp.FormatVersion != persist.CheckpointFormatCurrent ||
		cp.Workflow != "evolve" || cp.WorkflowDigest == "" ||
		cp.Mode != "explorer" || cp.Lifecycle != "idea" || cp.Iteration != 2 {
		t.Errorf("evolve checkpoint = %+v found=%v err=%v, want bound explorer/idea iteration 2", cp, found, err)
	}
	state := mustChainState(t, root)
	if state.Status != "completed" || strings.Join(state.CompletedStages, ",") != "build,evolve" {
		t.Errorf("state = %+v, want completed build→evolve", state)
	}
	if state.RunID == "" {
		t.Error("chain state must retain the shared tracer run_id")
	}
}

func TestChainMaxAgentCallsIsSharedIntoEvolveIterations(t *testing.T) {
	root := t.TempDir()
	first := humanChainWorkflow("build", "evolve", "planner")
	writeChainAsset(t, root, "evolve", externalChainWorkflow())
	approveChainStage(t, root, "build")
	o := chainOpts(root)
	o.mode, o.maxAgentCalls = "explorer", 2

	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), first, o)
	})
	if code != 1 || !strings.Contains(out, "execution 3 after 2 completed") {
		t.Fatalf("shared cap exit=%d output=%q, want third chain-wide spawn refused", code, out)
	}
	state := mustChainState(t, root)
	if state.Status != "failed" || state.CurrentStage != "evolve" || state.AgentCalls != 2 {
		t.Errorf("state = %+v, want failed evolve after two actual calls", state)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found || cp.Iteration != 1 {
		t.Errorf("checkpoint = %+v found=%v err=%v, want only evolve iteration 1 complete", cp, found, err)
	}
}

func TestChainAgentCounterIsConcurrentSafe(t *testing.T) {
	var wg sync.WaitGroup
	counter := &chainAgentCounter{}
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			counter.charge(0)
		}()
	}
	wg.Wait()
	if got := counter.count(); got != 100 {
		t.Errorf("concurrent count = %d, want 100", got)
	}
}

func TestStatusDisplaysDurableChainState(t *testing.T) {
	root := t.TempDir()
	want := chainState{
		RunID: "run-chain", Status: "waiting_approval", CurrentStage: "review",
		CompletedStages: []string{"discover", "design"}, Reason: "awaiting approval", AgentCalls: 7,
	}
	if err := saveChainState(root, want); err != nil {
		t.Fatal(err)
	}
	text := captureStdout(t, func() { cmdStatus([]string{"--root", root}) })
	for _, part := range []string{"status=waiting_approval", "current=review", "discover → design", "agent_calls=7"} {
		if !strings.Contains(text, part) {
			t.Errorf("status text missing %q:\n%s", part, text)
		}
	}
	raw := captureStdout(t, func() { cmdStatus([]string{"--root", root, "--json"}) })
	var got statusJSON
	if err := json.Unmarshal([]byte(raw), &got); err != nil {
		t.Fatalf("decode status JSON: %v\n%s", err, raw)
	}
	if got.Chain == nil || got.Chain.Status != want.Status || got.Chain.CurrentStage != want.CurrentStage {
		t.Errorf("status JSON chain = %+v, want %+v", got.Chain, want)
	}
}

func TestChainStateRejectsUnknownFormatAndStatusSurfacesIt(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(chainStatePath(root), []byte(`{"_format":"forgeos.chain-state.v99","status":"running"}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, found, err := loadChainState(root); !found || err == nil || !strings.Contains(err.Error(), "unsupported chain state format") {
		t.Fatalf("unknown format: found=%v err=%v, want fail-closed", found, err)
	}
	out := captureStdout(t, func() { cmdStatus([]string{"--root", root}) })
	if !strings.Contains(out, "chain: unreadable") || !strings.Contains(out, "v99") {
		t.Errorf("status must surface unknown chain generation:\n%s", out)
	}
}
