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

// parseReviewerVerdict on the two contracted last lines returns the NORMALIZED token with
// ok=true — the signal Engine.AgentVerdict reads back to drive (or not) a loop-back. The
// findings written ABOVE the VERDICT line are ignored; only the last line decides.
func TestParseReviewerVerdict_ApproveAndRequestChanges(t *testing.T) {
	approve := "## Review\n- file.go:10 nit: rename\nLGTM overall.\nVERDICT: APPROVE"
	if v, ok := parseReviewerVerdict(approve); !ok || v != VerdictApprove {
		t.Errorf("a trailing VERDICT: APPROVE must parse to (APPROVE,true); got (%q,%v)", v, ok)
	}
	changes := "## Review\n- main.go:42 HIGH: nil deref, guard it\nVERDICT: REQUEST_CHANGES"
	if v, ok := parseReviewerVerdict(changes); !ok || v != VerdictRequestChanges {
		t.Errorf("a trailing VERDICT: REQUEST_CHANGES must parse to (REQUEST_CHANGES,true); got (%q,%v)", v, ok)
	}
}

// A trailing blank line (or whitespace) after the VERDICT line must NOT mask it — the
// last NON-EMPTY line is what counts, so a stray newline the agent appended is tolerated.
func TestParseReviewerVerdict_TrailingBlankToleratedAndTrimmed(t *testing.T) {
	if v, ok := parseReviewerVerdict("findings...\nVERDICT: APPROVE\n\n  "); !ok || v != VerdictApprove {
		t.Errorf("a trailing blank line must not mask the verdict; got (%q,%v)", v, ok)
	}
}

// HONESTY (mirrors the cost parser's branches): a last line that is NOT exactly one of the
// two contracted forms yields ok=false — a missing, wrapped (backtick/quote/bullet), or
// mid-text verdict is "no signal", and the orchestrator then fails open (proceeds). The
// parser NEVER defaults to a verdict it did not literally see.
func TestParseReviewerVerdict_MalformedOrMissingIsNotOK(t *testing.T) {
	for _, out := range []string{
		"",                                 // empty
		"   ",                              // blank
		"just a review with no verdict",    // no verdict line
		"VERDICT: APPROVE\nbut wait, more", // verdict not on the LAST line
		"`VERDICT: APPROVE`",               // wrapped in backticks (not top-aligned)
		"- VERDICT: REQUEST_CHANGES",       // bulleted (wrapped)
		"VERDICT: MAYBE",                   // unknown token
		"verdict: approve",                 // wrong case (exact match only)
		"VERDICT:APPROVE",                  // missing the contracted space
	} {
		if v, ok := parseReviewerVerdict(out); ok || v != "" {
			t.Errorf("malformed/missing verdict %q must yield (\"\",false); got (%q,%v)", out, v, ok)
		}
	}
}

// A claude JSON envelope whose `result` ends in the VERDICT line must be UNWRAPPED first,
// then scanned — proving parseReviewerVerdict sees through the claude envelope exactly as
// the cost parser does, while an echo/stub fake sentinel (non-JSON) is scanned verbatim.
func TestParseReviewerVerdict_UnwrapsClaudeEnvelopeAndEchoSentinel(t *testing.T) {
	envelope := `{"type":"result","total_cost_usd":0.01,"result":"## Review\nlooks good\nVERDICT: APPROVE"}`
	if v, ok := parseReviewerVerdict(envelope); !ok || v != VerdictApprove {
		t.Errorf("a claude envelope's result must be unwrapped before scanning; got (%q,%v)", v, ok)
	}
	// An echo fake (plain text, not JSON) ending in the sentinel must still be caught —
	// this is exactly how the echo state-machine smoke test drives a loop-back.
	if v, ok := parseReviewerVerdict("fake reviewer output\nVERDICT: REQUEST_CHANGES"); !ok || v != VerdictRequestChanges {
		t.Errorf("an echo fake sentinel must be scanned verbatim; got (%q,%v)", v, ok)
	}
}

// The claude --executor=command path must install the 529 overload recognizer, and a stub
// (echo) must NOT — symmetric to the cost Observe / RenderLog wiring. This proves the vendor
// recognizer reaches the generic executor only for claude, so a failing stub can never be
// mistaken for a transient overload.
func TestAgentExecutor_ClaudeInstallsOverloadRecognizer(t *testing.T) {
	claudeEx := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, nil, nil, nil, nil, nil, nil, nil, nil)
	ce := claudeEx.(orchestrator.CommandExecutor)
	if ce.ClassifyOverload == nil {
		t.Fatal("the claude executor must install a ClassifyOverload recognizer")
	}
	// The installed recognizer must be the real 529 classifier: it fires on the strong signal
	// and stays quiet on a normal envelope.
	if !ce.ClassifyOverload(`{"is_error":true,"api_error_status":529}`) {
		t.Error("installed recognizer must classify a real 529 envelope as overloaded")
	}
	if ce.ClassifyOverload(realClaudeJSON) {
		t.Error("installed recognizer must NOT classify a normal envelope as overloaded")
	}

	echoEx := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, func(string) {}, nil, nil, nil, nil, nil, nil, nil, nil)
	if ec := echoEx.(orchestrator.CommandExecutor); ec.ClassifyOverload != nil {
		t.Error("a stub (echo) must NOT get the overload recognizer (back-compat: a failing stub stays KindFailed)")
	}
}

