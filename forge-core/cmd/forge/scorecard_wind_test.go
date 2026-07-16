package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/attribution"
	"forgeos/forge-core/internal/routing"
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

// distinctScorecardPairs reads the DISTINCT (model, task_type) pairs the run actually BILLED
// from the trace's model-stamped cost events: two implementer phases at the SAME stamped model
// collapse to one pair, a phase with no cost event (the harness/gate phase) contributes none,
// and the model comes from the trace (the actual billed tier), not a recomputed route.
func TestDistinctScorecardPairs_FromTraceDedupAndSkipUnbilled(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "plan", Agent: "planner"},
		{Name: "impl-a", Agent: "implementer"},
		{Name: "impl-b", Agent: "implementer"}, // same stamped model -> dedup with impl-a
		{Name: "gate", Agent: "harness"},       // a gate phase bills no cost event
		{Name: "review", Agent: "reviewer"},
	}}
	root := t.TempDir()
	writeTrace(t, root,
		`{"seq":1,"kind":"agent","name":"plan","status":"ok","cost_usd_micros":10000,"model":"opus"}`,
		`{"seq":2,"kind":"agent","name":"impl-a","status":"ok","cost_usd_micros":20000,"model":"sonnet"}`,
		`{"seq":3,"kind":"agent","name":"impl-b","status":"ok","cost_usd_micros":21000,"model":"sonnet"}`,
		`{"seq":4,"kind":"gate","name":"gate","status":"PASS","duration_ms":900}`, // no model/cost
		`{"seq":5,"kind":"agent","name":"review","status":"ok","cost_usd_micros":30000,"model":"opus"}`,
	)
	pairs := distinctScorecardPairs(wf, tracePath(root))

	// Expected = the DISTINCT (stamped model, agent task_type) over the BILLED phases.
	want := map[scorecardPair]bool{}
	for _, e := range []struct{ agent, model string }{
		{"planner", "opus"}, {"implementer", "sonnet"}, {"reviewer", "opus"},
	} {
		tt, _ := attribution.TaskTypeForAgent(e.agent)
		want[scorecardPair{Model: e.model, TaskType: tt}] = true
	}

	got := map[scorecardPair]int{}
	for _, p := range pairs {
		got[p]++
		if !want[p] {
			t.Errorf("unexpected pair %+v (the gate phase bills nothing; only billed roles emit)", p)
		}
		if got[p] > 1 {
			t.Errorf("pair %+v appears %d times; pairs must be distinct", p, got[p])
		}
	}
	if len(got) != len(want) {
		t.Errorf("distinct pairs = %d (%+v), want %d (%+v)", len(got), pairs, len(want), want)
	}
}

// THE ATTRIBUTION FIX: a phase whose routed tier was sonnet but which budget-DOWN-TIERED to
// haiku near the cap is stamped by costEmitter with the model it ACTUALLY ran (haiku). The
// pair must carry that STAMPED model — not the un-adjusted route — else a scorecard-update
// query for the original tier finds zero matching cost and the down-tiered spend is LOST.
func TestDistinctScorecardPairs_BudgetDowngradeAttributedToActualModel(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "impl", Agent: "implementer"}}}
	root := t.TempDir()
	// implementer normally routes to sonnet; near budget it ran (and was stamped) as haiku.
	writeTrace(t, root,
		`{"seq":1,"kind":"agent","name":"impl","status":"ok","cost_usd_micros":4000,"model":"haiku"}`,
	)
	pairs := distinctScorecardPairs(wf, tracePath(root))
	ttImpl, _ := attribution.TaskTypeForAgent("implementer")
	want := scorecardPair{Model: "haiku", TaskType: ttImpl}
	if len(pairs) != 1 || pairs[0] != want {
		t.Errorf("down-tiered cost must attribute to the ACTUAL stamped model %+v, got %+v", want, pairs)
	}
}

