package attribution

import (
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/asset"
)

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

// evolveShapedWorkflow builds an asset.Workflow with the same phase/agent
// pairs as the real .agent/workflows/evolve.yml.
func evolveShapedWorkflow() asset.Workflow {
	return asset.Workflow{Phases: []asset.Phase{
		{Name: "scan", Agent: "explorer"},
		{Name: "gap-analysis", Agent: "architect"},
		{Name: "roadmap-update", Agent: "planner"},
		{Name: "implement", Agent: "implementer"},
		{Name: "review", Agent: "reviewer"},
		{Name: "evaluate", Agent: "qa"},
	}}
}

// BUG REPRO (heuristic alone): TaskTypeForRebuildEvent's substring/EqualFold
// fallback against AgentTaskType's agent-ROLE keys finds NOTHING for any of
// evolve.yml's phase NAMES — none of them contain or equal their agent role
// substring (implement/implementer, review/reviewer, evaluate/qa,
// gap-analysis/architect, roadmap-update/planner). Without ground truth
// (nil/empty phaseTaskTypes), every one of these must fail to resolve — this
// pins down exactly the silent-drop bug a fresh-context review found.
func TestTaskTypeForRebuildEvent_HeuristicAloneFailsEvolveYmlPhaseNames(t *testing.T) {
	for _, name := range []string{"gap-analysis", "roadmap-update", "implement", "review", "evaluate"} {
		if tt, ok := TaskTypeForRebuildEvent(name, nil); ok {
			t.Errorf("TaskTypeForRebuildEvent(%q, nil) = (%q, true), want ok=false (the substring heuristic must NOT accidentally match evolve.yml phase names)", name, tt)
		}
	}
}

// THE FIX: given the GROUND-TRUTH phaseTaskTypes map (what PhaseTaskTypes
// builds from the real workflow definition), every evolve.yml phase resolves to
// its agent's correct task_type — the exact case the substring heuristic drops.
func TestTaskTypeForRebuildEvent_GroundTruthResolvesEvolveYmlPhases(t *testing.T) {
	phaseTaskTypes := map[string]string{}
	for phase, agent := range evolveYmlPhaseAgent {
		if tt, ok := TaskTypeForAgent(agent); ok {
			phaseTaskTypes[phase] = tt
		}
	}
	want := map[string]string{
		"gap-analysis":   "architecture",
		"roadmap-update": "implementation",
		"implement":      "implementation",
		"review":         "reviewer",
		"evaluate":       "test",
	}
	for name, wantTT := range want {
		tt, ok := TaskTypeForRebuildEvent(name, phaseTaskTypes)
		if !ok || tt != wantTT {
			t.Errorf("TaskTypeForRebuildEvent(%q, ground-truth-map) = (%q,%v), want (%q,true)", name, tt, ok, wantTT)
		}
	}
	// "scan" (agent: explorer) has no task_type mapping at all — ground truth
	// correctly declines it too, same as the heuristic would.
	if tt, ok := TaskTypeForRebuildEvent("scan", phaseTaskTypes); ok {
		t.Errorf("TaskTypeForRebuildEvent(\"scan\", ...) = (%q,true), want ok=false (explorer has no task_type mapping)", tt)
	}
}

// A phase name NOT present in phaseTaskTypes must still fall back to the
// substring heuristic (e.g. a trace produced by build.yml, whose phase names
// equal their agent names) — the ground-truth map only ever ADDS coverage, it
// never removes the pre-existing fallback path.
func TestTaskTypeForRebuildEvent_FallsBackToHeuristicWhenNotInGroundTruth(t *testing.T) {
	phaseTaskTypes := map[string]string{"implement": "implementation"} // only knows evolve's "implement"
	tt, ok := TaskTypeForRebuildEvent("implementer", phaseTaskTypes)   // build.yml-shaped phase name
	if !ok || tt != "implementation" {
		t.Errorf("a phase name absent from the ground-truth map must still resolve via the substring heuristic; got (%q,%v)", tt, ok)
	}
}

// PhaseTaskTypes must read evolve.yml-shaped phases' REAL phase->agent pairs
// and resolve all 5 mapped phases, omitting "scan" (agent: explorer has no
// task_type mapping).
func TestPhaseTaskTypes_BuildsGroundTruthFromWorkflowPhases(t *testing.T) {
	got := PhaseTaskTypes([]asset.Workflow{evolveShapedWorkflow()})
	want := map[string]string{
		"gap-analysis":   "architecture",
		"roadmap-update": "implementation",
		"implement":      "implementation",
		"review":         "reviewer",
		"evaluate":       "test",
	}
	for name, wantTT := range want {
		if tt := got[name]; tt != wantTT {
			t.Errorf("PhaseTaskTypes(...)[%q] = %q, want %q (full map: %+v)", name, tt, wantTT, got)
		}
	}
	if _, ok := got["scan"]; ok {
		t.Errorf("phase \"scan\" (agent: explorer) has no task_type mapping and must be absent; got %+v", got)
	}
}

// The FIRST workflow in the slice wins a phase-name collision — a later
// workflow naming the SAME phase with a DIFFERENT agent must not overwrite it.
func TestPhaseTaskTypes_FirstWorkflowWinsPhaseNameCollision(t *testing.T) {
	first := asset.Workflow{Phases: []asset.Phase{{Name: "implement", Agent: "implementer"}}}
	second := asset.Workflow{Phases: []asset.Phase{{Name: "implement", Agent: "reviewer"}}}

	got := PhaseTaskTypes([]asset.Workflow{first, second})
	wantTT, _ := TaskTypeForAgent("implementer")
	if got["implement"] != wantTT {
		t.Errorf("first-workflow-seen must win the collision; got %q, want %q (map: %+v)", got["implement"], wantTT, got)
	}
}

