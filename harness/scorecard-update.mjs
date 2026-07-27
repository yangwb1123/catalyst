#!/usr/bin/env node
// ForgeOS scorecard-update — the RUNNABLE learning-loop step that closes
// Eval -> scorecard -> Router. It turns ONE real acceptance verdict into one
// persisted scorecard row.
//
// Flow: acceptance.collect() runs the gate's probes -> acceptance.decide()
// renders ONE binary verdict (accepted true/false) -> that single sample feeds
// scorecard.synthesize() -> the fresh row is merge()'d (sample-weighted) into
// any stored row for the same (model, task_type) -> the merged array is written
// back to the --out JSON file (default .agent/routing/scorecards.json).
//
// Design: the pure fold (updateScorecards) is split from all I/O so it is unit-
// testable with zero spawning and zero filesystem. Timestamps are passed IN at
// the I/O boundary (read --now or default once) — NEVER Date.now() inside the
// pure core — keeping synthesis deterministic.
//
// TRAJECTORY: the verdict can carry how many rounds the task took to converge
// (--iterations) and whether a reviewer bounced it (--rework). These roll into
// the scorecard's avg_iterations / rework_rate (see scorecard.mjs). HONESTY:
// auto-collection of trajectory is NOT wired yet — the natural sources are the
// iteration count in `forge evolve`'s .forge/trace.jsonl and the reviewer's
// bounce-back verdict. Until that pipe is connected, the orchestrator/CLI
// supplies these via the flags below; omitting them yields the legacy binary
// row (rework_rate 0, avg_iterations absent).
//
// COST/LATENCY TELEMETRY: the row can also carry tail latency and average cost
// (scorecard.schema.yml p95_latency_ms / avg_cost_usd / window). Two injection
// paths, both honest about where the number comes from:
//   - --trace <path>: reads forge-core's .forge/trace.jsonl (one JSON event per
//       line; see forge-core/internal/trace/trace.go) and aggregates BOTH measured
//       signals it records — each event's `duration_ms` (a REAL wall-clock span ->
//       a genuine p95) AND each event's `cost_usd_micros` when present (the LLM
//       executor's REAL billed dollars -> avg_cost_usd). THIS IS THE REAL DATA PATH.
//   - --latency-ms / --cost-usd: comma-separated samples injected directly (for
//       an orchestrator that already holds them, or for tests).
// HONESTY on cost: cost is now READ FROM THE TRACE when a real LLM executor recorded
// it — forge writes `cost_usd_micros` straight from claude's billed total_cost_usd
// (the actual charge, MORE accurate than a token x unit-price estimate). A dry/echo
// run bills nothing and writes no cost field, so those events contribute nothing and
// avg_cost_usd is OMITTED unless a real-cost trace event or --cost-usd supplies one.
// Any telemetry with no samples is omitted entirely — never a fabricated 0.
//
// CLI:
//   node harness/scorecard-update.mjs --model <m> --task-type <t> \
//        [--out <path>] [--now <iso>] [--iterations <n>] [--rework <0|1>] \
//        [--trace <path>] [--run-id <id>] [--latency-ms "12,45,90"] [--cost-usd "0.01,0.02"] \
//        [--window 30d]
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { collect, decide } from './acceptance.mjs';
import { synthesize, merge, decayWeight } from './scorecard.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);
const DEFAULT_OUT = join(ROOT, '.agent', 'routing', 'scorecards.json');

// --- pure core (zero I/O, fully unit-testable) -------------------------------

const MS_PER_DAY = 86400000;
const TRACE_FORMAT_V1 = 'forgeos.trace.v1';

function supportedTraceEvent(ev) {
  return ev && (ev._format === undefined || ev._format === '' || ev._format === TRACE_FORMAT_V1);
}

