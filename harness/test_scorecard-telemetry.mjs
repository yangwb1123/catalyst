// Tests for harness/scorecard.mjs cost/latency TELEMETRY (node:test, zero deps).
// Run: node --test harness/test_scorecard-telemetry.mjs
//
// Split out of test_scorecard.mjs to keep each test file under the size gate.
// These pin the telemetry half of the Eval->scorecard->Router writer: the pure
// percentile/mean statistics, synthesize()'s p95_latency_ms / avg_cost_usd /
// window enrichment, and merge()'s weighted fold of those fields. The load-
// bearing contract is HONESTY: latency is a REAL percentile of measured samples,
// cost a real mean of injected estimates, and — critically — when NO telemetry
// is supplied the fields are OMITTED entirely (never a fabricated 0). The core
// scorecard semantics (quality_score / pass_rate / samples / trajectory / decay)
// live in test_scorecard.mjs.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { synthesize, merge, percentile, mean } from './scorecard.mjs';

const TS = '2026-06-19T00:00:00Z';
const OLD = '2026-06-01T00:00:00Z';
// A LEGACY verdict is shaped like acceptance.decide()'s return: only `.accepted`.
const v = (accepted) => ({ accepted });

// --- percentile: a REAL statistic (the anti-fabrication core) -----------------
test('percentile: p95 of [10,20,30,40,50] is 48 (interpolated tail)', () => {
  // rank = 0.95 * (5-1) = 3.8 -> between xs[3]=40 and xs[4]=50, frac 0.8 -> 48.
  assert.equal(percentile([10, 20, 30, 40, 50], 95), 48);
});

test('percentile: p50 (median) and the p0/p100 extremes', () => {
  assert.equal(percentile([10, 20, 30, 40, 50], 50), 30, 'median = middle element');
  assert.equal(percentile([10, 20, 30, 40, 50], 0), 10, 'p0 = min');
  assert.equal(percentile([10, 20, 30, 40, 50], 100), 50, 'p100 = max');
});

test('percentile: a single element IS the percentile (the only observation)', () => {
  assert.equal(percentile([42], 95), 42);
  assert.equal(percentile([42], 0), 42);
});

test('percentile: an empty array returns undefined (no data, NOT a fabricated 0)', () => {
  assert.equal(percentile([], 95), undefined);
});

test('percentile: UNSORTED input is sorted first (order independence)', () => {
  // Same multiset as the canonical case, shuffled -> identical p95.
  assert.equal(percentile([50, 10, 40, 20, 30], 95), 48);
  assert.equal(percentile([30, 30, 10, 20], 50), 25, 'median of even-count sorted set');
});

test('percentile: the input array is NOT mutated (works on a copy)', () => {
  const input = [50, 10, 40, 20, 30];
  percentile(input, 95);
  assert.deepEqual(input, [50, 10, 40, 20, 30], 'caller array left in original order');
});

test('percentile: non-finite samples are filtered before ranking (NaN never ranks)', () => {
  assert.equal(percentile([10, NaN, 20, Infinity, 30, 40, 50], 95), 48, 'garbage dropped, p95 over the finite 5');
  assert.equal(percentile([NaN, Infinity], 95), undefined, 'all-garbage -> no data -> undefined');
});

test('percentile: a garbage p is clamped into [0,100] (no out-of-range index)', () => {
  assert.equal(percentile([10, 20, 30, 40, 50], 999), 50, 'p>100 clamps to max');
  assert.equal(percentile([10, 20, 30, 40, 50], -50), 10, 'p<0 clamps to min');
});

// --- mean: arithmetic mean with the same "no data" honesty --------------------
test('mean: averages finite samples; empty/garbage -> undefined', () => {
  assert.equal(mean([0.01, 0.02, 0.03]), 0.02);
  assert.equal(mean([]), undefined, 'no data -> undefined, not 0');
  assert.equal(mean([NaN, Infinity]), undefined, 'all-garbage -> undefined');
  assert.equal(mean([0.02, NaN, 0.04]), 0.03, 'garbage filtered, mean over the finite 2');
});

// --- synthesize: cost/latency telemetry (injected, omitted when absent) -------
test('synthesize: latenciesMs -> p95_latency_ms (rounded real percentile)', () => {
  const out = synthesize({
    model: 'sonnet', task_type: 'implementation', verdicts: [v(true)], updated_at: TS,
    latenciesMs: [10, 20, 30, 40, 50],
  });
  assert.equal(out.p95_latency_ms, 48, 'p95 of the sample = 48, rounded');
});

test('synthesize: costsUsd -> avg_cost_usd (rounded mean)', () => {
  const out = synthesize({
    model: 'sonnet', task_type: 'implementation', verdicts: [v(true)], updated_at: TS,
    costsUsd: [0.01, 0.02, 0.03],
  });
  assert.equal(out.avg_cost_usd, 0.02);
});

test('synthesize: window is carried through verbatim', () => {
  const out = synthesize({
    model: 'sonnet', task_type: 'implementation', verdicts: [v(true)], updated_at: TS,
    window: '30d',
  });
  assert.equal(out.window, '30d');
});

test('synthesize: all telemetry together rides on a normal entry', () => {
  const out = synthesize({
    model: 'opus', task_type: 'architecture', verdicts: [v(true), v(false)], updated_at: TS,
    latenciesMs: [100, 200, 300], costsUsd: [0.1, 0.3], window: '7d',
  });
  assert.equal(out.quality_score, 0.5, 'core fields still correct');
  assert.equal(out.p95_latency_ms, percentileRound([100, 200, 300], 95));
  assert.equal(out.avg_cost_usd, 0.2);
  assert.equal(out.window, '7d');
});

