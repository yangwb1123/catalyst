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
type Phase struct {
	Name          string   `json:"name"`
	Agent         string   `json:"agent"`
	Readonly      bool     `json:"readonly"`
	RequiredGates []string `json:"required_gates"`
	RequiredWhen  string   `json:"required_when"`
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
type StopCondition struct {
	Type          string      `json:"type"`
	AllOf         []Criterion `json:"all_of"`
	AntiPattern   string      `json:"anti_pattern"`
	HumanApproval string      `json:"human_approval"`
	OnApproved    OnApproved  `json:"on_approved"`
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
