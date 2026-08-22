import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { GRAPH_SNAPSHOT_EXPECTED_FILES } from './graph-snapshot-copy-fragment.mjs';


export const GRAPH_SNAPSHOT_LEGACY_FILES = GRAPH_SNAPSHOT_EXPECTED_FILES;
export const GRAPH_SNAPSHOT_REGISTRY_VERSION = 39;

export function assertGraphSnapshotRegistryVersion(target) {
  const registry = readFileSync(
    join(target, '.agent', 'engineering', 'governance-contracts.yml'), 'utf8');
  assert.match(registry,
    new RegExp(`^version: ${GRAPH_SNAPSHOT_REGISTRY_VERSION}$`, 'm'));
  return registry;
}

function assertGraphSnapshotFilesAndPolicy(target) {
  for (const relative of GRAPH_SNAPSHOT_LEGACY_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `GraphSnapshot scaffold asset missing: ${relative}`);
  }
  const registry = assertGraphSnapshotRegistryVersion(target);
  assert.match(registry, /^graph_snapshot:$/m);
  assert.match(registry, /^graph_snapshot_test_source:$/m);
  assert.match(registry,
    /shipped_projectors: \[graph_snapshot, graph_snapshot_test_source\]/);
  assert.match(registry, /satisfies_g3_or_assessment_join: false/);
  const skill = readFileSync(
    join(target, '.agent', 'skills', 'knowledge-graph-curation.md'), 'utf8');
  assert.match(skill, /PARTIAL/);
  assert.match(skill, /system knowledge.*UNKNOWN/i);
  assert.match(skill, /ADR-0066/);
  assert.match(skill, /test-source/);
  assert.match(skill, /Assessment Join/);
  const legacyGolden = readFileSync(join(
    target, 'docs', 'contracts', 'fixtures', 'graph-snapshot-v1.json'));
  assert.equal(createHash('sha256').update(legacyGolden).digest('hex'),
    '8ce8418e840c97ef28ed77dfd5112c4c4b7d7ae8d843b714674e102d6322b03e',
    'ADR-0065 golden bytes must remain unchanged');
}

function assertGraphSnapshotGoldens(target) {
  const golden = spawnSync(
    'python3', ['-B', 'harness/graph_snapshot_contract_check.py', '--golden', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(golden.status, 0,
    `GraphSnapshot golden must validate\n${golden.stdout}\n${golden.stderr}`);
  assert.match(golden.stdout, /VALID_AUTHORITY_FREE_GRAPH_SNAPSHOT_V1/);
  const testSourceGolden = spawnSync(
    'python3', [
      '-B', 'harness/graph_snapshot_contract_check.py',
      '--test-source-golden', '.',
    ],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(testSourceGolden.status, 0,
    `test-source GraphSnapshot golden must validate\n${testSourceGolden.stdout}\n${testSourceGolden.stderr}`);
  assert.match(testSourceGolden.stdout,
    /VALID_AUTHORITY_FREE_GO_TEST_SOURCE_GRAPH_SNAPSHOT_V1/);
}

function assertTestSourceFixture(target) {
  const testSourceFixture = JSON.parse(readFileSync(join(
    target, 'docs', 'contracts', 'fixtures',
    'graph-snapshot-go-test-source-v1.json'), 'utf8'));
  const testSourceEnvelope = JSON.parse(
    testSourceFixture.expected.canonical_envelope_json);
  const snapshot = testSourceEnvelope.snapshot;
  assert.deepEqual([
    snapshot.nodes.length, snapshot.edges.length,
    snapshot.unresolved_nodes.length, snapshot.unresolved_edges.length,
    snapshot.adr_0062_node_crosswalk.length,
  ], [11, 14, 3, 11, 8]);
  const partial = Object.fromEntries(snapshot.coverage.surfaces
    .filter(({ status }) => status === 'partial')
    .map(({ surface, node_count, edge_count }) => [
      surface, [node_count, edge_count],
    ]));
  assert.deepEqual(partial, {
    go_module_package_lexical: [9, 10],
    test_verification: [2, 4],
  });
}

export function assertGraphSnapshotScaffold(target) {
  assertGraphSnapshotFilesAndPolicy(target);
  assertGraphSnapshotGoldens(target);
  assertTestSourceFixture(target);
  const governance = spawnSync(
    'python3', ['-B', 'harness/governance_engineering/test_graph_snapshot.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(governance.status, 0,
    `GraphSnapshot governance tests must pass\n${governance.stdout}\n${governance.stderr}`);
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'GraphSnapshot scaffold must not install Catalyst-only Go runtime or CLI');
}
