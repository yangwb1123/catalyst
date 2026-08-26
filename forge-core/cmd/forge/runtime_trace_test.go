package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/trace"
)

func TestWireEngineTraceMapsRuntimeEventsAndPreservesObserver(t *testing.T) {
	var output bytes.Buffer
	tracer := trace.NewTracer(&output)
	tracer.RunID = "runtime-trace-run"
	observed := 0
	engine := orchestrator.Engine{OnRuntimeEvent: func(string, string, string, string, string) { observed++ }}
	wireEngineTrace(&engine, tracer, func(string) {})

	for _, event := range runtimeTraceFixtureEvents() {
		engine.OnRuntimeEvent(event.kind, event.name, event.status, event.errorType, event.detail)
	}

	events := decodeRuntimeTrace(t, output.String())
	if observed != 4 || len(events) != 4 {
		t.Fatalf("observer calls=%d trace events=%d, want 4/4", observed, len(events))
	}
	wantKinds := []string{"decision", "overload_backoff", "stale_increment", "error"}
	wantStatuses := []string{"ok", "retry", "stale", "failed"}
	for i, event := range events {
		if event.Seq != i+1 || event.Kind != wantKinds[i] || event.Status != wantStatuses[i] {
			t.Errorf("event[%d]=%+v, want seq=%d kind=%s status=%s", i, event, i+1, wantKinds[i], wantStatuses[i])
		}
		if event.Format != trace.FormatV1 || event.RunID != tracer.RunID {
			t.Errorf("event[%d] format/run=%q/%q", i, event.Format, event.RunID)
		}
		if event.Model != "" || event.CostUsdMicros != 0 {
			t.Errorf("runtime event must not masquerade as billed agent work: %+v", event)
		}
	}
	if strings.Contains(events[3].Detail, "private-value") || !strings.Contains(events[3].Detail, "[REDACTED]") {
		t.Errorf("error detail was not redacted: %q", events[3].Detail)
	}
}

func TestWireRuntimeTraceIgnoresUnknownKind(t *testing.T) {
	var output bytes.Buffer
	engine := orchestrator.Engine{}
	wireRuntimeTrace(&engine, trace.NewTracer(&output), func(string) {})
	engine.OnRuntimeEvent("future", "", "", "", "")
	if output.Len() != 0 {
		t.Fatalf("unknown runtime kind must not fabricate a trace event: %q", output.String())
	}
}

func TestWireRuntimeTraceWriteFailureWarnsWithoutPanicking(t *testing.T) {
	var logs []string
	engine := orchestrator.Engine{
		Exec: &runtimeTraceSequenceExecutor{errs: []error{&orchestrator.ExecError{
			Phase: "implement", Kind: orchestrator.KindOverloaded,
		}}},
		MaxRetries: 1,
		Sleep:      func(time.Duration) {},
	}
	wireRuntimeTrace(&engine, trace.NewTracer(runtimeTraceFailWriter{}), func(line string) {
		logs = append(logs, line)
	})
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "implement", Agent: "implementer",
	}}}
	if err := engine.Run(wf, "balanced"); err != nil {
		t.Fatalf("trace write failure changed workflow outcome: %v", err)
	}
	if len(logs) != 1 {
		t.Fatalf("trace write failure logs=%v, want one backoff warning", logs)
	}
	for _, line := range logs {
		if !strings.Contains(line, "WARNING trace emit failed") {
			t.Errorf("trace warning=%q", line)
		}
	}
}

func TestWireEngineTraceCapturesRealOverloadLifecycle(t *testing.T) {
	var output bytes.Buffer
	executor := &runtimeTraceSequenceExecutor{errs: []error{&orchestrator.ExecError{
		Phase: "implement", Kind: orchestrator.KindOverloaded,
	}}}
	engine := orchestrator.Engine{Exec: executor, MaxRetries: 1, Sleep: func(time.Duration) {}}
	wireEngineTrace(&engine, trace.NewTracer(&output), func(string) {})
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{{
		Name: "implement", Agent: "implementer",
	}}}
	if err := engine.Run(wf, "balanced"); err != nil {
		t.Fatalf("run after transient overload: %v", err)
	}
	events := decodeRuntimeTrace(t, output.String())
	if len(events) != 1 || events[0].Kind != "overload_backoff" || events[0].Status != "retry" {
		t.Fatalf("runtime trace events=%+v, want one overload backoff", events)
	}
}

func TestWireEngineTraceCapturesRealStaleIncrement(t *testing.T) {
	var output bytes.Buffer
	engine := orchestrator.Engine{Exec: orchestrator.DryRunExecutor{}}
	wireEngineTrace(&engine, trace.NewTracer(&output), func(string) {})
	loop := orchestrator.NewLoopEngine(engine, asset.StopCondition{Type: "external"},
		func() converge.Signals { return converge.Signals{RoadmapCompletion: 0.5} }, 3, 1, nil)
	wf := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{{
		Name: "scan", Agent: "planner", Readonly: true,
	}}}
	if outcome, err := loop.Run(wf, "balanced"); err != nil || outcome.Iterations != 2 {
		t.Fatalf("loop outcome=%+v err=%v", outcome, err)
	}
	events := decodeRuntimeTrace(t, output.String())
	if len(events) != 1 || events[0].Kind != "stale_increment" || events[0].Name != "iter 2" {
		t.Fatalf("runtime trace events=%+v, want one real stale increment", events)
	}
}

type runtimeTraceInput struct {
	kind, name, status, errorType, detail string
}

func runtimeTraceFixtureEvents() []runtimeTraceInput {
	return []runtimeTraceInput{
		{kind: "decision", name: "review", detail: "loop-back committed"},
		{kind: "overload_backoff", name: "implement", detail: "backoff 2s before retry 1/3"},
		{kind: "stale_increment", name: "iter 2", detail: "count=1/2 roadmap=50% gates_green=false"},
		{kind: "error", name: "implement", errorType: "config", status: "failed", detail: "ANTHROPIC_API_KEY=private-value"},
	}
}

func decodeRuntimeTrace(t *testing.T, jsonl string) []trace.Event {
	t.Helper()
	lines := strings.Split(strings.TrimSpace(jsonl), "\n")
	events := make([]trace.Event, 0, len(lines))
	for _, line := range lines {
		var event trace.Event
		if err := json.Unmarshal([]byte(line), &event); err != nil {
			t.Fatalf("decode trace line %q: %v", line, err)
		}
		events = append(events, event)
	}
	return events
}

type runtimeTraceFailWriter struct{}

func (runtimeTraceFailWriter) Write([]byte) (int, error) {
	return 0, errors.New("trace sink unavailable")
}

type runtimeTraceSequenceExecutor struct {
	errs  []error
	calls int
}

func (executor *runtimeTraceSequenceExecutor) Execute(context.Context, asset.Phase, string) error {
	index := executor.calls
	executor.calls++
	if index < len(executor.errs) {
		return executor.errs[index]
	}
	return nil
}