// classifyClaudeOverload on a REAL claude 529 envelope must return true — the strong signal
// is the model's own api_error_status:529. This is the positive case the executor wires onto
// the claude --executor=command path to turn an overload into a retryable KindOverloaded.
func TestClassifyClaudeOverload_RealOverloadEnvelopeIsTrue(t *testing.T) {
	// The shape `claude -p --output-format json` emits when the API returns 529 overloaded_error:
	// a result envelope with is_error and the terminating HTTP status.
	for _, env := range []string{
		`{"type":"result","subtype":"success","is_error":true,"api_error_status":529,"result":"","total_cost_usd":0.0}`,
		`{"type":"result","is_error":true,"api_error_status":529}`,
		// Textual fallback: a failed envelope whose result names the overload, no api_error_status.
		`{"type":"result","is_error":true,"result":"API Error: overloaded_error (Overloaded)"}`,
		// Plain (non-JSON) transport dump carrying the strong marker.
		"Error: 529 Overloaded — please retry",
	} {
		if !classifyClaudeOverload(env) {
			t.Errorf("a real overload signal must classify true; missed %q", env)
		}
	}
}

// HONESTY / NO FALSE POSITIVE (the dangerous direction): normal output, a real business
// failure, and empty output must ALL return false — a non-overload must never be upgraded to a
// transient infinite-backoff retry. This is the decisive "prove it doesn't mis-fire" assertion.
func TestClassifyClaudeOverload_NonOverloadIsFalse(t *testing.T) {
	for _, env := range []string{
		realClaudeJSON, // a normal successful cost envelope
		`{"type":"result","subtype":"success","is_error":false,"result":"done editing main.go"}`,
		// A SUCCESSFUL run that merely MENTIONS overload in prose must not be misread (is_error gate).
		`{"type":"result","is_error":false,"result":"I refactored the 529-line file and noted it was overloaded with helpers"}`,
		// A real terminal business failure (non-zero exit, no overload signal) stays false.
		`{"type":"result","subtype":"error_during_execution","is_error":true,"result":"compile error: undefined symbol"}`,
		`{"type":"result","is_error":true,"api_error_status":401}`, // a different API error is NOT an overload
		`{"type":"result","is_error":true,"api_error_status":500}`, // a 5xx that is not 529 is NOT an overload
		"",                       // empty
		"   ",                    // blank
		"build failed: 2 errors", // plain failure, no marker
		`{"total_cost_usd":0.0005290,"result":""}`, // a cost containing the digits 529 must not trip the token check
		"processed 15290 tokens",                   // a count containing 529 must not trip it
		"not json at all",                          // arbitrary non-JSON without a marker
	} {
		if classifyClaudeOverload(env) {
			t.Errorf("a non-overload output must classify false (fail-closed); mis-fired on %q", env)
		}
	}
}

// costEmitter converts billed USD to integer microdollars and emits one kind:"agent"
// cost event on the trace, STAMPED with the routed model — the values the scorecard's
// --trace reader aggregates and attributes per model.
func TestCostEmitter_EmitsMicrodollarAgentEvent(t *testing.T) {
	var buf bytes.Buffer
	sink := costEmitter(trace.NewTracer(&buf), func(string) {})
	sink("implementer", "sonnet", 0.0544035)
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "agent" || ev.Name != "implementer" || ev.Status != "ok" {
		t.Errorf("cost event = %+v, want agent/implementer/ok", ev)
	}
	if ev.Model != "sonnet" {
		t.Errorf("cost event must be stamped with the routed model; got Model=%q, want sonnet", ev.Model)
	}
	// 0.0544035 USD x 1e6 = 54403.5, math.Round (half away from zero) -> 54404 micros.
	if want := int64(math.Round(0.0544035 * 1e6)); ev.CostUsdMicros != want {
		t.Errorf("CostUsdMicros = %d, want %d (round(0.0544035*1e6))", ev.CostUsdMicros, want)
	}
}

