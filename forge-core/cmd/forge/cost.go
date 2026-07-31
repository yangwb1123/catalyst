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
	"fmt"
	"math"
	"strconv"
	"strings"
	"sync"
	"time"

	"forgeos/forge-core/internal/trace"
)

// runBudget owns the run-level cumulative dollar cap. The orchestrator sees
// only BudgetExhausted's boolean; vendor cost parsing remains below.
//
// capMicros and spentBaseMicros are the canonical checkpoint representation.
// spentDelta contains only costs observed since the last restore, so an exact
// persisted base never drifts through a micro -> float -> micro cycle.
type runBudget struct {
	mu sync.Mutex

	spent           float64
	spentBaseMicros int64
	spentDelta      float64

	cap       float64
	capMicros int64
}

// newRunBudget canonicalizes a configured cap once to checkpoint v3
// micro-dollars. Empty and explicit zero both mean unset.
func newRunBudget(flagVal string) (*runBudget, error) {
	s := strings.TrimSpace(flagVal)
	if s == "" {
		return &runBudget{}, nil
	}
	v, err := strconv.ParseFloat(s, 64)
	if err != nil {
		return nil, fmt.Errorf("--run-budget-usd %q is not a number: %w", flagVal, err)
	}
	if v < 0 || math.IsNaN(v) || math.IsInf(v, 0) {
		return nil, fmt.Errorf(
			"--run-budget-usd must be a non-negative finite dollar amount, got %q",
			flagVal,
		)
	}
	scaled := v * 1e6
	// float64(math.MaxInt64) rounds to 2^63, so that boundary is unsafe too.
	if math.IsInf(scaled, 0) || scaled >= float64(math.MaxInt64) {
		return nil, fmt.Errorf(
			"--run-budget-usd %q is too large to persist as micro-USD", flagVal,
		)
	}
	micros := int64(math.Round(scaled))
	if v > 0 && micros == 0 {
		return nil, fmt.Errorf(
			"--run-budget-usd %q is below the persisted micro-USD precision; "+
				"use at least 0.0000005",
			flagVal,
		)
	}
	return &runBudget{
		cap:       float64(micros) / 1e6,
		capMicros: micros,
	}, nil
}

// feed accumulates a billed phase and forwards the original observation.
func (b *runBudget) feed(
	inner func(phase, model string, usd float64, latency time.Duration),
) func(phase, model string, usd float64, latency time.Duration) {
	return func(phase, model string, usd float64, latency time.Duration) {
		b.mu.Lock()
		b.spent += usd
		b.spentDelta += usd
		b.mu.Unlock()
		if inner != nil {
			inner(phase, model, usd, latency)
		}
	}
}

// SpentUsdMicros returns the exact restored base plus rounded new spend.
// Saturation is fail-closed: an unrepresentably large bill exhausts any
// representable cap and never wraps the persisted counter negative.
func (b *runBudget) SpentUsdMicros() int64 {
	b.mu.Lock()
	defer b.mu.Unlock()

	deltaMicros := usdMicrosSaturating(b.spentDelta)
	if deltaMicros == 0 {
		return b.spentBaseMicros
	}
	if deltaMicros == math.MaxInt64 ||
		deltaMicros > math.MaxInt64-b.spentBaseMicros {
		return math.MaxInt64
	}
	return b.spentBaseMicros + deltaMicros
}

func usdMicrosSaturating(usd float64) int64 {
	if usd <= 0 || math.IsNaN(usd) {
		return 0
	}
	scaled := usd * 1e6
	if math.IsInf(scaled, 0) || scaled >= float64(math.MaxInt64) {
		return math.MaxInt64
	}
	return int64(math.Round(scaled))
}

// CapUsdMicros returns the canonical cap without a second float round-trip.
// The fallback supports package-local tests that construct runBudget directly.
func (b *runBudget) CapUsdMicros() int64 {
	if b.capMicros != 0 || b.cap == 0 {
		return b.capMicros
	}
	scaled := math.Round(b.cap * 1e6)
	if scaled <= 0 {
		return 0
	}
	if math.IsInf(scaled, 0) || scaled >= float64(math.MaxInt64) {
		return math.MaxInt64
	}
	return int64(scaled)
}

