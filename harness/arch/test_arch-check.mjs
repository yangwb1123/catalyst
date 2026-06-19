// Unit tests for the architecture checks. They run over SYNTHETIC scan models
// (no filesystem), so they are fast and deterministic — and, crucially, each
// includes a NEGATIVE fixture proving the check actually CATCHES its violation
// (a check that never fails is worthless). The live dogfood run
// (node harness/arch/arch-check.mjs) covers the real-tree path.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkLayering, checkPackage, checkFanin, checkCognitive, checkAntiPatterns } from './arch-check.mjs';

const rules = {
  architecture: { forbidden: ['domain -> infrastructure', 'domain -> application'] },
  package: { max_files: 2, max_exports: 3 },
  fanin: { max_importers: 1 },
  cognitive: { max_root_modules: 1 },
};

function file(rel, layer, imports = [], extra = {}) {
  return { rel, dir: rel.replace(/\/[^/]+$/, ''), layer, isTest: false, exports: 0, imports, ...extra };
}

test('layering: a clean inward-pointing model has no violations', () => {
  const m = { files: [
    file('app/service/s.go', 'application', [{ kind: 'internal', layer: 'domain', rel: 'app/domain/d.go' }]),
    file('app/domain/d.go', 'domain', []),
  ] };
  assert.equal(checkLayering(m, rules).length, 0);
});

test('layering: domain importing infrastructure IS flagged (inner->outer forbidden)', () => {
  const m = { files: [
    file('app/domain/d.go', 'domain', [{ kind: 'internal', layer: 'infrastructure', rel: 'app/store/m.go' }]),
  ] };
  const v = checkLayering(m, rules);
  assert.equal(v.length, 1);
  assert.match(v[0], /forbidden domain -> infrastructure/);
});

test('layering: a test file that violates direction is NOT flagged (tests skipped)', () => {
  const m = { files: [{
    ...file('app/domain/d_test.go', 'domain', [{ kind: 'internal', layer: 'infrastructure', rel: 'x' }]),
    isTest: true,
  }] };
  assert.equal(checkLayering(m, rules).length, 0);
});

test('package: > max non-test files is flagged; tests are excluded from the budget', () => {
  const m = { files: [
    file('p/a.go', null), file('p/b.go', null), file('p/c.go', null), // 3 non-test > max 2
    { ...file('p/c_test.go', null), isTest: true }, // excluded
  ] };
  assert.ok(checkPackage(m, rules).some((x) => /3 files/.test(x)));
});

test('fanin: a target imported by more than max_importers is flagged', () => {
  const m = { files: [
    file('a.go', null, [{ kind: 'internal', dir: '/t', layer: null }]),
    file('b.go', null, [{ kind: 'internal', dir: '/t', layer: null }]), // /t imported by 2 > 1
  ] };
  assert.equal(checkFanin(m, rules).length, 1);
});

test('cognitive: too many top-level source modules is flagged', () => {
  const m = { files: [file('one/a.go', null), file('two/b.go', null)] }; // 2 > 1
  assert.equal(checkCognitive(m, rules).length, 1);
});

test('anti-pattern naming: a utils/ grab-bag directory IS flagged', () => {
  const r = { naming: { anti_patterns: ['utils', 'common'] }, architecture: { dir_aliases: { service: 'application' } } };
  const m = { files: [file('src/utils/x.go', null), file('src/domain/d.go', null)] };
  const v = checkAntiPatterns(m, r);
  assert.equal(v.length, 1);
  assert.match(v[0], /utils/);
});

test('anti-pattern naming: a name blessed as a layer in dir_aliases is NOT flagged', () => {
  // `service` is a technical-role name, but here it is a deliberate layer
  // (service -> application), so it is exempt — not a junk drawer.
  const r = { naming: { anti_patterns: ['service'] }, architecture: { dir_aliases: { service: 'application' } } };
  const m = { files: [file('app/service/s.go', null)] };
  assert.equal(checkAntiPatterns(m, r).length, 0);
});
