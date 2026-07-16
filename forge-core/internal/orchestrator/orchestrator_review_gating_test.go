package orchestrator

// Review-stage mode-gating tests: the WHOLE-STAGE skip for the new
// Discover→Design→★Review★→Build→Evolve spine's REVIEW stage (security /
// distributed / performance+reliability / CTO executive synthesis), driven by
// ModePolicy.ReviewDepth. Mirrors the discover-stage-skip tests in
// orchestrator_modegating_test.go byte-for-byte in structure; split into its own
// file to keep orchestrator_modegating_test.go under the 500-line structural cap.
// The shared recorder/allOK/contains/containsLine helpers and loadGating/
// loadDiscover fixtures live in orchestrator_test.go / orchestrator_modegating_test.go
// (same package).

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
)

// reviewWorkflow mirrors design.yml's REVIEW stage shape: stage "review" with four
// read-only agent phases (security, distributed, performance+reliability, CTO
// executive synthesis) and NO gate phases. It is the fixture for the review-stage
// skip (explorer) vs run (engineering) — the ReviewDepth half of the central knob,
// mirroring discoverWorkflow exactly.
const reviewWorkflow = `{
  "stage": "review",
  "phases": [
    {"name": "security-review", "agent": "security-engineer", "readonly": true, "required_gates": []},
    {"name": "distributed-review", "agent": "distributed-engineer", "readonly": true, "required_gates": []},
    {"name": "performance-review", "agent": "performance-engineer", "readonly": true, "required_gates": []},
    {"name": "cto-synthesis", "agent": "cto", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadReview(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(reviewWorkflow))
	if err != nil {
		t.Fatalf("load review fixture: %v", err)
	}
	return wf
}

// ★ Explorer skips the WHOLE review stage: NONE of its agent phases run, and the
// documented skip line is logged. This is the review-depth half of the central
// knob — explorer's "go straight to build" without the deep review.
func TestRun_ExplorerSkipsReviewStage(t *testing.T) {
	wf := loadReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run review under explorer: %v", err)
	}
	if len(rec.executed) != 0 {
		t.Errorf("explorer must run NO review phases; executed=%v", rec.executed)
	}
	if !containsLine(rec.logs, "review stage skipped (mode gating: explorer skips deep review)") {
		t.Errorf("explorer skip must log the documented reason; logs=%v", rec.logs)
	}
}

// Balanced/engineering run the FULL review stage: every agent phase executes
// (review depth = standard/full, never skip). The contrast with explorer proves
// the gating is real.
func TestRun_EngineeringRunsReviewStage(t *testing.T) {
	wf := loadReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}

	if err := eng.Run(wf, "engineering"); err != nil {
		t.Fatalf("Run review under engineering: %v", err)
	}
	want := []string{"security-review", "distributed-review", "performance-review", "cto-synthesis"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("engineering should run all review phases; executed=%v want=%v", rec.executed, want)
	}
	if containsLine(rec.logs, "review stage skipped") {
		t.Errorf("engineering must not log a review skip; logs=%v", rec.logs)
	}
}

// ★ Production override on the review dimension: explorer+production must NOT
// skip the deep review — the safety veto restores the stage, so all phases run.
func TestRun_ProductionRestoresReviewEvenForExplorer(t *testing.T) {
	wf := loadReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "production")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run review explorer+production: %v", err)
	}
	if len(rec.executed) != 4 {
		t.Errorf("explorer+production must run ALL review phases (override); executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "review stage skipped") {
		t.Errorf("explorer+production must not skip the deep review; logs=%v", rec.logs)
	}
}

// Back-compat #1: the ZERO-VALUE policy never skips the review stage — the stage
// runs unfiltered exactly as before review gating existed.
func TestRun_ZeroPolicyRunsReviewBackCompat(t *testing.T) {
	wf := loadReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log} // ModePolicy zero

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run review with zero policy: %v", err)
	}
	if len(rec.executed) != 4 {
		t.Errorf("zero policy must run all review phases; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "review stage skipped") {
		t.Errorf("zero policy must not skip the review stage; logs=%v", rec.logs)
	}
}

// Back-compat #2: review gating is STAGE-scoped — the build-stage fixture, run
// under the explorer policy (which skips REVIEW), still runs every build phase. A
// loose review depth must never bleed into a non-review stage. Also proves the
// converse: the discover-stage fixture run under explorer must never log a review
// skip (only a discover skip).
func TestRun_ExplorerDoesNotSkipBuildOrDiscoverStageForReview(t *testing.T) {
	wf := loadGating(t) // stage "build"
	rec := &recorder{}
	gt := &gateTracker{}
	eng := Engine{Exec: rec.executor(), RunGate: gt.run, Log: rec.log,
		ModePolicy: mode.Effective("explorer", "idea")}

	if err := eng.Run(wf, "explorer"); err != nil {
		t.Fatalf("Run build under explorer: %v", err)
	}
	if !contains(rec.executed, "implementer") || !contains(rec.executed, "qa") {
		t.Errorf("explorer must NOT skip the build stage; executed=%v", rec.executed)
	}
	if containsLine(rec.logs, "review stage skipped") {
		t.Errorf("a build-stage run must never log a review skip; logs=%v", rec.logs)
	}

	dwf := loadDiscover(t) // stage "discover"
	drec := &recorder{}
	deng := Engine{Exec: drec.executor(), RunGate: allOK, Log: drec.log,
		ModePolicy: mode.Effective("explorer", "idea")}
	if err := deng.Run(dwf, "explorer"); err != nil {
		t.Fatalf("Run discover under explorer: %v", err)
	}
	if containsLine(drec.logs, "review stage skipped") {
		t.Errorf("a discover-stage run must never log a review skip; logs=%v", drec.logs)
	}
}

// optionalReviewWorkflow mirrors review.yml's ACTUAL shape more precisely than
// reviewWorkflow: performance-review declares optional_for:["balanced"] (like
// review.yml's real performance-reliability-review phase), so balanced mode
// alone may skip it while security/distributed/cto never carry that marker.
const optionalReviewWorkflow = `{
  "stage": "review",
  "phases": [
    {"name": "security-review", "agent": "security-engineer", "readonly": true, "required_gates": []},
    {"name": "distributed-review", "agent": "distributed-engineer", "readonly": true, "required_gates": []},
    {"name": "performance-review", "agent": "performance-engineer", "readonly": true, "required_gates": [],
     "optional_for": ["balanced"]},
    {"name": "cto-synthesis", "agent": "cto", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadOptionalReview(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(optionalReviewWorkflow))
	if err != nil {
		t.Fatalf("load optional-review fixture: %v", err)
	}
	return wf
}

// Balanced/idea (no lifecycle floor): optional_for:["balanced"] fires and
// performance-review is skipped — the standard "two most critical dimensions"
// posture modes.yml documents for balanced.
func TestRun_BalancedSkipsOptionalReviewPhase(t *testing.T) {
	wf := loadOptionalReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("balanced", "idea")}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run optional-review under balanced/idea: %v", err)
	}
	if contains(rec.executed, "performance-review") {
		t.Errorf("balanced/idea should skip the optional_for performance-review phase; executed=%v", rec.executed)
	}
	for _, want := range []string{"security-review", "distributed-review", "cto-synthesis"} {
		if !contains(rec.executed, want) {
			t.Errorf("balanced/idea must still run %q; executed=%v", want, rec.executed)
		}
	}
}

