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
// integer microdollars and emits one `kind:"agent"` trace event carrying it, so the
// scorecard's --trace reader can aggregate a real avg_cost_usd. Microdollars match
// trace.Event.CostUsdMicros (integer, jitter-free). The conversion (USD x 1e6, rounded)
// is owned here — trace.go stays oblivious to dollars.
func costEmitter(tracer *trace.Tracer, logln func(string)) func(phase string, usd float64) {
	return func(phase string, usd float64) {
		emitTrace(tracer, trace.Event{
			Kind: "agent", Name: phase, Status: "ok",
			CostUsdMicros: int64(math.Round(usd * 1e6)),
		}, logln)
	}
}
