// Tests for harness/scorecard-update.mjs pure core (node:test, zero deps).
// Run: node --test harness/test_scorecard-update.mjs
//
// scorecard-update.mjs is the RUNNABLE learning-loop step: it folds ONE
// acceptance verdict into a persisted (model x task_type) scorecard row. These
// tests pin the PURE fold updateScorecards(): inserting a brand-new pair,
// sample-weighted merge into an existing pair (samples summed), leaving sibling
// rows untouched/ordered, immutability of the input, and that the produced row
// matches the scorecard.schema.yml field shape the Router consumes.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { updateScorecards } from './scorecard-update.mjs';

const TS = '2026-06-19T00:00:00Z';
const OLD = '2026-06-01T00:00:00Z';

// The required fields per scorecard.schema.yml `entry:`.
const SCHEMA_FIELDS = ['model', 'task_type', 'quality_score', 'samples', 'updated_at'];

function assertSchemaShape(row) {
  for (const f of SCHEMA_FIELDS) {
    assert.ok(Object.prototype.hasOwnProperty.call(row, f), `missing field: ${f}`);
  }
  assert.equal(typeof row.model, 'string');
  assert.equal(typeof row.task_type, 'string');
  assert.equal(typeof row.quality_score, 'number');
  assert.ok(row.quality_score >= 0 && row.quality_score <= 1, 'quality_score in [0,1]');
  assert.equal(typeof row.samples, 'number');
  assert.ok(Number.isInteger(row.samples) && row.samples >= 0, 'samples is a non-negative int');
  assert.equal(typeof row.updated_at, 'string');
}

const row = (model, task_type, quality_score, samples, updated_at = TS) => ({
  model, task_type, quality_score, samples, pass_rate: quality_score, updated_at,
});

// A TRAJECTORY-bearing row: adds the optional rework_rate / avg_iterations the
// learning loop now persists, on top of the required schema fields.
const trow = (model, task_type, quality_score, samples, rework_rate, avg_iterations, updated_at = TS) => ({
  model, task_type, quality_score, samples, pass_rate: quality_score, rework_rate, avg_iterations, updated_at,
});

// --- insert: new (model, task_type) pair -------------------------------------
test('updateScorecards: inserts a new pair when none matches', () => {
  const existing = [row('opus', 'architecture', 0.9, 10)];
  const fresh = row('sonnet', 'implementation', 1.0, 1);
  const out = updateScorecards(existing, fresh);

  assert.equal(out.length, 2);
  const added = out.find((r) => r.model === 'sonnet' && r.task_type === 'implementation');
  assert.deepEqual(added, fresh);
  assertSchemaShape(added);
});

test('updateScorecards: empty/undefined existing yields a one-row array', () => {
  const fresh = row('haiku', 'docs', 0.0, 1);
  assert.deepEqual(updateScorecards([], fresh), [fresh]);
  assert.deepEqual(updateScorecards(undefined, fresh), [fresh]);
});

// --- merge: existing (model, task_type) pair, sample-weighted ----------------
test('updateScorecards: merges into existing pair, samples summed + weighted', () => {
  // stored (0.8, n=10) absorbs fresh (0.4, n=30) -> 0.5 / 40, newer ts wins.
  const existing = [row('opus', 'crud', 0.8, 10, OLD)];
  const fresh = row('opus', 'crud', 0.4, 30, TS);
  const out = updateScorecards(existing, fresh);

  assert.equal(out.length, 1, 'no new row appended on a matching pair');
  const m = out[0];
  assert.equal(m.quality_score, 0.5);
  assert.equal(m.samples, 40);
  assert.equal(m.updated_at, TS, 'incoming (newer) timestamp wins');
  assertSchemaShape(m);
});

test('updateScorecards: a single accepted verdict (n=1, q=1) bumps the stored row', () => {
  // The real CLI feeds exactly one verdict per call; stored 0.5/4 + 1.0/1 = 0.6/5.
  const existing = [row('sonnet', 'implementation', 0.5, 4, OLD)];
  const fresh = row('sonnet', 'implementation', 1.0, 1, TS);
  const [m] = updateScorecards(existing, fresh);
  assert.equal(m.samples, 5);
  assert.equal(m.quality_score, 0.6);
});

