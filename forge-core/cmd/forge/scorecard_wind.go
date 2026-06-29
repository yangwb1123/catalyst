// scorecard_wind.go — the PRODUCER end of the Eval -> scorecard -> Router learning
// loop, run at the END of a `forge run` / `forge evolve` (the "wind-down"). It turns
// the run's REAL billed cost (already recorded as model-stamped trace events by
// costEmitter) into persisted scorecard rows by shelling the runnable harness step
// `node harness/scorecard-update.mjs` once per (model, task_type) the run exercised.
//
// This is the auto-collection Option B: the orchestrator never touches the scorecard;
// the CLI (this file) reads the trace it already wrote and drives the harness. Layering
// stays bright: scorecard knowledge lives in cmd/forge, the orchestrator is oblivious
// (the same line cost.go / attribution.go / prompt_context.go draw).
//
// Two honesty gates, both modeled on recordMemory's "don't fabricate what the run didn't
// do":
//   - GATE-ON-REAL-COST: if the trace holds NO model-bearing cost event (a dry/echo run
//     bills nothing and writes none), the whole wind-down is SKIPPED — we never synthesize
//     a scorecard row for a run that never paid for a model, exactly as recordMemory never
//     invents findings a dry loop didn't make.
//   - FAIL-LOUD-AND-CONTINUE: a per-pair scorecard-update failure prints a stderr WARNING
//     but NEVER changes the run's exit code — the scorecard is ENRICHMENT, not correctness,
//     so a learning-loop hiccup must not flip a green run red (or a red one green). This
//     mirrors checkpointHook/recordMemory's deliberate non-fail-closed posture.
//
// TRACE-FLUSH ORDERING (load-bearing invariant): trace.Tracer.Emit writes each line
// straight to the *os.File via f.Write with NO buffering (see trace.go), and the cost
// events are emitted synchronously inside RunFrom BEFORE this wind-down runs. So reading
// <root>/.forge/trace.jsonl here — while the file is still open, BEFORE closeTrace() — sees
// every cost event the run produced. Do NOT introduce a buffered writer in trace without
// flushing before this read, or the gate-on-real-cost check would miss late cost events.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/trace"
)

// scorecardPath is where the producer writes the merged (model, task_type) rows the
// Router consumes — the SAME .agent/routing/scorecards.json scorecard-update.mjs defaults
// to and routing.LoadScorecards reads. Kept absolute so the shell-out is cwd-independent.
func scorecardPath(root string) string {
	return filepath.Join(root, ".agent", "routing", "scorecards.json")
}

// tracePath is the run's trace JSONL — the producer's input (real billed, model-stamped
// cost events) and the file the gate-on-real-cost check reads. Mirrors openTracer's path.
func tracePath(root string) string {
	return filepath.Join(forgeDir(root), "trace.jsonl")
}

// scorecardPair is one distinct (model, task_type) the run billed against — the scorecard
// primary key (scorecard.schema.yml). The producer is invoked once per distinct pair.
type scorecardPair struct {
	model, taskType string
}

// windDownScorecards is the wind-down entry point: after a run's workflow finishes, attribute
// its real billed cost into the scorecards. It is GATED on the trace actually carrying a
// model-bearing cost event (dry/echo bill nothing -> skip, no fabrication) and FAIL-LOUD-AND-
// CONTINUE per pair (a scorecard-update failure warns but never changes the run's outcome).
// It returns nothing and is called purely for its side effect (writing scorecards.json), so a
// caller wires it AFTER computing the run's real exit code — the scorecard is enrichment.
func windDownScorecards(wf asset.Workflow, o runOpts, logln func(string)) {
	tp := tracePath(o.root)
	if !traceHasModelCost(tp) {
		// No real billed-cost event (dry/echo, or a run that spawned no LLM phase):
		// skip entirely rather than persist a row for work no model was paid for.
		return
	}
	for _, p := range distinctScorecardPairs(wf, tp) {
		runScorecardUpdate(o.root, p, logln)
	}
}

