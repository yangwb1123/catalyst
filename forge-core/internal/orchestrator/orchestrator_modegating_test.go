package orchestrator

// Mode-gating tests: the central knob's Workflow-depth dimensions applied by the
// orchestrator — gate-set filtering + optional-reviewer skip (build stage), the
// discover STAGE skip (explorer) vs run (engineering/production), and ADR-gating
// narration (design stage). Split out of orchestrator_test.go to keep each test
// file under the 500-line structural cap; the shared recorder/execFunc/allOK
// helpers and contains/containsLine live in orchestrator_test.go (same package).

import (
	"sort"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
)

// gatingWorkflow mirrors build.yml's full shape for mode-gating tests: a gate
// phase carrying the FULL gate catalog, plus a reviewer phase gated on the
// modes.yml reviewer fragment (the phase the explorer policy must skip).
const gatingWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "harness-gates", "agent": "harness", "readonly": true,
     "required_gates": ["lint", "test", "build", "complexity", "arch", "security"]},
    {"name": "reviewer", "agent": "reviewer", "readonly": true, "required_gates": [],
     "required_when": "../policies/modes.yml#workflow_depth.reviewer"},
    {"name": "qa", "agent": "test-agent", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadGating(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(gatingWorkflow))
	if err != nil {
		t.Fatalf("load gating fixture: %v", err)
	}
	return wf
}

// gateTracker is a RunGate that records every gate name it was asked to run, so a
// test can assert WHICH gates the mode filter let through.
type gateTracker struct{ ran []string }

func (g *gateTracker) run(name string) gate.Result {
	g.ran = append(g.ran, name)
	return gate.Result{Name: name, OK: true}
}

func sortedCSV(ss []string) string {
	c := append([]string(nil), ss...)
	sort.Strings(c)
	return strings.Join(c, ",")
}

// Explorer policy: the gate phase runs ONLY lint+build (complexity/arch/security
// filtered out), and the reviewer phase is SKIPPED (reviewer off) with the
// documented log line. The implementer/qa agent phases still run.
func TestRun_ExplorerPolicyFiltersGatesAndSkipsReviewer(t *testing.T) {
	wf := loadGating(t)
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run under explorer: %v", err)
	}
	if got := sortedCSV(gt.ran); got != "build,lint" {
		t.Errorf("explorer ran gates %q, want only build,lint (complexity/arch/security filtered)", got)
	}
	if contains(rec.executed, "reviewer") {
		t.Errorf("reviewer phase must be SKIPPED under explorer; executed=%v", rec.executed)
	}
	if !containsLine(rec.logs, "phase reviewer skipped (mode gating: reviewer off)") {
		t.Errorf("explorer skip must log the documented reason; logs=%v", rec.logs)
	}
	// The non-reviewer agent phases still run.
	for _, want := range []string{"implementer", "qa"} {
		if !contains(rec.executed, want) {
			t.Errorf("phase %q should still run under explorer; executed=%v", want, rec.executed)
		}
	}
}

func TestRun_StrictQATestGateSurvivesExplorerPolicy(t *testing.T) {
	rec := &recorder{}
	gt := &gateTracker{}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{
			Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
			RequiredGates: []string{"test"},
			OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
		},
	}}
	eng := Engine{
		Exec: rec.executor(), RunGate: gt.run, AgentVerdict: acceptedStrictQAVerdict,
		ModePolicy: mode.Effective("explorer", "idea"),
	}
	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("strict QA under explorer: %v", err)
	}
	if got := sortedCSV(gt.ran); got != "test" {
		t.Fatalf("strict QA gates=%q, want non-filterable test", got)
	}
}

// Engineering policy: ALL gates run and the reviewer phase is NOT skipped.
func TestRun_EngineeringPolicyRunsAllGatesAndReviewer(t *testing.T) {
	wf := loadGating(t)
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}

	if err := eng.Run(wf, "engineering"); err != nil {
		t.Fatalf("Run under engineering: %v", err)
	}
	if got := sortedCSV(gt.ran); got != "arch,build,complexity,lint,security,test" {
		t.Errorf("engineering ran gates %q, want the full set", got)
	}
	if !contains(rec.executed, "reviewer") {
		t.Errorf("reviewer phase must run under engineering; executed=%v", rec.executed)
	}
}

