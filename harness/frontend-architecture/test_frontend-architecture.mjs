import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';

import { runFrontendArchitectureCheck } from './check.mjs';
import { validateBaseline, validateContract, validateWaivers } from './contract.mjs';
import { applyGovernance, evaluateTarget } from './graph.mjs';

const DEFAULT_BUDGETS = {
  enforcement: 'review', filesPerModule: 20, publicSymbols: 8,
  directModuleDependencies: 10, directoryDepth: 5, godSignalFamilies: 3,
  physicalLines: 300, topLevelDeclarations: 20, imports: 20, exports: 10,
  stateCalls: 12, effectCalls: 8, eventHandlers: 8, branchPoints: 15,
};

function tempRepo(t) {
  const root = mkdtempSync(join(tmpdir(), 'forge-frontend-architecture-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, '.arch'), { recursive: true });
  return root;
}

function target(overrides = {}) {
  return {
    id: 'web', root: 'src', adapter: 'typescript', project: 'tsconfig.json',
    ownership: 'complete',
    modules: [
      { id: 'page.orders', root: 'pages/orders', layer: 'page', publicEntrypoints: ['index.ts'], testEntrypoints: ['testing.ts'] },
      { id: 'feature.orders', root: 'features/orders', layer: 'feature', publicEntrypoints: ['index.ts'], testEntrypoints: ['testing.ts'] },
    ],
    moduleSets: [],
    allowedDependencies: { page: ['feature'], feature: [] },
    budgets: {},
    ...overrides,
  };
}

function contract(targets = []) {
  return {
    schema: 'forgeos.frontend-architecture/v1', mode: 'shadow', targets,
    adapterSupport: {
      typescript: 'available_when_project_typescript_resolves',
      vue: 'planned_inconclusive_when_configured', dart: 'planned_inconclusive_when_configured',
    },
    budgetDefaults: DEFAULT_BUDGETS,
    nonWaivableRules: ['FEARCH-CONFIG-001', 'FEARCH-OWNERSHIP-001', 'FEARCH-DIRECTION-001', 'FEARCH-CYCLE-001'],
    waivableRules: ['FEARCH-PUBLIC-001', 'FEARCH-BUDGET-001'],
  };
}

function writeBundle(root, value = contract()) {
  writeFileSync(join(root, '.arch', 'frontend-architecture.v1.json'), JSON.stringify(value));
  writeFileSync(join(root, '.arch', 'frontend-architecture-baseline.v1.json'), JSON.stringify({
    schema: 'forgeos.frontend-architecture-baseline/v1', findings: [],
  }));
  writeFileSync(join(root, '.arch', 'frontend-architecture-waivers.v1.json'), JSON.stringify({
    schema: 'forgeos.frontend-architecture-waivers/v1', waivers: [],
  }));
}

function createModuleDirs(root) {
  mkdirSync(join(root, 'src', 'pages', 'orders'), { recursive: true });
  mkdirSync(join(root, 'src', 'features', 'orders'), { recursive: true });
}

function exposeProjectTypeScript(root) {
  const require = createRequire(import.meta.url);
  const packagePath = require.resolve('typescript/package.json');
  const version = require('typescript').version;
  mkdirSync(join(root, 'node_modules'), { recursive: true });
  symlinkSync(dirname(packagePath), join(root, 'node_modules', 'typescript'), 'dir');
  writeFileSync(join(root, 'package.json'), JSON.stringify({ devDependencies: { typescript: version } }));
}

function file(path, metrics = {}) {
  return {
    path, absolutePath: `/repo/src/${path}`, isTest: false,
    edges: [],
    metrics: {
      physicalLines: 10, topLevelDeclarations: 2, imports: 0, exports: 1,
      stateCalls: 0, effectCalls: 0, eventHandlers: 0, branchPoints: 0,
      ...metrics,
    },
  };
}

test('no configured target is reported NOT_APPLICABLE, never a fake PASS', async (t) => {
  const root = tempRepo(t);
  writeBundle(root);
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'not_applicable');
  assert.equal(result.detector.state, 'shadow');
  assert.equal(result.completionAuthority, 'forge_accept');
});

test('malformed contract is INCONCLUSIVE and fails closed', async (t) => {
  const root = tempRepo(t);
  const malformed = contract();
  malformed.mode = 'enforce';
  writeBundle(root, malformed);
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'inconclusive');
  assert.match(result.contractIssues.join('\n'), /mode must remain shadow/);
});

