// Package asset loads ForgeOS declarative workflow assets.
//
// Workflows are authored as YAML under .agent/workflows/. The CLI first parses
// the shipped subset with internal/yaml2json (pure Go, zero dependencies), then
// decodes the normalized JSON here. The Python converter is only a compatibility
// fallback for input outside the native subset.
//
// Parsing tolerates missing or extra top-level fields, but machine identities
// are fail-closed: phase names and per-phase emit targets must be unambiguous.
// The governance layer (harness/check.py) enforces the same structural contract
// before assets reach this runtime boundary.
package asset

import (
	"encoding/json"
	"fmt"
	"path"
	"strings"
)

// Phase is one step in a workflow: an agent acts under a stage label, possibly
// read-only, possibly fronted by required gates that must pass before it runs.
// `agent: harness` is the reserved gate-only pseudo-agent; every other phase
// continues to its agent after its required gates pass.
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
// default the orchestrator reads as "this phase writes no ADR". Dry-run narrates
// the policy decision; command execution validates the condition/target, grants
// the bounded ADR write scope, and applies the post-run ADR artifact contract.
//
// FeedsForward carries an OPTIONAL marker that THIS phase's output is RELEVANT to
// LATER phases and so should be remembered and injected into their prompts (build.yml /
// evolve.yml author it on the planner: feeds_forward: true — its sprint split / acceptance
// criteria steer the implementer and reviewer). It is a plain bool so a phase without
// the key loads as false — the fault-tolerant default the orchestrator reads as "this
// phase's output is NOT fed forward" (byte-for-byte the pre-feed-forward behavior: no
// phase output is remembered, the downstream prompt is unchanged). asset stays a
// GENERIC carrier: the bool says only "remember my output", with no vendor or
// phase-output semantics — the cmd/forge layer owns the ledger that records it and the
// prompt block that injects it (the same bright-line ModelTier draws for `claude
// --model`). CORRECTNESS: only a planning/task-definition role should set it; a
// reviewer must NOT (feeding the reviewer its own prior self-report would pollute the
// fresh-context independence that makes its judgement trustworthy).
// DependsOn carries an OPTIONAL list of phase NAMES this phase must run AFTER — the
// declarative input to the OPT-IN parallel orchestrator (`forge run/evolve --parallel`).
// When a workflow declares depends_on, the parallel engine groups phases into
// dependency-ordered WAVES and runs the mutually-independent phases within a wave
// concurrently (the Discover scan/market/capability fan-out, or fan-out implementers,
// no longer block each other). An EMPTY DependsOn (every phase, in every existing
// workflow today) means "no declared dependency": the field changes NOTHING for the
// default SERIAL engine (RunFrom ignores it), so adding it is byte-for-byte back-compat.
//
// FreshContext is an OPTIONAL flag (default false) that tells the prompt builder to
// give this phase a "clean slate" — skip feeding forward prior phase outputs, gate
// results, and findings from other phases. This implements the engineering rule
// (BOOTSTRAP.md, AGENTS.md) that Reviewer must be a fresh-context independent agent
// that does not see the implementer's output or prior gate results, preventing
// anchoring bias (asset-runtime-gap.md §1.2). A zero-value (false) means "normal
// context inheritance", byte-for-byte back-compat with every existing workflow.
//
// Emits is an OPTIONAL list of repository-relative file paths that this phase is
// declared to produce (including valid root files such as report.md). These declare the
// inter-phase data dependencies that are currently implicit in YAML comments.
// When populated, the prompt builder can read and inject the actual content of
// emitted files into downstream phases that depend on them. A zero-value (nil/empty)
// means "this phase emits no declarative artifacts", byte-for-byte back-compat.
//
// ConfidenceMetric is an OPTIONAL metric name that this phase's agent output is
// expected to carry a confidence score for (discover.yml's requirement-discovery
// phase: confidence_metric: requirement_confidence). Its value is parsed from the
// agent's output (e.g. "Confidence: 85/100") and fed into converge.Signals so the
// stop condition can evaluate it. An empty string means "this phase does not
// produce a confidence metric", byte-for-byte back-compat.
//
// OptionalFor is an OPTIONAL list of mode names for which this phase MAY be
// skipped (discover.yml's market-research: optional_for: [balanced]). When the
// current mode is in this list, the orchestrator's skipByMode may skip the phase;
// when absent or the mode is not listed, the phase always runs (the default).
// A nil/empty list means "always run", unchanged from existing behavior.
//
// UsesTemplate is an OPTIONAL path to an AI-SDLC template file (review.yml's
// phases reference .ai/prompts/*.md via uses_template). When populated, the
// prompt builder can read the template content and inject it as contextual
// guidance for this phase. An empty string means "this phase does not reference
// an external template", byte-for-byte back-compat.
//
// RequiresTools is an OPTIONAL list of tool names a phase's agent needs to do
// real work (discover.yml's market-research: requires_tools: [web_search,
// web_fetch] — the phase's own comment states the intended behavior: degrade to
// advisory + flag when the tool is absent, never silently fabricate results).
// The command prompt builder consumes it through requiresToolsGuard: an
// unavailable/unconfirmed tool degrades visibly to advisory. A nil/empty list
// means "this phase declares no tool requirement", byte-for-byte back-compat.
//
// Readonly is an OPTIONAL marker (authored at BOTH workflow level and per-phase
// across every .agent/workflows/*.yml — e.g. review.yml sets it true at the
// top level and again on every phase; build.yml varies it per phase: false on
// implementer, true on planner/harness-gates/reviewer/qa) that the phase's
// agent is expected to only read, never write product code or other state.
// Command execution enforces it with `dontAsk` and a validated per-role write
// scope for declared analysis artifacts; product-code writes remain denied.
// A zero value (false) means "no read-only declaration", byte-for-byte back-compat.
//
// Effect is an OPTIONAL machine-readable phase side-effect class. Evolve
// workflows use observe|propose|mutate|verify so proposal-only mode can locate
// the explicit product-mutation boundary without trusting a phase or agent name.
// The orchestrator validates the complete Evolve effect shape before applying
// that policy. Other workflow stages and legacy zero-policy callers ignore an
// empty Effect, preserving their existing behavior.
//
// VerdictContract is an OPTIONAL machine-verdict protocol selected explicitly by
// the workflow. The zero value preserves the advisory verdict behavior; qa_v1
// requires the phase's final non-empty output line to carry the strict Build QA
// handshake. Runtime code keys on this field, never on a phase or agent name.
//
// ScanContract is an OPTIONAL machine-report protocol selected explicitly by an
// Evolve observe phase. evolve_scan_v1 binds the phase to the effective
// mode×lifecycle EvolveDepth, requires a strict evidence-bearing final-line report,
// and feeds its normalized complete output to later phases. The zero value keeps
// custom/legacy workflows unchanged and makes no content-breadth claim.
//
// SecondaryTemplate is an OPTIONAL second AI-SDLC template path alongside
// UsesTemplate (review.yml's performance-reliability-review phase pairs
// uses_template: .../05-performance-review.md with secondary_template:
// .../06-production-readiness.md — one phase, two review dimensions). The prompt
// builder injects it with the same validated containment rules as UsesTemplate,
// and doctor validates the reference. An empty string means "no secondary
// template", byte-for-byte back-compat.
type Phase struct {
	Name              string     `json:"name"`
	Agent             string     `json:"agent"`
	VerdictContract   string     `json:"verdict_contract,omitempty"`
	ScanContract      string     `json:"scan_contract,omitempty"`
	Description       string     `json:"description,omitempty"`
	RequiredGates     []string   `json:"required_gates"`
	RequiredWhen      string     `json:"required_when"`
	OnFail            *OnFail    `json:"on_fail"`
	ModelTier         string     `json:"model_tier"`
	WritesADR         *WritesADR `json:"writes_adr"`
	FeedsForward      bool       `json:"feeds_forward"`
	DependsOn         []string   `json:"depends_on"`
	FreshContext      bool       `json:"fresh_context,omitempty"`
	Emits             []string   `json:"emits,omitempty"`
	ConfidenceMetric  string     `json:"confidence_metric,omitempty"`
	OptionalFor       []string   `json:"optional_for,omitempty"`
	UsesTemplate      string     `json:"uses_template,omitempty"`
	RequiresTools     []string   `json:"requires_tools,omitempty"`
	Readonly          bool       `json:"readonly,omitempty"`
	Effect            string     `json:"effect,omitempty"`
	SecondaryTemplate string     `json:"secondary_template,omitempty"`
}