// half-life for recency decay = policy.yml `history.recency_half_life_days`. A
// stored scorecard score older than this many days has its weight halved when a
// fresh batch folds in (see scorecard.merge's decayFactor). HONESTY: 30 comes
// from policy; this repo's history is currently thin so the practical effect is
// small, but the logic is in place so the Router always consumes a quality_score
// already decayed by recency — old scores stop dragging a model after it iterates.
const DEFAULT_HALF_LIFE_DAYS = 30;

// Two rows are the SAME scorecard entry iff their primary key (model, task_type)
// matches — see scorecard.schema.yml `primary_key: [model, task_type]`.
function samePair(a, b) {
  return a.model === b.model && a.task_type === b.task_type;
}

// Recency decay factor for a STORED row, given the externally-supplied `now`.
// PURE: `now` is passed in (never Date.now() here); parsing the row's ISO
// `updated_at` is a deterministic string->epoch, and the age is plain
// subtraction — so this stays unit-testable with a fixed --now + fixture row.
//   ageDays = (now - updated_at) / 86400000   ->   decayWeight(ageDays, halfLife)
// Fails OPEN to 1 (no decay) when `now`/`updated_at` is absent or unparseable:
// decayWeight already collapses a non-finite age to 1, so a row with no usable
// timestamp simply isn't decayed rather than being zeroed or NaN-poisoned.
function decayForRow(row, now, halfLifeDays) {
  const nowMs = Date.parse(now);
  const updatedMs = Date.parse(row && row.updated_at);
  const ageDays = (nowMs - updatedMs) / MS_PER_DAY;
  return decayWeight(ageDays, halfLifeDays);
}

// updateScorecards(existingArray, newRow, opts?) -> a NEW array (input untouched).
//
// If a row for newRow's (model, task_type) already exists, it is replaced by the
// sample-weighted merge() of the stored row and newRow (samples summed). Every
// other row is carried through unchanged and in order. If no row matches, newRow
// is appended. Pure: no clock, no disk — the caller supplies newRow's timestamp.
//
// opts.now (ISO) drives RECENCY DECAY: the matched stored row's age (now minus
// its updated_at) is turned into a decayFactor via decayWeight(age, half-life)
// and handed to merge(), so an older stored score loses weight to the fresh
// batch. BACKWARD COMPAT: omit opts (or opts.now) and decayFactor stays 1 — the
// merge is bit-for-bit the pre-decay behavior, so every existing call is intact.
export function updateScorecards(existing, newRow, opts = {}) {
  const rows = Array.isArray(existing) ? existing : [];
  const { now, halfLifeDays = DEFAULT_HALF_LIFE_DAYS } = opts;
  let merged = false;
  const out = rows.map((row) => {
    if (!merged && samePair(row, newRow)) {
      merged = true;
      // No `now` supplied -> decayFactor 1 (decay disabled, legacy behavior).
      const decayFactor = now === undefined ? 1 : decayForRow(row, now, halfLifeDays);
      return merge(row, newRow, decayFactor);
    }
    return row;
  });
  if (!merged) out.push(newRow);
  return out;
}

// parseTraceLatencies(text, model?) -> array of REAL per-event wall-clock durations
// (ms) extracted from forge-core trace JSONL. `text` is the raw file contents: one
// JSON object per line (see forge-core/internal/trace/trace.go `Event`). We read
// each event's `duration_ms` (the on-disk json tag) and keep the finite ones.
//
// MODEL FILTER (optional): when `model` is supplied, only events whose `model` field
// equals it are counted — so a trace that interleaves several models' cost/latency
// (forge stamps each billed agent event with its routed tier) yields a per-model p95.
// `model === undefined` does NOT filter (every event counts), keeping the existing
// `--trace` callers — `forge route`, any pre-attribution trace — byte-for-byte intact.
//
// PURE: text in, numbers out — no filesystem here (the read happens at the I/O
// boundary), so this is unit-testable against a fixture string. Robust by design
// because a trace can legitimately interleave shapes:
//   - blank lines are skipped
//   - a line that is not valid JSON is skipped (never throws — a corrupt tail
//     line must not sink the whole aggregation)
//   - an event with no numeric `duration_ms` (e.g. an instantaneous 0 is kept;
//     a missing/NaN field is skipped) contributes nothing
// The result feeds scorecard.percentile for a genuine p95 — these are measured
// latencies, not estimates.
export function parseTraceLatencies(text, model, runId, phaseNames) {
  if (typeof text !== 'string') return [];
  const out = [];
  for (const line of text.split('\n')) {
    const s = line.trim();
    if (s === '') continue;
    let ev;
    try { ev = JSON.parse(s); } catch { continue; } // skip a malformed line
    if (!supportedTraceEvent(ev)
      || (model !== undefined && ev.model !== model)
      || (runId !== undefined && ev.run_id !== runId)
      || (phaseNames !== undefined && !phaseNames.has(ev.name))) continue;
    if (Number.isFinite(ev.duration_ms)) out.push(ev.duration_ms);
  }
  return out;
}