test('contract rejects unknown fields and overlapping rule authority', () => {
  const value = contract();
  value.magic = true;
  value.waivableRules.push('FEARCH-CYCLE-001');
  const issues = validateContract(value);
  assert.ok(issues.some((item) => item.includes('unknown field magic')));
  assert.ok(issues.some((item) => item.includes('both waivable and non-waivable')));
});

test('project contract cannot downgrade canonical non-waivable rules', () => {
  const value = contract();
  value.nonWaivableRules = value.nonWaivableRules.filter((rule) => rule !== 'FEARCH-DIRECTION-001');
  value.waivableRules.push('FEARCH-DIRECTION-001');
  const issues = validateContract(value);
  assert.ok(issues.some((item) => item.includes('required non-waivable rule FEARCH-DIRECTION-001 is missing')));
  assert.ok(issues.some((item) => item.includes('FEARCH-DIRECTION-001 is not allowed to be waivable')));
});

test('contract requires owned modules and a complete layer matrix', () => {
  const empty = target({ modules: [], moduleSets: [], allowedDependencies: { page: [] } });
  const missingLayer = target({ allowedDependencies: { page: [] } });
  assert.ok(validateContract(contract([empty])).some((item) => item.includes('at least one module or moduleSet')));
  assert.ok(validateContract(contract([missingLayer])).some((item) => item.includes('layer feature is missing')));
});

test('non-waivable rules cannot be hidden by a baseline', () => {
  const blocker = {
    ruleId: 'FEARCH-DIRECTION-001', severity: 'block', targetId: 'web',
    source: 'features/orders/index.ts', target: 'pages/orders/index.ts',
    message: 'forbidden feature -> page', evidence: {},
    fingerprint: `sha256:${'a'.repeat(64)}`,
  };
  const baseline = { findings: [{ findingFingerprint: blocker.fingerprint, ruleId: blocker.ruleId }] };
  const governed = applyGovernance([blocker], baseline, { waivers: [] }, contract().nonWaivableRules);
  assert.equal(governed[0].disposition, 'open');
  const baselineIssues = validateBaseline({
    schema: 'forgeos.frontend-architecture-baseline/v1',
    findings: [{
      findingFingerprint: blocker.fingerprint, ruleId: blocker.ruleId,
      owner: 'orders', debtLink: 'ARCH-2', firstSeenRevision: 'abc', removalTrigger: 'boundary fixed',
    }],
  }, contract());
  assert.ok(baselineIssues.some((item) => item.includes('cannot be baselined')));
});

test('target and compiler project realpaths cannot escape the repository', async (t) => {
  const root = tempRepo(t);
  const external = mkdtempSync(join(tmpdir(), 'forge-frontend-architecture-external-'));
  t.after(() => rmSync(external, { recursive: true, force: true }));
  mkdirSync(join(external, 'pages', 'orders'), { recursive: true });
  mkdirSync(join(external, 'features', 'orders'), { recursive: true });
  writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({ include: ['src/**/*.ts'] }));
  symlinkSync(external, join(root, 'src'));
  writeBundle(root, contract([target()]));
  const targetResult = await runFrontendArchitectureCheck(root);
  assert.equal(targetResult.status, 'inconclusive');
  assert.match(
    [...targetResult.contractIssues, ...targetResult.findings.map((item) => item.message)].join('\n'),
    /realpath escapes repository root/,
  );

  rmSync(join(root, 'src'));
  createModuleDirs(root);
  const outsideProject = join(external, 'tsconfig.json');
  writeFileSync(outsideProject, JSON.stringify({ include: [] }));
  rmSync(join(root, 'tsconfig.json'));
  symlinkSync(outsideProject, join(root, 'tsconfig.json'));
  const projectResult = await runFrontendArchitectureCheck(root);
  assert.equal(projectResult.status, 'inconclusive');
  assert.match(projectResult.findings.map((item) => item.message).join('\n'), /realpath escapes target root/);
});

test('graph allows declared direction through a public entrypoint', (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  const analysis = { status: 'analyzed', diagnostics: [], files: [
    { ...file('pages/orders/index.ts'), edges: [{ specifier: '@feature/orders', targetPath: 'features/orders/index.ts', kind: 'value', importedSymbols: ['Order'] }] },
    file('features/orders/index.ts'),
  ] };
  const findings = evaluateTarget(root, target(), analysis, DEFAULT_BUDGETS);
  assert.deepEqual(findings.filter((item) => item.severity !== 'review'), []);
});

