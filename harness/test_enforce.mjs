// Tests for the ENFORCE-strictness resolution: the gate's warn|block level wired
// to the central knob (mode×lifecycle in .agent/project.yml × modes.yml), NOT
// policies.yml's single global enforce (node:test, zero external deps).
// Run: node --test harness/test_enforce.mjs   (or: node --test harness/)
//
// The pure decision (enforceRank / stricterEnforce / computeEnforce) and its I/O
// boundary (resolveEnforce) live in adapters.mjs alongside the coverage-threshold
// pair they mirror; the END-TO-END gate behavior they drive (warn reports-but-
// exits-0 / block exits-1 / production override) is pinned in test_gate.mjs. This
// file isolates the resolution itself: the strictness ordering (block > warn), the
// "take the stricter of base×floor" production override, the floor-absent-is-normal
// case (mvp has no enforce_floor), and the fail-safe to the policies fallback.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import {
  ENFORCE_LEVELS,
  enforceRank,
  stricterEnforce,
  computeEnforce,
  resolveEnforce,
  computeMaxFileLines,
  resolveMaxFileLines,
} from './adapters.mjs';
import { parseRules } from './arch/scan.mjs';

// The repo's real modes.yml (one level up from harness/), staged into temp roots so
// the on-disk mode×lifecycle resolution runs against the ACTUAL policy file — proving
// the base+floor round-trip from disk through parseRules.
const REPO_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const REAL_MODES = readFileSync(join(REPO_ROOT, '.agent', 'policies', 'modes.yml'), 'utf8');

// writeAgent: stage a chosen project.yml against (by default) the repo's own modes.yml
// under a temp root's .agent/. Passing null for either skips writing it (to exercise
// the missing-file fail-safe). inTmp: run fn in a throwaway dir, always cleaned up.
function writeAgent(root, projectYml, modesYml = REAL_MODES) {
  mkdirSync(join(root, '.agent', 'policies'), { recursive: true });
  if (projectYml !== null) writeFileSync(join(root, '.agent', 'project.yml'), projectYml);
  if (modesYml !== null) writeFileSync(join(root, '.agent', 'policies', 'modes.yml'), modesYml);
}
const inTmp = (fn) => { const d = mkdtempSync(join(tmpdir(), 'enforce-')); try { return fn(d); } finally { rmSync(d, { recursive: true, force: true }); } };

// --- ENFORCE_LEVELS / enforceRank: the strictness ordering -------------------

test('ENFORCE_LEVELS / enforceRank: block is stricter than warn; unknowns rank -1', () => {
  assert.deepEqual(ENFORCE_LEVELS, ['warn', 'block']);
  assert.ok(enforceRank('block') > enforceRank('warn'), 'block must outrank warn');
  assert.equal(enforceRank('warn'), 0);
  assert.equal(enforceRank('block'), 1);
  assert.equal(enforceRank('bogus'), -1, 'unknown -> -1 so it can never win a max');
  assert.equal(enforceRank(undefined), -1);
});

test('stricterEnforce: the production override lives here (warn vs block -> block)', () => {
  assert.equal(stricterEnforce('warn', 'block'), 'block', 'production floor block overrides mode warn');
  assert.equal(stricterEnforce('block', 'warn'), 'block', 'a block mode is not loosened by a warn floor');
  assert.equal(stricterEnforce('warn', 'warn'), 'warn');
  assert.equal(stricterEnforce('block', 'block'), 'block');
  // Floor absent is NORMAL (mvp/idea-on-some-modes) — base alone wins, not a fallback.
  assert.equal(stricterEnforce('warn', undefined), 'warn', 'no floor -> use the mode base');
  assert.equal(stricterEnforce('block', undefined), 'block');
  assert.equal(stricterEnforce(undefined, 'block'), 'block', 'no base -> use the floor');
  // Only when NEITHER is a known level is there nothing to resolve.
  assert.equal(stricterEnforce(undefined, undefined), null);
  assert.equal(stricterEnforce('garbage', 'junk'), null, 'two unknowns -> null (caller fails safe)');
});

// --- computeEnforce: the PURE mode×lifecycle resolution ----------------------

test('computeEnforce: per-mode base (explorer warn / balanced warn / engineering block)', () => {
  // idea floor is warn (never tightens past a block mode), mvp has no floor — both
  // isolate the per-mode BASE here.
  const M = parseRules(REAL_MODES);
  assert.equal(computeEnforce(M, 'explorer', 'mvp', 'block'), 'warn');
  assert.equal(computeEnforce(M, 'balanced', 'mvp', 'block'), 'warn');
  assert.equal(computeEnforce(M, 'engineering', 'mvp', 'block'), 'block');
  assert.equal(computeEnforce(M, 'cto', 'mvp', 'block'), 'warn');
  // A warn-floor (idea) does NOT loosen a block mode — engineering×idea stays block.
  assert.equal(computeEnforce(M, 'engineering', 'idea', 'block'), 'block', 'floor warn must not loosen mode block');
});