// No workflows (or workflows with no mappable phases) yields an empty map,
// never nil-panic or garbage.
func TestPhaseTaskTypes_EmptyInputYieldsEmptyMap(t *testing.T) {
	if got := PhaseTaskTypes(nil); len(got) != 0 {
		t.Errorf("PhaseTaskTypes(nil) = %+v, want empty map", got)
	}
	if got := PhaseTaskTypes([]asset.Workflow{}); len(got) != 0 {
		t.Errorf("PhaseTaskTypes([]) = %+v, want empty map", got)
	}
}

// writeTraceFile drops a trace JSONL at <dir>/trace.jsonl with the given lines.
func writeTraceFile(t *testing.T, dir string, lines ...string) string {
	t.Helper()
	body := ""
	for _, l := range lines {
		body += l + "\n"
	}
	path := filepath.Join(dir, "trace.jsonl")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

// END-TO-END BUG REPRO / FIX: an evolve.yml-shaped trace (billed cost events
// named after evolve's phases, not its agents) rebuilt WITHOUT ground truth
// (phaseTaskTypes=nil, i.e. the pre-fix heuristic-only behavior) silently drops
// every pair; rebuilt WITH PhaseTaskTypes's ground truth, every billed
// evolve-loop phase is correctly attributed. This is the exact scenario the
// fresh-context review flagged: `forge scorecard rebuild --from <evolve trace>`
// must not come back empty.
func TestExtractRebuildPairs_EvolveYmlShapedTraceAttributesCorrectly(t *testing.T) {
	dir := t.TempDir()
	traceFile := writeTraceFile(t, dir,
		`{"seq":1,"kind":"agent","name":"gap-analysis","status":"ok","cost_usd_micros":9000,"model":"opus"}`,
		`{"seq":2,"kind":"agent","name":"roadmap-update","status":"ok","cost_usd_micros":5000,"model":"sonnet"}`,
		`{"seq":3,"kind":"agent","name":"implement","status":"ok","cost_usd_micros":20000,"model":"sonnet"}`,
		`{"seq":4,"kind":"agent","name":"review","status":"ok","cost_usd_micros":15000,"model":"opus"}`,
		`{"seq":5,"kind":"agent","name":"evaluate","status":"ok","cost_usd_micros":4000,"model":"haiku"}`,
	)

	// PRE-FIX behavior (no ground truth): every event is dropped.
	prefix, err := ExtractRebuildPairs(traceFile, nil)
	if err != nil {
		t.Fatalf("ExtractRebuildPairs (no ground truth): %v", err)
	}
	if len(prefix) != 0 {
		t.Errorf("sanity: heuristic-only extraction of an evolve.yml-shaped trace should still find nothing (got %+v) — otherwise this test isn't exercising the bug", prefix)
	}

	// THE FIX: with PhaseTaskTypes's ground truth, all 5 billed phases attribute.
	phaseTaskTypes := PhaseTaskTypes([]asset.Workflow{evolveShapedWorkflow()})
	pairs, err := ExtractRebuildPairs(traceFile, phaseTaskTypes)
	if err != nil {
		t.Fatalf("ExtractRebuildPairs (ground truth): %v", err)
	}
	want := map[ScorecardPair]bool{
		{Model: "opus", TaskType: "architecture"}:     true, // gap-analysis
		{Model: "sonnet", TaskType: "implementation"}: true, // roadmap-update AND implement collapse (same pair)
		{Model: "opus", TaskType: "reviewer"}:         true, // review
		{Model: "haiku", TaskType: "test"}:            true, // evaluate
	}
	if len(pairs) != len(want) {
		t.Fatalf("got %d distinct pairs %+v, want %d %+v", len(pairs), pairs, len(want), want)
	}
	for _, p := range pairs {
		if !want[p] {
			t.Errorf("unexpected pair %+v", p)
		}
	}
}

// A missing trace file is an error (unlike the trace-presence GATE checks
// elsewhere in this loop, ExtractRebuildPairs is the disaster-recovery
// entry point and must surface a missing --from file loudly, not silently
// return zero pairs).
func TestExtractRebuildPairs_MissingFileErrors(t *testing.T) {
	_, err := ExtractRebuildPairs(filepath.Join(t.TempDir(), "does-not-exist.jsonl"), nil)
	if err == nil {
		t.Error("a missing trace file must return an error, not silently succeed")
	}
}

// Corrupt/blank lines are skipped, never fatal — a single readable
// model-bearing cost event is still extracted.
func TestExtractRebuildPairs_RobustToCorruptLines(t *testing.T) {
	dir := t.TempDir()
	traceFile := writeTraceFile(t, dir,
		``,
		`not json`,
		`{"seq":2,"kind":"agent","name":"implement","status":"ok","cost_usd_micros":12000,"model":"opus"}`,
	)
	phaseTaskTypes := PhaseTaskTypes([]asset.Workflow{evolveShapedWorkflow()})
	pairs, err := ExtractRebuildPairs(traceFile, phaseTaskTypes)
	if err != nil {
		t.Fatalf("ExtractRebuildPairs: %v", err)
	}
	want := ScorecardPair{Model: "opus", TaskType: "implementation"}
	if len(pairs) != 1 || pairs[0] != want {
		t.Errorf("pairs = %+v, want exactly [%+v]", pairs, want)
	}
}
