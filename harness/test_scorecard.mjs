// Tests for harness/scorecard.mjs pure functions (node:test, zero external deps).
// Run: node --test harness/test_scorecard.mjs   (or: node --test 'harness/test_*.mjs')
//
// scorecard.mjs is the Eval->scorecard->Router learning-loop writer: it folds a
// batch of acceptance verdicts into one (model × task_type) scorecard entry and
// merges fresh entries into stored ones. These tests pin the honesty contract
// (one accepted verdict == one positive sample), the sample-weighted merge, and
// the [0,1] / non-negative invariants the routing schema requires.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { synthesize, merge } from './scorecard.mjs';

const TS = '2026-06-19T00:00:00Z';
const OLD = '2026-06-01T00:00:00Z';
// A LEGACY verdict is shaped like acceptance.decide()'s return: only `.accepted`.
const v = (accepted) => ({ accepted });
// A TRAJECTORY verdict additionally carries iterations (rounds to green) and a
// reworked flag (reviewer bounced it). These feed avg_iterations / rework_rate.
const vt = (accepted, iterations, reworked) => ({ accepted, iterations, reworked });

// --- synthesize: batch of verdicts -> one entry ------------------------------
test('synthesize: an all-accepted batch yields quality_score 1.0', () => {
  const out = synthesize({
    model: 'sonnet',
    task_type: 'implementation',
    verdicts: [v(true), v(true), v(true)],
    updated_at: TS,
  });
  assert.equal(out.quality_score, 1.0);
  assert.equal(out.samples, 3);
  assert.equal(out.pass_rate, 1.0);
  assert.equal(out.model, 'sonnet');
  assert.equal(out.task_type, 'implementation');
  assert.equal(out.updated_at, TS);
});

test('synthesize: a half-accepted batch yields quality_score 0.5', () => {
  const out = synthesize({
    model: 'opus',
    task_type: 'architecture',
    verdicts: [v(true), v(false), v(true), v(false)],
    updated_at: TS,
  });
  assert.equal(out.quality_score, 0.5);
  assert.equal(out.samples, 4);
  assert.equal(out.pass_rate, 0.5);
});

test('synthesize: an all-rejected batch yields quality_score 0.0', () => {
  const out = synthesize({
    model: 'haiku',
    task_type: 'docs',
    verdicts: [v(false), v(false)],
    updated_at: TS,
  });
  assert.equal(out.quality_score, 0.0);
  assert.equal(out.samples, 2);
});

test('synthesize: an empty batch is samples 0 / score 0 (no divide-by-zero)', () => {
  const out = synthesize({ model: 'sonnet', task_type: 'test', verdicts: [], updated_at: TS });
  assert.equal(out.samples, 0);
  assert.equal(out.quality_score, 0);
  assert.ok(out.samples >= 0, 'samples is never negative');
});

test('synthesize: only .accepted === true counts (honesty — N/A never reaches here)', () => {
  // A verdict with accepted=false (e.g. an N/A-laden reject) is a negative sample,
  // never silently dropped or counted as positive.
  const out = synthesize({
    model: 'sonnet',
    task_type: 'bugfix',
    verdicts: [v(true), v(false), { accepted: 'yes' }, {}],
    updated_at: TS,
  });
  // Only the single literal `true` is accepted; 1/4 = 0.25.
  assert.equal(out.quality_score, 0.25);
  assert.equal(out.samples, 4);
});

// --- merge: sample-weighted mean ---------------------------------------------
test('merge: sample-weighted mean — (0.8,10) + (0.4,30) -> 0.5 / samples 40', () => {
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 0.8, samples: 10, updated_at: '2026-06-01T00:00:00Z' },
    { model: 'opus', task_type: 'crud', quality_score: 0.4, samples: 30, updated_at: TS },
  );
  assert.equal(out.quality_score, 0.5);
  assert.equal(out.samples, 40);
  assert.equal(out.pass_rate, 0.5);
  // incoming is the newer observation: its timestamp wins.
  assert.equal(out.updated_at, TS);
});

test('merge: into an empty (zero-sample) row just adopts the incoming row', () => {
  const out = merge(
    { model: 'sonnet', task_type: 'refactor_medium', quality_score: 0, samples: 0, updated_at: TS },
    { model: 'sonnet', task_type: 'refactor_medium', quality_score: 0.9, samples: 5, updated_at: TS },
  );
  assert.equal(out.quality_score, 0.9);
  assert.equal(out.samples, 5);
});