test('computeEnforce: production enforce_floor=block OVERRIDES a loose mode warn (★safety★)', () => {
  const M = parseRules(REAL_MODES);
  // Every loose mode forced to block under production — the non-negotiable override.
  assert.equal(computeEnforce(M, 'explorer', 'production', 'block'), 'block', 'explorer+production must block');
  assert.equal(computeEnforce(M, 'balanced', 'production', 'block'), 'block');
  assert.equal(computeEnforce(M, 'cto', 'production', 'block'), 'block');
  assert.equal(computeEnforce(M, 'engineering', 'production', 'block'), 'block');
  // growth floor is warn -> does not tighten a warn mode (stays the base).
  assert.equal(computeEnforce(M, 'explorer', 'growth', 'block'), 'warn', 'growth floor warn -> explorer stays warn');
});

test('computeEnforce: FAIL-SAFE — unknown mode×lifecycle -> the policies fallback (never looser)', () => {
  const M = parseRules(REAL_MODES);
  // Both base and floor unknown -> use the fallback verbatim when it is a valid level.
  assert.equal(computeEnforce(M, 'nope', 'nope', 'block'), 'block', 'unknown pair -> fallback block');
  assert.equal(computeEnforce(M, 'nope', 'nope', 'warn'), 'warn', 'unknown pair -> fallback warn');
  assert.equal(computeEnforce({}, 'balanced', 'mvp', 'block'), 'block', 'empty modes -> fallback');
  assert.equal(computeEnforce(null, 'balanced', 'mvp', 'block'), 'block', 'null modes -> fallback');
  assert.equal(computeEnforce(M, undefined, undefined, 'warn'), 'warn');
  // A GARBAGE fallback (broken policies.yml) hardens to block — never a no-op gate.
  assert.equal(computeEnforce(M, 'nope', 'nope', 'garbage'), 'block', 'unknown fallback -> conservative block');
  assert.equal(computeEnforce(M, 'nope', 'nope', undefined), 'block');
});

// --- resolveEnforce: the I/O boundary (project.yml × modes.yml off disk) ------

test('resolveEnforce reads project.yml × modes.yml off disk (mode×lifecycle -> warn/block)', () => {
  // explorer×mvp -> warn (mode base), engineering×mvp -> block (mirrors this repo).
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: explorer\nlifecycle: mvp\n'); return resolveEnforce(d, 'block'); }), 'warn');
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: engineering\nlifecycle: mvp\n'); return resolveEnforce(d, 'warn'); }), 'block', 'engineering base block even with warn fallback');
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: balanced\nlifecycle: idea\n'); return resolveEnforce(d, 'block'); }), 'warn');
  // ★ explorer×production -> block: the production override resolved OFF DISK.
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: explorer\nlifecycle: production\n'); return resolveEnforce(d, 'block'); }), 'block', 'explorer+production override -> block off disk');
});

test('resolveEnforce: FAIL-SAFE to the policies fallback when project.yml or modes.yml is missing', () => {
  assert.equal(inTmp((d) => resolveEnforce(d, 'block')), 'block', 'no .agent -> fallback block');
  assert.equal(inTmp((d) => resolveEnforce(d, 'warn')), 'warn', 'no .agent -> fallback warn (verbatim)');
  assert.equal(inTmp((d) => { writeAgent(d, 'mode: explorer\nlifecycle: mvp\n', null); return resolveEnforce(d, 'block'); }), 'block', 'modes.yml missing -> fallback');
  assert.equal(inTmp((d) => { writeAgent(d, null); return resolveEnforce(d, 'block'); }), 'block', 'project.yml missing -> fallback');
  // A garbage fallback through the missing-file path still hardens to block.
  assert.equal(inTmp((d) => resolveEnforce(d, 'garbage')), 'block', 'missing files + garbage fallback -> conservative block');
});