test('partial ownership becomes INCONCLUSIVE when an edge crosses its coverage boundary', (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  mkdirSync(join(root, 'src', 'legacy'), { recursive: true });
  const analysis = { status: 'analyzed', diagnostics: [], files: [
    { ...file('features/orders/index.ts'), edges: [{ specifier: '../../legacy/order', targetPath: 'legacy/order.ts', kind: 'value', importedSymbols: [] }] },
    { ...file('legacy/order.ts'), edges: [{ specifier: '../features/orders', targetPath: 'features/orders/index.ts', kind: 'value', importedSymbols: [] }] },
  ] };
  const findings = evaluateTarget(root, target({ ownership: 'partial' }), analysis, DEFAULT_BUDGETS);
  assert.ok(findings.some((item) => item.ruleId === 'FEARCH-OWNERSHIP-001' && item.severity === 'inconclusive'));
});

test('cross-module deep import and reverse edge are deterministic blockers', (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  const analysis = { status: 'analyzed', diagnostics: [], files: [
    file('pages/orders/index.ts'),
    { ...file('features/orders/internal.ts'), edges: [{ specifier: '@page/orders', targetPath: 'pages/orders/index.ts', kind: 'value', importedSymbols: [] }] },
  ] };
  analysis.files[0].edges = [{ specifier: '@feature/orders/internal', targetPath: 'features/orders/internal.ts', kind: 'value', importedSymbols: [] }];
  const findings = evaluateTarget(root, target(), analysis, DEFAULT_BUDGETS);
  assert.ok(findings.some((item) => item.ruleId === 'FEARCH-PUBLIC-001' && item.severity === 'block'));
  assert.ok(findings.some((item) => item.ruleId === 'FEARCH-DIRECTION-001' && item.severity === 'block'));
  assert.ok(findings.some((item) => item.ruleId === 'FEARCH-CYCLE-001' && item.severity === 'block'));
});

test('three static signal families create REVIEW only, not a compensating score', (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  const risky = file('features/orders/index.ts', {
    physicalLines: 301, stateCalls: 13, eventHandlers: 9,
  });
  const analysis = { status: 'analyzed', diagnostics: [], files: [risky] };
  const findings = evaluateTarget(root, target({ ownership: 'partial' }), analysis, DEFAULT_BUDGETS);
  const god = findings.find((item) => item.ruleId === 'FEARCH-GOD-001');
  assert.equal(god.severity, 'review');
  assert.deepEqual(god.evidence.signalFamilies.map((item) => item.family), ['size', 'state', 'handlers']);
});

test('exact waiver applies once; wildcard, self-approved, expired waiver is invalid', (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  const analysis = { status: 'analyzed', diagnostics: [], files: [
    { ...file('pages/orders/index.ts'), edges: [{ specifier: 'x', targetPath: 'features/orders/internal.ts', kind: 'value', importedSymbols: [] }] },
    file('features/orders/internal.ts'),
  ] };
  const findings = evaluateTarget(root, target(), analysis, DEFAULT_BUDGETS);
  const deep = findings.find((item) => item.ruleId === 'FEARCH-PUBLIC-001');
  const waiver = {
    id: 'w-1', ruleId: deep.ruleId, findingFingerprint: deep.fingerprint,
    source: deep.source, target: deep.target, reason: 'bounded migration',
    riskOwner: 'orders', approver: 'architecture', compensatingProofs: ['contract-test:orders'],
    debtLink: 'ARCH-1', createdAt: '2026-08-01', expiresAt: '2026-09-30',
    removalTrigger: 'adapter removed', maxOccurrences: 1,
  };
  const governed = applyGovernance(findings, { findings: [] }, { waivers: [waiver] });
  assert.equal(governed.find((item) => item.fingerprint === deep.fingerprint).disposition, 'waived');
  const invalid = { ...waiver, source: '**', riskOwner: 'architecture', expiresAt: '2026-08-01' };
  const issues = validateWaivers(
    { schema: 'forgeos.frontend-architecture-waivers/v1', waivers: [invalid] },
    contract(), new Date('2026-08-09T00:00:00Z'),
  );
  assert.ok(issues.some((item) => item.includes('wildcards are forbidden')));
  assert.ok(issues.some((item) => item.includes('self-approval is forbidden')));
  assert.ok(issues.some((item) => item.includes('waiver is expired')));
  const overlong = { ...waiver, createdAt: '2026-01-01', expiresAt: '2026-09-30' };
  const durationIssues = validateWaivers(
    { schema: 'forgeos.frontend-architecture-waivers/v1', waivers: [overlong] },
    contract(), new Date('2026-08-09T00:00:00Z'),
  );
  assert.ok(durationIssues.some((item) => item.includes('duration exceeds 90 days')));
});

const hasTypeScript = (() => {
  try { createRequire(import.meta.url).resolve('typescript'); return true; } catch { return false; }
})();

