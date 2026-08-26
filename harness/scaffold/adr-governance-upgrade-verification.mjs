import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  ADR_GOVERNANCE_EXPECTED_FILES,
} from './adr-governance-copy-fragment.mjs';

export const ADR_GOVERNANCE_LEGACY_FILES = ADR_GOVERNANCE_EXPECTED_FILES;
const PACKAGE_ROOT = join('skills', 'adr-governance');
const MANIFEST = join(PACKAGE_ROOT, 'references', 'package-manifest.json');
const SEMANTIC_ADR = join(
  'docs', 'adr', '0067-proposed-only-adr-v2-frontmatter.md',
);
const PORTABLE_ADR = join(
  'docs', 'adr',
  'ADR-0074-portable-adr-governance-proposed-document-validation-skill.md',
);
const SCHEMA = join(
  'docs', 'contracts', 'architecture-decision-record-v2.schema.json',
);
const GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'ADR-9001-proposed-boundary.md',
);
const PINS = [
  [MANIFEST,
    'c1f84e909414878eec6ed62e6605ce7c26758f1940fb1a4660ecef7dcb56fab7',
    'ADR Governance package manifest'],
  [SEMANTIC_ADR,
    '78c7d484cfb0e448c4c896440d4ea272a8e32a60f947539a3ad739baaeead71e',
    'ADR-0067'],
  [SCHEMA,
    'ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b',
    'ArchitectureDecisionRecord v2 Schema'],
  [GOLDEN,
    'b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194',
    'ArchitectureDecisionRecord v2 golden'],
  [PORTABLE_ADR,
    '21d452845cf0f2889fcc5fa22f450cc4a40d5fb694f5b1f202d4b3cfd79f2eb2',
    'ADR-0074'],
  [join('harness', 'governance_engineering', 'adr_governance_portable.py'),
    '20bbce90227803329d192597eed1859b69508be3e24f9c5249be86441ba445a6',
    'ADR Governance module'],
  [join('harness', 'governance_engineering', 'test_adr_governance_portable.py'),
    'f943fc18d083da69b343fcb5026d7f5422487ca2baca50258db791214b024a9b',
    'ADR Governance module test'],
];
const SUCCESS = (
  'STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document ' +
  'bytes only; no identity, ownership, approver, evidence, claim, graph, ' +
  'acceptance, compliance, persistence, transition, execution, or effect ' +
  'attestation)\n'
);

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function runPython(target, argv, options = {}) {
  return spawnSync('python3', argv, {
    cwd: target,
    encoding: 'utf8',
    env: {
      PATH: process.env.PATH,
      PYTHONDONTWRITEBYTECODE: '1',
      ...(options.pythonPath ? { PYTHONPATH: options.pythonPath } : {}),
    },
    ...(options.input === undefined ? {} : { input: options.input }),
  });
}

function assertSuccess(result, label) {
  assert.equal(result.status, 0,
    `${label} must pass\n${result.stdout}\n${result.stderr}`);
}

function assertPinnedBytes(target) {
  for (const [relative, expected, label] of PINS) {
    assert.equal(sha256(join(target, relative)), expected,
      `${label} must match its frozen physical pin`);
  }
  const copies = [
    [join(PACKAGE_ROOT, 'references',
      'architecture-decision-record-v2.schema.json'), SCHEMA],
    [join(PACKAGE_ROOT, 'references', 'fixtures',
      'ADR-9001-proposed-boundary.md'), GOLDEN],
  ];
  for (const [packaged, authoritative] of copies) {
    assert.deepEqual(
      readFileSync(join(target, packaged)),
      readFileSync(join(target, authoritative)),
      `${packaged} must exactly preserve the ADR-0067 artifact`,
    );
  }
}

function assertCopiedPackageAndTests(target) {
  const checks = [
    ['package checker', ['-I', '-B',
      'skills/adr-governance/scripts/check_package.py']],
    ['portable adapter tests', ['-I', '-B',
      'skills/adr-governance/tests/test_portable_adapter.py']],
    ['package integrity tests', ['-I', '-B',
      'skills/adr-governance/tests/test_package_integrity.py']],
  ];
  for (const [label, argv] of checks) {
    assertSuccess(runPython(target, argv), label);
  }
  const golden = readFileSync(join(target, GOLDEN));
  const adapter = runPython(target, [
    '-I', '-B',
    'skills/adr-governance/scripts/validate_declared_proposed_adr.py',
    'ADR-9001-proposed-boundary.md',
  ], { input: golden });
  assertSuccess(adapter, 'copied golden through the portable adapter');
  assert.equal(adapter.stdout, SUCCESS,
    'portable golden result must preserve the exact marker and LF');
  assert.equal(adapter.stderr, '');
}

function assertCopiedGovernance(target) {
  const governance = runPython(target, [
    '-B', 'harness/governance_engineering/test_adr_governance_portable.py',
  ], { pythonPath: 'harness' });
  assertSuccess(governance, 'portable ADR Governance copied governance test');
  const registry = readFileSync(join(
    target, '.agent', 'engineering', 'governance-contracts.yml'), 'utf8');
  assert.match(registry, /^version: 39$/m,
    'portable ADR Governance scaffold requires active registry v39');
}

function assertNoRuntimeOrAuthorityInstall(target) {
  const forbidden = [
    ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
    ['.codex', 'skills', 'adr-governance'],
    ...[
      'writes_adr', 'writes-adr', 'artifact', 'artifacts', 'receipt',
      'receipts', 'persistence', 'state', 'key', 'keys', 'authority',
      'runtime',
    ].flatMap((name) => [[name], ['.forge', name]]),
    ['.forge', 'adr-governance'],
  ];
  for (const parts of forbidden) {
    assert.equal(existsSync(join(target, ...parts)), false,
      `source-only ADR Governance scaffold must not install ${parts.join('/')}`);
  }
}

export function assertADRGovernanceScaffold(target) {
  for (const relative of ADR_GOVERNANCE_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `ADR Governance scaffold asset missing: ${relative}`);
  }
  assertPinnedBytes(target);
  assertCopiedPackageAndTests(target);
  assertCopiedGovernance(target);
  assertNoRuntimeOrAuthorityInstall(target);
}