// A trace with NO billed (model+cost) event — only iteration/gate rows — yields no pairs:
// nothing was paid for, so nothing is attributed (mirrors traceHasModelCost's gate-out).
func TestDistinctScorecardPairs_NoBilledCostYieldsNone(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "impl", Agent: "implementer"}}}
	root := t.TempDir()
	writeTrace(t, root,
		`{"seq":1,"kind":"iteration","name":"1","status":"ok","duration_ms":4200}`,
	)
	if pairs := distinctScorecardPairs(wf, tracePath(root)); len(pairs) != 0 {
		t.Errorf("a trace with no billed cost must yield no pairs; got %+v", pairs)
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
	windDownScorecards(wf, runOpts{root: root, mode: "balanced"}, func(s string) { logs = append(logs, s) }, 1, false)

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
	writeTrace(t, root, // event name MUST match the phase name "impl" (mapped to its task_type)
		`{"seq":1,"kind":"agent","name":"impl","status":"ok","cost_usd_micros":54404,"model":"sonnet"}`,
	)
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "impl", Agent: "implementer"}}}
	var warned int
	// Returning normally (no panic, no os.Exit) IS the pass condition; we also assert a
	// WARNING was surfaced (fail-LOUD), so the failure is never silently swallowed.
	windDownScorecards(wf, runOpts{root: root, mode: "balanced"}, func(s string) {
		if strings.Contains(s, "WARNING scorecard-update failed") {
			warned++
		}
	}, 2, true)
	if warned == 0 {
		t.Error("a producer failure must surface a loud WARNING (fail-loud), never be swallowed")
	}
}

// ── forge scorecard display tests ─────────────────────────────────────────

func TestScorecard_NoDataColdStart(t *testing.T) {
	cards := []routing.Scorecard{}
	if len(cards) != 0 {
		t.Fatal("test setup: cards must be empty")
	}
}

func TestScorecard_TableRendersAllEntries(t *testing.T) {
	cards := []routing.Scorecard{
		{Model: "opus", TaskType: "implementation", QualityScore: 0.91, Samples: 30, UpdatedAt: "2026-06-01T00:00:00Z"},
		{Model: "sonnet", TaskType: "implementation", QualityScore: 0.82, Samples: 25, UpdatedAt: "2026-06-01T00:00:00Z"},
		{Model: "haiku", TaskType: "test", QualityScore: 0.87, Samples: 5, UpdatedAt: "2026-06-01T00:00:00Z"},
	}
	if len(cards) != 3 {
		t.Fatal("test setup: cards must have 3 entries")
	}
	if !anyThin(cards, 20) {
		t.Error("anyThin should be true when haiku/test has 5 samples < 20")
	}
	if anyThin(cards, 5) {
		t.Error("anyThin should be false when min-samples=5 and all >=5")
	}
}

func TestScorecard_ThinEntryDetection(t *testing.T) {
	tests := []struct {
		name     string
		cards    []routing.Scorecard
		min      int
		wantThin bool
	}{
		{"empty", nil, 20, false},
		{"all_above_min", []routing.Scorecard{{Samples: 25}, {Samples: 30}}, 20, false},
		{"one_below_min", []routing.Scorecard{{Samples: 25}, {Samples: 5}}, 20, true},
		{"exact_min", []routing.Scorecard{{Samples: 20}}, 20, false},
		{"below_min", []routing.Scorecard{{Samples: 19}}, 20, true},
		{"zero_samples", []routing.Scorecard{{Samples: 0}}, 20, true},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := anyThin(tc.cards, tc.min)
			if got != tc.wantThin {
				t.Errorf("anyThin(cards, %d) = %v, want %v", tc.min, got, tc.wantThin)
			}
		})
	}
}

func TestScorecard_SummaryAggregation(t *testing.T) {
	cards := []routing.Scorecard{
		{Model: "opus", TaskType: "implementation", QualityScore: 0.91, Samples: 30},
		{Model: "sonnet", TaskType: "implementation", QualityScore: 0.82, Samples: 25},
		{Model: "haiku", TaskType: "test", QualityScore: 0.87, Samples: 10},
	}
	printSummary(cards, 20)
	printTable(cards, 20)
}

func TestScorecard_SummaryEdgeCases(t *testing.T) {
	tests := []struct {
		name  string
		cards []routing.Scorecard
	}{
		{"single_entry", []routing.Scorecard{{Model: "opus", TaskType: "architecture", QualityScore: 0.90, Samples: 25}}},
		{"same_type_multiple_models", []routing.Scorecard{
			{Model: "opus", TaskType: "implementation", QualityScore: 0.88, Samples: 30},
			{Model: "sonnet", TaskType: "implementation", QualityScore: 0.85, Samples: 25},
			{Model: "haiku", TaskType: "implementation", QualityScore: 0.70, Samples: 10},
		}},
		{"all_thin", []routing.Scorecard{
			{Model: "opus", TaskType: "security", QualityScore: 0.90, Samples: 3},
			{Model: "haiku", TaskType: "docs", QualityScore: 0.92, Samples: 2},
		}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			printSummary(tc.cards, 20)
			printTable(tc.cards, 20)
		})
	}
}

