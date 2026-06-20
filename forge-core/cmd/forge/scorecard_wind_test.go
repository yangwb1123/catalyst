package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

// writeTrace drops a trace.jsonl with the given lines under <root>/.forge, returning root.
func writeTrace(t *testing.T, root string, lines ...string) {
	t.Helper()
	dir := filepath.Join(root, ".forge")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	body := ""
	for _, l := range lines {
		body += l + "\n"
	}
	if err := os.WriteFile(filepath.Join(dir, "trace.jsonl"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
}

// distinctScorecardPairs derives the (model, task_type) pairs the run attributes to:
// PhaseTier(p,mode) x agentTaskType[agent], DEDUPED, with unmapped (harness) phases
// SKIPPED. Two implementer phases at the same tier must collapse to one pair, and the
// harness/gate phase must produce none.
func TestDistinctScorecardPairs_DedupAndSkipUnmapped(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "plan", Agent: "planner"},
		{Name: "impl-a", Agent: "implementer"},
		{Name: "impl-b", Agent: "implementer"}, // same (tier, implementation) as impl-a -> dedup
		{Name: "gate", Agent: "harness"},       // no mapping -> skipped entirely
		{Name: "review", Agent: "reviewer"},
	}}
	pairs := distinctScorecardPairs(wf, "balanced")

	// The invariant: the result is exactly the DISTINCT set of (PhaseTier, task_type)
	// over the MAPPED phases — harness skipped, planner+implementer folded to the
	// implementation bucket (deduped if they share a tier), reviewer its own pair. We
	// rebuild that expected set the same way the producer does and compare counts.
	want := map[scorecardPair]bool{}
	for _, p := range []asset.Phase{
		{Agent: "planner"}, {Agent: "implementer"}, {Agent: "reviewer"},
	} {
		tt, _ := taskTypeForAgent(p.Agent)
		want[scorecardPair{model: orchestrator.PhaseTier(p, "balanced"), taskType: tt}] = true
	}

	got := map[scorecardPair]int{}
	for _, p := range pairs {
		got[p]++
		if !want[p] {
			t.Errorf("unexpected pair %+v (harness must be skipped; only mapped roles emit)", p)
		}
		if got[p] > 1 {
			t.Errorf("pair %+v appears %d times; pairs must be distinct", p, got[p])
		}
	}
	if len(got) != len(want) {
		t.Errorf("distinct pairs = %d (%+v), want %d (%+v)", len(got), pairs, len(want), want)
	}
}

// A workflow whose phases are ALL unmapped (e.g. only harness/gate phases) yields no
// pairs — nothing to attribute.
func TestDistinctScorecardPairs_AllUnmappedYieldsNone(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "g1", Agent: "harness"}, {Name: "g2", Agent: "gate"},
	}}
	if pairs := distinctScorecardPairs(wf, "balanced"); len(pairs) != 0 {
		t.Errorf("all-unmapped workflow must yield no pairs; got %+v", pairs)
	}
}

// traceHasModelCost is the gate-on-real-cost signal: TRUE only when the trace carries an
// event with BOTH a non-empty model and a non-zero cost (a real billed claude phase).
func TestTraceHasModelCost_TrueOnModelBearingCost(t *testing.T) {
	root := t.TempDir()
	writeTrace(t, root,
		`{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4200}`,
		`{"seq":2,"kind":"agent","name":"implementer","status":"ok","cost_usd_micros":54404,"model":"sonnet"}`,
	)
	if !traceHasModelCost(tracePath(root)) {
		t.Error("a trace with a model-stamped cost event must report real cost")
	}
}