// costEmitter with an empty model (no resolver wired) must OMIT the model field on the
// wire — omitempty keeps a cost event without attribution byte-compatible, never a "".
func TestCostEmitter_EmptyModelOmitted(t *testing.T) {
	var buf bytes.Buffer
	costEmitter(trace.NewTracer(&buf), func(string) {})("implementer", "", 0.01)
	if strings.Contains(buf.String(), `"model"`) {
		t.Errorf("an empty model must be omitted from the cost event JSON; got %q", buf.String())
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Model != "" || ev.CostUsdMicros == 0 {
		t.Errorf("event = %+v, want empty Model + a real cost", ev)
	}
}

// HONESTY end-to-end: with agentCmd=echo (NOT claude), the executor runs but echo's
// output is not a claude JSON envelope, so the Observe sink parses ok=false and NO cost
// event is emitted — the cost path never fabricates a 0 for a non-billing executor.
// Also proves echo is given no claude-only flags (no --output-format json on the argv).
func TestAgentExecutor_EchoEmitsNoCostEvent(t *testing.T) {
	var buf bytes.Buffer
	costCalls := 0
	costSink := func(phase, model string, usd float64) {
		costCalls++
		costEmitter(trace.NewTracer(&buf), func(string) {})(phase, model, usd)
	}
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, func(string) {}, costSink, nil, nil, nil, nil, nil, nil, nil)
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
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", agentPermission: "acceptEdits", root: t.TempDir()}, func(string) {}, nil, nil, nil, nil, nil, nil, nil, nil)
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
	var gotPhase, gotModel string
	var gotUsd float64
	costSink := func(phase, model string, usd float64) {
		gotPhase, gotModel, gotUsd = phase, model, usd
		costEmitter(trace.NewTracer(&buf), func(string) {})(phase, model, usd)
	}
	// phaseModel resolves the routed model for the phase name — the SAME injection
	// point production uses (phaseModelResolver), here a fixed stub so the attribution
	// is deterministic without depending on routing's tier table.
	phaseModel := func(string) string { return "sonnet" }
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, costSink, phaseModel, nil, nil, nil, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	if ce.Observe == nil {
		t.Fatal("claude executor must install an Observe sink")
	}
	ce.Observe("implementer", realClaudeJSON)
	if gotPhase != "implementer" || gotModel != "sonnet" || math.Abs(gotUsd-0.0544035) > 1e-9 {
		t.Errorf("Observe(real JSON) must drive costSink(implementer, sonnet, 0.0544035); got (%q,%q,%v)", gotPhase, gotModel, gotUsd)
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "agent" || ev.Model != "sonnet" || ev.CostUsdMicros == 0 {
		t.Errorf("a real claude cost must trace a non-zero, model-stamped agent cost event; got %+v", ev)
	}
}

// buildLoop must thread the costSink into the agent executor so a real claude phase's
// billed cost reaches the trace, ATTRIBUTED to the phase's routed model. With agentCmd=
// claude the produced CommandExecutor carries an Observe sink (the claude cost hook); the
// wiring is the same tracer execLoop owns. Driving the installed Observe with a real claude
// JSON fixture must emit a kind:"agent" cost event stamped with the model buildLoop's
// internal phaseModelResolver(wf, mode) computes — proving buildLoop -> executor ->
// costSink(model) -> trace is connected end to end, with the model resolved from the wf.
func TestBuildLoop_ThreadsCostSinkIntoExecutor(t *testing.T) {
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	var buf bytes.Buffer
	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	// A wf with the implementer phase so buildLoop's phaseModelResolver attributes the
	// cost to implementer's routed tier (PhaseTier(implementer, balanced)), proving the
	// model flows through the SAME resolver production wires — not a test-only stub.
	wf := asset.Workflow{Stage: "evolve", Stop: asset.StopCondition{Type: "external"},
		Phases: []asset.Phase{{Name: "implementer", Agent: "implementer"}}}
	wantModel := orchestrator.PhaseTier(wf.Phases[0], "balanced")
	loop := buildLoop(wf, o, 1, func(string) {}, costEmitter(trace.NewTracer(&buf), func(string) {}), &runBudget{})

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
	if ev.Model != wantModel {
		t.Errorf("cost event must be attributed to the routed model %q; got Model=%q", wantModel, ev.Model)
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
		trace.NewTracer(&buf), &runBudget{}, func(string) {})

	hook(2, converge.Signals{RoadmapCompletion: 0.75, GatesGreen: true}, 4200)

	// The TRACE iteration event carries no cost (cost_usd_micros is a per-phase agent-event
	// field). The checkpoint's SpentUsdMicros is a SEPARATE concern (checkpoint JSON, not the
	// trace), and stays 0 here since the budget never billed — both invariants hold at once.
	if strings.Contains(buf.String(), "cost_usd_micros") {
		t.Errorf("an iteration event must not carry cost (cost is per-phase); got %q", buf.String())
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "iteration" || ev.CostUsdMicros != 0 || ev.DurationMs != 4200 {
		t.Errorf("iteration event = %+v, want kind=iteration, no cost, duration 4200", ev)
	}
}
