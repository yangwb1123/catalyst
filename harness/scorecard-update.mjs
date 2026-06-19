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
// CLI:
//   node harness/scorecard-update.mjs --model <m> --task-type <t> \
//        [--out <path>] [--now <iso>] [--iterations <n>] [--rework <0|1>]
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { collect, decide } from './acceptance.mjs';
import { synthesize, merge } from './scorecard.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);
const DEFAULT_OUT = join(ROOT, '.agent', 'routing', 'scorecards.json');

// --- pure core (zero I/O, fully unit-testable) -------------------------------

// Two rows are the SAME scorecard entry iff their primary key (model, task_type)
// matches — see scorecard.schema.yml `primary_key: [model, task_type]`.
function samePair(a, b) {
  return a.model === b.model && a.task_type === b.task_type;
}

// updateScorecards(existingArray, newRow) -> a NEW array (input untouched).
//
// If a row for newRow's (model, task_type) already exists, it is replaced by the
// sample-weighted merge() of the stored row and newRow (samples summed). Every
// other row is carried through unchanged and in order. If no row matches, newRow
// is appended. Pure: no clock, no disk — the caller supplies newRow's timestamp.
export function updateScorecards(existing, newRow) {
  const rows = Array.isArray(existing) ? existing : [];
  let merged = false;
  const out = rows.map((row) => {
    if (!merged && samePair(row, newRow)) {
      merged = true;
      return merge(row, newRow);
    }
    return row;
  });
  if (!merged) out.push(newRow);
  return out;
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

// Run the acceptance gate once and fold its single verdict into the scorecards
// at `out`. Returns the fresh (pre-merge) row for logging. `now` is the ISO
// timestamp stamped onto the row (supplied by the caller — no Date.now() here).
// `iterations` / `reworked` are the optional trajectory the caller injects.
function runUpdate({ model, taskType, out, now, iterations, reworked }) {
  const verdict = withTrajectory(decide(collect()), { iterations, reworked });
  const newRow = synthesize({
    model,
    task_type: taskType,
    verdicts: [verdict],
    updated_at: now,
  });
  const merged = updateScorecards(readScorecards(out), newRow);
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

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.model || !args['task-type']) {
    console.error('usage: scorecard-update --model <m> --task-type <t> [--out <path>] [--now <iso>] [--iterations <n>] [--rework <0|1>]');
    process.exit(2);
  }
  const out = args.out || DEFAULT_OUT;
  const now = args.now || new Date().toISOString();
  const { verdict, newRow } = runUpdate({
    model: args.model,
    taskType: args['task-type'],
    out,
    now,
    iterations: parseIterations(args.iterations),
    reworked: parseRework(args.rework),
  });
  // avg_iterations is absent when no trajectory was supplied — print n/a so the
  // log never implies a fabricated count.
  const ai = newRow.avg_iterations === undefined ? 'n/a' : newRow.avg_iterations;
  console.log(
    `scorecard-update: ${verdict.accepted ? 'ACCEPTED' : 'REJECTED'} -> `
    + `${newRow.model}/${newRow.task_type} quality_score=${newRow.quality_score} `
    + `samples=${newRow.samples} rework_rate=${newRow.rework_rate} avg_iterations=${ai} -> ${out}`,
  );
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