// --- isolation: other rows untouched -----------------------------------------
test('updateScorecards: leaves non-matching rows untouched and ordered', () => {
  const a = row('haiku', 'docs', 0.2, 5);
  const b = row('opus', 'crud', 0.8, 10, OLD);
  const c = row('opus', 'architecture', 0.9, 7);
  const fresh = row('opus', 'crud', 1.0, 10, TS);
  const out = updateScorecards([a, b, c], fresh);

  assert.equal(out.length, 3, 'no row added/removed');
  assert.deepEqual(out[0], a, 'sibling row a is byte-for-byte unchanged');
  assert.deepEqual(out[2], c, 'sibling row c is byte-for-byte unchanged');
  // only the matching middle row changed
  assert.equal(out[1].samples, 20);
  assert.equal(out[1].quality_score, 0.9);
});

test('updateScorecards: merges only the FIRST matching row (primary key is unique)', () => {
  // Defensive: a malformed file with duplicate pairs must not double-merge.
  const dupA = row('opus', 'crud', 0.8, 10);
  const dupB = row('opus', 'crud', 0.2, 10);
  const fresh = row('opus', 'crud', 1.0, 10, TS);
  const out = updateScorecards([dupA, dupB], fresh);
  assert.equal(out.length, 2);
  assert.notDeepEqual(out[0], dupA, 'first match is merged');
  assert.deepEqual(out[1], dupB, 'second duplicate is left untouched');
});

// --- trajectory: optional avg_iterations / rework_rate carry through ---------
test('updateScorecards: inserts a trajectory row intact (optional fields preserved)', () => {
  const fresh = trow('sonnet', 'implementation', 1.0, 1, 0, 2);
  const out = updateScorecards([], fresh);
  assert.deepEqual(out, [fresh]);
  assert.equal(out[0].avg_iterations, 2);
  assert.equal(out[0].rework_rate, 0);
});

test('updateScorecards: merging trajectory rows folds avg_iterations + rework_rate', () => {
  // stored q=1 n=2 (2 accepted) avg=1 rework=0 ; fresh q=0.5 n=4 (2 accepted)
  // avg=3 rework=0.5 -> avg weighted by accepted (1*2+3*2)/4 = 2 ; rework
  // total-weighted (0*2+0.5*4)/6 = 0.333 ; quality (1*2+0.5*4)/6 = 0.667.
  const existing = [trow('sonnet', 'implementation', 1.0, 2, 0, 1, OLD)];
  const fresh = trow('sonnet', 'implementation', 0.5, 4, 0.5, 3, TS);
  const [m] = updateScorecards(existing, fresh);
  assert.equal(m.samples, 6);
  assert.equal(m.avg_iterations, 2);
  assert.ok(Math.abs(m.rework_rate - 1 / 3) < 1e-9, `rework ~0.333, got ${m.rework_rate}`);
  assertSchemaShape(m);
});

test('updateScorecards: a legacy stored row absorbs a trajectory verdict gracefully', () => {
  // Stored row predates trajectory (no rework_rate / avg_iterations); the fresh
  // verdict's average is adopted and rework folds with the legacy side as 0.
  const existing = [row('opus', 'crud', 1.0, 2, OLD)];
  const fresh = trow('opus', 'crud', 1.0, 2, 0.5, 4, TS);
  const [m] = updateScorecards(existing, fresh);
  assert.equal(m.avg_iterations, 4, 'incoming avg adopted over legacy (weight-0) side');
  // rework: legacy 0 over n=2, incoming 0.5 over n=2 -> (0+1)/4 = 0.25
  assert.equal(m.rework_rate, 0.25);
  assertSchemaShape(m);
});

// --- recency decay: opts.now ages the stored row down in the merge -----------
// Fixtures with a controlled gap between the stored row's updated_at and `now`.
const NOW = '2026-06-30T00:00:00Z';
const THIRTY_DAYS_AGO = '2026-05-31T00:00:00Z';   // exactly one 30-day half-life before NOW

test('updateScorecards: omitting opts.now keeps decay OFF (bit-for-bit legacy merge)', () => {
  // The canonical (0.8,10)+(0.4,30)->0.5/40 merge must be identical with no opts.
  const existing = [row('opus', 'crud', 0.8, 10, THIRTY_DAYS_AGO)];
  const fresh = row('opus', 'crud', 0.4, 30, NOW);
  const [m] = updateScorecards(existing, fresh);
  assert.equal(m.quality_score, 0.5, 'no opts.now -> decayFactor 1 -> un-decayed mean');
  assert.equal(m.samples, 40);
});

