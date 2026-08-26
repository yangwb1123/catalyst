import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  CHANGE_IMPACT_COST_RISK_EXPECTED_FILES,
  CHANGE_IMPACT_COST_RISK_PACKAGE_FILES,
} from './change-impact-cost-risk-copy-fragment.mjs';

export const CHANGE_IMPACT_COST_RISK_LEGACY_FILES =
  CHANGE_IMPACT_COST_RISK_EXPECTED_FILES;

const PACKAGE_ROOT = join('skills', 'change-impact-cost-risk');
const MANIFEST = join(PACKAGE_ROOT, 'references', 'package-manifest.json');
const ADR_0053 = join('docs', 'adr',
  '0053-local-go-package-dependency-graph-observation-producer-v1.md');
const ADR_0062 = join('docs', 'adr',
  '0062-local-go-package-impact-prescan-v1.md');
const ADR_0076 = join('docs', 'adr',
  'ADR-0076-portable-change-impact-cost-risk-lexical-prescan-skill.md');
const SCHEMA = join('docs', 'contracts',
  'local-go-package-impact-prescan-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'local-go-package-impact-prescan-v1.json');
const PINS = [
  [ADR_0053, '4bd8ed3e14478e3d41c0ecf8d04f50d65e5e14c5362a96b9be1207f6f90fbe99'],
  [ADR_0062, '9e4e2cc3b99d78fb26c5b55e23079a38463060667f175b7da2d82950029cb678'],
  [SCHEMA, 'a4592c63a938c090ccc4d6c8187bba8f37909ef6c2d2253fd06f656623c2bb25'],
  [GOLDEN, 'bc364e387705651d307a3ff18137b857a3fad2c518685a358bba169a835a68d9'],
  [MANIFEST, '50f56d957e9198c6e52fd6ab1506c23cef894c4e1d5049c1b3222f89e57101f6'],
  [ADR_0076, 'd7df301a4236be84e866a05c54089e79507db13ffba08ab85f955d27c3dc8b01'],
];

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function runPython(target, argv, input) {
  return spawnSync('python3', argv, {
    cwd: target,
    encoding: 'utf8',
    env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1', PYTHONPATH: 'harness' },
    ...(input === undefined ? {} : { input }),
  });
}

function assertSuccess(result, label) {
  assert.equal(result.status, 0,
    `${label} must pass\n${result.stdout}\n${result.stderr}`);
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value).sort().map(
      (key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`,
    ).join(',')}}`;
  }
  return JSON.stringify(value);
}

function assertClosedOwnership(target) {
  const universal = [
    ADR_0053, ADR_0062, SCHEMA, GOLDEN,
    join('.agent', 'skills', 'change-impact-cost-risk.md'),
  ];
  const overlap = CHANGE_IMPACT_COST_RISK_EXPECTED_FILES.filter(
    (path) => universal.includes(path),
  );
  assert.deepEqual(overlap, [],
    'portable fragment must not duplicate semantic or repo-adapter ownership');
  for (const relative of CHANGE_IMPACT_COST_RISK_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Change Impact Cost Risk scaffold asset missing: ${relative}`);
  }
  const manifest = JSON.parse(readFileSync(join(target, MANIFEST), 'utf8'));
  const declared = manifest.files.map(({ path }) => join(PACKAGE_ROOT, path));
  declared.push(MANIFEST);
  assert.deepEqual([...declared].sort(), [...CHANGE_IMPACT_COST_RISK_PACKAGE_FILES].sort(),
    'portable package fragment must equal the closed 32-file manifest');
}

function assertPinnedBytes(target) {
  for (const [relative, expected] of PINS) {
    assert.equal(sha256(join(target, relative)), expected,
      `${relative} must match its frozen physical pin`);
  }
  const copies = [
    [join(PACKAGE_ROOT, 'references',
      'local-go-package-impact-prescan-v1.schema.json'), SCHEMA],
    [join(PACKAGE_ROOT, 'references', 'fixtures',
      'local-go-package-impact-prescan-v1.json'), GOLDEN],
  ];
  for (const [packaged, semantic] of copies) {
    assert.deepEqual(readFileSync(join(target, packaged)),
      readFileSync(join(target, semantic)), `${packaged} semantic copy drifted`);
  }
}

function assertPackageAndGovernance(target) {
  const checks = [
    ['package checker', ['-I', '-B',
      'skills/change-impact-cost-risk/scripts/check_package.py']],
    ['portable projector tests', ['-I', '-B',
      'skills/change-impact-cost-risk/tests/test_portable_projector.py']],
    ['package integrity tests', ['-I', '-B',
      'skills/change-impact-cost-risk/tests/test_package_integrity.py']],
    ['copied governance tests', ['-B',
      'harness/governance_engineering/test_change_impact_cost_risk_portable.py']],
  ];
  for (const [label, argv] of checks) assertSuccess(runPython(target, argv), label);
}

function assertGoldenProjector(target) {
  const fixture = JSON.parse(readFileSync(join(target, GOLDEN), 'utf8'));
  const expected = fixture.expected.canonical_envelope_json;
  const request = canonicalJson(JSON.parse(expected).request);
  const script = 'skills/change-impact-cost-risk/scripts/'
    + 'project_local_go_package_impact_prescan.py';
  const result = runPython(target, ['-I', '-B', script], request);
  assertSuccess(result, `${script} exact golden request`);
  assert.equal(result.stdout, `${expected}\n`, `${script} output bytes drifted`);
  assert.equal(result.stderr, '', `${script} success stderr must be empty`);
}

export function assertNoChangeImpactCostRiskInstall(target) {
  const forbidden = [
    ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
    ['.codex', 'skills', 'change-impact-cost-risk'],
    ...[
      'impact', 'impact-analysis', 'full-impact', 'change-impact',
      'change-impact-cost-risk', 'cost', 'cost-model', 'risk', 'risk-model',
      'materiality', 'runtime', 'state', 'authority', 'persistence',
      'database', 'db',
    ].map((name) => ['.forge', name]),
  ];
  for (const parts of forbidden) {
    assert.equal(existsSync(join(target, ...parts)), false,
      `source-only lexical prescan must not install ${parts.join('/')}`);
  }
  const registry = readFileSync(
    join(target, '.agent', 'engineering', 'governance-contracts.yml'), 'utf8');
  assert.match(registry, /^version: 39$/m);
  assert.match(registry, /^change_impact_cost_risk_portable_projection:$/m);
}

export function assertChangeImpactCostRiskScaffold(target) {
  assertClosedOwnership(target);
  assertPinnedBytes(target);
  assertPackageAndGovernance(target);
  assertGoldenProjector(target);
  assertNoChangeImpactCostRiskInstall(target);
}
