package main

import (
	"bytes"
	"math"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/trace"
)

// realClaudeJSON is the shape claude `-p --output-format json` actually emits: a
// single object carrying the BILLED total_cost_usd plus a human-readable result. The
// cost value is the real measured charge from a live run.
const realClaudeJSON = `{"type":"result","total_cost_usd":0.0544035,"result":"done editing main.go","is_error":false}`

// parseClaudeCostUsd on a real claude envelope returns the exact billed dollars with
// ok=true — the value the cost sink converts to microdollars and traces.
func TestParseClaudeCostUsd_RealJSON(t *testing.T) {
	usd, ok := parseClaudeCostUsd(realClaudeJSON)
	if !ok {
		t.Fatal("a real claude JSON envelope must parse ok=true")
	}
	if math.Abs(usd-0.0544035) > 1e-9 {
		t.Errorf("total_cost_usd = %v, want 0.0544035", usd)
	}
}

// HONESTY branch 1: non-JSON output (echo/dry/stub, or a multi-line agent log) is NOT
// a claude envelope -> ok=false, so the caller emits no cost event and never fabricates 0.
func TestParseClaudeCostUsd_NonJSONIsNotOK(t *testing.T) {
	for _, out := range []string{"implementer balanced", "", "   ", "not json at all", "line1\nline2"} {
		if usd, ok := parseClaudeCostUsd(out); ok || usd != 0 {
			t.Errorf("non-JSON %q must yield (0,false), got (%v,%v)", out, usd, ok)
		}
	}
}

// HONESTY branch 2: valid JSON that simply has NO total_cost_usd field -> ok=false
// (a *float64 pointer distinguishes absent from a real 0.0), never an invented cost.
func TestParseClaudeCostUsd_MissingFieldIsNotOK(t *testing.T) {
	if usd, ok := parseClaudeCostUsd(`{"type":"result","result":"ok"}`); ok || usd != 0 {
		t.Errorf("JSON without total_cost_usd must yield (0,false), got (%v,%v)", usd, ok)
	}
}

// HONESTY branch 3: a legitimate billed cost of EXACTLY 0.0 is real data (ok=true),
// distinguished from an absent field by the pointer — a free/cached call still counts.
func TestParseClaudeCostUsd_ExplicitZeroIsOK(t *testing.T) {
	usd, ok := parseClaudeCostUsd(`{"total_cost_usd":0.0,"result":"cached"}`)
	if !ok || usd != 0 {
		t.Errorf("an explicit total_cost_usd:0.0 is real data; want (0,true), got (%v,%v)", usd, ok)
	}
}

// HONESTY branch 4: a non-finite cost (NaN/Inf, however it arose) is not a real charge
// -> ok=false. JSON has no NaN/Inf literal, so this drives the guard via the parsed value.
func TestParseClaudeCostUsd_NonFiniteIsNotOK(t *testing.T) {
	// 1e400 overflows float64 to +Inf on unmarshal — the non-finite guard must reject it.
	if usd, ok := parseClaudeCostUsd(`{"total_cost_usd":1e400}`); ok || usd != 0 {
		t.Errorf("a non-finite cost must yield (0,false), got (%v,%v)", usd, ok)
	}
}

// unwrapClaudeResult pulls the human-readable result out of a claude envelope for the
// log line, and returns NON-claude output verbatim (so the generic log path is unchanged).
func TestUnwrapClaudeResult(t *testing.T) {
	if got := unwrapClaudeResult(realClaudeJSON); got != "done editing main.go" {
		t.Errorf("unwrap result = %q, want the result string", got)
	}
	for _, raw := range []string{"plain echo output", "{not json", `{"no":"result"}`} {
		if got := unwrapClaudeResult(raw); got != raw {
			t.Errorf("non-result output %q must pass through verbatim, got %q", raw, got)
		}
	}
}

// costEmitter converts billed USD to integer microdollars and emits one kind:"agent"
// cost event on the trace — the value the scorecard's --trace reader aggregates.
func TestCostEmitter_EmitsMicrodollarAgentEvent(t *testing.T) {
	var buf bytes.Buffer
	sink := costEmitter(trace.NewTracer(&buf), func(string) {})
	sink("implementer", 0.0544035)
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "agent" || ev.Name != "implementer" || ev.Status != "ok" {
		t.Errorf("cost event = %+v, want agent/implementer/ok", ev)
	}
	// 0.0544035 USD x 1e6 = 54403.5, math.Round (half away from zero) -> 54404 micros.
	if want := int64(math.Round(0.0544035 * 1e6)); ev.CostUsdMicros != want {
		t.Errorf("CostUsdMicros = %d, want %d (round(0.0544035*1e6))", ev.CostUsdMicros, want)
	}
}