test('updateScorecards: opts.now halves a one-half-life-old stored row\'s weight', () => {
  // Stored row is exactly 30 days (one half-life) before NOW -> decayFactor 0.5.
  // existing q=1.0 n=10 -> weight 5 ; fresh q=0.0 n=10 -> weight 10.
  // quality = (1*5 + 0*10)/15 = 1/3 ; samples still the honest 20.
  const existing = [row('opus', 'crud', 1.0, 10, THIRTY_DAYS_AGO)];
  const fresh = row('opus', 'crud', 0.0, 10, NOW);
  const [m] = updateScorecards(existing, fresh, { now: NOW });
  assert.ok(Math.abs(m.quality_score - 1 / 3) < 1e-9, `decayed quality ~0.333, got ${m.quality_score}`);
  assert.equal(m.samples, 20, 'decay weights the average, not the reported sample count');
  assertSchemaShape(m);
});

test('updateScorecards: a fresh (age 0) stored row is NOT decayed even with opts.now', () => {
  // Stored row's updated_at == NOW -> ageDays 0 -> decayFactor 1 -> plain mean.
  const existing = [row('opus', 'crud', 1.0, 10, NOW)];
  const fresh = row('opus', 'crud', 0.0, 10, NOW);
  const [m] = updateScorecards(existing, fresh, { now: NOW });
  assert.equal(m.quality_score, 0.5, 'age 0 -> no decay -> equal-weight mean');
});

test('updateScorecards: an explicit half-life shortens the decay window', () => {
  // Same 30-day-old row, but a 15-day half-life makes it TWO half-lives old ->
  // decayFactor 0.25. existing q=1.0 n=10 -> weight 2.5 ; fresh q=0 n=10 -> 10.
  // quality = (1*2.5 + 0*10)/12.5 = 0.2.
  const existing = [row('opus', 'crud', 1.0, 10, THIRTY_DAYS_AGO)];
  const fresh = row('opus', 'crud', 0.0, 10, NOW);
  const [m] = updateScorecards(existing, fresh, { now: NOW, halfLifeDays: 15 });
  assert.ok(Math.abs(m.quality_score - 0.2) < 1e-9, `quality ~0.2, got ${m.quality_score}`);
});

test('updateScorecards: decay only touches the matched row, siblings untouched', () => {
  const a = row('haiku', 'docs', 0.2, 5, THIRTY_DAYS_AGO);
  const b = row('opus', 'crud', 1.0, 10, THIRTY_DAYS_AGO);
  const fresh = row('opus', 'crud', 0.0, 10, NOW);
  const out = updateScorecards([a, b], fresh, { now: NOW });
  assert.deepEqual(out[0], a, 'non-matching sibling is byte-for-byte unchanged (not decayed)');
  assert.ok(Math.abs(out[1].quality_score - 1 / 3) < 1e-9, 'only the matched row is decayed');
});

test('updateScorecards: a stored row with no parseable updated_at fails open (no decay)', () => {
  // A malformed/absent updated_at must not zero or NaN-poison the merge.
  const existing = [row('opus', 'crud', 1.0, 10, 'not-a-date')];
  const fresh = row('opus', 'crud', 0.0, 10, NOW);
  const [m] = updateScorecards(existing, fresh, { now: NOW });
  assert.equal(m.quality_score, 0.5, 'unparseable updated_at -> age non-finite -> decayFactor 1');
  assert.ok(Number.isFinite(m.quality_score));
});

// --- immutability: input array/rows are not mutated --------------------------
test('updateScorecards: does not mutate the input array or its rows', () => {
  const stored = row('opus', 'crud', 0.8, 10, OLD);
  const existing = [stored];
  const before = structuredClone(existing);
  updateScorecards(existing, row('opus', 'crud', 0.4, 30, TS));
  assert.deepEqual(existing, before, 'input array + rows are unchanged');
  assert.equal(stored.samples, 10, 'stored row object not mutated in place');
});

test('updateScorecards: decay path also leaves the input array immutable', () => {
  const stored = row('opus', 'crud', 1.0, 10, THIRTY_DAYS_AGO);
  const existing = [stored];
  const before = structuredClone(existing);
  updateScorecards(existing, row('opus', 'crud', 0.0, 10, NOW), { now: NOW });
  assert.deepEqual(existing, before, 'decay merge does not mutate inputs');
});