// ★ Regression (fresh-review finding): balanced+production must NOT skip the
// optional_for performance-review phase. mode.Effective raises ReviewDepth to
// "full" under production for every base mode (the safety veto: "a loose mode
// can never relax enforcement here") — before this fix, skipByMode consulted
// only the RAW mode name ("balanced" is in optional_for) and ignored that
// production had already forced full rigor, so the phase was silently
// skipped anyway. stageDepthAtMax must suppress the optional_for skip once
// ReviewDepth is at its max, exactly like the whole-stage skip already does.
func TestRun_ProductionRestoresOptionalReviewPhaseForBalanced(t *testing.T) {
	wf := loadOptionalReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("balanced", "production")}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run optional-review under balanced/production: %v", err)
	}
	want := []string{"security-review", "distributed-review", "performance-review", "cto-synthesis"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("balanced+production must run ALL FOUR review phases (production override reaches optional_for too); executed=%v want=%v", rec.executed, want)
	}
	if containsLine(rec.logs, "phase performance-review skipped") {
		t.Errorf("balanced+production must not skip performance-review; logs=%v", rec.logs)
	}
}

// Back-compat: engineering (already ReviewDepth=full at baseline, no floor
// needed) never carries "engineering" in performance-review's optional_for
// list, so it was never skipped in the first place — proves stageDepthAtMax
// is not the ONLY reason engineering runs every phase.
func TestRun_EngineeringRunsOptionalReviewPhase(t *testing.T) {
	wf := loadOptionalReview(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		ModePolicy: mode.Effective("engineering", "mvp")}

	if err := eng.Run(wf, "engineering"); err != nil {
		t.Fatalf("Run optional-review under engineering: %v", err)
	}
	if !contains(rec.executed, "performance-review") {
		t.Errorf("engineering must run performance-review; executed=%v", rec.executed)
	}
}

// stageDepthAtMax must be stage-scoped: a discover-stage phase's optional_for
// is governed by DiscoverDepth, never ReviewDepth (and vice versa), so raising
// one dimension to full must not spuriously suppress an optional_for skip
// gated by the OTHER dimension.
func TestStageDepthAtMax_ScopedPerStage(t *testing.T) {
	eng := Engine{ModePolicy: mode.Effective("balanced", "production")}
	if !eng.stageDepthAtMax("discover") {
		t.Error("balanced+production should raise DiscoverDepth to full")
	}
	if !eng.stageDepthAtMax("review") {
		t.Error("balanced+production should raise ReviewDepth to full")
	}
	if eng.stageDepthAtMax("build") {
		t.Error("build has no modeled depth dimension; stageDepthAtMax must stay false (unchanged raw-mode optional_for behavior)")
	}
	balancedIdea := Engine{ModePolicy: mode.Effective("balanced", "idea")}
	if balancedIdea.stageDepthAtMax("review") {
		t.Error("balanced/idea (no lifecycle floor) must NOT report ReviewDepth at max")
	}
}