// ★ Production override ★: even with mode=explorer, the production lifecycle
// FORCES the full gate-set and keeps the reviewer — a loose mode never relaxes
// enforcement. This is the orchestrator-level proof of the safety veto.
func TestRun_ProductionOverrideForcesFullEnforcementEvenForExplorer(t *testing.T) {
	wf := loadGating(t)
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "production")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run explorer+production: %v", err)
	}
	if got := sortedCSV(gt.ran); got != "arch,build,complexity,lint,security,test" {
		t.Errorf("explorer+production ran %q, want the FULL set (production override)", got)
	}
	if !contains(rec.executed, "reviewer") {
		t.Errorf("explorer+production must STILL run the reviewer (override); executed=%v", rec.executed)
	}
}

// Back-compat: the ZERO-VALUE ModePolicy must run EVERY required gate and skip NO
// phase — byte-for-byte the pre-gating behavior. This is the contract the
// existing Engine tests (which never set ModePolicy) depend on.
func TestRun_ZeroPolicyIsFullyOpenBackCompat(t *testing.T) {
	wf := loadGating(t)
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log} // ModePolicy zero

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run with zero policy: %v", err)
	}
	if got := sortedCSV(gt.ran); got != "arch,build,complexity,lint,security,test" {
		t.Errorf("zero policy ran %q, want the FULL required set (no filtering)", got)
	}
	if !contains(rec.executed, "reviewer") {
		t.Errorf("zero policy must NOT skip the reviewer phase; executed=%v", rec.executed)
	}
	// And no mode-gating log lines leak when gating is inactive.
	if containsLine(rec.logs, "mode gating") {
		t.Errorf("zero policy must not emit mode-gating logs; logs=%v", rec.logs)
	}
}

