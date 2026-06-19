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
// A verdict is shaped like acceptance.decide()'s return: only `.accepted` matters.
const v = (accepted) => ({ accepted });

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
