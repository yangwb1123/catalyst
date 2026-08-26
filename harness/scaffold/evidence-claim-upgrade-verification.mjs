import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  EVIDENCE_CLAIM_EXPECTED_FILES,
} from './evidence-claim-copy-fragment.mjs';

export const EVIDENCE_CLAIM_LEGACY_FILES = EVIDENCE_CLAIM_EXPECTED_FILES;
const DELIVERY_FILES = [
  ...EVIDENCE_CLAIM_EXPECTED_FILES,
  join('harness', 'governance_engineering', 'evidence_claim_portable.py'),
  join('harness', 'governance_engineering', 'test_evidence_claim_portable.py'),
];
const SCHEMA = join('docs', 'contracts',
  'governance-evidence-claim-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'governance-evidence-claim-v1.json');
const MANIFEST = join('skills', 'evidence-claim-management', 'references',
  'package-manifest.json');
const ADR = join('docs', 'adr',
  'ADR-0072-portable-evidence-claim-validation-skill.md');
const SCHEMA_SHA256 = 'b2f8824c95012d94e71b4643756890a7a23f67dc1b9e0e8ecacf979b016864e8';
const GOLDEN_SHA256 = 'db111600f93e63b3533b1f06b14d7520eb4cbec0e4c6d0e3a6e0fd7e2740824a';
const MANIFEST_SHA256 = 'ee8f1ee8644a04826aa0b718f76eb59e817ce2d315d54c8fad0ded9b7abf2ea0';
const ADR_SHA256 = '5ed33ea8d0a7e44e0ff401fad438c0fce0a875914da1187a64cb6cc3452b4929';

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target, encoding: 'utf8',
    env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1' },
  });
}

function assertPythonSuccess(result) {
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
}

export function assertEvidenceClaimScaffold(target) {
  for (const relative of DELIVERY_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Evidence Claim scaffold asset missing: ${relative}`);
  }
  assert.equal(sha256(join(target, SCHEMA)), SCHEMA_SHA256,
    'Evidence Claim Schema must match the frozen physical pin');
  assert.equal(sha256(join(target, GOLDEN)), GOLDEN_SHA256,
    'Evidence Claim golden must match the frozen physical pin');
  assert.equal(sha256(join(target, MANIFEST)), MANIFEST_SHA256,
    'Evidence Claim manifest must match the closed package pin');
  assert.equal(sha256(join(target, ADR)), ADR_SHA256,
    'ADR-0072 must match the frozen physical pin');

  const packageCheck = runPython(target, [
    '-I', '-B', 'skills/evidence-claim-management/scripts/check_package.py',
  ]);
  assertPythonSuccess(packageCheck);
  assert.match(packageCheck.stdout, /portable package VALID/);
  const packageTests = runPython(target, [
    '-I', '-B',
    'skills/evidence-claim-management/tests/test_package_integrity.py',
  ]);
  assertPythonSuccess(packageTests);
  const portableTests = runPython(target, [
    '-I', '-B',
    'skills/evidence-claim-management/tests/test_portable_scripts.py',
  ]);
  assertPythonSuccess(portableTests);
  const goldenCheck = runPython(target, [
    '-B', 'harness/governance_contract_check.py', '--golden', '.',
  ]);
  assertPythonSuccess(goldenCheck);
  assert.match(goldenCheck.stdout, /STRUCTURALLY_VALID/);

  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal scaffold must not copy the Catalyst Evidence Claim Go runtime');
  assert.equal(existsSync(join(target, 'forge-runtime')), false,
    'universal scaffold must not copy the Catalyst Evidence Claim Rust runtime');
  assert.equal(existsSync(join(target, '.codex', 'skills',
    'evidence-claim-management')), false,
  'universal scaffold must not install a host Evidence Claim Skill');
}
