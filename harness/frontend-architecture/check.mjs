#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  BASELINE_REF, CONTRACT_REF, WAIVER_REF, loadArchitectureBundle,
} from './contract.mjs';
import { applyGovernance, evaluateTarget } from './graph.mjs';
import { analyzeTypescriptTarget } from './typescript-adapter.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = resolve(HERE, '..', '..');

export async function runFrontendArchitectureCheck(repoRoot = DEFAULT_ROOT) {
  const root = resolve(repoRoot);
  const bundle = loadArchitectureBundle(root);
  const configDigest = digestFiles(root, [CONTRACT_REF, BASELINE_REF, WAIVER_REF]);
  if (bundle.issues.length > 0) return report('inconclusive', configDigest, [], bundle.issues, []);
  if (bundle.contract.targets.length === 0) return report('not_applicable', configDigest, [], [], []);
  const allFindings = [];
  const analyses = [];
  for (const target of bundle.contract.targets) {
    const analysis = await analyzeTarget(root, target);
    analyses.push({
      targetId: target.id,
      adapter: analysis.adapter,
      status: analysis.status,
      fileCount: analysis.files.length,
      toolchain: analysis.toolchain ?? null,
    });
    allFindings.push(...evaluateTarget(
      root, target, analysis, bundle.contract.budgetDefaults,
    ));
  }
  const governed = applyGovernance(
    allFindings, bundle.baseline, bundle.waivers, bundle.contract.nonWaivableRules,
  );
  const open = governed.filter((item) => item.disposition === 'open');
  const status = open.some((item) => item.severity === 'block')
    ? 'fail'
    : open.some((item) => item.severity === 'inconclusive') ? 'inconclusive' : 'pass';
  return report(status, configDigest, governed, [], analyses);
}

export function validateFrontendArchitectureAssets(repoRoot = DEFAULT_ROOT) {
  const root = resolve(repoRoot);
  const bundle = loadArchitectureBundle(root);
  return {
    status: bundle.issues.length === 0 ? 'valid' : 'invalid',
    issues: bundle.issues,
    configDigest: digestFiles(root, [CONTRACT_REF, BASELINE_REF, WAIVER_REF]),
  };
}

async function analyzeTarget(repoRoot, target) {
  if (target.adapter === 'typescript') return analyzeTypescriptTarget(repoRoot, target);
  return {
    adapter: target.adapter,
    targetId: target.id,
    status: 'inconclusive',
    files: [],
    diagnostics: [
      `target ${target.id}: ${target.adapter} adapter is planned but unavailable in v1; configured targets cannot claim PASS`,
    ],
  };
}

function report(status, configDigest, findings, contractIssues, analyses) {
  const counts = { block: 0, review: 0, inconclusive: 0, baselined: 0, waived: 0 };
  for (const item of findings) {
    counts[item.severity] = (counts[item.severity] ?? 0) + 1;
    if (item.disposition === 'baselined') counts.baselined += 1;
    if (item.disposition === 'waived') counts.waived += 1;
  }
  const body = {
    schema: 'forgeos.frontend-architecture-report/v1',
    detector: { id: 'frontend.code_architecture', version: '1.0.0', state: 'shadow' },
    status, configDigest, counts, contractIssues, analyses, findings,
    completionAuthority: 'forge_accept',
  };
  const canonical = JSON.stringify(body);
  return { ...body, reportDigest: `sha256:${createHash('sha256').update(canonical).digest('hex')}` };
}

function digestFiles(root, refs) {
  const hash = createHash('sha256');
  for (const ref of refs) {
    hash.update(ref);
    hash.update('\0');
    try { hash.update(readFileSync(resolve(root, ref))); } catch { hash.update('<missing>'); }
    hash.update('\0');
  }
  return `sha256:${hash.digest('hex')}`;
}

function printHuman(result) {
  const upper = result.status.toUpperCase();
  console.log(`frontend-architecture-check: ${upper} (shadow; no completion authority)`);
  console.log(`  block=${result.counts.block} review=${result.counts.review} inconclusive=${result.counts.inconclusive} baselined=${result.counts.baselined} waived=${result.counts.waived}`);
  for (const issue of result.contractIssues) console.log(`  [CONFIG] ${issue}`);
  for (const item of result.findings) {
    const location = [item.source, item.target].filter(Boolean).join(' -> ');
    console.log(`  [${item.severity.toUpperCase()}] ${item.ruleId}${location ? ` ${location}` : ''}: ${item.message} (${item.disposition})`);
  }
}

export async function main(argv = process.argv.slice(2)) {
  const json = argv.includes('--json');
  const contractOnly = argv.includes('--contract-only');
  const positional = argv.filter((item) => !['--json', '--contract-only'].includes(item));
  if (contractOnly) {
    const result = validateFrontendArchitectureAssets(positional[0] ?? process.cwd());
    if (json) console.log(JSON.stringify(result, null, 2));
    else console.log(`frontend-architecture-contract: ${result.status.toUpperCase()}${result.issues.length ? ` — ${result.issues.join('; ')}` : ''}`);
    return result.status === 'valid' ? 0 : 2;
  }
  const result = await runFrontendArchitectureCheck(positional[0] ?? process.cwd());
  if (json) console.log(JSON.stringify(result, null, 2));
  else printHuman(result);
  if (result.status === 'fail') return 1;
  if (result.status === 'inconclusive') return 2;
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(await main());
}