// HONESTY (dry/echo gate-out): a trace with NO model-bearing cost event — iteration/gate
// events, or agent events lacking model and/or cost — must report FALSE, so a dry/echo run
// skips the wind-down and never fabricates a scorecard row.
func TestTraceHasModelCost_FalseOnNonBillingTraces(t *testing.T) {
	cases := map[string][]string{
		"only iteration/gate (dry loop)": {
			`{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4200}`,
			`{"seq":2,"kind":"gate","name":"test","status":"PASS","duration_ms":900}`,
		},
		"agent cost but no model (legacy/pre-attribution)": {
			`{"seq":1,"kind":"agent","name":"implementer","status":"ok","cost_usd_micros":54404}`,
		},
		"agent model but zero cost (omitempty drops cost)": {
			`{"seq":1,"kind":"agent","name":"implementer","status":"ok","model":"sonnet"}`,
		},
	}
	for name, lines := range cases {
		root := t.TempDir()
		writeTrace(t, root, lines...)
		if traceHasModelCost(tracePath(root)) {
			t.Errorf("%s: must NOT report real cost (gate-out)", name)
		}
	}
}

// A missing trace file reports false (no run cost) and never errors.
func TestTraceHasModelCost_MissingFileIsFalse(t *testing.T) {
	if traceHasModelCost(tracePath(t.TempDir())) {
		t.Error("a missing trace must report no cost, not panic/true")
	}
}

// A blank/corrupt line in the trace must not sink detection: a single readable
// model-bearing cost event still trips it.
func TestTraceHasModelCost_RobustToCorruptLines(t *testing.T) {
	root := t.TempDir()
	writeTrace(t, root,
		``,
		`this is not json`,
		`{ broken tail`,
		`{"seq":4,"kind":"agent","name":"implementer","status":"ok","cost_usd_micros":12000,"model":"opus"}`,
	)
	if !traceHasModelCost(tracePath(root)) {
		t.Error("a readable model-bearing cost event among corrupt lines must still trip detection")
	}
}

// END-TO-END gate-on-real-cost SKIP (坐实不写文件): windDownScorecards on a trace with NO
// real cost (a dry/echo run) must NOT write scorecards.json — it skips before ever
// shelling the producer. This is the honest "dry/echo bills nothing -> persist nothing".
func TestWindDownScorecards_SkipsWhenNoRealCost(t *testing.T) {
	root := t.TempDir()
	writeTrace(t, root,
		`{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4200}`,
	)
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "impl", Agent: "implementer"}}}
	var logs []string
	windDownScorecards(wf, runOpts{root: root, mode: "balanced"}, func(s string) { logs = append(logs, s) })

	if _, err := os.Stat(scorecardPath(root)); !os.IsNotExist(err) {
		t.Errorf("a no-real-cost run must NOT write scorecards.json (gate-on-real-cost skip); stat err=%v", err)
	}
	for _, l := range logs {
		if l != "" {
			t.Errorf("a skipped wind-down must emit no WARNING; got log %q", l)
		}
	}
}

// FAIL-LOUD-AND-CONTINUE: when the trace HAS real cost but the producer cannot run (no
// harness in this throwaway root, so `node harness/scorecard-update.mjs` fails), the
// wind-down must emit a stderr WARNING per pair and RETURN normally — it never panics and
// never signals failure back (the run's exit code is the caller's, set before this).
func TestWindDownScorecards_FailLoudAndContinue(t *testing.T) {
	root := t.TempDir() // no harness/ dir -> the node shell-out fails to find the script
	writeTrace(t, root,
		`{"seq":1,"kind":"agent","name":"implementer","status":"ok","cost_usd_micros":54404,"model":"sonnet"}`,
	)
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "impl", Agent: "implementer"}}}
	var warned int
	// Returning normally (no panic, no os.Exit) IS the pass condition; we also assert a
	// WARNING was surfaced (fail-LOUD), so the failure is never silently swallowed.
	windDownScorecards(wf, runOpts{root: root, mode: "balanced"}, func(s string) {
		if strings.Contains(s, "WARNING scorecard-update failed") {
			warned++
		}
	})
	if warned == 0 {
		t.Error("a producer failure must surface a loud WARNING (fail-loud), never be swallowed")
	}
}
