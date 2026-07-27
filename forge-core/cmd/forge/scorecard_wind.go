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
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/attribution"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/routing"
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

// scorecardPair aliases attribution.ScorecardPair — the scorecard primary key
// (scorecard.schema.yml), one distinct (model, task_type) a run billed
// against. The vocabulary lives in internal/attribution (shared with the
// `forge scorecard rebuild` disaster-recovery path below); this alias just
// keeps this file's many call sites unqualified.
type scorecardPair = attribution.ScorecardPair

// windDownScorecards is the wind-down entry point: after a run's workflow finishes, attribute
// its real billed cost into the scorecards. It is GATED on the trace actually carrying a
// model-bearing cost event (dry/echo bill nothing -> skip, no fabrication) and FAIL-LOUD-AND-
// CONTINUE per pair (a scorecard-update failure warns but never changes the run's outcome).
// It returns nothing and is called purely for its side effect (writing scorecards.json), so a
// caller wires it AFTER computing the run's real exit code — the scorecard is enrichment.
//
// iterations is the real loop-count (1 for forge run, outcome.Iterations for forge evolve).
// reworked is true when a reviewer issued REQUEST_CHANGES during this run. Both are wired
// into scorecard-update.mjs as --iterations and --rework, populating avg_iterations and
// rework_rate in the scorecard row. Omitted when zero/false (legacy no-trajectory path).
func windDownScorecards(wf asset.Workflow, o runOpts, logln func(string), iterations int, reworked bool) {
	windDownScorecardsForRun(wf, o, logln, iterations, reworked, "")
}