func (b *runBudget) restore(capMicros, spentMicros int64) error {
	if capMicros < 0 || spentMicros < 0 {
		return fmt.Errorf("persisted run budget values must be non-negative")
	}
	currentCap := b.CapUsdMicros()
	if currentCap != 0 && currentCap != capMicros {
		return fmt.Errorf(
			"persisted run budget cap is %d micro-USD, but this invocation configured %d",
			capMicros, currentCap,
		)
	}
	if currentCap == 0 {
		b.capMicros = capMicros
		b.cap = float64(capMicros) / 1e6
	}
	b.seed(spentMicros)
	return nil
}

// seed restores a checkpoint's exact micro-dollar base. A later feed
// accumulates separately on top. Non-positive input remains a no-op for the
// fresh/unbilled compatibility path; restore rejects negative values first.
func (b *runBudget) seed(micros int64) {
	if micros <= 0 {
		return
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	b.spent = float64(micros) / 1e6
	b.spentBaseMicros = micros
	b.spentDelta = 0
}

func (b *runBudget) exhausted() bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.cap > 0 && b.spent >= b.cap
}

// SpendRatio exposes only a dimensionless routing signal.
func (b *runBudget) SpendRatio() float64 {
	if b.cap <= 0 {
		return 0
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.spent / b.cap
}

func (b *runBudget) BudgetExhaustedFunc() func() bool {
	if b.cap <= 0 {
		return nil
	}
	return b.exhausted
}

// parseClaudeCostUsd extracts the real billed dollar cost from a claude `-p
// --output-format json` envelope's `total_cost_usd`. A *float64 pointer distinguishes
// an ABSENT field from a legitimate 0.0. ok=false (and the caller emits NO cost event,
// never a fabricated 0) when the output is not a single JSON object, carries no
// total_cost_usd, or the value is negative/non-finite — the honesty branches that keep
// an echo/dry/stub output (not claude JSON) or malformed telemetry from inventing cost
// or reducing the run's cumulative spend.
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
	if *env.TotalCostUsd < 0 || math.IsNaN(*env.TotalCostUsd) || math.IsInf(*env.TotalCostUsd, 0) {
		return 0, false // negative/non-finite -> not a real billed cost
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

// Executive-verdict tokens — the machine-readable last line .agent/agents/cto.md's
// NEW "executive-review" section (review.yml P4, .agent/workflows/review.yml) contracts
// the CTO's synthesis phase to emit. Five values, not the reviewer's binary two, because
// review.yml's stated intent is "产出唯一裁决:Approve / Approve with Simplification /
// Redesign / Delay / Reject" (docs/adr/0004). VerdictApprove is DELIBERATELY REUSED
// (not redeclared) — the reviewer's plain "APPROVE" and the executive's "APPROVE" are the
// same normalized token, so a downstream reader (reviewStatus in gates.go) never has to
// special-case which parser produced it. The other four are new, executive-only tokens.
const (
	VerdictApproveWithSimplification = "APPROVE_WITH_SIMPLIFICATION"
	VerdictRedesign                  = "REDESIGN"
	VerdictDelay                     = "DELAY"
	VerdictReject                    = "REJECT"
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

// parseExecutiveVerdict extracts the CTO's machine-readable executive-review verdict
// from its raw output — structurally IDENTICAL to parseReviewerVerdict (same
// unwrapClaudeResult + lastNonEmptyLine + exact-match approach, same isolation
// rationale: cost.go is the ONLY place that knows either contract's string shape),
// but matched against the FIVE executive tokens (review.yml P4's "唯一裁决" vocabulary)
// instead of the reviewer's binary two. ok=false (missing/wrapped/malformed last line,
// or the reviewer's own two tokens spelled differently than "APPROVE") means "no
// signal" — never a fabricated verdict; the caller (observeFor) tries this ONLY after
// parseReviewerVerdict has already failed to match, so an ordinary binary reviewer
// phase's output is never routed here.
func parseExecutiveVerdict(output string) (verdict string, ok bool) {
	last := lastNonEmptyLine(unwrapClaudeResult(output))
	switch last {
	case "VERDICT: " + VerdictApprove:
		return VerdictApprove, true
	case "VERDICT: " + VerdictApproveWithSimplification:
		return VerdictApproveWithSimplification, true
	case "VERDICT: " + VerdictRedesign:
		return VerdictRedesign, true
	case "VERDICT: " + VerdictDelay:
		return VerdictDelay, true
	case "VERDICT: " + VerdictReject:
		return VerdictReject, true
	default:
		return "", false // missing/wrapped/malformed -> no signal (caller fails open)
	}
}

// confidenceContract is the literal prefix the requirement-discovery phase's last
// line must carry — .agent/agents/product-manager.md's `CONFIDENCE: <N>` contract
// (discover.yml's requirement_confidence stop signal, the NUMERIC counterpart to
// the reviewer's binary VERDICT: token and the executive's five-way VERDICT: token).
const confidenceContract = "CONFIDENCE: "

// parseConfidenceScore extracts the requirement-discovery phase's self-reported
// confidence score (0-100) from its raw output — this file's THIRD verdict-shaped
// parser (after parseReviewerVerdict and parseExecutiveVerdict), same isolation
// rationale and same unwrapClaudeResult + lastNonEmptyLine pipeline, but the
// payload is a bounded INTEGER rather than a fixed token, so this parser
// additionally validates it. ok=false (never a fabricated score) whenever the
// last line does not start with the exact "CONFIDENCE: " prefix, the remainder
// is not a plain base-10 integer, or the value falls outside the contracted
// [0,100] range — a malformed or out-of-range score is "no signal", exactly like
// a missing/wrapped VERDICT line, so the caller (observeFor) never records a
// value that RequirementConfidence's honest 0-default cannot already represent.
func parseConfidenceScore(output string) (score float64, ok bool) {
	last := lastNonEmptyLine(unwrapClaudeResult(output))
	numStr, hasPrefix := strings.CutPrefix(last, confidenceContract)
	if !hasPrefix {
		return 0, false
	}
	n, err := strconv.Atoi(numStr)
	if err != nil {
		return 0, false // not a plain base-10 integer (e.g. "85%", "85.0", "eighty-five")
	}
	if n < 0 || n > 100 {
		return 0, false // out of the contracted [0,100] range
	}
	return float64(n), true
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
// integer microdollars and emits one `kind:"agent"` trace event carrying it, the model it
// was billed against, AND the phase's measured wall-clock LATENCY (DurationMs), so the
// scorecard's --trace reader can aggregate a real avg_cost_usd AND a real p95_latency_ms,
// BOTH attributed per model. Microdollars match trace.Event.CostUsdMicros (integer,
// jitter-free); the conversion (USD x 1e6, rounded) is owned here — trace.go stays oblivious
// to dollars and what a model is.
//
// HONESTY on `latency` (the (c) fix): DurationMs is now the phase's OWN wall-clock span,
// measured by CommandExecutor.Execute bracketing cmd.Run() (the generic OS-level duration,
// no claude knowledge) and handed to the cost sink alongside the bytes. It is therefore TRUE
// PER-MODEL — every agent event carries the duration of THAT phase, so scorecard-update's
// per-model latency filter yields a genuine per-model p95. This supersedes the (c) gap where
// agent events carried NO DurationMs (0), so the only non-zero duration lived on the
// iteration event (un-stamped, model-less) and every pair shared the iteration's span. PR1's
// claim "cost/latency are real per-model" is now fully delivered for latency too: cost was
// already per-model, latency now joins it. duration.Milliseconds() truncates sub-ms toward
// zero (so a <1ms phase records 0, a finite sample kept by parseTraceLatencies, never
// fabricated) — DurationMs has no omitempty, matching the iteration event that already
// always writes it; an agent event's duration is meaningful (a 0 means "ran in under a ms",
// not "absent"), so it is always recorded.
//
// HONESTY on `model`: this is the BUDGET-ADJUSTED routed tier — orchestrator.PhaseTier
// post-filtered by routing.BudgetAdjustTier (so a phase down-tiered near budget is stamped
// with the CHEAPER model it actually ran, not the un-adjusted route) — the SAME value handed
// to `claude --model` at the call site, resolved through the ONE shared phaseTierResolver so
// `--model`, this cost stamp, and the prompt's tier can never disagree. It is NOT a field read
// back out of the claude JSON. Under v1's claude-only pool, claude bills the tier `--model`
// selected, so requested == billed == stamped and attributing the cost to the (possibly
// down-tiered) routed tier is accurate — a near-budget phase correctly lands in the cheaper
// model's scorecard bucket. (If a future provider could silently downgrade the served model,
// this would become requested-not-served; the comment is the honest caveat.)
func costEmitter(tracer *trace.Tracer, logln func(string)) func(phase, model string, usd float64, latency time.Duration) {
	return func(phase, model string, usd float64, latency time.Duration) {
		emitTrace(tracer, trace.Event{
			Kind: "agent", Name: phase, Status: "ok",
			DurationMs:    latency.Milliseconds(),
			CostUsdMicros: usdMicrosSaturating(usd),
			Model:         model,
		}, logln)
	}
}