// distinctScorecardPairs derives the DISTINCT (model, task_type) pairs the run actually
// BILLED, read from the trace's model-stamped cost events — NOT recomputed from the
// workflow's un-adjusted routed tier. This is the attribution fix: when a phase is budget-
// DOWN-TIERED near the cap, costEmitter (cost.go) stamps the trace with the CHEAPER model it
// ACTUALLY ran, while orchestrator.PhaseTier names the original tier. Querying scorecard-
// update with that un-adjusted tier would filter the trace to a model that billed NOTHING for
// the phase (zero match) and the down-tiered cost would be LOST. Reading the model from the
// trace makes the query model == the stamped model, so every billed dollar is attributed; and
// it cannot be recomputed at wind-down anyway (the per-phase spend ratio at the time it ran is
// gone). task_type still comes from the phase's AGENT: the cost event carries the phase NAME,
// mapped here (ttByPhase) to its agent's task_type. A phase whose agent has no task_type
// mapping (a harness/gate phase) bills no cost event, so it contributes no pair. Distinct
// (model, task_type) pairs collapse (two implementer phases at the same stamped model -> one
// pair), in first-seen order for a deterministic shell-out sequence.
func distinctScorecardPairs(wf asset.Workflow, tracePath string) []scorecardPair {
	ttByPhase := map[string]string{}
	for _, p := range wf.Phases {
		if tt, ok := taskTypeForAgent(p.Agent); ok {
			ttByPhase[p.Name] = tt
		}
	}
	f, err := os.Open(tracePath)
	if err != nil {
		return nil // no trace (or unreadable): nothing billed to attribute
	}
	defer f.Close()
	seen := map[scorecardPair]bool{}
	var pairs []scorecardPair
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024) // tolerate long trace lines
	for sc.Scan() {
		var ev trace.Event
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
			continue // skip a blank/corrupt line, never fatal
		}
		if ev.Model == "" || ev.CostUsdMicros == 0 {
			continue // not a billed model-bearing cost event
		}
		tt, ok := ttByPhase[ev.Name]
		if !ok {
			continue // a cost event for a phase with no task_type mapping: not attributed
		}
		pair := scorecardPair{model: ev.Model, taskType: tt}
		if seen[pair] {
			continue
		}
		seen[pair] = true
		pairs = append(pairs, pair)
	}
	return pairs
}

// runScorecardUpdate shells the runnable learning-loop step for ONE pair:
//
//	node harness/scorecard-update.mjs --model <m> --task-type <tt> \
//	     --trace <root>/.forge/trace.jsonl --out <root>/.agent/routing/scorecards.json
//
// run from the repo root (cmd.Dir, the SAME convention internal/gate uses so
// `harness/...` resolves) with ABSOLUTE --trace/--out so the paths are unambiguous. The
// producer's own --model filter (scorecard-update.mjs) makes it aggregate ONLY this
// model's cost/latency from the shared trace, so each pair's row carries its own numbers.
//
// FAIL-LOUD-AND-CONTINUE: a non-zero exit (or a failure to even start node) prints a
// stderr WARNING with the captured output, then returns — the loop moves to the next pair
// and the run's exit code is untouched. The scorecard is enrichment; a producer hiccup
// must never abort or re-color the run.
func runScorecardUpdate(root string, p scorecardPair, logln func(string)) {
	cmd := exec.Command("node", "harness/scorecard-update.mjs",
		"--model", p.model, "--task-type", p.taskType,
		"--trace", tracePath(root), "--out", scorecardPath(root))
	cmd.Dir = root
	if out, err := cmd.CombinedOutput(); err != nil {
		logln(fmt.Sprintf(
			"forge: WARNING scorecard-update failed for %s/%s (scorecard NOT updated; run outcome unaffected): %v\n%s",
			p.model, p.taskType, err, string(out)))
	}
}

// traceHasModelCost reports whether the trace at path carries at least one model-bearing
// cost event — the gate-on-real-cost signal. It scans the JSONL for an event with BOTH a
// non-empty `model` and a non-zero `cost_usd_micros` (exactly what costEmitter stamps for a
// real claude phase; an iteration/gate event and an echo/dry agent phase carry neither, so
// they never trip this). A missing trace, or a trace of only non-billing events, yields
// false -> the wind-down skips. It is robust like the harness parsers: blank/corrupt lines
// are skipped, never fatal — a single readable cost event is enough to proceed.
func traceHasModelCost(path string) bool {
	f, err := os.Open(path)
	if err != nil {
		return false // no trace (or unreadable) -> treat as "no real cost", skip wind-down
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024) // tolerate long trace lines
	for sc.Scan() {
		var ev trace.Event
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
			continue // skip a blank/corrupt line, never fatal
		}
		if ev.Model != "" && ev.CostUsdMicros != 0 {
			return true
		}
	}
	return false
}