// windDownScorecardsForRun scopes append-only trace consumption to the current
// process run. It also passes the workflow phases belonging to each task type to
// the producer, preventing another stage/model event from contaminating the row.
func windDownScorecardsForRun(wf asset.Workflow, o runOpts, logln func(string), iterations int, reworked bool, runID string) {
	tp := tracePath(o.root)
	if !traceHasModelCostForRun(tp, runID) {
		// No real billed-cost event (dry/echo, or a run that spawned no LLM phase):
		// skip entirely rather than persist a row for work no model was paid for.
		return
	}
	for _, p := range distinctScorecardPairsForRun(wf, tp, runID) {
		runScorecardUpdateScoped(o.root, p, logln, iterations, reworked, runID, phasesForPair(wf, p))
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
	return distinctScorecardPairsForRun(wf, tracePath, "")
}

func distinctScorecardPairsForRun(wf asset.Workflow, tracePath, runID string) []scorecardPair {
	ttByPhase := map[string]string{}
	for _, p := range wf.Phases {
		if tt, ok := attribution.TaskTypeForAgent(p.Agent); ok {
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
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil || trace.ValidateFormat(ev.Format) != nil {
			continue // skip a blank/corrupt line, never fatal
		}
		if ev.Model == "" || ev.CostUsdMicros == 0 {
			continue // not a billed model-bearing cost event
		}
		if runID != "" && ev.RunID != runID {
			continue
		}
		tt, ok := ttByPhase[ev.Name]
		if !ok {
			continue // a cost event for a phase with no task_type mapping: not attributed
		}
		pair := scorecardPair{Model: ev.Model, TaskType: tt}
		if seen[pair] {
			continue
		}
		seen[pair] = true
		pairs = append(pairs, pair)
	}
	return pairs
}

func phasesForPair(wf asset.Workflow, pair scorecardPair) []string {
	var phases []string
	for _, p := range wf.Phases {
		if tt, ok := attribution.TaskTypeForAgent(p.Agent); ok && tt == pair.TaskType {
			phases = append(phases, p.Name)
		}
	}
	return phases
}

// runScorecardUpdate shells the runnable learning-loop step for ONE pair:
//
//	node harness/scorecard-update.mjs --model <m> --task-type <tt> \
//	     --trace <root>/.forge/trace.jsonl --out <root>/.agent/routing/scorecards.json \
//	     [--iterations <n>] [--rework 0|1]
//
// run from the repo root (cmd.Dir, the SAME convention internal/gate uses so
// `harness/...` resolves) with ABSOLUTE --trace/--out so the paths are unambiguous. The
// producer's own --model filter (scorecard-update.mjs) makes it aggregate ONLY this
// model's cost/latency from the shared trace, so each pair's row carries its own numbers.
// --iterations and --rework carry the real trajectory (rounds-to-green, reviewer bounce)
// into the scorecard row's avg_iterations / rework_rate fields. They are omitted when
// iterations<=0 (legacy no-trajectory path) so the row degrades gracefully.
//
// FAIL-LOUD-AND-CONTINUE: a non-zero exit (or a failure to even start node) prints a
// stderr WARNING with the captured output, then returns — the loop moves to the next pair
// and the run's exit code is untouched. The scorecard is enrichment; a producer hiccup
// must never abort or re-color the run.
func runScorecardUpdate(root string, p scorecardPair, logln func(string), iterations int, reworked bool) {
	runScorecardUpdateWithOut(root, p, scorecardPath(root), logln, iterations, reworked)
}

// runScorecardUpdateWithOut is like runScorecardUpdate but writes to an explicit
// output path instead of the default scorecardPath(root). Used by forge scorecard
// rebuild to write to an arbitrary --out file.
func runScorecardUpdateWithOut(root string, p scorecardPair, outFile string, logln func(string), iterations int, reworked bool) {
	runScorecardUpdateScopedWithOut(root, p, outFile, logln, iterations, reworked, "", nil)
}

func runScorecardUpdateScoped(root string, p scorecardPair, logln func(string), iterations int, reworked bool, runID string, phases []string) {
	runScorecardUpdateScopedWithOut(root, p, scorecardPath(root), logln, iterations, reworked, runID, phases)
}

func runScorecardUpdateScopedWithOut(root string, p scorecardPair, outFile string, logln func(string), iterations int, reworked bool, runID string, phases []string) {
	args := []string{"harness/scorecard-update.mjs",
		"--model", p.Model, "--task-type", p.TaskType,
		"--trace", tracePath(root), "--out", outFile}
	if runID != "" {
		args = append(args, "--run-id", runID)
	}
	if len(phases) > 0 {
		args = append(args, "--phase-names", strings.Join(phases, ","))
	}
	if iterations > 0 {
		args = append(args, "--iterations", fmt.Sprintf("%d", iterations))
		reworkFlag := "0"
		if reworked {
			reworkFlag = "1"
		}
		args = append(args, "--rework", reworkFlag)
	}
	cmd := exec.Command("node", args...)
	cmd.Dir = root
	if out, err := cmd.CombinedOutput(); err != nil {
		logln(fmt.Sprintf(
			"forge: WARNING scorecard-update failed for %s/%s (scorecard NOT updated; run outcome unaffected): %v\n%s",
			p.Model, p.TaskType, err, string(out)))
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
	return traceHasModelCostForRun(path, "")
}

func traceHasModelCostForRun(path, runID string) bool {
	f, err := os.Open(path)
	if err != nil {
		return false // no trace (or unreadable) -> treat as "no real cost", skip wind-down
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024) // tolerate long trace lines
	for sc.Scan() {
		var ev trace.Event
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil || trace.ValidateFormat(ev.Format) != nil {
			continue // skip a blank/corrupt line, never fatal
		}
		if ev.Model != "" && ev.CostUsdMicros != 0 && (runID == "" || ev.RunID == runID) {
			return true
		}
	}
	return false
}

// ── forge scorecard: read-only CLI for learning-loop observability ────────
//
// cmdScorecard, printTable, printSummary, anyThin — the `forge scorecard`
// subcommand that makes the scorecard data observable without needing to know
// the file path or JSON schema. See scorecard_test.go for tests.
//
// Honesty: a missing scorecards.json (cold start, no learning loop history yet)
// renders "no scorecard data" rather than an error — cold start is normal, not
// a malfunction. A corrupt file surfaces the parse error honestly.

// historyMinSamples mirrors routing's policy.history.min_samples (20). Duplicated
// here so the scorecard display can flag entries below this threshold WITHOUT
// importing the route command or adding a public API — the display is cosmetic,
// not functional, so a stale value would only mis-colorize a table cell, never
// change a routing decision.
const scorecardDefaultMinSamples = 20

// cmdScorecard implements `forge scorecard [--root DIR] [--summary] [--min-samples N]`
// and subcommands like `forge scorecard rebuild --from <trace.jsonl>`.
func cmdScorecard(args []string) int {
	if len(args) > 0 && args[0] == "rebuild" {
		return cmdScorecardRebuild(args[1:])
	}
	fs := flag.NewFlagSet("scorecard", flag.ContinueOnError)
	var root string
	var summary bool
	var minSamples int
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.BoolVar(&summary, "summary", false, "show condensed summary (one row per task_type)")
	fs.IntVar(&minSamples, "min-samples", scorecardDefaultMinSamples, "minimum samples threshold for flagging insufficient data")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)

	cards, err := routing.LoadScorecards(scorecardPath(root))
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge scorecard: %v\n", err)
		return 1
	}
	if len(cards) == 0 {
		fmt.Println("forge scorecard: no scorecard data (cold start — run a workflow to generate history)")
		return 0
	}

	if summary {
		printSummary(cards, minSamples)
	} else {
		printTable(cards, minSamples)
	}
	return 0
}

// ── forge scorecard rebuild: disaster recovery from a trace JSONL alone ───
//
// `forge scorecard rebuild --from <trace.jsonl> [--out <scorecards.json>]
// [--workflow <name>] [--root DIR]` reconstructs scorecards.json when it is
// lost or corrupted (scan-new-angles.md §方向4). The pure (model, task_type)
// derivation — including the ground-truth phase-name -> task_type resolution
// a workflow like evolve.yml needs (its phase names differ from their agent
// roles) — lives in internal/attribution; this glue only globs/loads the
// workflow file(s) off disk (via loadWorkflow, the same native-parser/fallback
// path cmdRun uses) and drives the shared scorecard-update.mjs producer.

// cmdScorecardRebuild implements `forge scorecard rebuild --from <trace.jsonl>
// [--out <scorecards.json>] [--workflow <name>] [--root DIR]`. It reads a trace
// JSONL file, extracts every (model, task_type) pair with billed cost events,
// and rebuilds the scorecard by calling scorecard-update.mjs for each pair.
func cmdScorecardRebuild(args []string) int {
	root, traceFile, outFile, workflowName, code, ok := parseScorecardRebuildFlags(args)
	if !ok {
		return code
	}

	// Verify trace exists and has model-bearing cost events.
	if !traceHasModelCost(traceFile) {
		fmt.Fprintf(os.Stderr, "forge scorecard rebuild: no model-bearing cost events in %s\n", traceFile)
		return 1
	}

	phaseTaskTypes := resolvePhaseTaskTypes(root, workflowName)
	pairs, err := attribution.ExtractRebuildPairs(traceFile, phaseTaskTypes)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge scorecard rebuild: %v\n", err)
		return 1
	}
	if len(pairs) == 0 {
		fmt.Fprintf(os.Stderr, "forge scorecard rebuild: no attributable (model, task_type) pairs found\n")
		return 1
	}

	logln := func(s string) { fmt.Println(s) }
	logln(fmt.Sprintf("forge scorecard rebuild: rebuilding %d scorecard row(s) from %s", len(pairs), traceFile))
	for _, p := range pairs {
		runScorecardUpdateWithOut(root, p, outFile, logln, 0, false)
	}
	logln(fmt.Sprintf("forge scorecard rebuild: done — %d row(s) written to %s", len(pairs), outFile))
	return 0
}

