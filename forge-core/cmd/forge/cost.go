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

// claudeOverloadStatus is the HTTP status the Anthropic API returns when the model
// pool is momentarily at capacity: 529 overloaded_error (retryable per the API's own
// error contract). It is the STRONG, unambiguous signal classifyClaudeOverload keys on.
const claudeOverloadStatus = 529

// classifyClaudeOverload is the claude-specific 529 recognizer — the EXACT mirror of
// parseClaudeCostUsd's isolation: ALL knowledge that an "overload" means a claude/Anthropic
// HTTP 529 lives HERE in cmd/forge, never in the orchestrator (which consumes only an opaque
// bool via CommandExecutor.ClassifyOverload -> classifyRunErr's isOverload). It is wired ONLY
// onto the `--executor=command` claude path (dry/echo never get it), so a non-claude command's
// failure can never be mistaken for an overload.
//
// RECOGNITION BASIS (verified against the real `claude -p --output-format json` result envelope —
// the SDK ResultMessage: {type:"result", subtype, is_error, result, total_cost_usd, api_error_status}):
//   - PRIMARY (strong): the envelope carries `api_error_status` == 529. This is the model's own
//     report of the terminating HTTP status, so it is exact — no string-matching guesswork.
//   - SECONDARY (narrow textual fallback): a FAILED envelope (is_error==true) whose result text
//     carries an explicit overload marker — the literal API error type "overloaded_error", or a
//     standalone "529", or the word "Overloaded". This covers a stderr/plain-text overload that
//     reached us without a parseable `api_error_status` (e.g. a transport-level dump).
//
// MISJUDGEMENT BOUNDARY — deliberately FAIL-CLOSED / "rather miss than mis-fire":
//   - A MISS (overload not recognized) is SAFE: the failure stays KindFailed and the run aborts
//     fail-closed, exactly the pre-existing behavior. We lose a retry opportunity, nothing worse.
//   - A FALSE POSITIVE (a real terminal KindFailed mislabeled transient) is DANGEROUS: it would be
//     retried-with-backoff up to MaxRetries, burning budget on a failure that can never succeed —
//     so it is what this recognizer is tuned AGAINST. Hence: require a STRONG signal (a literal 529
//     status, or an is_error envelope plus an explicit overload word) and never infer overload from
//     a bare non-zero exit, a generic "error", or a 5xx that is not 529. When unsure, return false.
//   - The textual fallback is gated on is_error==true so an agent that merely WROTE the word
//     "overloaded" into a SUCCESSFUL result (e.g. summarizing this very code) is not misread as a
//     backend overload — a successful envelope is never an overload regardless of its prose.
//
// Honesty on the timeout boundary: a deadline-SIGKILLed run can carry a truncated envelope that
// trips the textual fallback, but the generic layer checks DeadlineExceeded BEFORE this bool (see
// classifyRunErr), so a timeout is never downgraded to an overload — this recognizer's verdict only
// matters when the run was NOT a timeout.
func classifyClaudeOverload(output string) bool {
	trimmed := strings.TrimSpace(output)
	var env struct {
		IsError        *bool   `json:"is_error"`
		APIErrorStatus *int    `json:"api_error_status"`
		Result         *string `json:"result"`
	}
	if err := json.Unmarshal([]byte(trimmed), &env); err == nil {
		// PRIMARY: the model's own terminating HTTP status is the exact, strong signal.
		if env.APIErrorStatus != nil && *env.APIErrorStatus == claudeOverloadStatus {
			return true
		}
		// SECONDARY (parsed): a FAILED envelope whose result text names an overload. Gated on
		// is_error so a successful run that merely mentions the word is never misclassified.
		if env.IsError != nil && *env.IsError && env.Result != nil && hasOverloadMarker(*env.Result) {
			return true
		}
		// A parseable envelope with neither strong signal is NOT an overload (fail-closed) —
		// don't fall through to scanning the JSON source, which could match an incidental word.
		return false
	}
	// Non-envelope output (a plain stderr/transport dump, not a parseable result JSON): accept it
	// only on an explicit overload marker. A bare "error"/non-zero exit with no such marker stays
	// false, so a generic failure is never upgraded to a transient retry.
	return hasOverloadMarker(trimmed)
}

// hasOverloadMarker reports whether s carries a STRONG, unambiguous overload signal: the literal
// Anthropic API error type "overloaded_error", a standalone "529" status, or the word "Overloaded".
// Case-insensitive for the words; "529" is matched as a bounded token (digit-isolated) so an
// unrelated number that merely contains the digits 529 (e.g. a cost like 0.0005290, a duration, or
// a byte count) cannot trip it. Deliberately narrow — see classifyClaudeOverload's misjudgement note.
func hasOverloadMarker(s string) bool {
	lower := strings.ToLower(s)
	if strings.Contains(lower, "overloaded_error") || strings.Contains(lower, "overloaded") {
		return true
	}
	return containsToken529(s)
}

// containsToken529 reports whether s contains "529" as a standalone numeric token — i.e. not
// flanked by another digit. This keeps the strong "HTTP 529" signal while rejecting incidental
// substrings (e.g. "15290", "0.05290") that happen to contain the digits but are not the status.
func containsToken529(s string) bool {
	for from := 0; ; {
		rel := strings.Index(s[from:], "529")
		if rel < 0 {
			return false
		}
		i := from + rel
		before := i == 0 || !isDigit(s[i-1])
		after := i+3 >= len(s) || !isDigit(s[i+3])
		if before && after {
			return true
		}
		from = i + 1 // overlapping search: advance one byte past this match
	}
}

func isDigit(b byte) bool { return b >= '0' && b <= '9' }

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