// parseTraceCosts(text, model?) -> array of REAL per-event USD costs extracted from
// forge-core trace JSONL, mirroring parseTraceLatencies. forge writes each LLM
// executor event's billed cost as integer `cost_usd_micros` (microdollars, USD x
// 1e6) precisely to avoid the float-JSON drift a raw dollar double would print; we
// read that field and divide back to dollars. Only events that ACTUALLY billed carry
// it (a real claude phase); iteration/gate/converge and echo/dry agent events omit it
// and contribute nothing — honest "no data", never a fabricated 0.
//
// MODEL FILTER (optional): identical to parseTraceLatencies — when `model` is supplied,
// only events whose `model` field equals it count, so a multi-model trace yields a
// per-model avg cost; `model === undefined` does NOT filter (backward-compatible).
//
// PURE: text in, numbers out (the read is at the I/O boundary), so it is unit-testable
// against a fixture string, and identically robust to parseTraceLatencies:
//   - blank lines are skipped
//   - a non-JSON line is skipped (a corrupt tail must not sink the aggregation)
//   - an event with no finite cost_usd_micros contributes nothing
export function parseTraceCosts(text, model, runId, phaseNames) {
  if (typeof text !== 'string') return [];
  const out = [];
  for (const line of text.split('\n')) {
    const s = line.trim();
    if (s === '') continue;
    let ev;
    try { ev = JSON.parse(s); } catch { continue; } // skip a malformed line
    if (!supportedTraceEvent(ev)
      || (model !== undefined && ev.model !== model)
      || (runId !== undefined && ev.run_id !== runId)
      || (phaseNames !== undefined && !phaseNames.has(ev.name))) continue;
    if (Number.isFinite(ev.cost_usd_micros)) out.push(ev.cost_usd_micros / 1e6);
  }
  return out;
}

// parseNumberList("12,45,90") -> [12,45,90]. Splits on commas, trims, and keeps
// only finite numbers from NON-EMPTY tokens. Empty tokens are dropped FIRST,
// before Number(): note `Number("")` is 0, not NaN, so a stray "0.01,,0.02"
// must not coerce the empty slot into a phantom 0 cost (that would fabricate
// data — exactly what the honesty contract forbids). An undefined input ->
// undefined (field not injected); an explicit but empty/all-garbage list -> []
// which downstream treats as "no data" (the field is then omitted, never a 0).
export function parseNumberList(raw) {
  if (raw === undefined) return undefined;
  return raw
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t !== '')
    .map((t) => Number(t))
    .filter((n) => Number.isFinite(n));
}

// --- I/O boundary -------------------------------------------------------------

// Minimal flag parser for --k v pairs (zero-dep). Unknown flags are ignored.
function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a.startsWith('--') && i + 1 < argv.length) {
      out[a.slice(2)] = argv[i + 1];
      i += 1;
    }
  }
  return out;
}

// Read the stored scorecards array from `path`; a missing file is an empty
// array (first run), not an error. A malformed/ non-array file is rejected loud.
function readScorecards(path) {
  if (!existsSync(path)) return [];
  const parsed = JSON.parse(readFileSync(path, 'utf8'));
  if (!Array.isArray(parsed)) {
    throw new Error(`scorecards file is not a JSON array: ${path}`);
  }
  return parsed;
}

