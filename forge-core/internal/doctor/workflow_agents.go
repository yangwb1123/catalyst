package doctor

import (
	"encoding/json"
	"fmt"
)

// CheckWorkflowAgents extracts every `agent:` reference from a workflow's
// already-parsed YAML→JSON bytes and checks each resolves to a known
// .agent/agents/*.md card — the (no --models) `forge validate` agent check.
// Phases are decoded via encoding/json (top-level `phases:` for one-shot
// spine stages, or nested `loop.phases:` for standing loops like evolve.yml)
// — never by scanning the JSON text line-by-line, which only matches
// pretty-printed, space-after-colon output and silently finds nothing
// against the compact JSON this codebase actually produces (json.Marshal on
// a decoded YAML document, single-line, no space after ":").
//
// label identifies the workflow in returned finding messages; the caller's
// choice of path form (full glob match or root-relative) round-trips
// verbatim, unexamined. Pure: no I/O, mirrors EvaluateWorkflowModels's
// finding-return convention (findings ← caller renders/prints, this
// function decides only PASS/FAIL).
func CheckWorkflowAgents(label string, out []byte, known map[string]bool) (bool, []ModelsFinding) {
	var doc struct {
		Phases []struct {
			Agent string `json:"agent"`
		} `json:"phases"`
		Loop struct {
			Phases []struct {
				Agent string `json:"agent"`
			} `json:"phases"`
		} `json:"loop"`
	}
	if err := json.Unmarshal(out, &doc); err != nil {
		return false, []ModelsFinding{{Level: "FAIL",
			Message: fmt.Sprintf("%s — unparseable JSON: %v", label, err)}}
	}
	phases := doc.Phases
	if len(phases) == 0 {
		phases = doc.Loop.Phases
	}
	seen := map[string]bool{}
	for _, p := range phases {
		if p.Agent != "" {
			seen[p.Agent] = true
		}
	}

	var findings []ModelsFinding
	ok := true
	for agent := range seen {
		if !known[agent] && agent != "harness" {
			findings = append(findings, ModelsFinding{Level: "FAIL",
				Message: fmt.Sprintf("%s — unknown agent %q (no .agent/agents/%s.md)", label, agent, agent)})
			ok = false
		}
	}
	return ok, findings
}
