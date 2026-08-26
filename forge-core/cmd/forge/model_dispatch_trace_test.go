package main

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/trace"
)

func TestFinalModelDecisionUsesPlannerHintOnceAtDispatch(t *testing.T) {
	engine, output := finalModelTraceEngine(t, 0)
	if err := engine.Run(finalModelWorkflow(), "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	events := decodeRuntimeTrace(t, output.String())
	decisions := modelTraceEvents(events, "decision")
	agents := modelTraceEvents(events, "agent")
	if len(decisions) != 1 || decisions[0].Detail != "final_model=opus dispatch=command" {
		t.Fatalf("decisions=%+v, want one final planner-adjusted dispatch", decisions)
	}
	if len(agents) != 1 || agents[0].Model != "opus" {
		t.Fatalf("agent events=%+v, want one cost observation stamped opus", agents)
	}
}

func TestFinalModelDecisionEmitsOncePerRetryDispatch(t *testing.T) {
	engine, output := finalModelTraceEngine(t, 2)
	engine.MaxRetries = 2
	engine.Sleep = func(time.Duration) {}
	if err := engine.Run(finalModelWorkflow(), "balanced"); err != nil {
		t.Fatalf("Run after overload retries: %v", err)
	}
	events := decodeRuntimeTrace(t, output.String())
	decisions := modelTraceEvents(events, "decision")
	if len(decisions) != 3 {
		t.Fatalf("decision events=%d, want one for each of three dispatches", len(decisions))
	}
	for _, event := range decisions {
		if event.Detail != "final_model=opus dispatch=command" {
			t.Errorf("retry decision detail=%q", event.Detail)
		}
	}
	if agents := modelTraceEvents(events, "agent"); len(agents) != 1 {
		t.Fatalf("successful cost observations=%d, want 1 without decision replay", len(agents))
	}
}

func TestNonClaudeCommandDoesNotEmitFinalModelDecision(t *testing.T) {
	root := t.TempDir()
	engine, output := finalModelTraceEngineFor(t, root, "true")
	if err := engine.Run(finalModelWorkflow(), "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	if output.Len() != 0 {
		t.Fatalf("non-Claude command fabricated model trace: %s", output.String())
	}
}

func TestInvalidCommandOptionsDoNotEmitFinalModelDecision(t *testing.T) {
	engine, output := finalModelTraceEngine(t, 0)
	executor, ok := engine.Exec.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor type %T, want CommandExecutor", engine.Exec)
	}
	executor.MaxOutputBytes = -1
	engine.Exec = executor
	if err := engine.Run(finalModelWorkflow(), "balanced"); err == nil {
		t.Fatal("invalid output cap must fail before dispatch")
	}
	if output.Len() != 0 {
		t.Fatalf("pre-dispatch refusal fabricated model trace: %s", output.String())
	}
}

func finalModelTraceEngine(t *testing.T, overloads int) (orchestrator.Engine, *bytes.Buffer) {
	t.Helper()
	root := t.TempDir()
	agent := writeModelTraceClaude(t, root, overloads)
	return finalModelTraceEngineFor(t, root, agent)
}

func finalModelTraceEngineFor(t *testing.T, root, agent string) (orchestrator.Engine, *bytes.Buffer) {
	t.Helper()
	plans := newPhaseOutputLedger()
	plans.record("planner", "TASK_LIST:\n- [ ] T001: implement — acceptance: pass — files: a.go — depends_on: none — model: opus — roadmap: v1")
	var output bytes.Buffer
	tracer := trace.NewTracer(&output)
	tracer.RunID = "final-model-run"
	logln := func(string) {}
	options := tracedEngineBuildOptions(tracer, logln)
	engine, _, _, _, _ := buildRunEngineWithPhaseOutput(
		finalModelWorkflow(), runOpts{root: root, executor: "command", agentCmd: agent, mode: "balanced"},
		logln, costEmitter(tracer, logln), func(string) gate.Result { return gate.Result{OK: true} },
		mode.Policy{}, &runBudget{}, "", nil, nil, nil, plans, options...,
	)
	return engine, &output
}

func finalModelWorkflow() asset.Workflow {
	return asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "implement", Agent: "implementer",
	}}}
}

func writeModelTraceClaude(t *testing.T, root string, overloads int) string {
	t.Helper()
	path := filepath.Join(root, "claude")
	script := fmt.Sprintf(`#!/bin/sh
count=0
if [ -f "$0.calls" ]; then IFS= read -r count < "$0.calls"; fi
count=$((count + 1))
printf '%%s' "$count" > "$0.calls"
if [ "$count" -le %d ]; then
  printf '%%s\n' '{"type":"result","subtype":"error_during_execution","is_error":true,"api_error_status":529,"result":"overloaded"}'
  exit 1
fi
printf '%%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"implemented","total_cost_usd":0.01}'
`, overloads)
	if err := os.WriteFile(path, []byte(script), 0o755); err != nil {
		t.Fatalf("write claude stub: %v", err)
	}
	return path
}

func modelTraceEvents(events []trace.Event, kind string) []trace.Event {
	filtered := make([]trace.Event, 0, len(events))
	for _, event := range events {
		if strings.EqualFold(event.Kind, kind) {
			filtered = append(filtered, event)
		}
	}
	return filtered
}
