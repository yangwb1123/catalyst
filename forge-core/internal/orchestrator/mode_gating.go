package orchestrator

import (
	"fmt"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
)

// gatingActive reports whether a mode policy was actually injected. The
// zero-value Policy ({nil Gates, Reviewer:false}) means "no mode gating" and
// gating stays INACTIVE (full back-compat: nothing is filtered). Any real mode
// is distinguishable from the zero value — it carries gates, or (cto, the only
// empty-gate mode) forces the reviewer on — so this never mistakes a configured
// policy for the zero value. See mode.Policy's zero-value contract.
func (e Engine) gatingActive() bool {
	return len(e.ModePolicy.Gates) > 0 || e.ModePolicy.Reviewer
}

// gatesFor returns the gates a gate phase should actually run: with gating
// inactive (zero policy) it is the phase's full required_gates (back-compat);
// with gating active it is the INTERSECTION of required_gates and the policy's
// gate-set, so explorer drops complexity/arch/security while a production-forced
// full policy keeps them all. qa_v1's independently required test gate is the
// explicit exception: mode filtering can never remove that enforcement boundary.
// Order follows the phase's declaration. An empty result is legal for phases
// without a strict verdict contract.
func (e Engine) gatesFor(p asset.Phase) []string {
	if !e.gatingActive() {
		return p.RequiredGates
	}
	kept := make([]string, 0, len(p.RequiredGates))
	for _, g := range p.RequiredGates {
		if e.ModePolicy.Allows(g) ||
			(p.VerdictContract == asset.VerdictContractQAV1 && g == "test") {
			kept = append(kept, g)
		}
	}
	return kept
}

// skipByMode reports whether a non-gate phase should be skipped by the mode
// policy. Two conditions:
//  1. Phase RequiredWhen resolves to "reviewer" and policy says no reviewer.
//  2. Phase declares optional_for containing the current mode (discover.yml's
//     market-research: optional_for: [balanced]; review.yml's
//     performance-reliability-review: optional_for: [balanced]) AND this
//     stage's depth dimension has not been raised to full rigor (see
//     stageDepthAtMax) — a lifecycle floor (production) must be able to
//     override a per-phase optional_for escape hatch, not just the whole-
//     stage skip.
//
// With gating inactive (zero policy) nothing is ever skipped — full back-compat.
func (e Engine) skipByMode(p asset.Phase, stage string) bool {
	if !e.gatingActive() {
		return false
	}
	if requiredWhenKey(p.RequiredWhen) == "reviewer" && !e.ModePolicy.Reviewer {
		return true
	}
	// optional_for: if the phase declares a list of modes that may skip it,
	// the current mode is in that list, AND this stage's depth hasn't been
	// raised to full by a lifecycle floor, skip this phase.
	if len(p.OptionalFor) > 0 && !e.stageDepthAtMax(stage) {
		m := e.ModePolicy.Mode
		for _, optional := range p.OptionalFor {
			if optional == m {
				return true
			}
		}
	}
	return false
}

// stageDepthAtMax reports whether the workflow_depth dimension tied to stage
// (discover -> DiscoverDepth, review -> ReviewDepth) has been raised to its
// maximum "full" rigor for this run — e.g. by the production lifecycle floor
// (mode.Effective always raises production to DiscoverFull/ReviewFull
// regardless of the base mode). When a stage's depth is already at max, a
// per-phase optional_for skip must NOT fire: production's safety veto ("a
// loose mode can never relax enforcement here", modes.yml) would otherwise be
// silently defeated by an escape hatch that only ever looked at the raw mode
// name, never the lifecycle-resolved depth — exactly how balanced+production
// could still skip review.yml's performance-reliability-review despite
// ReviewDepth being "full". Stages with no modeled depth dimension (build,
// design, evolve) return false — optional_for keeps its original raw-mode-
// only behavior there, unchanged.
func (e Engine) stageDepthAtMax(stage string) bool {
	switch stage {
	case "discover":
		return e.ModePolicy.DiscoverDepth == mode.DiscoverFull
	case "review":
		return e.ModePolicy.ReviewDepth == mode.ReviewFull
	default:
		return false
	}
}

