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
//
// Staging note: gate.mjs now imports ./adapters.mjs (-> ./arch/scan.mjs ->
// ./arch/scan-functions.mjs) for resolveEnforce, so a runnable temp gate needs the
// whole module graph copied, not just gate.mjs. stageGate copies those four modules
// verbatim from the real harness, then writes the given policies.yml; opts can add a
// `.agent/{project.yml,policies/modes.yml}` (to drive the mode×lifecycle resolution)
// and extra files (e.g. an oversized fixture to force a violation).
const HARNESS_DIR = dirname(GATE_PATH);
// The repo's real modes.yml (one level up from harness/), staged into temp roots so
// the mode×lifecycle resolution runs against the actual policy file.
const REAL_MODES = readFileSync(join(dirname(HARNESS_DIR), '.agent', 'policies', 'modes.yml'), 'utf8');

function stageGate(policyText, opts = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'forge-gate-'));
  const harness = join(dir, 'harness');
  mkdirSync(join(harness, 'arch'), { recursive: true });
  writeFileSync(join(harness, 'policies.yml'), policyText);
  // Copy the live module graph so the temp gate resolves its imports.
  for (const rel of ['gate.mjs', 'adapters.mjs', 'arch/scan.mjs', 'arch/scan-functions.mjs']) {
    writeFileSync(join(harness, rel), readFileSync(join(HARNESS_DIR, rel), 'utf8'));
  }
  // Optional central-knob config: project.yml mode×lifecycle + the real modes.yml.
  if (opts.projectYml !== undefined) {
    mkdirSync(join(dir, '.agent', 'policies'), { recursive: true });
    if (opts.projectYml !== null) writeFileSync(join(dir, '.agent', 'project.yml'), opts.projectYml);
    writeFileSync(join(dir, '.agent', 'policies', 'modes.yml'), opts.modesYml ?? REAL_MODES);
  }
  // Optional extra files (relative to repo root) — e.g. a violating fixture.
  for (const [rel, content] of Object.entries(opts.files ?? {})) {
    const full = join(dir, rel);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, content);
  }
  return dir;
}

function runGateIn(dir) {
  return spawnSync(process.execPath, [join(dir, 'harness', 'gate.mjs')], { cwd: dir, encoding: 'utf8' });
}