test('resolveEnforce on THIS repo agrees with computeEnforce over its OWN mode×lifecycle', () => {
  // Host-AGNOSTIC live wire — this file ships VERBATIM into every scaffolded project
  // (it is in forge-init's COPIED_FILES), so it must assert only what holds for ANY
  // ForgeOS project. Read the host's OWN project.yml × modes.yml and assert the
  // on-disk resolve equals the pure computation for that exact pair — proving the
  // wire is live WITHOUT binding to a specific value (engineering×mvp -> block in
  // the source repo; balanced×mvp -> warn in a default scaffold; both must pass).
  const project = parseRules(readFileSync(join(REPO_ROOT, '.agent', 'project.yml'), 'utf8'));
  const modes = parseRules(REAL_MODES);
  const expected = computeEnforce(modes, project.mode, project.lifecycle, 'block');
  assert.equal(resolveEnforce(REPO_ROOT, 'block'), expected, `this repo (${project.mode}×${project.lifecycle}) resolves to ${expected}`);
  // Host-agnostic backward-compat anchor: the resolved value is a REAL enforce
  // level (never silently undefined/garbage), whatever the host's mode×lifecycle.
  assert.ok(ENFORCE_LEVELS.includes(expected), `resolved enforce must be a real level (warn|block); got ${expected}`);
});

// --- computeMaxFileLines: the file-size cap's mode×lifecycle resolution -------
// The THIRD harness knob, mirroring computeEnforce above. explorer relaxes the
// per-file line cap to 800 for prototypes; production clamps any loose mode back
// down to 500 (file-size is tighten-only: smaller is stricter, so a ceiling only
// LOWERS). cto declares no cap -> the policies fallback passes through.

test('computeMaxFileLines: per-mode base (explorer 800 / balanced 500 / engineering 500)', () => {
  const M = parseRules(REAL_MODES);
  // mvp declares no max_file_lines ceiling -> the per-mode BASE is isolated here.
  assert.equal(computeMaxFileLines(M, 'explorer', 'mvp', 500), 800, 'explorer relaxes to 800 (the gap this fix closes)');
  assert.equal(computeMaxFileLines(M, 'balanced', 'mvp', 500), 500);
  assert.equal(computeMaxFileLines(M, 'engineering', 'mvp', 500), 500);
  assert.equal(computeMaxFileLines(M, 'cto', 'mvp', 500), 500, 'cto declares none -> fallback');
});

test('computeMaxFileLines: production ceiling CLAMPS a loose mode back to 500 (★tighten-only★)', () => {
  const M = parseRules(REAL_MODES);
  // The safety override: explorer's relaxed 800 can never apply in production.
  assert.equal(computeMaxFileLines(M, 'explorer', 'production', 500), 500, 'explorer+production clamped to 500');
  assert.equal(computeMaxFileLines(M, 'balanced', 'production', 500), 500);
  // A baseline already at/under the ceiling is never RAISED by it.
  assert.equal(computeMaxFileLines(M, 'engineering', 'production', 500), 500);
});

test('computeMaxFileLines: FAIL-SAFE — missing/garbage base or modes -> the policies fallback', () => {
  const M = parseRules(REAL_MODES);
  assert.equal(computeMaxFileLines(M, 'nope', 'mvp', 444), 444, 'unknown mode -> fallback');
  assert.equal(computeMaxFileLines({}, 'explorer', 'mvp', 444), 444, 'empty modes -> fallback');
  assert.equal(computeMaxFileLines(null, 'explorer', 'mvp', 444), 444, 'null modes -> fallback');
  // A non-positive/garbage ceiling is ignored (base passes through), never a 0-cap.
  const bad = { modes: { explorer: { harness: { max_file_lines: 800 } } }, lifecycle_modifiers: { production: { max_file_lines: 'junk' } } };
  assert.equal(computeMaxFileLines(bad, 'explorer', 'production', 500), 800, 'garbage ceiling ignored, not a 0-cap');
});

test('resolveMaxFileLines on THIS repo agrees with computeMaxFileLines over its OWN mode×lifecycle', () => {
  // Host-AGNOSTIC live wire (ships verbatim into every scaffold): assert only the
  // disk resolve equals the pure computation for the host's own pair.
  const project = parseRules(readFileSync(join(REPO_ROOT, '.agent', 'project.yml'), 'utf8'));
  const modes = parseRules(REAL_MODES);
  const expected = computeMaxFileLines(modes, project.mode, project.lifecycle, 500);
  assert.equal(resolveMaxFileLines(REPO_ROOT, 500), expected, `this repo (${project.mode}×${project.lifecycle}) resolves to ${expected}`);
  assert.ok(Number.isInteger(expected) && expected > 0, `resolved cap must be a positive integer; got ${expected}`);
});

test('resolveMaxFileLines: FAIL-SAFE to the fallback when project.yml or modes.yml is missing', () => {
  inTmp((root) => {
    assert.equal(resolveMaxFileLines(root, 500), 500, 'no .agent/ -> fallback');
    writeAgent(root, 'mode: explorer\nlifecycle: mvp\n', null); // modes.yml absent
    assert.equal(resolveMaxFileLines(root, 500), 500, 'modes.yml missing -> fallback');
  });
});
