// Package asset loads ForgeOS declarative workflow assets.
//
// Workflows are authored as YAML under .agent/workflows/, but YAML is not in
// the Go standard library and forge-core takes zero external dependencies. So
// the orchestration runtime consumes JSON: assets are transcoded to JSON
// out-of-band (a python shim today; a Go YAML lib later) and parsed here with
// the stdlib encoding/json.
//
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing. The
// governance layer already has a strict validator (harness/check.py); this
// loader's job is to feed the engine, not to re-litigate schema validity.
package asset

import (
	"encoding/json"
	"fmt"
)

// Phase is one step in a workflow: an agent acts under a stage label, possibly
// read-only, possibly fronted by required gates that must pass before the
// agent runs (the harness phase in build.yml is the canonical example).
//
// RequiredWhen carries an OPTIONAL mode-gating condition for the phase
// (build.yml's reviewer phase: required_when:
// ../policies/modes.yml#workflow_depth.reviewer). The loader stores the value
// VERBATIM, fragment and all — asset's job is to feed the engine, not to resolve
// policy references. The orchestrator interprets it by its trailing identifier
// (the part after '#...'), e.g. "reviewer", and consults the mode Policy. An
// empty RequiredWhen means "always run" (the default for every other phase), so
// adding the field changes no existing phase's behavior.
//
// OnFail carries an OPTIONAL directed loop-back for a GATE phase (build.yml's
// harness-gates/reviewer/qa phases: on_fail: {action: loop_back, target_phase:
// implementer}). It is a POINTER so a phase without the key loads as nil — the
// fault-tolerant default the orchestrator reads as "no loop-back, abort on a red
// gate" (byte-for-byte the pre-loop-back behavior). The orchestrator acts on it
// only when Action=="loop_back": it jumps back to the phase named TargetPhase and
// re-runs forward to here, bounded by Engine.MaxLoopBack (fail-closed when spent).
//
// ModelTier carries an OPTIONAL per-phase model-tier OVERRIDE authored in the
// workflow (build.yml implementer: model_tier: sonnet, reviewer: opus; design.yml
// solution-architect: opus). It is a plain string so a phase without the key loads
// as "" — the fault-tolerant default the orchestrator reads as "no override, use
// the agent's routed tier". When set, it can only RAISE the routed tier, never
// lower a safety floor: the orchestrator (phaseTier) takes the HIGHER of this hint
// and routing.TierFor's verdict, so a phase that writes model_tier: haiku on the
// reviewer/architect still routes to Opus. Honest scope: under the dry-run executor
// the tier is narrative/prompt-fidelity only — no model is actually invoked.
//
// WritesADR carries an OPTIONAL marker that THIS phase produces an Architecture
// Decision Record (design.yml's solution-architect: writes_adr: {condition, target}).
// It is a POINTER so a phase without the key loads as nil — the fault-tolerant
// default the orchestrator reads as "this phase writes no ADR". The orchestrator
// acts on it only for the design stage: when a phase declares writes_adr it NARRATES
// whether an ADR is required under the mode policy (Policy.ADR). Honest scope: under
// the dry-run executor this is a gating-decision narration — whether an ADR is
// required, not a real ADR written; that needs a real agent (the target dir is
// enabled from v2 per design.yml).
type Phase struct {
	Name          string     `json:"name"`
	Agent         string     `json:"agent"`
	Readonly      bool       `json:"readonly"`
	RequiredGates []string   `json:"required_gates"`
	RequiredWhen  string     `json:"required_when"`
	OnFail        *OnFail    `json:"on_fail"`
	ModelTier     string     `json:"model_tier"`
	WritesADR     *WritesADR `json:"writes_adr"`
}

// WritesADR is the subset of a phase's writes_adr block forge-core reads:
// Condition is the human-readable rule the asset authored (design.yml: "mode in
// [engineering, cto]"), Target the destination dir (docs/adr/, enabled from v2).
// Its mere PRESENCE (a non-nil pointer) is the signal the orchestrator keys on —
// this phase is the one that would write an ADR — while the actual required/not
// verdict comes from the mode Policy, not from re-parsing Condition here.
type WritesADR struct {
	Condition string `json:"condition"`
	Target    string `json:"target"`
}

// OnFail is a gate phase's directed loop-back directive: when its gates FAIL,
// Action=="loop_back" tells the orchestrator to jump back to the phase named
// TargetPhase (by name) and re-run from there rather than aborting. Any other
// Action (or a nil OnFail) leaves the legacy abort-on-red behavior intact.
type OnFail struct {
	Action      string `json:"action"`
	TargetPhase string `json:"target_phase"`
}

