import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  mkdirSync, mkdtempSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  assertKnowledgeGraphCurationScaffold,
  assertNoRuntimeOrAuthorityInstall,
} from './knowledge-graph-curation-upgrade-verification.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

test('fresh source-only scaffold passes the Knowledge Graph Curation verifier', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'kg-curation-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', 'kg-curation-focused',
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.doesNotThrow(() => assertKnowledgeGraphCurationScaffold(target));
});

test('source-only verifier rejects persistence, impact, cost and risk state', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'kg-curation-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const registryDir = join(target, '.agent', 'engineering');
  mkdirSync(registryDir, { recursive: true });
  writeFileSync(join(registryDir, 'governance-contracts.yml'),
    'version: 39\nknowledge_graph_curation_portable_projection:\n');

  assert.doesNotThrow(() => assertNoRuntimeOrAuthorityInstall(target));
  for (const name of ['persistence', 'impact', 'cost', 'risk']) {
    const forbidden = join(target, '.forge', name);
    mkdirSync(forbidden, { recursive: true });
    assert.throws(
      () => assertNoRuntimeOrAuthorityInstall(target),
      new RegExp(`must not install \\.forge/${name}`),
    );
    rmSync(forbidden, { recursive: true, force: true });
  }
});
