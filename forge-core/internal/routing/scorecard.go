package routing

// This file is the consumer end of the Eval -> scorecard -> Router learning
// loop, and the final, previously-missing link in policy.yml's decision chain:
//   score -> tier -> floor -> safety_override -> budget_guard -> history-tiebreak
//
// TierForScore (routing.go) stops at budget_guard. HistoryTiebreak below adds
// the last step: among the same-tier candidate models for a task_type, prefer
// the one with the best historical quality_score — the "优" (pick-the-best) half
// of the loop, reading the scorecards the Eval Engine writes.
//
// Honest scope of v1: provider_pool is claude-only (policy.yml D4), and each
// tier band almost always holds a SINGLE candidate model. So in v1 this tiebreak
// is overwhelmingly "single candidate passes through, history is merely made
// observable" — there is rarely a real choice to make. The genuine multi-model
// shoot-out is v3's cross-vendor pool (Qwen/DeepSeek/local via LiteLLM). What
// this code buys today is (a) decision-chain completeness — the chain no longer
// silently drops its tail — and (b) the plumbing + observability so that when v3
// widens tiers.models, the selection logic is already here and already tested.
//
// IO vs. pure split is deliberate: LoadScorecards touches disk (and is the only
// thing that can fail on IO); Lookup and HistoryTiebreak are pure functions over
// an in-memory slice, so the decision logic is unit-testable with no filesystem.

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
)

// Scorecard is one (model, task_type, mode) row of historical performance, matching
// scorecard.schema.yml and the shared scorecards.json data contract produced by
// the Eval Engine. JSON tags are the wire contract; keep them aligned with the
// parallel producer. QualityScore is what policy.history.tiebreak_on reads
// (higher is better); Samples gates trust (policy.history.min_samples).
//
// Mode is DECLARED-BUT-INERT, not honest yet: the design intent (edgecases
// §5.4) is that it would carry the execution mode (explorer/balanced/
// engineering/cto) that produced this row, so the Router could filter by
// mode compatibility and avoid cross-mode scoring bias (an explorer-mode row
// with no reviewer may show inflated quality that misleads engineering-mode
// routing). As of this writing the only producer, harness/scorecard-update.mjs
// (via synthesize() in harness/scorecard.mjs), never sets it, and no
// consumer in cmd/forge/gates.go or scorecard_wind.go reads/filters by it —
// verified by repo-wide grep, not asserted from memory. It always decodes to
// "", which every consumer already treats as "compatible with all modes"
// (the intended backward-compat behavior for a legacy row), so this is a
// dead-but-harmless field today, not an active bug — flagged here honestly
// rather than describing a filter that does not yet run.
// The PassRate/AvgIterations/ReworkRate trio is optional enrichment (schema
// "optional:" block) — absent fields decode to their zero value, which callers
// must treat as "unknown", not "zero performance".
type Scorecard struct {
	Model         string  `json:"model"`
	TaskType      string  `json:"task_type"`
	QualityScore  float64 `json:"quality_score"`
	Samples       int     `json:"samples"`
	UpdatedAt     string  `json:"updated_at"`
	Mode          string  `json:"mode,omitempty"`
	PassRate      float64 `json:"pass_rate,omitempty"`
	AvgIterations float64 `json:"avg_iterations,omitempty"`
	ReworkRate    float64 `json:"rework_rate,omitempty"`
}