// StopCondition is the workflow's convergence predicate, matching the real
// .agent/workflows schema. ForgeOS forbids round-count termination: Type names
// the condition shape (e.g. "conjunction"), AllOf carries its typed criteria,
// and AntiPattern records the forbidden shortcut (e.g. "round_count").
// forge-core loads AllOf and evaluates it live (internal/converge) against
// roadmap completion and gate state — convergence is computed, not just declared.
//
// HumanApproval names the human_gate variant (design.yml): when it is "required"
// (or Type=="human_gate"), the stage's single convergence key is an explicit
// human approval signal, NOT a conjunction. It is the highest-leverage gate in
// the system and is non-bypassable — see internal/converge.Converge. OnApproved
// carries what an approval unlocks (the next spine stage), surfaced in the report.
// OnUnmet carries an OPTIONAL directed restart for a conjunction stop that did
// NOT converge this iteration (build.yml: on_unmet: {action:
// loop_to_next_roadmap_item, target_phase: planner}). Like Phase.OnFail it is a
// POINTER so its absence loads as nil — the fault-tolerant default the loop reads
// as "no directed restart" (the next iteration re-runs every phase, byte-for-byte
// the pre-on_unmet behavior). When Action=="loop_to_next_roadmap_item" the loop
// begins each subsequent iteration at the phase named TargetPhase (the planner,
// to pull the next roadmap item) rather than at phase 0.
type StopCondition struct {
	Type          string      `json:"type"`
	AllOf         []Criterion `json:"all_of"`
	AntiPattern   string      `json:"anti_pattern"`
	HumanApproval string      `json:"human_approval"`
	OnApproved    OnApproved  `json:"on_approved"`
	OnUnmet       *OnUnmet    `json:"on_unmet"`
}

// OnUnmet is a conjunction stop's directed-restart directive: when the stop is
// not yet met, Action=="loop_to_next_roadmap_item" tells the loop to begin the
// next iteration at the phase named TargetPhase (the planner). A nil OnUnmet (or
// any other Action) keeps the legacy whole-workflow replay each iteration.
type OnUnmet struct {
	Action      string `json:"action"`
	TargetPhase string `json:"target_phase"`
}

// OnApproved is the subset of a human_gate's on_approved block forge-core needs:
// NextStage is the spine stage an approval unlocks (design.yml -> "build"). The
// emit list is materialized by the agent layer, not the runtime, so it is not
// modeled here — only the routing-relevant NextStage is.
type OnApproved struct {
	NextStage string `json:"next_stage"`
}

// Criterion is one structured stop criterion. The real build.yml authors these
// as JSON objects (e.g. {metric: roadmap_completion, operator: '==', threshold:
// 100}); some fixtures author them as a bare human-readable string. UnmarshalJSON
// accepts both forms — an object populates the typed fields, a bare string is
// preserved in Raw so nothing is silently dropped.
type Criterion struct {
	Metric    string   `json:"metric"`
	Operator  string   `json:"operator"`
	Threshold *float64 `json:"threshold"`
	Value     string   `json:"value"`
	Raw       string   `json:"-"` // set only when the criterion was a bare string
}

// UnmarshalJSON accepts a Criterion as either a JSON object or a bare JSON
// string. A bare string is stored in Raw (left for human-string fixtures); an
// object decodes into the typed fields via an alias to avoid infinite recursion.
func (c *Criterion) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err == nil {
		*c = Criterion{Raw: s}
		return nil
	}
	type alias Criterion // alias drops the custom UnmarshalJSON
	var a alias
	if err := json.Unmarshal(data, &a); err != nil {
		return fmt.Errorf("asset: criterion must be a string or object: %w", err)
	}
	*c = Criterion(a)
	return nil
}

// Workflow is a loaded, machine-runnable workflow: an ordered list of phases
// for one spine stage plus the stop condition that decides when it converges.
type Workflow struct {
	Stage  string        `json:"stage"`
	Phases []Phase       `json:"phases"`
	Stop   StopCondition `json:"stop_condition"`
}

// LoadWorkflowJSON parses a workflow from its JSON encoding.
//
// It is fault tolerant by design: only a syntactically invalid document is an
// error. A document missing fields (no stage, no phases, no stop) yields a
// zero-valued-but-usable Workflow so the engine can still run what is present
// rather than crashing on partial assets.
func LoadWorkflowJSON(data []byte) (Workflow, error) {
	var wf Workflow
	if err := json.Unmarshal(data, &wf); err != nil {
		return Workflow{}, fmt.Errorf("asset: invalid workflow JSON: %w", err)
	}
	return wf, nil
}
