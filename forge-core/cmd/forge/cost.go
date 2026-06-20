// cost.go — the claude-specific cost-telemetry boundary of the forge CLI. ALL
// knowledge of the claude `-p --output-format json` envelope (its total_cost_usd /
// result fields) lives here, deliberately isolated from the generic runtime: the
// orchestrator's CommandExecutor (command_executor.go) and LoopEngine (loop.go) know
// nothing of claude or cost — they expose a generic Observe sink, and THIS file is the
// only place that parses claude JSON out of it and turns a billed dollar figure into a
// trace event. That keeps the layering bright-line: generic spawner below, vendor
// (claude JSON cost) knowledge here in cmd/forge, so the cost path stays honest and the
// runtime stays vendor-free.
package main

import (
	"encoding/json"
	"math"
	"strings"

	"forgeos/forge-core/internal/trace"
)

// parseClaudeCostUsd extracts the real billed dollar cost from a claude `-p
// --output-format json` envelope's `total_cost_usd`. A *float64 pointer distinguishes
// an ABSENT field from a legitimate 0.0. ok=false (and the caller emits NO cost event,
// never a fabricated 0) when the output is not a single JSON object, carries no
// total_cost_usd, or the value is non-finite — the four honesty branches that keep an
// echo/dry/stub output (not claude JSON) from inventing a cost.
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
	var env struct {
		TotalCostUsd *float64 `json:"total_cost_usd"`
	}
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
		return 0, false // not a single JSON object (echo/dry/stub, or a multi-line tail)
	}
	if env.TotalCostUsd == nil {
		return 0, false // valid JSON but no total_cost_usd -> never fabricate one
	}
	if math.IsNaN(*env.TotalCostUsd) || math.IsInf(*env.TotalCostUsd, 0) {
		return 0, false // non-finite -> not a real cost
	}
	return *env.TotalCostUsd, true
}

// Reviewer-verdict tokens — the machine-readable last line .agent/agents/reviewer.md
// contracts the reviewer to emit (verbatim, top-aligned). VerdictApprove lets the run
// proceed; VerdictRequestChanges drives a directed loop-back to the implementer. These
// are the NORMALIZED constants parseReviewerVerdict returns and the orchestrator compares
// against — the single place the reviewer's prose token is turned into an objective signal.
const (
	VerdictApprove        = "APPROVE"
	VerdictRequestChanges = "REQUEST_CHANGES"
)

// parseReviewerVerdict extracts the reviewer's machine-readable verdict from its raw
// output — the EXACT mirror of parseClaudeCostUsd's isolation: ALL knowledge of the
// reviewer's `VERDICT: …` last-line contract lives HERE in cmd/forge, never in the
// orchestrator (which only reads back an opaque token via Engine.AgentVerdict). It first
// unwrapClaudeResult's the output (so a real claude JSON envelope is reduced to its
// `result` text, while an echo/stub is scanned verbatim — a fake sentinel still matches),
// then takes the LAST non-empty line, trims it, and matches it EXACTLY against the two
// contracted forms. ok=false (and the caller fires NO verdict — never a fabricated one,
// the same honesty branch as cost) whenever the last line is anything else: a missing,
// wrapped, or malformed verdict is treated as "no signal", which the orchestrator maps to
// fail-open (proceed), per the reviewer card's stated contract.
func parseReviewerVerdict(output string) (verdict string, ok bool) {
	last := lastNonEmptyLine(unwrapClaudeResult(output))
	switch last {
	case "VERDICT: " + VerdictApprove:
		return VerdictApprove, true
	case "VERDICT: " + VerdictRequestChanges:
		return VerdictRequestChanges, true
	default:
		return "", false // missing/wrapped/malformed -> no signal (caller fails open)
	}
}

// lastNonEmptyLine returns the last line of s that is non-empty after trimming
// surrounding whitespace, or "" when every line is blank — so a trailing newline (or
// blank tail) the agent appended after its VERDICT line does not mask the verdict.
func lastNonEmptyLine(s string) string {
	lines := strings.Split(s, "\n")
	for i := len(lines) - 1; i >= 0; i-- {
		if t := strings.TrimSpace(lines[i]); t != "" {
			return t
		}
	}
	return ""
}

// unwrapClaudeResult renders a claude JSON envelope down to its human-readable
// `result` string for the log line; any non-JSON output (echo/stub) is returned
// verbatim, so the generic executor's logging stays claude-free and unchanged for
// non-claude commands.
func unwrapClaudeResult(output string) string {
	var env struct {
		Result *string `json:"result"`
	}
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil || env.Result == nil {
		return output
	}
	return *env.Result
}

// costEmitter builds the per-phase cost sink: it converts a billed USD figure to
// integer microdollars and emits one `kind:"agent"` trace event carrying it AND the
// model it was billed against, so the scorecard's --trace reader can aggregate a real
// avg_cost_usd attributed to that model. Microdollars match trace.Event.CostUsdMicros
// (integer, jitter-free); the conversion (USD x 1e6, rounded) is owned here — trace.go
// stays oblivious to both dollars and what a model is.
//
// HONESTY on `model`: this is the REQUESTED/ROUTED tier (orchestrator.PhaseTier — the
// SAME value handed to `claude --model` at the call site), not a field read back out of
// the claude JSON. Under v1's claude-only pool, claude bills the tier `--model` selected,
// so requested == billed and attributing the cost to the routed tier is accurate. (If a
// future provider could silently downgrade the served model, this would become
// requested-not-served; the comment is the honest caveat.)
func costEmitter(tracer *trace.Tracer, logln func(string)) func(phase, model string, usd float64) {
	return func(phase, model string, usd float64) {
		emitTrace(tracer, trace.Event{
			Kind: "agent", Name: phase, Status: "ok",
			CostUsdMicros: int64(math.Round(usd * 1e6)),
			Model:         model,
		}, logln)
	}
}