// taskTypeForAgent/agentTaskType vocabulary tests now live in
// internal/attribution/attribution_test.go, next to the exported
// TaskTypeForAgent/AgentTaskType they cover.

// ── forge scorecard rebuild: glue tests (relocated from scorecard_rebuild_test.go) ──
//
// The PURE (model, task_type) derivation these tests used to exercise directly
// (taskTypeForRebuildEvent, extractRebuildPairs) now lives in
// internal/attribution and is tested there (rebuild_test.go). What remains
// here is the cmd/forge-specific GLUE: resolvePhaseTaskTypes's file I/O
// (globbing/loading real .agent/workflows/*.yml off disk) and
// parseScorecardRebuildFlags's flag wiring.

// evolveYmlPhaseAgent mirrors .agent/workflows/evolve.yml's real phase/agent
// pairs (the flagship autonomous-loop workflow) — its phase NAMES deliberately
// differ from their AGENT roles, unlike build.yml where they happen to coincide.
var evolveYmlPhaseAgent = map[string]string{
	"scan":           "explorer", // unmapped role (harness/observe-only); contributes no pair
	"gap-analysis":   "architect",
	"roadmap-update": "planner",
	"implement":      "implementer",
	"review":         "reviewer",
	"evaluate":       "qa",
}

// writeEvolveShapedWorkflow drops a MINIMAL evolve.yml-shaped workflow (same
// phase/agent pairs as the real .agent/workflows/evolve.yml, phases nested
// under `loop:` per its `type: loop` standing-loop shape) at
// <root>/.agent/workflows/<name>.yml.
func writeEvolveShapedWorkflow(t *testing.T, root, name string) {
	t.Helper()
	dir := filepath.Join(root, ".agent", "workflows")
	mkdir(t, dir)
	body := `id: ` + name + `
stage: evolve
type: loop
loop:
  loop_back_to: scan
  phases:
    - name: scan
      agent: explorer
    - name: gap-analysis
      agent: architect
    - name: roadmap-update
      agent: planner
    - name: implement
      agent: implementer
    - name: review
      agent: reviewer
    - name: evaluate
      agent: qa
`
	writeFile(t, filepath.Join(dir, name+".yml"), body)
}

// resolvePhaseTaskTypes(root, "") — no --workflow given — scans every workflow
// under .agent/workflows/*.yml and must read evolve.yml's REAL phase->agent
// pairs off disk, correctly resolving all 5 mapped phases.
func TestRebuildResolvePhaseTaskTypes_ScansAllWorkflowsAndReadsEvolveYmlGroundTruth(t *testing.T) {
	root := t.TempDir()
	writeEvolveShapedWorkflow(t, root, "evolve")

	got := resolvePhaseTaskTypes(root, "")
	want := map[string]string{
		"gap-analysis":   "architecture",
		"roadmap-update": "implementation",
		"implement":      "implementation",
		"review":         "reviewer",
		"evaluate":       "test",
	}
	for name, wantTT := range want {
		if tt := got[name]; tt != wantTT {
			t.Errorf("resolvePhaseTaskTypes(root, \"\")[%q] = %q, want %q (full map: %+v)", name, tt, wantTT, got)
		}
	}
	if _, ok := got["scan"]; ok {
		t.Errorf("phase \"scan\" (agent: explorer) has no task_type mapping and must be absent; got %+v", got)
	}
}

// --workflow <name> restricts the scan to exactly that workflow, ignoring any
// other workflow file present (even one that would map the SAME phase name to a
// DIFFERENT task_type) — the explicit flag must win over ambient scanning.
func TestRebuildResolvePhaseTaskTypes_WorkflowFlagRestrictsToNamedWorkflow(t *testing.T) {
	root := t.TempDir()
	writeEvolveShapedWorkflow(t, root, "evolve")
	// A second, conflicting workflow: same phase name "implement", DIFFERENT agent.
	dir := filepath.Join(root, ".agent", "workflows")
	writeFile(t, filepath.Join(dir, "other.yml"), `id: other
phases:
  - name: implement
    agent: reviewer
`)

	got := resolvePhaseTaskTypes(root, "evolve")
	wantTT, _ := attribution.TaskTypeForAgent("implementer")
	if got["implement"] != wantTT {
		t.Errorf("--workflow evolve must resolve \"implement\" from evolve.yml's agent (implementer -> %q), got %q (map: %+v)", wantTT, got["implement"], got)
	}
}