// ★ THE ANTI-FABRICATION PROOF: no telemetry args -> NONE of the fields appear ★
test('synthesize: NO telemetry args -> p95/cost/window are OMITTED (never fabricated 0)', () => {
  const out = synthesize({
    model: 'sonnet', task_type: 'implementation', verdicts: [v(true), v(false)], updated_at: TS,
  });
  assert.ok(!('p95_latency_ms' in out), 'no fabricated p95_latency_ms');
  assert.ok(!('avg_cost_usd' in out), 'no fabricated avg_cost_usd');
  assert.ok(!('window' in out), 'no fabricated window');
});

test('synthesize: EMPTY telemetry arrays still OMIT the fields (empty != 0)', () => {
  const out = synthesize({
    model: 'sonnet', task_type: 'implementation', verdicts: [v(true)], updated_at: TS,
    latenciesMs: [], costsUsd: [], window: '',
  });
  assert.ok(!('p95_latency_ms' in out), 'empty latencies -> no field, not 0');
  assert.ok(!('avg_cost_usd' in out), 'empty costs -> no field, not 0');
  assert.ok(!('window' in out), 'empty-string window -> no field');
});

test('synthesize: a legit ZERO sample is recorded (0 latency/cost is data, not absence)', () => {
  // Honesty cuts both ways: a real measured 0 must NOT be dropped as "no data".
  const out = synthesize({
    model: 'sonnet', task_type: 'docs', verdicts: [v(true)], updated_at: TS,
    latenciesMs: [0, 0, 0], costsUsd: [0],
  });
  assert.equal(out.p95_latency_ms, 0, 'a measured 0 latency is recorded');
  assert.equal(out.avg_cost_usd, 0, 'a real 0 cost is recorded');
});

// --- merge: telemetry folds (cost weighted-mean, p95 approx, window passthrough)
test('merge: avg_cost_usd is a sample-weighted mean (like quality)', () => {
  // (0.02 * 10 + 0.06 * 30) / 40 = 0.05.
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, avg_cost_usd: 0.02, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 30, avg_cost_usd: 0.06, updated_at: TS },
  );
  assert.equal(out.avg_cost_usd, 0.05);
});

test('merge: p95_latency_ms is the documented sample-weighted-mean APPROXIMATION', () => {
  // (100 * 10 + 200 * 30) / 40 = 175, rounded.
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, p95_latency_ms: 100, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 30, p95_latency_ms: 200, updated_at: TS },
  );
  assert.equal(out.p95_latency_ms, 175);
});

test('merge: window comes from the newer (incoming) side, else the existing', () => {
  const fromIncoming = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, window: '30d', updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, window: '7d', updated_at: TS },
  );
  assert.equal(fromIncoming.window, '7d', 'incoming window wins');
  const fromExisting = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, window: '30d', updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, updated_at: TS },
  );
  assert.equal(fromExisting.window, '30d', 'incoming missing -> existing window kept');
});

test('merge: telemetry survives when only ONE side carries it (legacy other side)', () => {
  // Legacy stored row has no telemetry; the incoming values are adopted cleanly
  // (the legacy side contributes weight 0 — no NaN, no phantom).
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 5, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 5, p95_latency_ms: 90, avg_cost_usd: 0.04, window: '1d', updated_at: TS },
  );
  assert.equal(out.p95_latency_ms, 90, 'incoming p95 adopted');
  assert.equal(out.avg_cost_usd, 0.04, 'incoming cost adopted');
  assert.equal(out.window, '1d');
});

test('merge: telemetry stays ABSENT when neither side carries it (no conjuring)', () => {
  const out = merge(
    { model: 'haiku', task_type: 'docs', quality_score: 0.5, samples: 4, updated_at: OLD },
    { model: 'haiku', task_type: 'docs', quality_score: 0.5, samples: 4, updated_at: TS },
  );
  assert.ok(!('p95_latency_ms' in out), 'no p95 conjured from nothing');
  assert.ok(!('avg_cost_usd' in out), 'no cost conjured from nothing');
  assert.ok(!('window' in out), 'no window conjured from nothing');
});

test('merge: a NaN telemetry value collapses to the other side (NaN never propagates)', () => {
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, avg_cost_usd: NaN, p95_latency_ms: NaN, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 10, avg_cost_usd: 0.05, p95_latency_ms: 80, updated_at: TS },
  );
  assert.equal(out.avg_cost_usd, 0.05, 'existing NaN -> weight 0 -> incoming adopted');
  assert.equal(out.p95_latency_ms, 80);
  assert.ok(Number.isFinite(out.avg_cost_usd) && Number.isFinite(out.p95_latency_ms));
});

test('merge: a fully decayed (decayFactor 0) existing lets incoming telemetry dominate', () => {
  const out = merge(
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 50, avg_cost_usd: 0.5, p95_latency_ms: 900, updated_at: OLD },
    { model: 'opus', task_type: 'crud', quality_score: 1, samples: 4, avg_cost_usd: 0.02, p95_latency_ms: 50, updated_at: TS },
    0,
  );
  assert.equal(out.avg_cost_usd, 0.02, 'decayed existing -> incoming cost dominates');
  assert.equal(out.p95_latency_ms, 50, 'decayed existing -> incoming p95 dominates');
});

// Helper: expected rounded p95 for an assertion (keeps the test self-checking).
function percentileRound(values, p) {
  return Math.round(percentile(values, p));
}