const (
	VerdictContractQAV1       = "qa_v1"
	VerdictContractReviewerV1 = "reviewer_v1"
	VerdictContractReviewerV2 = "reviewer_v2"
)

// WritesADR is the subset of a phase's writes_adr block forge-core reads:
// Condition is the rule the asset authored (design.yml: "mode in
// [engineering, cto]"), Target the bounded destination directory (docs/adr/).
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
	DurableWait   bool        `json:"durable_wait"`
	Expression    string      `json:"expression"`
	OnApproved    OnApproved  `json:"on_approved"`
	OnUnmet       *OnUnmet    `json:"on_unmet"`
	OnRejected    *LoopBack   `json:"on_rejected,omitempty"`
	OnMet         *OnMet      `json:"on_met,omitempty"`
}

// OnUnmet is a conjunction stop's directed-restart directive: when the stop is
// not yet met, Action=="loop_to_next_roadmap_item" tells the loop to begin the
// next iteration at the phase named TargetPhase (the planner). A nil OnUnmet (or
// any other Action) keeps the legacy whole-workflow replay each iteration.
type OnUnmet struct {
	Action      string `json:"action"`
	TargetPhase string `json:"target_phase"`
}

// OnApproved is the supported subset of an on_approved transition: NextStage is
// the spine stage an approval unlocks. Approval has routing semantics only;
// artifacts must be owned by phase-level Emits/WritesADR. The governance checker
// rejects on_approved.emit so an unsupported side effect cannot be silently lost.
type OnApproved struct {
	NextStage string `json:"next_stage"`
}

