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

// --- immutability: input array/rows are not mutated --------------------------
test('updateScorecards: does not mutate the input array or its rows', () => {
  const stored = row('opus', 'crud', 0.8, 10, OLD);
  const existing = [stored];
  const before = structuredClone(existing);
  updateScorecards(existing, row('opus', 'crud', 0.4, 30, TS));
  assert.deepEqual(existing, before, 'input array + rows are unchanged');
  assert.equal(stored.samples, 10, 'stored row object not mutated in place');
});