// discoverWorkflow mirrors discover.yml's shape: stage "discover" with three
// read-only agent phases and NO gate phases (discovery emits analysis, not code).
// It is the fixture for the discover-stage skip (explorer) vs run (engineering).
const discoverWorkflow = `{
  "stage": "discover",
  "phases": [
    {"name": "requirement-discovery", "agent": "product-manager", "readonly": true, "required_gates": []},
    {"name": "market-research", "agent": "researcher", "readonly": true, "required_gates": []},
    {"name": "product-design", "agent": "product-manager", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadDiscover(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(discoverWorkflow))
	if err != nil {
		t.Fatalf("load discover fixture: %v", err)
	}
	return wf
}

// ★ Explorer skips the WHOLE discover stage: NONE of its agent phases run, and the
// documented skip line is logged. This is the discover-depth half of the central
// knob — explorer's "go straight to build".
func TestRun_ExplorerSkipsDiscoverStage(t *testing.T) {
	wf := loadDiscover(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run discover under explorer: %v", err)
	}
	if len(rec.executed) != 0 {
		t.Errorf("explorer must run NO discover phases; executed=%v", rec.executed)
	}
	if !containsLine(rec.logs, "discover stage skipped (mode gating: explorer skips discovery)") {
		t.Errorf("explorer skip must log the documented reason; logs=%v", rec.logs)
	}
}

// Engineering runs the FULL discover stage: every agent phase executes (discover
// depth = full). The contrast with explorer proves the gating is real.
func TestRun_EngineeringRunsDiscoverStage(t *testing.T) {
	wf := loadDiscover(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}

	if err := eng.Run(wf, "engineering"); err != nil {
		t.Fatalf("Run discover under engineering: %v", err)
	}
	want := []string{"requirement-discovery", "market-research", "product-design"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("engineering should run all discover phases; executed=%v want=%v", rec.executed, want)
	}
	if containsLine(rec.logs, "discover stage skipped") {
		t.Errorf("engineering must not log a discover skip; logs=%v", rec.logs)
	}
}

// ★ Production override on the discover dimension: explorer+production must NOT
// skip discovery — the safety veto restores the stage, so all phases run.
func TestRun_ProductionRestoresDiscoverEvenForExplorer(t *testing.T) {
	wf := loadDiscover(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "production")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run discover explorer+production: %v", err)
	}
	if len(rec.executed) != 3 {
		t.Errorf("explorer+production must run ALL discover phases (override); executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "discover stage skipped") {
		t.Errorf("explorer+production must not skip discovery; logs=%v", rec.logs)
	}
}

// Back-compat #1: the ZERO-VALUE policy never skips discovery — the stage runs
// unfiltered exactly as before discover gating existed.
func TestRun_ZeroPolicyRunsDiscoverBackCompat(t *testing.T) {
	wf := loadDiscover(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log} // ModePolicy zero

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run discover with zero policy: %v", err)
	}
	if len(rec.executed) != 3 {
		t.Errorf("zero policy must run all discover phases; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "discover stage skipped") {
		t.Errorf("zero policy must not skip discovery; logs=%v", rec.logs)
	}
}

// Back-compat #2: discover gating is STAGE-scoped — the build-stage fixture, run
// under the explorer policy (which skips DISCOVER), still runs every build phase.
// A loose discover depth must never bleed into a non-discover stage.
func TestRun_ExplorerDoesNotSkipBuildStage(t *testing.T) {
	wf := loadGating(t) // stage "build"
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run build under explorer: %v", err)
	}
	// The build stage's agent phases still run (reviewer is skipped for a DIFFERENT
	// reason — reviewer off — but implementer/qa run); the stage is NOT elided.
	if !contains(rec.executed, "implementer") || !contains(rec.executed, "qa") {
		t.Errorf("explorer must NOT skip the build stage; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "discover stage skipped") {
		t.Errorf("a build-stage run must never log a discover skip; logs=%v", rec.logs)
	}
}

// designWorkflow mirrors design.yml's shape: stage "design" with a writes_adr
// solution-architect phase and a plain proposal phase. The fixture for ADR-gating
// narration (required under engineering/cto, not required under explorer/balanced).
const designWorkflow = `{
  "stage": "design",
  "phases": [
    {"name": "solution-architect", "agent": "architect", "readonly": true, "required_gates": [],
     "model_tier": "opus", "writes_adr": {"condition": "mode in [engineering, cto]", "target": "docs/adr/"}},
    {"name": "proposal-generator", "agent": "cto", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadDesign(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(designWorkflow))
	if err != nil {
		t.Fatalf("load design fixture: %v", err)
	}
	return wf
}

// Engineering: the writes_adr phase narrates "ADR required". Both design phases
// still run (ADR gating is a narration, not a skip).
func TestRun_EngineeringNarratesADRRequired(t *testing.T) {
	wf := loadDesign(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}

	if err := eng.Run(wf, "engineering"); err != nil {
		t.Fatalf("Run design under engineering: %v", err)
	}
	if !containsLine(rec.logs, "solution-architect: ADR required") {
		t.Errorf("engineering must narrate ADR required; logs=%v", rec.logs)
	}
	if containsLine(rec.logs, "ADR not required") {
		t.Errorf("engineering must not narrate ADR not-required; logs=%v", rec.logs)
	}
	if !contains(rec.executed, "solution-architect") || !contains(rec.executed, "proposal-generator") {
		t.Errorf("ADR gating must not skip design phases; executed=%v", rec.executed)
	}
}

// Explorer: the writes_adr phase narrates "ADR not required" (explorer writes no
// ADR). The phases still run; only the narration differs.
func TestRun_ExplorerNarratesADRNotRequired(t *testing.T) {
	wf := loadDesign(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run design under explorer: %v", err)
	}
	if !containsLine(rec.logs, "solution-architect: ADR not required") {
		t.Errorf("explorer must narrate ADR not required; logs=%v", rec.logs)
	}
	if containsLine(rec.logs, "ADR required (") {
		t.Errorf("explorer must not narrate ADR required; logs=%v", rec.logs)
	}
}

// Back-compat: the zero-value policy narrates NO ADR line (gating inactive), and a
// non-design stage never narrates ADR even with a policy that requires one.
func TestRun_ADRNarrationGuards(t *testing.T) {
	// Zero policy on the design stage: no narration.
	wf := loadDesign(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log} // ModePolicy zero
	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run design with zero policy: %v", err)
	}
	if containsLine(rec.logs, "ADR required") || containsLine(rec.logs, "ADR not required") {
		t.Errorf("zero policy must not narrate ADR; logs=%v", rec.logs)
	}
	// Engineering policy on the BUILD stage (no writes_adr phase): no narration.
	bwf := loadGating(t)
	brec := &recorder{}
	beng := Engine{Exec: brec.executor(), RunGate: allOK, Log: brec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}
	if err := beng.Run(bwf, "engineering"); err != nil {
		t.Fatalf("Run build under engineering: %v", err)
	}
	if containsLine(brec.logs, "ADR required") || containsLine(brec.logs, "ADR not required") {
		t.Errorf("build stage must never narrate ADR; logs=%v", brec.logs)
	}
}