// A missing/malformed workflow (no .agent/workflows dir at all, or a --workflow
// name that doesn't exist on disk) must yield an EMPTY map, never panic or
// error — the caller (attribution.TaskTypeForRebuildEvent) then falls back to
// the substring heuristic for every phase, exactly the pre-fix behavior.
func TestRebuildResolvePhaseTaskTypes_MissingWorkflowYieldsEmptyMapNotPanic(t *testing.T) {
	root := t.TempDir() // no .agent/workflows dir at all
	if got := resolvePhaseTaskTypes(root, ""); len(got) != 0 {
		t.Errorf("no workflows on disk must yield an empty map, got %+v", got)
	}
	if got := resolvePhaseTaskTypes(root, "nonexistent"); len(got) != 0 {
		t.Errorf("--workflow naming a file that doesn't exist must yield an empty map, got %+v", got)
	}
}

// END-TO-END WIRING: resolvePhaseTaskTypes (cmd/forge's file I/O glue, reading
// a REAL evolve.yml-shaped workflow off disk) feeds attribution.ExtractRebuildPairs
// (the pure trace-derivation logic) correctly — an evolve.yml-shaped trace (billed
// cost events named after evolve's phases, not its agents) must attribute every
// billed phase, not come back empty. This pins the exact seam a refactor could
// silently break even if each half's own unit tests still pass: the fresh-context
// review's original regression (`forge scorecard rebuild --from <evolve trace>`
// must not come back empty) is exercised through the REAL glue, not a mock.
func TestResolvePhaseTaskTypes_EndToEndFeedsExtractRebuildPairs(t *testing.T) {
	root := t.TempDir()
	writeEvolveShapedWorkflow(t, root, "evolve")
	traceFile := filepath.Join(root, "trace.jsonl")
	lines := []string{
		`{"seq":1,"kind":"agent","name":"gap-analysis","status":"ok","cost_usd_micros":9000,"model":"opus"}`,
		`{"seq":2,"kind":"agent","name":"implement","status":"ok","cost_usd_micros":20000,"model":"sonnet"}`,
		`{"seq":3,"kind":"agent","name":"evaluate","status":"ok","cost_usd_micros":4000,"model":"haiku"}`,
	}
	body := ""
	for _, l := range lines {
		body += l + "\n"
	}
	if err := os.WriteFile(traceFile, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}

	phaseTaskTypes := resolvePhaseTaskTypes(root, "evolve")
	pairs, err := attribution.ExtractRebuildPairs(traceFile, phaseTaskTypes)
	if err != nil {
		t.Fatalf("attribution.ExtractRebuildPairs: %v", err)
	}
	want := map[scorecardPair]bool{
		{Model: "opus", TaskType: "architecture"}:     true,
		{Model: "sonnet", TaskType: "implementation"}: true,
		{Model: "haiku", TaskType: "test"}:            true,
	}
	if len(pairs) != len(want) {
		t.Fatalf("got %d distinct pairs %+v, want %d %+v (real-glue rebuild must not come back empty)", len(pairs), pairs, len(want), want)
	}
	for _, p := range pairs {
		if !want[p] {
			t.Errorf("unexpected pair %+v", p)
		}
	}
}

// cmdScorecardRebuild wiring sanity: parseScorecardRebuildFlags surfaces
// --workflow so cmdScorecardRebuild can pass it through to resolvePhaseTaskTypes.
func TestParseScorecardRebuildFlags_SurfacesWorkflowFlag(t *testing.T) {
	root := t.TempDir()
	_, _, _, workflowName, code, ok := parseScorecardRebuildFlags([]string{
		"--root", root, "--from", "t.jsonl", "--workflow", "evolve",
	})
	if !ok || code != 0 {
		t.Fatalf("parseScorecardRebuildFlags failed: code=%d ok=%v", code, ok)
	}
	if workflowName != "evolve" {
		t.Errorf("--workflow must round-trip through parseScorecardRebuildFlags; got %q", workflowName)
	}
}
