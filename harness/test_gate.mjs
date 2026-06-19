// Tests for harness/gate.mjs pure functions (node:test, zero external deps).
// Run: node --test harness/test_gate.mjs   (or: node --test harness/)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { tmpdir } from 'node:os';

import * as gate from './gate.mjs';
const { parsePolicies, checkFileSizes, checkRootCount } = gate;

const GATE_PATH = join(dirname(fileURLToPath(import.meta.url)), 'gate.mjs');

// --- smoke: importing gate.mjs must have no side effects (main() not run) ---
// Observe the real invariant directly in a child process: a bare `import()`
// must print nothing and exit 0. The previous version only checked exported
// types from THIS module — which a stray main() that happened to exit 0 could
// have satisfied silently. Running import in a subprocess and asserting empty
// stdout + exit 0 catches any accidental output or process.exit on import.
test('importing gate.mjs produces no output and exits 0 (no side effects)', () => {
  const specifier = pathToFileURL(GATE_PATH).href;
  const res = spawnSync(
    process.execPath,
    ['-e', `import(${JSON.stringify(specifier)})`],
    { encoding: 'utf8' },
  );
  assert.equal(res.status, 0, `exit 0 expected; stderr:\n${res.stderr}`);
  assert.equal(res.stdout, '', `import must print nothing; got:\n${res.stdout}`);
  // The exports must still be present for the rest of this suite to use.
  assert.equal(typeof parsePolicies, 'function');
  assert.equal(typeof checkFileSizes, 'function');
  assert.equal(typeof checkRootCount, 'function');
});

// --- parsePolicies -----------------------------------------------------------
test('parsePolicies reads numbers as Number and keeps words as strings', () => {
  const sample = [
    '# ForgeOS harness policies — sample',
    'max_file_lines: 500',
    'max_function_lines: 50         # trailing comment stripped',
    'max_root_files: 15',
    'enforce: block                 # warn | block',
    '',
    '# fully commented line: should_not_appear: 999',
  ].join('\n');

  const out = parsePolicies(sample);

  assert.equal(out.max_file_lines, 500);
  assert.equal(typeof out.max_file_lines, 'number');
  assert.equal(out.max_function_lines, 50);
  assert.equal(out.max_root_files, 15);
  assert.equal(out.enforce, 'block');
  assert.equal(typeof out.enforce, 'string');
});

test('parsePolicies ignores comment-only and blank lines', () => {
  const out = parsePolicies('# only a comment\n\n   \n');
  assert.deepEqual(out, {});
});

test('parsePolicies ignores keys that are commented out', () => {
  const out = parsePolicies('# max_file_lines: 999\nmax_root_files: 15');
  assert.equal(out.max_file_lines, undefined);
  assert.equal(out.max_root_files, 15);
});

test('parsePolicies strips surrounding quotes (number stays numeric, word stays word)', () => {
  const out = parsePolicies("max_file_lines: '500'\nenforce: \"block\"");
  assert.equal(out.max_file_lines, 500);
  assert.equal(typeof out.max_file_lines, 'number');
  assert.equal(out.enforce, 'block');
  assert.equal(typeof out.enforce, 'string');
});

// --- main() validation (fail-closed on bad policy values) --------------------
// A bad cap must NOT silently disable the gate: garbage numerics and an unknown
// `enforce` value must exit 2, not degrade to a no-op pass.
function runGateWithPolicy(policyText) {
  const dir = mkdtempSync(join(tmpdir(), 'forge-gate-policy-'));
  try {
    const harness = join(dir, 'harness');
    mkdirSync(harness);
    writeFileSync(join(harness, 'policies.yml'), policyText);
    writeFileSync(join(harness, 'gate.mjs'), readFileSync(GATE_PATH, 'utf8'));
    return spawnSync(process.execPath, [join(harness, 'gate.mjs')], {
      cwd: dir,
      encoding: 'utf8',
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('main exits 2 on a garbage numeric policy (does not silently disable gate)', () => {
  const res = runGateWithPolicy('max_file_lines: 500abc\nenforce: block\n');
  assert.equal(res.status, 2, `expected exit 2; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stderr, /max_file_lines/);
});

test('main exits 2 on an unknown enforce value', () => {
  const res = runGateWithPolicy('max_file_lines: 500\nenforce: bogus\n');
  assert.equal(res.status, 2, `expected exit 2; stderr:\n${res.stderr}`);
  assert.match(res.stderr, /enforce/);
});

test('main accepts a quoted enforce value (block)', () => {
  // A tiny clean tree with a quoted enforce must PASS, proving quote-stripping
  // feeds a valid value through to validation rather than tripping the guard.
  const res = runGateWithPolicy("max_file_lines: 500\nmax_root_files: 15\nenforce: 'block'\n");
  assert.equal(res.status, 0, `expected exit 0; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /PASS/);
});

// --- checkFileSizes ----------------------------------------------------------
test('checkFileSizes flags files over the line cap and passes compliant ones', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-gate-sizes-'));
  try {
    const big = join(dir, 'big.js');
    const small = join(dir, 'small.js');
    const skipped = join(dir, 'notes.md'); // non-code ext -> ignored
    // maxLines = 10; a 12-line file is over, a 3-line file is under.
    writeFileSync(big, Array(12).fill('x').join('\n'));
    writeFileSync(small, 'a\nb\nc');
    writeFileSync(skipped, Array(999).fill('y').join('\n'));

    const out = checkFileSizes([big, small, skipped], 10);

    assert.equal(out.length, 1, 'exactly one violation (the big code file)');
    assert.match(out[0], /big\.js/);
    assert.match(out[0], /max 10/);
    // compliant + non-code files must not be reported
    assert.ok(!out.some((l) => /small\.js/.test(l)));
    assert.ok(!out.some((l) => /notes\.md/.test(l)));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('checkFileSizes passes when every code file is within the cap', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-gate-ok-'));
  try {
    const f = join(dir, 'ok.mjs');
    writeFileSync(f, 'one\ntwo');
    assert.deepEqual(checkFileSizes([f], 500), []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- checkRootCount ----------------------------------------------------------
test('checkRootCount flags when root file count exceeds max', () => {
  // The repo root contains at least one file, so max=0 must produce a violation.
  const out = checkRootCount(0);
  assert.equal(out.length, 1);
  assert.match(out[0], /root has \d+ files \(max 0\)/);
});

test('checkRootCount passes when root file count is within max', () => {
  // A very high cap can never be exceeded -> no violation.
  assert.deepEqual(checkRootCount(1_000_000), []);
});