function runGateWithPolicy(policyText, opts = {}) {
  const dir = stageGate(policyText, opts);
  try {
    return runGateIn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// An oversized code file (lines > cap) is the simplest forced violation; the cap in
// these tests is small so this trips checkFileSizes deterministically.
function oversizedFile(lines) {
  return Array(lines).fill('const x = 1;').join('\n');
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

// === enforce strictness wired to mode×lifecycle (the C2 fix) ==================
// gate.mjs now resolves warn|block from .agent/project.yml × modes.yml (the central
// knob), not policies.yml's global enforce. These end-to-end runs stage a temp repo
// with a chosen mode×lifecycle + a forced oversized-file violation, and assert the
// HONESTY contract (warn ALWAYS reports the violation, only the exit code differs)
// and the SAFETY override (production forces block over a loose mode's warn).
//
// The fixture is 900 lines — over EVERY mode's file-size cap (explorer's relaxed
// 800 included), so src/big.js is the SOLE violation regardless of how max_file_lines
// now resolves per mode×lifecycle. (A 600-line fixture would NOT trip explorer, whose
// cap is 800, so these enforce-strictness tests use a violation that holds for all
// modes; the dedicated max_file_lines resolution is pinned separately below.) stageGate
// copies the harness module graph (gate/adapters/scan, all <500) into the temp tree for
// resolveEnforce's imports, so they never trip. `enforce` in policies.yml is the
// FALLBACK only — the live value comes from .agent.
const VIOLATE = { 'src/big.js': oversizedFile(900) };         // 900 lines > every mode's cap -> the only violation
const CAP500 = 'max_file_lines: 500\nmax_root_files: 99\nenforce: block\n';

test('warn mode (explorer×idea) REPORTS the violation but exits 0 (★honesty + speed★)', () => {
  // explorer×idea -> enforce warn. The violation must STILL be reported (never a
  // pretend-clean), yet the gate exits 0 so it does not block the fast path.
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: explorer\nlifecycle: idea\n', files: VIOLATE });
  assert.equal(res.status, 0, `warn must exit 0; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /WARN/, 'must announce WARN, not BLOCK');
  assert.match(res.stdout, /1 violation/, 'must report the violation COUNT (honesty)');
  assert.match(res.stdout, /src\/big\.js/, 'must list the offending FILE (never fake a clean tree)');
  assert.match(res.stdout, /mode×lifecycle/, 'must attribute the strictness to the central knob');
  assert.doesNotMatch(res.stdout, /PASS/, 'a tree WITH a violation must never claim PASS');
});

test('block mode (engineering×mvp) reports the violation AND exits 1 (blocks)', () => {
  // engineering×mvp -> enforce block (this repo's own setting). Same violation, but
  // now the gate stops the pipeline.
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: engineering\nlifecycle: mvp\n', files: VIOLATE });
  assert.equal(res.status, 1, `block must exit 1; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /BLOCK/);
  assert.match(res.stdout, /src\/big\.js/, 'block also reports the file');
  assert.match(res.stdout, /mode×lifecycle/);
});

test('production OVERRIDES a loose mode: explorer×production reports AND exits 1 (★safety★)', () => {
  // explorer's mode base is warn, but production's enforce_floor=block must win — a
  // violation under production BLOCKS even though the mode alone would only warn.
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: explorer\nlifecycle: production\n', files: VIOLATE });
  assert.equal(res.status, 1, `explorer+production must BLOCK (exit 1); stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /BLOCK/, 'production override -> BLOCK even for explorer');
  assert.match(res.stdout, /src\/big\.js/);
});

test('warn vs block report the SAME violations — only the exit code differs (honesty isolation)', () => {
  // Pin that warn does not HIDE anything block shows: identical violation report,
  // divergent exit code. This is the crux of the honesty requirement.
  const warn = runGateWithPolicy(CAP500, { projectYml: 'mode: explorer\nlifecycle: idea\n', files: VIOLATE });
  const block = runGateWithPolicy(CAP500, { projectYml: 'mode: engineering\nlifecycle: mvp\n', files: VIOLATE });
  assert.equal(warn.status, 0);
  assert.equal(block.status, 1);
  // Both list the same offending file and count; warn is not quieter about the facts.
  for (const r of [warn, block]) {
    assert.match(r.stdout, /1 violation/);
    assert.match(r.stdout, /src\/big\.js/);
  }
});

test('no violations -> PASS exit 0 under BOTH warn and block modes', () => {
  // A clean tree (no oversized file) passes regardless of strictness. Use a generous
  // cap so nothing trips; assert PASS + exit 0 for a warn mode and a block mode.
  const clean = 'max_file_lines: 500\nmax_root_files: 99\nenforce: block\n';
  const warn = runGateWithPolicy(clean, { projectYml: 'mode: explorer\nlifecycle: idea\n' });
  const block = runGateWithPolicy(clean, { projectYml: 'mode: engineering\nlifecycle: mvp\n' });
  for (const r of [warn, block]) {
    assert.equal(r.status, 0, `clean tree must exit 0; stdout:\n${r.stdout}`);
    assert.match(r.stdout, /PASS/);
  }
});

test('FAIL-SAFE: no .agent at all -> falls back to policies.yml enforce (block here)', () => {
  // With no central-knob config staged, resolveEnforce returns the policies.yml
  // fallback (block) — so a misconfigured project still BLOCKS on a violation.
  const res = runGateWithPolicy(CAP500, { files: VIOLATE });   // no projectYml => no .agent
  assert.equal(res.status, 1, `missing .agent -> policies fallback block -> exit 1; stdout:\n${res.stdout}`);
  assert.match(res.stdout, /BLOCK/);
});

test('backward-compat: THIS repo (engineering×mvp) still resolves to block, gate PASSes clean', () => {
  // The live gate on the real repo: engineering×mvp -> block, no violations -> PASS
  // exit 0. Proves the wiring did not change this repo's behavior.
  const res = spawnSync(process.execPath, [GATE_PATH], { cwd: dirname(HARNESS_DIR), encoding: 'utf8' });
  assert.equal(res.status, 0, `real repo must PASS; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /PASS/);
});

// === max_file_lines wired to mode×lifecycle (the file-size knob, end-to-end) ===
// gate.mjs now resolves the per-file line cap from .agent (explorer relaxes to 800
// for prototypes; production clamps any loose mode back to 500), not policies.yml's
// global 500. A 600-line file is the probe: tolerable under explorer, a violation
// under a strict mode or in production. The policies.yml fallback stays 500.
const RELAXED = { 'src/proto.js': oversizedFile(600) };       // 600: > 500 but <= explorer's 800

test('explorer relaxes max_file_lines to 800: a 600-line file PASSes (the gap this fix closes)', () => {
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: explorer\nlifecycle: idea\n', files: RELAXED });
  assert.equal(res.status, 0, `explorer (cap 800) must not flag 600 lines; stdout:\n${res.stdout}`);
  assert.match(res.stdout, /PASS/, 'a 600-line file is within explorer’s relaxed 800 cap');
});

test('engineering keeps max_file_lines at 500: the same 600-line file IS a violation', () => {
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: engineering\nlifecycle: mvp\n', files: RELAXED });
  assert.equal(res.status, 1, `engineering (cap 500, block) must flag 600 lines; stdout:\n${res.stdout}`);
  assert.match(res.stdout, /src\/proto\.js/);
});

test('production CLAMPS a loose mode’s relaxed cap: explorer×production flags 600 lines (★safety★)', () => {
  // explorer alone tolerates 600 (cap 800), but production clamps the cap back to 500
  // AND forces block — so a prototype-relaxed file size can never apply in production.
  const res = runGateWithPolicy(CAP500, { projectYml: 'mode: explorer\nlifecycle: production\n', files: RELAXED });
  assert.equal(res.status, 1, `explorer+production must clamp to 500 and BLOCK; stdout:\n${res.stdout}`);
  assert.match(res.stdout, /src\/proto\.js/);
});