// LoadScorecards reads the scorecards.json array at path.
//
// CONCURRENCY CONTRACT (caller): The producer (Eval Engine) MUST write
// scorecards atomically — write to a temp file, then rename over the
// target path. This guarantees LoadScorecards always reads a complete,
// internally consistent snapshot (POSIX rename is atomic on the same
// filesystem). Without this, concurrent writers and readers can observe
// partial writes or torn reads.
//
// Fault-tolerant but honest, and the distinction is load-bearing:
//   - Missing file -> (nil, nil). A cold start (Eval has never run / no loop
//     history yet) is a normal, expected state, NOT an error. Routing must keep
//     working on its policy defaults; policy.history.on_missing == tier_default
//     captures exactly this — no scorecard means fall back, not fail.
//   - Malformed JSON -> (nil, err). A corrupt scorecard file is a real fault and
//     is surfaced loudly rather than silently swallowed: silently treating
//     garbage as "no history" would mask a broken Eval producer and quietly
//     degrade routing. Honesty over convenience.
//
// A present-but-empty file ("[]" or whitespace) yields (nil, nil): syntactically
// valid, semantically just a cold start.
func LoadScorecards(path string) ([]Scorecard, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		// Absent file is the cold-start signal, not a failure. Any other IO error
		// (permissions, a directory in the way, …) is a genuine fault to report.
		if errors.Is(err, fs.ErrNotExist) {
			return nil, nil
		}
		return nil, fmt.Errorf("routing: read scorecards %q: %w", path, err)
	}

	var cards []Scorecard
	if err := json.Unmarshal(data, &cards); err != nil {
		// Surface malformed data; do not pretend it is an empty history.
		return nil, fmt.Errorf("routing: parse scorecards %q: %w", path, err)
	}
	if len(cards) == 0 {
		return nil, nil // empty array == cold start, normalized to the nil form
	}
	return cards, nil
}

// Lookup finds the scorecard for an exact (model, taskType) primary key
// (scorecard.schema.yml primary_key: [model, task_type]). Pure; returns the
// match and true, or the zero Scorecard and false when absent. On the (malformed)
// chance of duplicate keys, the first wins — Eval is the deduping authority, not
// this consumer.
func Lookup(cards []Scorecard, model, taskType string) (Scorecard, bool) {
	for _, c := range cards {
		if c.Model == model && c.TaskType == taskType {
			return c, true
		}
	}
	return Scorecard{}, false
}

// HistoryTiebreak is policy.yml's history step: among same-tier candidate models
// for a task_type, pick the one with the highest quality_score that also has at
// least minSamples observations. It is pure (no IO) and total (always returns).
//
// Returns (picked, reason). reason is a short, honest, human-readable trace of
// WHY — this function's main v1 payoff is observability, so the reason string is
// a first-class output, not a debug afterthought.
//
// Fallback semantics mirror policy.history exactly:
//   - candidates empty                       -> ("",            "no candidates -> tier_default")
//   - no candidate has a scorecard           -> (candidates[0], "no scorecard -> tier_default")
//   - candidate(s) exist but all under min   -> (candidates[0], "insufficient samples (...) -> tier_default")
//   - a qualifying winner exists             -> (winner,        "picked <m> by quality <q> (n samples)")
//
// The non-empty fallbacks return candidates[0] as the tier default: in v1 each
// tier band is effectively single-candidate, so candidates[0] IS the tier's
// default model. tiebreak_on is fixed to quality_score (policy.history); ties on
// quality_score are broken by encounter order (first candidate wins), keeping the
// result deterministic. recency_half_life_days decay is intentionally NOT applied
// here — that weighting belongs to the Eval Engine when it computes quality_score
// (it owns the time window), so the Router consumes an already-decayed number.
func HistoryTiebreak(candidates []string, taskType string, cards []Scorecard, minSamples int) (string, string) {
	if len(candidates) == 0 {
		return "", "no candidates -> tier_default"
	}

	var (
		best       string
		bestScore  float64
		bestN      int
		found      bool // any candidate had a scorecard at all
		qualifying bool // any candidate met minSamples
	)
	for _, model := range candidates {
		card, ok := Lookup(cards, model, taskType)
		if !ok {
			continue // no history for this candidate; skip, may still fall back
		}
		found = true
		if card.Samples < minSamples {
			continue // history exists but is too thin to trust (min_samples)
		}
		// Strictly-greater keeps the first candidate on ties -> deterministic.
		if !qualifying || card.QualityScore > bestScore {
			qualifying = true
			best, bestScore, bestN = model, card.QualityScore, card.Samples
		}
	}

	switch {
	case qualifying:
		return best, fmt.Sprintf("picked %s by quality %.2f (%d samples)", best, bestScore, bestN)
	case found:
		// Scorecards exist for the band but none clear min_samples: distrust the
		// noise and fall back to the tier default (candidates[0]).
		return candidates[0], fmt.Sprintf("insufficient samples (<%d) -> tier_default", minSamples)
	default:
		// No scorecard at all for any candidate (cold start for this task_type).
		return candidates[0], "no scorecard -> tier_default"
	}
}
