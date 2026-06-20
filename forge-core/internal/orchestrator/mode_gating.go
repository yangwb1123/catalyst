package orchestrator

import (
	"strings"

	"forgeos/forge-core/internal/asset"
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
// full policy keeps them all. Order follows the phase's declaration. An empty
// result is legal — that gate phase simply has no gate to run under this mode.
func (e Engine) gatesFor(p asset.Phase) []string {
	if !e.gatingActive() {
		return p.RequiredGates
	}
	kept := make([]string, 0, len(p.RequiredGates))
	for _, g := range p.RequiredGates {
		if e.ModePolicy.Allows(g) {
			kept = append(kept, g)
		}
	}
	return kept
}

// skipByMode reports whether a non-gate phase should be skipped by the mode
// policy. The only modeled condition (this slice) is the OPTIONAL reviewer: a
// phase whose RequiredWhen resolves to "reviewer" is skipped iff gating is active
// AND the policy makes the reviewer non-mandatory (explorer). With gating
// inactive (zero policy) nothing is ever skipped — full back-compat. A phase with
// no RequiredWhen, or a RequiredWhen naming any other condition, is never skipped
// here (honest: only the reviewer dimension is wired in this slice).
func (e Engine) skipByMode(p asset.Phase) bool {
	if !e.gatingActive() {
		return false
	}
	return requiredWhenKey(p.RequiredWhen) == "reviewer" && !e.ModePolicy.Reviewer
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

// narrateADR reports the ADR gating verdict for a design-stage phase that declares
// writes_adr (design.yml's solution-architect). It NARRATES only — when the mode
// policy requires an ADR (Policy.ADR, e.g. engineering/cto) it logs that an ADR is
// required; otherwise (explorer/balanced) it logs that an ADR is not required and
// will be skipped. It is a no-op unless gating is active, this is the design stage,
// and the phase actually carries writes_adr — so it never fires for build/discover
// or for a phase with no ADR marker (back-compat: zero policy narrates nothing).
//
// HONESTY: under the dry-run executor this is the gating DECISION + narration —
// whether an ADR is required, NOT a real ADR written (design.yml enables the ADR
// target dir from v2; a real agent writes the document). The runtime does not
// pretend an ADR was authored; it reports the required/not-required verdict the
// mode policy dictates.
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