test('merge: both rows empty -> score 0, samples 0 (no divide-by-zero)', () => {
  const out = merge(
    { model: 'haiku', task_type: 'docs', quality_score: 0, samples: 0, updated_at: TS },
    { model: 'haiku', task_type: 'docs', quality_score: 0, samples: 0, updated_at: TS },
  );
  assert.equal(out.quality_score, 0);
  assert.equal(out.samples, 0);
});

// --- invariants the routing schema requires ----------------------------------
test('merge: samples is never negative even with a malformed negative count', () => {
  const out = merge(
    { model: 'opus', task_type: 'security', quality_score: 0.5, samples: -100, updated_at: TS },
    { model: 'opus', task_type: 'security', quality_score: 0.5, samples: -50, updated_at: TS },
  );
  assert.ok(out.samples >= 0, `samples must be >= 0, got ${out.samples}`);
  assert.equal(out.samples, 0);
});

test('merge: quality_score is clamped into [0,1] for out-of-range inputs', () => {
  const out = merge(
    { model: 'opus', task_type: 'payment', quality_score: 5, samples: 10, updated_at: TS },
    { model: 'opus', task_type: 'payment', quality_score: -3, samples: 10, updated_at: TS },
  );
  assert.ok(out.quality_score >= 0 && out.quality_score <= 1, `score in [0,1], got ${out.quality_score}`);
  // 5 -> 1, -3 -> 0, weighted mean of equal samples = 0.5.
  assert.equal(out.quality_score, 0.5);
});

test('synthesize: quality_score always lands in [0,1]', () => {
  const out = synthesize({
    model: 'sonnet',
    task_type: 'reviewer',
    verdicts: [v(true), v(true), v(false)],
    updated_at: TS,
  });
  assert.ok(out.quality_score >= 0 && out.quality_score <= 1);
});

// --- trajectory: avg_iterations + rework_rate --------------------------------
test('synthesize: trajectory rolls avg_iterations (over accepted) and rework_rate', () => {
  // 4 tasks: 3 accepted (1,3,2 rounds), 1 rejected (2 rounds, excluded from avg);
  // 2 of the 4 were reworked. quality 3/4, rework 2/4, avg (1+3+2)/3 = 2.
  const out = synthesize({
    model: 'sonnet',
    task_type: 'implementation',
    verdicts: [vt(true, 1, false), vt(true, 3, false), vt(false, 2, true), vt(true, 2, true)],
    updated_at: TS,
  });
  assert.equal(out.quality_score, 0.75);
  assert.equal(out.pass_rate, 0.75);
  assert.equal(out.samples, 4);
  assert.equal(out.rework_rate, 0.5);
  assert.equal(out.avg_iterations, 2, 'avg over accepted tasks only (reject excluded)');
});

test('synthesize: avg_iterations is OMITTED when no accepted task reports a count', () => {
  // Rejected tasks carry counts but never feed the "rounds to green" average;
  // with no accepted+counted task, the field is absent (honest "no data").
  const out = synthesize({
    model: 'haiku',
    task_type: 'docs',
    verdicts: [vt(false, 5, true), vt(true, undefined, false)],
    updated_at: TS,
  });
  assert.ok(!('avg_iterations' in out), 'no fabricated avg_iterations');
  assert.equal(out.rework_rate, 0.5, 'rework still counts the one bounced task');
});

test('synthesize: avg_iterations floors at 1.0 (schema minimum)', () => {
  // A bogus sub-1 round count must not sink below the schema floor of 1.0.
  const out = synthesize({
    model: 'opus',
    task_type: 'crud',
    verdicts: [vt(true, 0, false), vt(true, 0.2, false)],
    updated_at: TS,
  });
  assert.ok(out.avg_iterations >= 1, `avg_iterations >= 1, got ${out.avg_iterations}`);
  assert.equal(out.avg_iterations, 1);
});

// --- BACKWARD COMPAT: legacy {accepted}-only verdicts ------------------------
test('synthesize: legacy {accepted}-only batch is bit-for-bit unchanged + safe new fields', () => {
  // Same batch as the half-accepted case above; the old fields must not move,
  // rework_rate defaults to 0, and avg_iterations is absent (no trajectory).
  const out = synthesize({
    model: 'opus',
    task_type: 'architecture',
    verdicts: [v(true), v(false), v(true), v(false)],
    updated_at: TS,
  });
  assert.equal(out.quality_score, 0.5, 'quality_score unchanged');
  assert.equal(out.samples, 4, 'samples unchanged');
  assert.equal(out.pass_rate, 0.5, 'pass_rate unchanged');
  assert.equal(out.model, 'opus');
  assert.equal(out.task_type, 'architecture');
  assert.equal(out.updated_at, TS);
  assert.equal(out.rework_rate, 0, 'legacy verdict -> rework_rate defaults to 0');
  assert.ok(!('avg_iterations' in out), 'legacy verdict -> no avg_iterations');
});

