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
  assertChangeImpactCostRiskScaffold,
  assertNoChangeImpactCostRiskInstall,
} from './change-impact-cost-risk-upgrade-verification.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

test('fresh source-only scaffold passes the lexical prescan verifier', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'change-impact-risk-fresh-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [
    'harness/scaffold/forge-init.mjs', target, '--name', 'impact-prescan-focused',
  ], { cwd: SOURCE_ROOT, encoding: 'utf8' });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.doesNotThrow(() => assertChangeImpactCostRiskScaffold(target));
});

test('source-only verifier rejects full impact, cost, risk and authority state', (t) => {
  const target = mkdtempSync(join(tmpdir(), 'change-impact-risk-negative-'));
  t.after(() => rmSync(target, { recursive: true, force: true }));
  const registryDir = join(target, '.agent', 'engineering');
  mkdirSync(registryDir, { recursive: true });
  writeFileSync(join(registryDir, 'governance-contracts.yml'),
    'version: 39\nchange_impact_cost_risk_portable_projection:\n');

  assert.doesNotThrow(() => assertNoChangeImpactCostRiskInstall(target));
  for (const name of [
    'impact', 'impact-analysis', 'full-impact', 'change-impact',
    'change-impact-cost-risk', 'cost', 'cost-model', 'risk', 'risk-model',
    'materiality', 'runtime', 'state', 'authority', 'persistence', 'database', 'db',
  ]) {
    const forbidden = join(target, '.forge', name);
    mkdirSync(forbidden, { recursive: true });
    assert.throws(
      () => assertNoChangeImpactCostRiskInstall(target),
      new RegExp(`must not install \\.forge/${name}`),
    );
    rmSync(forbidden, { recursive: true, force: true });
  }
});
