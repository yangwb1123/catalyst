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
type Phase struct {
	Name          string   `json:"name"`
	Agent         string   `json:"agent"`
	Readonly      bool     `json:"readonly"`
	RequiredGates []string `json:"required_gates"`
}

// StopCondition is the workflow's convergence predicate, matching the real
// .agent/workflows schema. ForgeOS forbids round-count termination: Type names
// the condition shape (e.g. "conjunction"), AllOf carries its typed criteria,
// and AntiPattern records the forbidden shortcut (e.g. "round_count").
// forge-core loads AllOf and evaluates it live (internal/converge) against
// roadmap completion and gate state — convergence is computed, not just declared.
type StopCondition struct {
	Type        string      `json:"type"`
	AllOf       []Criterion `json:"all_of"`
	AntiPattern string      `json:"anti_pattern"`
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
