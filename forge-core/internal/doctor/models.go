package doctor

import (
	"encoding/json"
	"fmt"
	"path/filepath"
)

// ModelsFinding is one line of `forge validate --models` output, classified
// by severity to match the CLI's [PASS]/[WARN]/[FAIL] markers.
type ModelsFinding struct {
	Level   string // "PASS" | "WARN" | "FAIL"
	Message string
}

// workflowPhase is the subset of one `phases:` (or `loop.phases:`) list entry
// that forge validate --models inspects, decoded straight off the workflow's
// parsed YAML→JSON bytes via encoding/json — NOT scraped from pretty-printed
// text, so it is immune to the source JSON's whitespace/formatting (compact
// or indented, space-or-no-space-after-colon all decode identically).
type workflowPhase struct {
	Name              string        `json:"name"`
	Agent             string        `json:"agent"`
	UsesTemplate      string        `json:"uses_template"`
	SecondaryTemplate string        `json:"secondary_template"`
	OnFail            *onFailTarget `json:"on_fail"`
}

// onFailTarget mirrors a phase's `on_fail:` block, the only sub-field of
// which forge validate --models checks.
type onFailTarget struct {
	TargetPhase string `json:"target_phase"`
}

// workflowDoc is the top-level shape of a parsed workflow YAML→JSON document.
// Phases live either directly under `phases:` (one-shot spine stages like
// build/design/discover/review) or nested under `loop.phases:` (standing
// loops like evolve.yml) — never both, so EvaluateWorkflowModels falls back
// to Loop.Phases only when Phases is empty.
type workflowDoc struct {
	Phases []workflowPhase `json:"phases"`
	Loop   struct {
		Phases []workflowPhase `json:"phases"`
	} `json:"loop"`
}

// EvaluateWorkflowModels performs the cross-model consistency checks (forge
// validate --models, sixth-wave-multimodel.md §方向1) against ONE workflow's
// already-parsed YAML→JSON bytes: every `agent` reference must resolve to a
// known .agent/agents/*.md card, `uses_template` AND the OPTIONAL
// `secondary_template` paths are each checked against aiTemplates (basenames
// present in .ai/prompts/ — review.yml's performance-reliability-review phase
// declares both, one phase paired with two review dimensions), and each
// on_fail.target_phase must name a phase declared EARLIER in this same
// sequential scan — a target_phase forward-reference is reported as
// not-found, since this mirrors a single-pass workflow model rather than a
// full two-pass one. Pure: no I/O.
func EvaluateWorkflowModels(rel string, out []byte, knownAgents, aiTemplates map[string]bool) (bool, []ModelsFinding) {
	var doc workflowDoc
	if err := json.Unmarshal(out, &doc); err != nil {
		return false, []ModelsFinding{{Level: "FAIL",
			Message: fmt.Sprintf("%s — unparseable workflow JSON: %v", rel, err)}}
	}
	phases := doc.Phases
	if len(phases) == 0 {
		phases = doc.Loop.Phases
	}

	var findings []ModelsFinding
	ok := true
	seenPhases := map[string]bool{}
	for _, p := range phases {
		if p.Agent != "" && p.Agent != "harness" && !knownAgents[p.Agent] {
			findings = append(findings, ModelsFinding{Level: "FAIL",
				Message: fmt.Sprintf("%s — agent %q has no .agent/agents/%s.md", rel, p.Agent, p.Agent)})
			ok = false
		}
		if p.UsesTemplate != "" {
			ok = evaluateUsesTemplate(rel, p.UsesTemplate, aiTemplates, &findings) && ok
		}
		if p.SecondaryTemplate != "" {
			ok = evaluateSecondaryTemplate(rel, p.SecondaryTemplate, aiTemplates, &findings) && ok
		}
		// A phase's own name is registered BEFORE its on_fail is checked, so a
		// self-referencing loop-back (a phase whose on_fail.target_phase names
		// itself — e.g. review.yml's security-review re-running itself on
		// failure) is a valid, already-"seen" reference, not a false FAIL.
		if p.Name != "" {
			seenPhases[p.Name] = true
		}
		if p.OnFail != nil && p.OnFail.TargetPhase != "" && !seenPhases[p.OnFail.TargetPhase] {
			findings = append(findings, ModelsFinding{Level: "FAIL",
				Message: fmt.Sprintf("%s — on_fail.target_phase %q not found in workflow phases", rel, p.OnFail.TargetPhase)})
			ok = false
		}
	}
	return ok, findings
}

// evaluateUsesTemplate appends a PASS/WARN finding for one phase's
// uses_template value; it never affects the workflow's overall ok/FAIL
// verdict (a missing template is a WARN, not a FAIL), so it always returns true.
func evaluateUsesTemplate(rel, tmpl string, aiTemplates map[string]bool, findings *[]ModelsFinding) bool {
	return evaluateTemplateField(rel, "uses_template", tmpl, aiTemplates, findings)
}

// evaluateSecondaryTemplate appends a PASS/WARN finding for one phase's
// OPTIONAL secondary_template value — the second AI-SDLC template review.yml's
// performance-reliability-review phase pairs alongside uses_template (05-
// performance-review.md + 06-production-readiness.md, one phase, two review
// dimensions). Mirrors evaluateUsesTemplate exactly: missing is a WARN, not a
// FAIL, so it always returns true.
func evaluateSecondaryTemplate(rel, tmpl string, aiTemplates map[string]bool, findings *[]ModelsFinding) bool {
	return evaluateTemplateField(rel, "secondary_template", tmpl, aiTemplates, findings)
}

// evaluateTemplateField appends a PASS/WARN finding for one phase's template
// field — shared by evaluateUsesTemplate and evaluateSecondaryTemplate so the
// existence check, PASS/WARN classification, and message shape stay in ONE
// place for both fields. fieldName (e.g. "uses_template" / "secondary_template")
// only feeds the finding message so it names WHICH field the path came from.
// Never affects the workflow's overall ok/FAIL verdict (a missing template is a
// WARN, not a FAIL), so it always returns true.
func evaluateTemplateField(rel, fieldName, tmpl string, aiTemplates map[string]bool, findings *[]ModelsFinding) bool {
	if !aiTemplates[filepath.Base(tmpl)] {
		*findings = append(*findings, ModelsFinding{Level: "WARN",
			Message: fmt.Sprintf("%s — %s %q not found in .ai/prompts/", rel, fieldName, tmpl)})
	} else {
		*findings = append(*findings, ModelsFinding{Level: "PASS",
			Message: fmt.Sprintf("%s — %s %q exists", rel, fieldName, tmpl)})
	}
	return true
}
