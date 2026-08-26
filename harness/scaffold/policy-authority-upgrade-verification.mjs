import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  POLICY_AUTHORITY_EXPECTED_FILES,
} from './policy-authority-copy-fragment.mjs';

export const POLICY_AUTHORITY_LEGACY_FILES = POLICY_AUTHORITY_EXPECTED_FILES;
const GRANT_SCHEMA = join('docs', 'contracts', 'capability-grant-v1.schema.json');
const APPROVAL_SCHEMA = join('docs', 'contracts', 'approval-record-v1.schema.json');
const GRANT_GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'capability-grant-v1.json',
);
const APPROVAL_GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'approval-record-v1.json',
);
const MANIFEST = join(
  'skills', 'policy-authority', 'references', 'package-manifest.json',
);
const ADR = join('docs', 'adr',
  'ADR-0073-portable-policy-authority-declaration-assessment-skill.md');
const GRANT_SCHEMA_SHA256 =
  'dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209';
const APPROVAL_SCHEMA_SHA256 =
  'bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64';
const GRANT_GOLDEN_SHA256 =
  '0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35';
const APPROVAL_GOLDEN_SHA256 =
  '501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978';
const MANIFEST_SHA256 =
  '73dd9b4fda1850faff838de812cc6540a841f9b73b8fecbe53ad1e647c21593f';
const ADR_SHA256 =
  'cb1a9adff937e39f3d42b052e19e7e0e1516968da967948508b45dd735bed619';

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target, encoding: 'utf8',
    env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1' },
  });
}

function assertPythonSuccess(result, label) {
  assert.equal(result.status, 0,
    `${label} must pass\n${result.stdout}\n${result.stderr}`);
}

function assertPinnedBytes(target) {
  const pins = [
    [GRANT_SCHEMA, GRANT_SCHEMA_SHA256, 'CapabilityGrant Schema'],
    [APPROVAL_SCHEMA, APPROVAL_SCHEMA_SHA256, 'ApprovalRecord Schema'],
    [GRANT_GOLDEN, GRANT_GOLDEN_SHA256, 'CapabilityGrant golden'],
    [APPROVAL_GOLDEN, APPROVAL_GOLDEN_SHA256, 'ApprovalRecord golden'],
    [MANIFEST, MANIFEST_SHA256, 'Policy Authority manifest'],
    [ADR, ADR_SHA256, 'ADR-0073'],
  ];
  for (const [relative, expected, label] of pins) {
    assert.equal(sha256(join(target, relative)), expected,
      `${label} must match the frozen physical pin`);
  }
}

function assertPackageChecks(target) {
  const checks = [
    ['package checker', ['-I', '-B',
      'skills/policy-authority/scripts/check_package.py']],
    ['portable adapter tests', ['-I', '-B',
      'skills/policy-authority/tests/test_portable_adapters.py']],
    ['package integrity tests', ['-I', '-B',
      'skills/policy-authority/tests/test_package_integrity.py']],
  ];
  for (const [label, argv] of checks) {
    assertPythonSuccess(runPython(target, argv), label);
  }
}

function assertContractChecks(target) {
  const grant = runPython(target, [
    '-B', 'harness/capability_grant_contract_check.py', '--golden', '.',
  ]);
  assertPythonSuccess(grant, 'CapabilityGrant golden checker');
  assert.match(grant.stdout, /declarations only; no authority/);
  const approval = runPython(target, [
    '-B', 'harness/approval_record_contract_check.py', '--golden', '.',
  ]);
  assertPythonSuccess(approval, 'ApprovalRecord golden checker');
  assert.match(approval.stdout, /declarations only; no authority/);
  assertPythonSuccess(runPython(target, [
    '-B', 'harness/governance_engineering/test_policy_authority_portable.py',
  ]), 'portable Policy Authority governance test');
}

function assertNoRuntimeOrAuthorityInstall(target) {
  const forbidden = [
    ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
    ['.codex', 'skills', 'policy-authority'],
    ['.forge', 'authority'], ['.forge', 'governance'],
    ['.forge', 'policy-authority'], ['.forge', 'keys'],
    ['.forge', 'trust-roots'], ['.forge', 'authority-state'],
    ['.forge', 'runtime-state'],
  ];
  for (const parts of forbidden) {
    assert.equal(existsSync(join(target, ...parts)), false,
      `source-only scaffold must not install ${parts.join('/')}`);
  }
}

export function assertPolicyAuthorityScaffold(target) {
  for (const relative of POLICY_AUTHORITY_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Policy Authority scaffold asset missing: ${relative}`);
  }
  assertPinnedBytes(target);
  assertPackageChecks(target);
  assertContractChecks(target);
  assertNoRuntimeOrAuthorityInstall(target);
}