// HONESTY end-to-end: with agentCmd=echo (NOT claude), the executor runs but echo's
// output is not a claude JSON envelope, so the Observe sink parses ok=false and NO cost
// event is emitted — the cost path never fabricates a 0 for a non-billing executor.
// Also proves echo is given no claude-only flags (no --output-format json on the argv).
func TestAgentExecutor_EchoEmitsNoCostEvent(t *testing.T) {
	var buf bytes.Buffer
	costCalls := 0
	costSink := func(phase string, usd float64) {
		costCalls++
		costEmitter(trace.NewTracer(&buf), func(string) {})(phase, usd)
	}
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, func(string) {}, costSink, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
	}
	// echo gets NO claude-only flags (the cost-JSON flag is claude-gated).
	argv := strings.Join(ce.Build(asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced"), " ")
	if strings.Contains(argv, "--output-format") {
		t.Errorf("echo (a stub) must NOT receive --output-format json; argv=%s", argv)
	}
	if err := ce.Execute(asset.Phase{Name: "implementer"}, "balanced"); err != nil {
		t.Fatalf("Execute(echo): %v", err)
	}
	if costCalls != 0 {
		t.Errorf("echo output is not claude JSON -> the cost sink must NOT fire; got %d calls", costCalls)
	}
	if buf.Len() != 0 {
		t.Errorf("no cost event must be traced for a non-billing executor; trace=%q", buf.String())
	}
}

// claude gets the cost-bearing flag: a claude-family command's argv carries
// --output-format json (so it emits the total_cost_usd envelope this CLI parses),
// alongside the existing --permission-mode/--model. This is the Build half of the wiring.
func TestAgentExecutor_ClaudeGetsOutputFormatJSON(t *testing.T) {
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", agentPermission: "acceptEdits", root: t.TempDir()}, func(string) {}, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	argv := strings.Join(ce.Build(asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced"), " ")
	if !strings.Contains(argv, "--output-format json") {
		t.Errorf("claude must carry --output-format json for cost telemetry; argv=%s", argv)
	}
}

// The positive sink path, exercised through the Observe closure agentExecutor installs
// for claude: feeding a REAL claude JSON envelope to Observe must drive the costSink with
// the billed dollars (and thus a traced cost event). Driven directly (no real claude
// spawn) by invoking the installed Observe hook with a fixture — proving the parse->sink
// wiring without burning a live call.
func TestAgentExecutor_ClaudeObserveDrivesCostSink(t *testing.T) {
	var buf bytes.Buffer
	var gotPhase string
	var gotUsd float64
	costSink := func(phase string, usd float64) {
		gotPhase, gotUsd = phase, usd
		costEmitter(trace.NewTracer(&buf), func(string) {})(phase, usd)
	}
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, costSink, nil)
	ce := ex.(orchestrator.CommandExecutor)
	if ce.Observe == nil {
		t.Fatal("claude executor must install an Observe sink")
	}
	ce.Observe("implementer", realClaudeJSON)
	if gotPhase != "implementer" || math.Abs(gotUsd-0.0544035) > 1e-9 {
		t.Errorf("Observe(real JSON) must drive costSink(implementer, 0.0544035); got (%q,%v)", gotPhase, gotUsd)
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "agent" || ev.CostUsdMicros == 0 {
		t.Errorf("a real claude cost must trace a non-zero agent cost event; got %+v", ev)
	}
}

// buildLoop must thread the costSink into the agent executor so a real claude phase's
// billed cost reaches the trace. With agentCmd=claude the produced CommandExecutor
// carries an Observe sink (the claude cost hook); the wiring is the same tracer execLoop
// owns. Driving the installed Observe with a real claude JSON fixture must emit a
// kind:"agent" cost event — proving buildLoop->executor->costSink->trace is connected.
func TestBuildLoop_ThreadsCostSinkIntoExecutor(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	var buf bytes.Buffer
	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	wf := asset.Workflow{Stage: "evolve", Stop: asset.StopCondition{Type: "external"}}
	loop := buildLoop(wf, o, 1, func(string) {}, costEmitter(trace.NewTracer(&buf), func(string) {}))

	ce, ok := loop.Engine.Exec.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("buildLoop executor must be a CommandExecutor, got %T", loop.Engine.Exec)
	}
	if ce.Observe == nil {
		t.Fatal("a claude executor built by buildLoop must carry the cost Observe sink")
	}
	ce.Observe("implementer", realClaudeJSON)
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "agent" || ev.Name != "implementer" || ev.CostUsdMicros == 0 {
		t.Errorf("cost sink must emit a non-zero agent cost event via the loop tracer; got %+v", ev)
	}
}

// The cost path is PER-PHASE (sink-driven inside RunFrom), NOT per-iteration: the
// checkpointHook's iteration event must therefore carry NO cost field, so the existing
// iteration-event shape is byte-for-byte preserved. Cost and iteration events coexist in
// the same trace, distinguished by kind — this pins that the iteration event stays clean.
func TestCheckpointHook_IterationEventCarriesNoCost(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	hook := checkpointHook(runOpts{root: root, mode: "balanced"}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), func(string) {})

	hook(2, converge.Signals{RoadmapCompletion: 0.75, GatesGreen: true}, 4200)

	if strings.Contains(buf.String(), "cost_usd_micros") {
		t.Errorf("an iteration event must not carry cost (cost is per-phase); got %q", buf.String())
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "iteration" || ev.CostUsdMicros != 0 || ev.DurationMs != 4200 {
		t.Errorf("iteration event = %+v, want kind=iteration, no cost, duration 4200", ev)
	}
}