test('TypeScript compiler resolves alias/index and ignores import-like comments', { skip: !hasTypeScript }, async (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  exposeProjectTypeScript(root);
  writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({
    compilerOptions: {
      target: 'ES2022', module: 'ESNext', moduleResolution: 'Node',
      baseUrl: 'src', paths: { '@feature/*': ['features/*'] },
    },
    include: ['src/**/*.ts'],
  }));
  writeFileSync(join(root, 'src', 'features', 'orders', 'internal.ts'), 'export const Order = 1;\n');
  writeFileSync(join(root, 'src', 'features', 'orders', 'index.ts'), "export { Order } from './internal';\n");
  writeFileSync(join(root, 'src', 'pages', 'orders', 'index.ts'), [
    "// import { fake } from '@feature/orders/internal'",
    "const note = \"import('@feature/orders/internal')\";",
    "import { Order } from '@feature/orders';",
    'export const page = Order + note.length;',
  ].join('\n'));
  writeBundle(root, contract([target()]));
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'pass', JSON.stringify(result, null, 2));
  assert.equal(result.findings.filter((item) => item.ruleId === 'FEARCH-PUBLIC-001').length, 0);
  assert.equal(result.analyses[0].toolchain.version, createRequire(import.meta.url)('typescript').version);
  assert.match(result.analyses[0].toolchain.sha256, /^sha256:[a-f0-9]{64}$/);
});

test('TypeScript compiler catches alias deep import', { skip: !hasTypeScript }, async (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  exposeProjectTypeScript(root);
  writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({
    compilerOptions: {
      target: 'ES2022', module: 'ESNext', moduleResolution: 'Node',
      baseUrl: 'src', paths: { '@feature/*': ['features/*'] },
    },
    include: ['src/**/*.ts'],
  }));
  writeFileSync(join(root, 'src', 'features', 'orders', 'internal.ts'), 'export const Order = 1;\n');
  writeFileSync(join(root, 'src', 'features', 'orders', 'index.ts'), "export { Order } from './internal';\n");
  writeFileSync(join(root, 'src', 'pages', 'orders', 'index.ts'), "import { Order } from '@feature/orders/internal';\nexport const page = Order;\n");
  writeBundle(root, contract([target()]));
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'fail');
  assert.ok(result.findings.some((item) => item.ruleId === 'FEARCH-PUBLIC-001'));
});

test('configured TypeScript target is INCONCLUSIVE when source is omitted from tsconfig', { skip: !hasTypeScript }, async (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  exposeProjectTypeScript(root);
  writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({
    compilerOptions: { target: 'ES2022', module: 'ESNext' },
    files: ['src/pages/orders/index.ts'],
  }));
  writeFileSync(join(root, 'src', 'pages', 'orders', 'index.ts'), 'export const page = 1;\n');
  writeFileSync(join(root, 'src', 'features', 'orders', 'index.ts'), "import '../../pages/orders/index';\n");
  writeBundle(root, contract([target()]));
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'inconclusive');
  assert.match(result.findings.map((item) => item.message).join('\n'), /source file is omitted/);
});

test('configured TypeScript target cannot borrow an undeclared host compiler', async (t) => {
  const root = tempRepo(t);
  createModuleDirs(root);
  writeFileSync(join(root, 'package.json'), JSON.stringify({ devDependencies: {} }));
  writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({ include: ['src/**/*.ts'] }));
  writeFileSync(join(root, 'src', 'pages', 'orders', 'index.ts'), 'export const page = 1;\n');
  writeFileSync(join(root, 'src', 'features', 'orders', 'index.ts'), 'export const feature = 1;\n');
  writeBundle(root, contract([target()]));
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'inconclusive');
  assert.match(result.findings.map((item) => item.message).join('\n'), /directly declares typescript/);
});

test('configured Vue or Dart target is INCONCLUSIVE until its compiler adapter exists', async (t) => {
  const root = tempRepo(t);
  mkdirSync(join(root, 'src', 'pages', 'orders'), { recursive: true });
  const vueTarget = target({
    adapter: 'vue', modules: [
      { id: 'page.orders', root: 'pages/orders', layer: 'page', publicEntrypoints: ['index.vue'], testEntrypoints: ['testing.ts'] },
    ],
    allowedDependencies: { page: [] },
  });
  writeBundle(root, contract([vueTarget]));
  const result = await runFrontendArchitectureCheck(root);
  assert.equal(result.status, 'inconclusive');
  assert.match(result.findings.map((item) => item.message).join('\n'), /adapter is planned but unavailable/);
});