// parseScorecardRebuildFlags parses `forge scorecard rebuild --from <trace.jsonl>
// [--out <scorecards.json>] [--workflow <name>] [--root DIR]` and resolves
// --from/--out defaults (tracePath(root)/scorecardPath(root)) when omitted.
// --workflow names the SINGLE workflow (under <root>/.agent/workflows/<name>.yml)
// to resolve phase-name -> task_type ground truth from; empty means "scan every
// workflow under .agent/workflows/*.yml" (see resolvePhaseTaskTypes). ok is false
// when the caller should return code immediately (a flag parse error).
func parseScorecardRebuildFlags(args []string) (root, traceFile, outFile, workflowName string, code int, ok bool) {
	fs := flag.NewFlagSet("scorecard rebuild", flag.ContinueOnError)
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&traceFile, "from", "", "trace JSONL file to rebuild from (required)")
	fs.StringVar(&outFile, "out", "", "output scorecards.json path (default: .agent/routing/scorecards.json under root)")
	fs.StringVar(&workflowName, "workflow", "", "workflow name to resolve phase-name -> task_type from (default: scan every .agent/workflows/*.yml)")
	if err := fs.Parse(args); err != nil {
		return "", "", "", "", 2, false
	}
	root = gate.RepoRoot(root)
	if traceFile == "" {
		traceFile = tracePath(root)
	}
	if outFile == "" {
		outFile = scorecardPath(root)
	}
	return root, traceFile, outFile, workflowName, 0, true
}