// --- merge: trajectory folds (rework total-weighted, avg accepted-weighted) ---
test('merge: avg_iterations is accepted-sample-weighted; rework_rate total-weighted', () => {
  // existing: q=1.0 n=2 (2 accepted) avg=1 rework=0  -> accepted weight 2
  // incoming: q=0.5 n=4 (2 accepted) avg=3 rework=0.5 -> accepted weight 2
  // quality (1*2+0.5*4)/6 = 0.6667 ; rework (0*2+0.5*4)/6 = 0.3333
  // avg (1*2 + 3*2)/(2+2) = 2
  const out = merge(
    { model: 'sonnet', task_type: 'implementation', quality_score: 1.0, samples: 2, pass_rate: 1.0, rework_rate: 0, avg_iterations: 1, updated_at: OLD },
    { model: 'sonnet', task_type: 'implementation', quality_score: 0.5, samples: 4, pass_rate: 0.5, rework_rate: 0.5, avg_iterations: 3, updated_at: TS },
  );
  assert.ok(Math.abs(out.quality_score - 2 / 3) < 1e-9, `quality ~0.667, got ${out.quality_score}`);
  assert.ok(Math.abs(out.rework_rate - 1 / 3) < 1e-9, `rework ~0.333, got ${out.rework_rate}`);
  assert.equal(out.avg_iterations, 2, 'avg weighted by accepted samples');
  assert.equal(out.samples, 6);
  assert.equal(out.updated_at, TS);
});

test('merge: avg_iterations survives when only ONE side recorded it (legacy other side)', () => {
  // Legacy stored row has no avg_iterations; the incoming average is adopted
  // (the legacy side contributes weight 0, never a phantom value).
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1.0, samples: 2, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1.0, samples: 2, rework_rate: 0, avg_iterations: 4, updated_at: TS },
  );
  assert.equal(out.avg_iterations, 4, 'incoming avg adopted, legacy side weight 0');
});

test('merge: avg_iterations stays ABSENT when neither side recorded it', () => {
  const out = merge(
    { model: 'haiku', task_type: 'docs', quality_score: 0.5, samples: 4, updated_at: OLD },
    { model: 'haiku', task_type: 'docs', quality_score: 0.5, samples: 4, updated_at: TS },
  );
  assert.ok(!('avg_iterations' in out), 'no avg_iterations conjured from nothing');
  assert.equal(out.rework_rate, 0, 'missing rework on both legacy rows -> 0');
});

test('merge: rework_rate is clamped and avg_iterations floored (fail-closed)', () => {
  // Out-of-range rework (5 -> 1, -3 -> 0) and a sub-1 avg are defended.
  const out = merge(
    { model: 'opus', task_type: 'payment', quality_score: 1.0, samples: 10, rework_rate: 5, avg_iterations: 0.5, updated_at: OLD },
    { model: 'opus', task_type: 'payment', quality_score: 1.0, samples: 10, rework_rate: -3, avg_iterations: 0.5, updated_at: TS },
  );
  assert.ok(out.rework_rate >= 0 && out.rework_rate <= 1, `rework in [0,1], got ${out.rework_rate}`);
  // 5 -> 1, -3 -> 0, equal samples -> 0.5
  assert.equal(out.rework_rate, 0.5);
  assert.ok(out.avg_iterations >= 1, `avg_iterations >= 1, got ${out.avg_iterations}`);
  assert.equal(out.avg_iterations, 1);
});

test('merge: NaN trajectory inputs collapse to safe values (NaN never propagates)', () => {
  const out = merge(
    { model: 'opus', task_type: 'security', quality_score: 0.5, samples: 10, rework_rate: NaN, avg_iterations: NaN, updated_at: OLD },
    { model: 'opus', task_type: 'security', quality_score: 0.5, samples: 10, rework_rate: 0.4, avg_iterations: 2, updated_at: TS },
  );
  assert.ok(Number.isFinite(out.rework_rate), 'rework_rate is finite');
  // existing NaN -> 0; (0*10 + 0.4*10)/20 = 0.2
  assert.equal(out.rework_rate, 0.2);
  // existing avg is NaN -> not finite -> contributes weight 0; incoming avg=2 adopted.
  assert.equal(out.avg_iterations, 2);
});