// OnMet carries the directive for when a conjunction stop converges MET
// (discover.yml, build.yml). NextStage names the spine stage to advance to
// (e.g. discover -> design, build -> evolve).
type OnMet struct {
	NextStage string `json:"next_stage"`
}

// LoopBack is a directed jump-back directive shared by StopCondition.OnRejected
// and Phase.OnFail. Action names the kind of jump ("loop_back") and TargetPhase
// names the phase to return to. A nil LoopBack means "no jump".
// This type exists so the same semantics (jump to phase, bounded by MaxLoopBack)
// are used by both the gate on_fail path and the human-gate on_rejected path
// without duplicating the struct definition.
type LoopBack struct {
	Action      string `json:"action"`
	TargetPhase string `json:"target_phase"`
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
//
// Loop carries a STANDING-LOOP workflow's body (evolve.yml, type: loop): its
// phases are authored under `loop:` (with loop_back_to forming the cycle) rather
// than at the top level. LoadWorkflowJSON HOISTS Loop.Phases into Phases when the
// top level declares none, so the engine runs the loop body. Without this, a
// `type: loop` workflow's `phases` (evolve.yml's 7: scan…evaluate) were dropped —
// the engine loaded ZERO phases and the loop reported converged=true over no work
// (a false-clean: zero work read as success). It is a POINTER so a non-loop
// workflow (build.yml) loads it as nil and is byte-for-byte unaffected.
//
// Readonly is an OPTIONAL workflow-level marker (every one of the 5
// .agent/workflows/*.yml authors it at the top level — discover.yml/review.yml
// true, build.yml/evolve.yml/design.yml false — a stage-wide default that a
// phase's own Readonly can narrow further). The command executor enforces the
// effective per-phase read-only boundary; the workflow value is retained for
// schema fidelity and stage policy checks. A zero value (false) means "no
// stage-wide read-only declaration", byte-for-byte back-compat.
type Workflow struct {
	ID                    string        `json:"id,omitempty"`
	Stage                 string        `json:"stage"`
	Phases                []Phase       `json:"phases"`
	Loop                  *LoopBody     `json:"loop"`
	Stop                  StopCondition `json:"stop_condition"`
	Readonly              bool          `json:"readonly,omitempty"`
	OutputBindingContract string        `json:"output_binding_contract,omitempty"`
}

// LoopBody is a standing-loop workflow's `loop:` block: the phases that form the
// cycle plus LoopBackTo, the phase the loop returns to after the last (evolve.yml:
// loop_back_to: scan). Only Phases is hoisted into the runnable Workflow today;
// LoopBackTo is carried so the cycle target is not silently dropped on load.
type LoopBody struct {
	LoopBackTo string  `json:"loop_back_to"`
	Phases     []Phase `json:"phases"`
}

// ValidateWorkflowStructure enforces the identity invariants shared by every
// workflow consumer. Phase names are machine identities: every runnable phase
// must have a non-blank, unique name so serial lookup, dependency waves and
// output-contract maps all resolve the same phase. Within one phase, emits must
// not name the same path more than once after portable slash/path cleaning.
//
// Duplicate emit detection is intentionally scoped to one phase. Reusing an
// emit in a later phase remains legal: a later owner may deliberately revise an
// earlier artifact, and that cross-phase ownership policy is not an identity
// ambiguity.
func ValidateWorkflowStructure(wf Workflow) error {
	seenNames := make(map[string]int, len(wf.Phases))
	for i, phase := range wf.Phases {
		if strings.TrimSpace(phase.Name) == "" {
			return fmt.Errorf("asset: phase[%d] has an empty name", i)
		}
		if phase.Agent == "release-engineer" && wf.Stage != "" &&
			wf.Stage != "deploy" && wf.Stage != "rollback" {
			return fmt.Errorf(
				"asset: release-engineer phase %q is only permitted in deploy/rollback workflows, not stage %q",
				phase.Name, wf.Stage,
			)
		}
		if err := validateVerdictContract(wf.Stage, phase); err != nil {
			return err
		}
		if first, ok := seenNames[phase.Name]; ok {
			return fmt.Errorf(
				"asset: phase[%d] duplicates phase name %q first declared at phase[%d]",
				i, phase.Name, first,
			)
		}
		seenNames[phase.Name] = i

		seenEmits := make(map[string]string, len(phase.Emits))
		for _, emit := range phase.Emits {
			normalized := normalizedEmitIdentity(emit)
			if first, ok := seenEmits[normalized]; ok {
				return fmt.Errorf(
					"asset: phase %q emit %q duplicates normalized target %q already declared as %q",
					phase.Name, emit, normalized, first,
				)
			}
			seenEmits[normalized] = emit
		}
	}
	if err := validateVerdictTargets(wf, seenNames); err != nil {
		return err
	}
	if err := validateOutputBindingContract(wf); err != nil {
		return err
	}
	if err := validateScanContracts(wf); err != nil {
		return err
	}
	return nil
}

func containsString(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

// normalizedEmitIdentity is platform-independent: workflow paths are portable
// repository paths, so both slash styles are compared with path (not host OS)
// semantics. It is an identity comparison only; containment and file-shape
// validation remain the output contract's responsibility.
func normalizedEmitIdentity(emit string) string {
	return path.Clean(strings.ReplaceAll(emit, `\`, "/"))
}

// LoadWorkflowJSON parses a workflow from its JSON encoding.
//
// It remains tolerant of omitted top-level fields: a document with no phases is
// a zero-valued-but-usable Workflow. Once phases are present, their machine
// identities are fail-closed through ValidateWorkflowStructure; invalid JSON,
// blank/duplicate phase names and duplicate per-phase emit targets are errors.
func LoadWorkflowJSON(data []byte) (Workflow, error) {
	var wf Workflow
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return Workflow{}, fmt.Errorf("asset: invalid workflow JSON: %w", err)
	}
	if err := json.Unmarshal(data, &wf); err != nil {
		return Workflow{}, fmt.Errorf("asset: invalid workflow JSON: %w", err)
	}
	// Hoist a loop body's phases to the top level so a standing-loop workflow
	// (evolve.yml) RUNS its 7 phases instead of loading zero (the false-clean the
	// Loop field documents). Only when the top level is empty — a workflow that
	// authors top-level phases keeps them verbatim.
	if len(wf.Phases) == 0 && wf.Loop != nil && len(wf.Loop.Phases) > 0 {
		wf.Phases = wf.Loop.Phases
	}
	if err := ValidateWorkflowStructure(wf); err != nil {
		return Workflow{}, err
	}
	return wf, nil
}