// discoverStageSkipped reports whether the WHOLE discover stage should be elided
// for this run — true iff gating is active AND this is the discover stage AND the
// policy's DiscoverDepth is "skip" (explorer's "go straight to build"). It gates
// the entire stage, not a single phase: discover.yml's phases are all agent phases,
// so when skip fires RunFrom runs none of them. With gating inactive (zero policy)
// it is always false — full back-compat, the stage runs unfiltered. light/full
// (or any non-discover stage) also return false, so only the explicit explorer
// skip suppresses the stage.
//
// HONESTY: this is the gating DECISION — the runtime does not run the discovery
// phases and says so; under the dry-run executor it cannot prove the skipped
// discovery was safe to elide (that judgement is a real agent's). It does NOT
// pretend discovery ran or that real work was skipped — only that the explorer
// policy elects to skip the stage.
func (e Engine) discoverStageSkipped(wf asset.Workflow) bool {
	return e.gatingActive() && wf.Stage == "discover" && e.ModePolicy.DiscoverSkipped()
}

// reviewStageSkipped reports whether the WHOLE review stage should be elided for
// this run — true iff gating is active AND this is the review stage AND the
// policy's ReviewDepth is "skip" (explorer's "go straight to build" without the
// deep security/distributed/performance/CTO review). Mirrors discoverStageSkipped
// byte-for-byte in structure. With gating inactive (zero policy) it is always
// false — full back-compat, the stage runs unfiltered. standard/full (or any
// non-review stage) also return false, so only the explicit explorer skip
// suppresses the stage.
//
// HONESTY: this is the gating DECISION — the runtime does not run the review
// phases and says so; under the dry-run executor it cannot prove the skipped
// review was safe to elide (that judgement is a real agent's). It does NOT
// pretend the review ran or that real work was skipped — only that the explorer
// policy elects to skip the stage.
func (e Engine) reviewStageSkipped(wf asset.Workflow) bool {
	return e.gatingActive() && wf.Stage == "review" && e.ModePolicy.ReviewSkipped()
}

// stageSkipped reports whether the WHOLE current stage should be elided by mode
// gating — discover (DiscoverDepth=="skip") or review (ReviewDepth=="skip") — and
// the log message RunFrom should report. It is the shared combinator behind both
// discoverStageSkipped and reviewStageSkipped, so RunFrom's early guard stays a
// single call site as more whole-stage skips are added. Each check is scoped to
// its own wf.Stage name, so at most one can ever fire for a given run — no
// ordering ambiguity between the two.
func (e Engine) stageSkipped(wf asset.Workflow) (bool, string) {
	if e.discoverStageSkipped(wf) {
		return true, "discover stage skipped (mode gating: explorer skips discovery)"
	}
	if e.reviewStageSkipped(wf) {
		return true, "review stage skipped (mode gating: explorer skips deep review)"
	}
	return false, ""
}

// prepareSerialWorkflow validates structure, applies whole-stage gating and the
// Evolve mutation boundary, then validates resume's executable start index.
func (e Engine) prepareSerialWorkflow(wf asset.Workflow, start int) (asset.Workflow, bool, error) {
	if err := asset.ValidateWorkflowStructure(wf); err != nil {
		return wf, false, fmt.Errorf("serial orchestration: invalid workflow structure: %w", err)
	}
	if e.checkStageSkip(wf) {
		return wf, true, nil
	}
	filtered, err := e.applyEvolveCutoff(wf)
	if err != nil {
		return wf, false, fmt.Errorf("serial orchestration: evolve policy: %w", err)
	}
	if start < 0 || start > len(filtered.Phases) {
		return wf, false, fmt.Errorf("serial orchestration: start phase %d outside executable range [0,%d]", start, len(filtered.Phases))
	}
	return filtered, false, nil
}

func (e Engine) checkStageSkip(wf asset.Workflow) bool {
	skipped, msg := e.stageSkipped(wf)
	if skipped {
		e.logf("%s", msg)
		e.reportStop(wf)
	}
	return skipped
}

func (e Engine) warnIfVacuous(wf asset.Workflow, phasesRan, start int) {
	if phasesRan == 0 && start < len(wf.Phases) {
		e.logf("⚠ vacuous run: mode gating filtered all %d phase(s) — no work was performed", len(wf.Phases))
	}
}

const (
	evolveEffectObserve = "observe"
	evolveEffectPropose = "propose"
	evolveEffectMutate  = "mutate"
	evolveEffectVerify  = "verify"
)