// resolvePhaseTaskTypes builds `forge scorecard rebuild`'s GROUND-TRUTH
// phase-name -> task_type map: it globs/loads the actual .agent/workflows/*.yml
// definition(s) off disk (the same loadWorkflow parsing path cmdRun uses) and
// hands the loaded []asset.Workflow to
// attribution.PhaseTaskTypes for the pure phase->task_type resolution — the
// FILE I/O stays here (cmd/forge owns loadWorkflow), the map-building logic
// is pure and lives in internal/attribution.
//
// workflowName, when non-empty, restricts the scan to that ONE workflow
// (<root>/.agent/workflows/<workflowName>.yml); empty scans every workflow under
// <root>/.agent/workflows/*.yml (sorted for determinism). A workflow file that
// fails to load (missing/malformed) is skipped, never fatal — the caller's
// fallback to the substring heuristic (attribution.TaskTypeForRebuildEvent)
// covers any phase name this map doesn't resolve, so ground truth is used when
// available and nothing is worse off than before when it isn't.
func resolvePhaseTaskTypes(root, workflowName string) map[string]string {
	var names []string
	if workflowName != "" {
		names = []string{workflowName}
	} else {
		matches, _ := filepath.Glob(filepath.Join(root, ".agent", "workflows", "*.yml"))
		sort.Strings(matches)
		for _, m := range matches {
			names = append(names, strings.TrimSuffix(filepath.Base(m), ".yml"))
		}
	}
	var workflows []asset.Workflow
	for _, name := range names {
		wf, err := loadWorkflow(root, name)
		if err != nil {
			continue // missing/malformed workflow: fall back to the heuristic for its phases
		}
		workflows = append(workflows, wf)
	}
	return attribution.PhaseTaskTypes(workflows)
}

// printTable displays every scorecard entry as a formatted table row.
func printTable(cards []routing.Scorecard, minSamples int) {
	fmt.Printf("%-18s %-16s %8s %8s  %s\n", "model", "task_type", "quality", "samples", "last_updated")
	fmt.Println(strings.Repeat("-", 75))
	for _, c := range cards {
		flag := ""
		if c.Samples < minSamples {
			flag = " *"
		}
		updated := c.UpdatedAt
		if len(updated) > 10 {
			updated = updated[:10] // trim timestamp to date
		}
		fmt.Printf("%-18s %-16s %8.3f %8d%s  %s\n",
			c.Model, c.TaskType, c.QualityScore, c.Samples, flag, updated)
	}
	if anyThin(cards, minSamples) {
		fmt.Printf("\n* fewer than %d samples — insufficient for routing influence (min_samples threshold)\n", minSamples)
	}
}

// printSummary shows a condensed view: one row per task_type, with the best
// quality score across all models and the total sample count.
func printSummary(cards []routing.Scorecard, minSamples int) {
	type taskRow struct {
		bestModel string
		bestScore float64
		totalSamp int
	}
	rows := make(map[string]*taskRow)
	order := make([]string, 0, len(cards))
	for _, c := range cards {
		r, ok := rows[c.TaskType]
		if !ok {
			r = &taskRow{}
			rows[c.TaskType] = r
			order = append(order, c.TaskType)
		}
		r.totalSamp += c.Samples
		if c.QualityScore > r.bestScore {
			r.bestScore = c.QualityScore
			r.bestModel = c.Model
		}
	}
	fmt.Printf("%-18s %8s %8s  %-16s\n", "task_type", "quality", "samples", "best_model")
	fmt.Println(strings.Repeat("-", 55))
	for _, tt := range order {
		r := rows[tt]
		flag := ""
		if r.totalSamp < minSamples {
			flag = " *"
		}
		fmt.Printf("%-18s %8.3f %8d%s  %-16s\n", tt, r.bestScore, r.totalSamp, flag, r.bestModel)
	}
	if anyThin(cards, minSamples) {
		fmt.Printf("\n* fewer than %d samples — insufficient for routing influence\n", minSamples)
	}
}

// anyThin reports whether any card has fewer than minSamples observations.
func anyThin(cards []routing.Scorecard, minSamples int) bool {
	for _, c := range cards {
		if c.Samples < minSamples {
			return true
		}
	}
	return false
}

// Agent→task_type vocabulary (AgentTaskType/TaskTypeForAgent) lives in
// internal/attribution — see distinctScorecardPairs above and
// engine_build.go's logPhaseHistory for its call sites.
