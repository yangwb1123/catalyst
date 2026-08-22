import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { GRAPH_SNAPSHOT_COPIED_FILES } from './graph-snapshot-copy-fragment.mjs';
import {
  KNOWLEDGE_GRAPH_CURATION_EXPECTED_FILES,
  KNOWLEDGE_GRAPH_CURATION_PACKAGE_FILES,
} from './knowledge-graph-curation-copy-fragment.mjs';

export const KNOWLEDGE_GRAPH_CURATION_LEGACY_FILES =
  KNOWLEDGE_GRAPH_CURATION_EXPECTED_FILES;

const PACKAGE_ROOT = join('skills', 'knowledge-graph-curation');
const MANIFEST = join(PACKAGE_ROOT, 'references', 'package-manifest.json');
const ADR = join('docs', 'adr',
  'ADR-0075-portable-knowledge-graph-curation-partial-projectors-skill.md');
const MODULE_SCHEMA = join('docs', 'contracts', 'graph-snapshot-v1.schema.json');
const TEST_SCHEMA = join(
  'docs', 'contracts', 'graph-snapshot-go-test-source-v1.schema.json',
);
const MODULE_GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'graph-snapshot-v1.json',
);
const TEST_GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'graph-snapshot-go-test-source-v1.json',
);
const PINS = [
  [join('docs', 'adr', '0065-authority-free-graph-snapshot-v1-contract.md'),
    'c8e6cc3cb67d847d9b01b45d8043d132168d64eb42ff453b75864a46679ab11a'],
  [join('docs', 'adr', '0066-local-go-lexical-test-source-graph-snapshot.md'),
    '5c3c59521e27f19d202639bf14d953e6fbe76559f7159de3ccaf7790510d141d'],
  [MODULE_SCHEMA,
    '9dcaf66cff5b6d10338af6d295c75b2a5925604238cc276f80b68d3783d72bff'],
  [TEST_SCHEMA,
    'bfada8bb3d183061f2758bfc3645b56dc038b35d38c3c0b779a8ef32afcd17be'],
  [MODULE_GOLDEN,
    '8ce8418e840c97ef28ed77dfd5112c4c4b7d7ae8d843b714674e102d6322b03e'],
  [TEST_GOLDEN,
    'df1b25a933ffa2503f750e2209c9866bfe126e273b28c1181bb211ce48cae5e9'],
  [MANIFEST,
    'c9b8397658c3bcecb474966a3efd155f0af550be4fe7319dcdbf23a63cec2008'],
  [ADR,
    '81c0690f1f305dbf714a7ee0afd8dae0f2226d95a015fe019f087c3429761f91'],
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
  const overlap = KNOWLEDGE_GRAPH_CURATION_EXPECTED_FILES.filter(
    (path) => GRAPH_SNAPSHOT_COPIED_FILES.includes(path),
  );
  assert.deepEqual(overlap, [],
    'portable fragment must not duplicate ADR-0065/0066, Schema or golden ownership');
  for (const relative of KNOWLEDGE_GRAPH_CURATION_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Knowledge Graph Curation scaffold asset missing: ${relative}`);
  }
  const manifest = JSON.parse(readFileSync(join(target, MANIFEST), 'utf8'));
  const declared = manifest.files.map(({ path }) => join(PACKAGE_ROOT, path));
  declared.push(MANIFEST);
  assert.deepEqual([...declared].sort(), [...KNOWLEDGE_GRAPH_CURATION_PACKAGE_FILES].sort(),
    'portable package fragment must equal the closed 46-file manifest');
}

function assertPinnedBytes(target) {
  for (const [relative, expected] of PINS) {
    assert.equal(sha256(join(target, relative)), expected,
      `${relative} must match its frozen physical pin`);
  }
  const copies = [
    [join(PACKAGE_ROOT, 'references', 'graph-snapshot-v1.schema.json'), MODULE_SCHEMA],
    [join(PACKAGE_ROOT, 'references',
      'graph-snapshot-go-test-source-v1.schema.json'), TEST_SCHEMA],
    [join(PACKAGE_ROOT, 'references', 'fixtures', 'graph-snapshot-v1.json'),
      MODULE_GOLDEN],
    [join(PACKAGE_ROOT, 'references', 'fixtures',
      'graph-snapshot-go-test-source-v1.json'), TEST_GOLDEN],
  ];
  for (const [packaged, semantic] of copies) {
    assert.deepEqual(readFileSync(join(target, packaged)),
      readFileSync(join(target, semantic)), `${packaged} semantic copy drifted`);
  }
}

function assertPackageAndGovernance(target) {
  const checks = [
    ['package checker', ['-I', '-B',
      'skills/knowledge-graph-curation/scripts/check_package.py']],
    ['portable projector tests', ['-I', '-B',
      'skills/knowledge-graph-curation/tests/test_portable_projectors.py']],
    ['package integrity tests', ['-I', '-B',
      'skills/knowledge-graph-curation/tests/test_package_integrity.py']],
    ['copied governance tests', ['-B',
      'harness/governance_engineering/test_knowledge_graph_curation_portable.py']],
  ];
  for (const [label, argv] of checks) assertSuccess(runPython(target, argv), label);
}

function assertGoldenProjector(target, fixturePath, script) {
  const fixture = JSON.parse(readFileSync(join(target, fixturePath), 'utf8'));
  const expected = fixture.expected.canonical_envelope_json;
  const request = canonicalJson(JSON.parse(expected).request);
  const result = runPython(target, ['-I', '-B', script], request);
  assertSuccess(result, `${script} exact golden request`);
  assert.equal(result.stdout, `${expected}\n`, `${script} output bytes drifted`);
  assert.equal(result.stderr, '', `${script} success stderr must be empty`);
}

export function assertNoRuntimeOrAuthorityInstall(target) {
  const forbidden = [
    ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
    ['.codex', 'skills', 'knowledge-graph-curation'],
    ...['graph', 'graph-store', 'database', 'db', 'state', 'authority',
      'knowledge-graph-curation', 'runtime', 'persistence', 'impact', 'cost',
      'risk'].map((name) => ['.forge', name]),
  ];
  for (const parts of forbidden) {
    assert.equal(existsSync(join(target, ...parts)), false,
      `source-only graph scaffold must not install ${parts.join('/')}`);
  }
  const registry = readFileSync(
    join(target, '.agent', 'engineering', 'governance-contracts.yml'), 'utf8');
  assert.match(registry, /^version: 39$/m);
  assert.match(registry, /^knowledge_graph_curation_portable_projection:$/m);
}

export function assertKnowledgeGraphCurationScaffold(target) {
  assertClosedOwnership(target);
  assertPinnedBytes(target);
  assertPackageAndGovernance(target);
  assertGoldenProjector(target, MODULE_GOLDEN,
    'skills/knowledge-graph-curation/scripts/project_module_package_snapshot.py');
  assertGoldenProjector(target, TEST_GOLDEN,
    'skills/knowledge-graph-curation/scripts/project_go_test_source_snapshot.py');
  assertNoRuntimeOrAuthorityInstall(target);
}