// Write the merged array back as pretty JSON, creating parent dirs as needed.
function writeScorecards(path, rows) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(rows, null, 2)}\n`);
}

// Enrich the gate's binary verdict with caller-supplied trajectory. `iterations`
// (rounds to green) and `reworked` (reviewer bounced it) are injected here at the
// I/O boundary — NOT auto-collected yet (see header HONESTY note). Undefined
// values are left off the verdict entirely so synthesize() sees the legacy shape
// and degrades gracefully (rework_rate 0, avg_iterations absent).
function withTrajectory(verdict, { iterations, reworked }) {
  const out = { ...verdict };
  if (iterations !== undefined) out.iterations = iterations;
  if (reworked !== undefined) out.reworked = reworked;
  return out;
}

// Read a forge-core trace file at `path` and parse out its measured latencies,
// optionally filtered to one `model`. I/O shell around the pure parseTraceLatencies; a
// missing file is an empty sample set (no trace yet), not an error — telemetry is
// optional and absence is honest. `undefined` path -> undefined (the --trace flag was
// not supplied). `model` is forwarded straight through (undefined -> no filter).
function readTraceLatencies(path, model, runId, phaseNames) {
  if (path === undefined) return undefined;
  if (!existsSync(path)) return [];
  return parseTraceLatencies(readFileSync(path, 'utf8'), model, runId, phaseNames);
}

// Read a forge-core trace file at `path` and parse out its REAL billed costs (optionally
// filtered to one `model`), the cost twin of readTraceLatencies. A missing file is an
// empty sample set (no trace yet), not an error; an undefined path -> undefined (--trace
// was not supplied), so the field is omitted rather than fabricated.
function readTraceCosts(path, model, runId, phaseNames) {
  if (path === undefined) return undefined;
  if (!existsSync(path)) return [];
  return parseTraceCosts(readFileSync(path, 'utf8'), model, runId, phaseNames);
}

// Combine the latency sources into one sample set for synthesize, or undefined
// when neither source was supplied (so the field is omitted, not a fabricated
// empty). Trace-measured latencies and directly-injected --latency-ms samples
// are concatenated: both are real measurements, just different ingress paths.
function combineLatencies(traceLatencies, injectedLatencies) {
  if (traceLatencies === undefined && injectedLatencies === undefined) return undefined;
  return [...(traceLatencies ?? []), ...(injectedLatencies ?? [])];
}

// Combine the cost sources into one sample set for synthesize, or undefined when
// neither was supplied (so the field is omitted, not a fabricated empty) — the cost
// twin of combineLatencies. Trace-recorded costs (forge's real billed cost_usd_micros)
// and directly-injected --cost-usd samples are concatenated: both are real dollars,
// just different ingress paths.
function combineCosts(traceCosts, injectedCosts) {
  if (traceCosts === undefined && injectedCosts === undefined) return undefined;
  return [...(traceCosts ?? []), ...(injectedCosts ?? [])];
}

// Run the acceptance gate once and fold its single verdict into the scorecards
// at `out`. Returns the fresh (pre-merge) row for logging. `now` is the ISO
// timestamp stamped onto the row (supplied by the caller — no Date.now() here).
// `iterations` / `reworked` are the optional trajectory the caller injects;
// `latenciesMs` / `costsUsd` / `window` are the optional cost/latency telemetry
// (each omitted from the row when absent — see synthesize's "no data" contract).
function runUpdate({ model, taskType, out, now, iterations, reworked, latenciesMs, costsUsd, window }) {
  const verdict = withTrajectory(decide(collect()), { iterations, reworked });
  const newRow = synthesize({
    model,
    task_type: taskType,
    verdicts: [verdict],
    updated_at: now,
    latenciesMs,
    costsUsd,
    window,
  });
  // Pass `now` so the stored row's age decays its weight in the merge (recency
  // half-life from policy); without it the fold would be un-decayed (legacy).
  const merged = updateScorecards(readScorecards(out), newRow, { now });
  writeScorecards(out, merged);
  return { verdict, newRow, merged };
}

// Parse the optional --iterations <float> flag. Absent -> undefined (the
// verdict stays legacy-shaped). A non-numeric value is rejected loud rather than
// silently poisoning the average.
function parseIterations(raw) {
  if (raw === undefined) return undefined;
  const n = Number(raw);
  if (!Number.isFinite(n)) throw new Error(`--iterations must be a number, got: ${raw}`);
  return n;
}

// Parse the optional --rework <0|1> flag into a boolean. Absent -> undefined
// (no rework signal). Only the literal "0" and "1" are accepted to keep the
// CLI contract unambiguous.
function parseRework(raw) {
  if (raw === undefined) return undefined;
  if (raw === '1') return true;
  if (raw === '0') return false;
  throw new Error(`--rework must be 0 or 1, got: ${raw}`);
}

// Format an optional telemetry field for the log: absent -> 'n/a' so the line
// never implies a fabricated number (mirrors avg_iterations).
function logField(v) {
  return v === undefined ? 'n/a' : v;
}

function phaseNameSet(raw) {
  return raw === undefined ? undefined : new Set(raw.split(',').map((s) => s.trim()).filter(Boolean));
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.model || !args['task-type']) {
    console.error('usage: scorecard-update --model <m> --task-type <t> [--out <path>] [--now <iso>] [--iterations <n>] [--rework <0|1>] [--trace <path>] [--run-id <id>] [--phase-names <a,b>] [--latency-ms "a,b,c"] [--cost-usd "a,b"] [--window 30d]');
    process.exit(2);
  }
  const out = args.out || DEFAULT_OUT;
  const now = args.now || new Date().toISOString();
  // Both latency AND cost now come from the trace (real measurements) and/or direct
  // injection: --trace feeds p95 latency (duration_ms) and avg cost (cost_usd_micros,
  // forge's real billed dollars), each concatenated with its direct-injection flag.
  // The trace reads are FILTERED to this row's --model, so a trace interleaving several
  // models (forge stamps each billed event with its routed tier) contributes only THIS
  // model's samples to THIS row — keeping per-model cost/latency honest. (For a legacy
  // single-model trace the filter is a no-op: every event already carries this model, or
  // — for a pre-attribution trace with no model field — none would match. The wind-down
  // only invokes us once it has confirmed model-stamped cost events exist, so the right
  // path is always the per-model filter.)
  const phaseNames = phaseNameSet(args['phase-names']);
  const latenciesMs = combineLatencies(
    readTraceLatencies(args.trace, args.model, args['run-id'], phaseNames),
    parseNumberList(args['latency-ms']),
  );
  const costsUsd = combineCosts(
    readTraceCosts(args.trace, args.model, args['run-id'], phaseNames),
    parseNumberList(args['cost-usd']),
  );
  const { verdict, newRow } = runUpdate({
    model: args.model,
    taskType: args['task-type'],
    out,
    now,
    iterations: parseIterations(args.iterations),
    reworked: parseRework(args.rework),
    latenciesMs,
    costsUsd,
    window: args.window,
  });
  // Optional fields are absent when no data was supplied — print n/a so the log
  // never implies a fabricated count/latency/cost.
  console.log(
    `scorecard-update: ${verdict.accepted ? 'ACCEPTED' : 'REJECTED'} -> `
    + `${newRow.model}/${newRow.task_type} quality_score=${newRow.quality_score} `
    + `samples=${newRow.samples} rework_rate=${newRow.rework_rate} `
    + `avg_iterations=${logField(newRow.avg_iterations)} `
    + `p95_latency_ms=${logField(newRow.p95_latency_ms)} `
    + `avg_cost_usd=${logField(newRow.avg_cost_usd)} -> ${out}`,
  );
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