// EvolvePhaseLimit returns the number of phases the effective policy may
// execute. Mutation authority is deliberately independent of EvolveDepth:
// lifecycle quality floors may deepen the iteration budget, but never turn a
// propose-only mode into an auto-acting one.
//
// A proposal-only workflow must declare exactly one effect=mutate phase. That
// phase is the name-independent agent-write boundary; every non-mutate phase
// must be readonly so a renamed/custom agent cannot smuggle an unrestricted
// writer into the proposal prefix. Trusted host gates/probes remain separate.
// Missing/ambiguous effects fail closed. A zero-value policy preserves the
// historical unfiltered behavior.
func EvolvePhaseLimit(wf asset.Workflow, policy mode.Policy) (int, error) {
	if wf.Stage != "evolve" || !policyConfigured(policy) {
		return len(wf.Phases), nil
	}
	boundary := -1
	for i, phase := range wf.Phases {
		switch phase.Effect {
		case evolveEffectMutate:
			if phase.Readonly {
				return 0, fmt.Errorf("evolve phase %q declares effect=mutate but is readonly", phase.Name)
			}
			if boundary >= 0 {
				return 0, fmt.Errorf("evolve workflow has multiple effect=mutate boundaries (%q and %q)",
					wf.Phases[boundary].Name, phase.Name)
			}
			boundary = i
		case evolveEffectObserve, evolveEffectPropose, evolveEffectVerify:
			if !phase.Readonly {
				return 0, fmt.Errorf("evolve phase %q effect=%s is an unrestricted writer before proposal-only policy can be enforced",
					phase.Name, phase.Effect)
			}
		default:
			return 0, fmt.Errorf("evolve phase %q has missing/unknown effect %q; want observe|propose|mutate|verify",
				phase.Name, phase.Effect)
		}
	}
	if boundary < 0 {
		return 0, fmt.Errorf("evolve workflow has no explicit effect=mutate boundary")
	}
	if !policy.BuildHalted() && !policy.EvolveProposalOnly() {
		return len(wf.Phases), nil
	}
	for _, phase := range wf.Phases[:boundary] {
		if phase.WritesADR != nil {
			return 0, fmt.Errorf("proposal-only evolve phase %q forbids directory-scoped writes_adr", phase.Name)
		}
		if len(phase.RequiredGates) > 0 {
			return 0, fmt.Errorf("proposal-only evolve phase %q forbids host required_gates", phase.Name)
		}
	}
	return boundary, nil
}

func policyConfigured(policy mode.Policy) bool {
	return len(policy.Gates) > 0 || policy.Reviewer
}

// applyEvolveCutoff returns the executable prefix selected by
// EvolvePhaseLimit, retaining logging at the point where the capability is
// removed. The shallow copy leaves the caller's asset unchanged.
func (e Engine) applyEvolveCutoff(wf asset.Workflow) (asset.Workflow, error) {
	limit, err := EvolvePhaseLimit(wf, e.ModePolicy)
	if err != nil {
		return wf, err
	}
	if limit == len(wf.Phases) {
		return wf, nil
	}
	reason := "evolve authority is proposal-only"
	if e.ModePolicy.BuildHalted() {
		reason = "build=halt forbids implementation"
	}
	e.logf("evolve stage cutoff before phase %s (mode gating: %s)", wf.Phases[limit].Name, reason)
	wf.Phases = wf.Phases[:limit]
	return wf, nil
}

// narrateADR reports the ADR gating verdict for a design-stage phase that declares
// writes_adr (design.yml's solution-architect). This function narrates only — when the mode
// policy requires an ADR (Policy.ADR, e.g. engineering/cto) it logs that an ADR is
// required; otherwise (explorer/balanced) it logs that an ADR is not required and
// will be skipped. It is a no-op unless gating is active, this is the design stage,
// and the phase actually carries writes_adr — so it never fires for build/discover
// or for a phase with no ADR marker (back-compat: zero policy narrates nothing).
//
// HONESTY: under the dry-run executor this is the gating DECISION + narration —
// whether an ADR is required, NOT a real ADR written. Under command execution a
// live agent must satisfy the separate post-run artifact contract; this layer
// never pretends narration itself authored a document.
func (e Engine) narrateADR(wf asset.Workflow, p asset.Phase) {
	if !e.gatingActive() || wf.Stage != "design" || p.WritesADR == nil {
		return
	}
	if e.ModePolicy.ADR {
		e.logf("phase %s: ADR required (mode gating: design writes an ADR) — narrated; real ADR needs a live agent", p.Name)
		return
	}
	e.logf("phase %s: ADR not required (mode gating: explorer/balanced skip ADR) — narrated", p.Name)
}

// requiredWhenKey extracts a RequiredWhen's trailing identifier — the part after
// the last '#' (a fragment) and the last '.' (a dotted path) — so build.yml's
// "../policies/modes.yml#workflow_depth.reviewer" reduces to "reviewer". A bare
// value (no '#'/'.') is returned as-is; an empty value yields "". This is the
// orchestrator interpreting the fragment the asset stored verbatim.
func requiredWhenKey(rw string) string {
	if i := strings.LastIndex(rw, "#"); i >= 0 {
		rw = rw[i+1:]
	}
	if i := strings.LastIndex(rw, "."); i >= 0 {
		rw = rw[i+1:]
	}
	return rw
}
