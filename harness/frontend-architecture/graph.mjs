import { createHash } from 'node:crypto';
import { readdirSync, realpathSync, statSync } from 'node:fs';
import { posix, resolve, sep } from 'node:path';

export function evaluateTarget(repoRoot, target, analysis, defaultBudgets) {
  const findings = [];
  const expansion = expandModules(repoRoot, target);
  findings.push(...expansion.findings);
  if (analysis.status === 'inconclusive') {
    for (const message of analysis.diagnostics) findings.push(finding(
      'FEARCH-ANALYSIS-001', 'inconclusive', target.id, '', '', message, {},
    ));
  }
  const ownership = classifyOwnership(target, analysis.files, expansion.modules);
  findings.push(...ownership.findings);
  const budgets = { ...defaultBudgets, ...(target.budgets ?? {}) };
  if (!findings.some((item) => item.severity === 'inconclusive')) {
    findings.push(...checkEdges(target, ownership.files));
    findings.push(...checkCycles(target, ownership.files));
    findings.push(...checkBudgets(target, ownership.files, expansion.modules, budgets));
    findings.push(...checkGodSignals(target, ownership.files, budgets));
  }
  return findings.map(withFingerprint).sort(findingOrder);
}

function expandModules(repoRoot, target) {
  const findings = [];
  const targetRoot = resolve(repoRoot, target.root);
  const actualTarget = inspectTargetRoot(repoRoot, target, targetRoot, findings);
  const modules = (target.modules ?? []).map((item) => normalizedModule(item));
  expandModuleSets(target, targetRoot, modules, findings);
  validateModules(target, targetRoot, actualTarget, modules, findings);
  findings.push(...moduleOverlapFindings(target, modules));
  return { modules, findings };
}

function inspectTargetRoot(repoRoot, target, targetRoot, findings) {
  try {
    if (!statSync(targetRoot).isDirectory()) throw new Error('not a directory');
    const actualRepo = realpathSync(repoRoot);
    const actualTarget = realpathSync(targetRoot);
    if (!inside(actualTarget, actualRepo)) findings.push(finding(
      'FEARCH-CONFIG-001', 'inconclusive', target.id, target.root, '',
      'target root realpath escapes repository root', {},
    ));
    return actualTarget;
  } catch (error) {
    findings.push(finding(
      'FEARCH-CONFIG-001', 'inconclusive', target.id, target.root, '',
      `target root unavailable (${error.code ?? error.message})`, {},
    ));
    return null;
  }
}

function expandModuleSets(target, targetRoot, modules, findings) {
  for (const set of target.moduleSets ?? []) {
    const base = resolve(targetRoot, set.root);
    let children = [];
    try {
      children = readdirSync(base, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
        .sort();
    } catch (error) {
      findings.push(finding(
        'FEARCH-CONFIG-001', 'inconclusive', target.id, set.root, '',
        `cannot enumerate moduleSet ${set.idPrefix} (${error.code ?? error.message})`, {},
      ));
    }
    for (const child of children) modules.push(normalizedModule({
      id: `${set.idPrefix}.${child}`,
      root: posix.join(set.root.replaceAll('\\', '/'), child),
      layer: set.layer,
      publicEntrypoints: set.publicEntrypoints,
      testEntrypoints: set.testEntrypoints,
    }));
  }
}

function validateModules(target, targetRoot, actualTarget, modules, findings) {
  const ids = new Set();
  for (const module of modules) {
    if (ids.has(module.id)) findings.push(finding('FEARCH-CONFIG-001', 'inconclusive', target.id, module.root, '', `duplicate module id ${module.id}`, {}));
    ids.add(module.id);
    const root = resolve(targetRoot, module.root);
    try {
      if (!statSync(root).isDirectory()) throw new Error('not a directory');
      const actualRoot = realpathSync(root);
      if (!actualTarget || !inside(actualRoot, actualTarget)) findings.push(finding('FEARCH-CONFIG-001', 'inconclusive', target.id, module.root, '', 'module realpath escapes target root', {}));
    } catch (error) {
      findings.push(finding('FEARCH-CONFIG-001', 'inconclusive', target.id, module.root, '', `module root unavailable (${error.code ?? error.message})`, {}));
    }
  }
}

function moduleOverlapFindings(target, modules) {
  const findings = [];
  for (let i = 0; i < modules.length; i += 1) {
    for (let j = i + 1; j < modules.length; j += 1) {
      if (pathContains(modules[i].root, modules[j].root) || pathContains(modules[j].root, modules[i].root)) {
        findings.push(finding('FEARCH-CONFIG-001', 'inconclusive', target.id, modules[i].root, modules[j].root, 'module roots overlap', {}));
      }
    }
  }
  return findings;
}

function normalizedModule(module) {
  return {
    ...module,
    root: clean(module.root),
    publicEntrypoints: (module.publicEntrypoints ?? []).map(clean),
    testEntrypoints: (module.testEntrypoints ?? []).map(clean),
  };
}

function classifyOwnership(target, files, modules) {
  const findings = [];
  const classified = [];
  for (const file of files) {
    const owners = modules.filter((module) => file.path === module.root || file.path.startsWith(`${module.root}/`));
    if (owners.length > 1) {
      findings.push(finding('FEARCH-OWNERSHIP-001', 'block', target.id, file.path, '', 'file belongs to multiple modules', { modules: owners.map((item) => item.id) }));
      continue;
    }
    if (owners.length === 0) {
      if (target.ownership === 'complete') findings.push(finding('FEARCH-OWNERSHIP-001', 'block', target.id, file.path, '', 'file has no declared module owner', {}));
      classified.push({ ...file, module: null });
      continue;
    }
    classified.push({ ...file, module: owners[0] });
  }
  if (target.ownership === 'partial') {
    const byPath = new Map(classified.map((file) => [file.path, file]));
    for (const source of classified) {
      for (const edge of source.edges) {
        const destination = byPath.get(edge.targetPath);
        if (!destination || Boolean(source.module) === Boolean(destination.module)) continue;
        findings.push(finding(
          'FEARCH-OWNERSHIP-001', 'inconclusive', target.id, source.path, destination.path,
          'dependency crosses the declared partial-ownership boundary',
          {
            sourceModule: source.module?.id ?? null,
            targetModule: destination.module?.id ?? null,
            specifier: edge.specifier,
          },
        ));
      }
    }
  }
  return { files: classified, findings };
}

function checkEdges(target, files) {
  const findings = [];
  const byPath = new Map(files.map((file) => [file.path, file]));
  for (const source of files) {
    if (!source.module) continue;
    for (const edge of source.edges) {
      const destination = byPath.get(edge.targetPath);
      if (!destination?.module || destination.module.id === source.module.id) continue;
      const allowed = new Set(target.allowedDependencies[source.module.layer] ?? []);
      if (!allowed.has(destination.module.layer)) {
        findings.push(finding(
          'FEARCH-DIRECTION-001', 'block', target.id, source.path, destination.path,
          `forbidden ${source.module.layer} -> ${destination.module.layer}`,
          { sourceModule: source.module.id, targetModule: destination.module.id, edgeKind: edge.kind, specifier: edge.specifier },
        ));
      }
      const local = destination.path.slice(destination.module.root.length + 1);
      const publicEntry = destination.module.publicEntrypoints.includes(local);
      const testEntry = source.isTest && destination.module.testEntrypoints.includes(local);
      if (!publicEntry && !testEntry) {
        findings.push(finding(
          'FEARCH-PUBLIC-001', 'block', target.id, source.path, destination.path,
          'cross-module import bypasses a declared public entrypoint',
          { sourceModule: source.module.id, targetModule: destination.module.id, edgeKind: edge.kind, specifier: edge.specifier },
        ));
      }
      if (!source.isTest && destination.module.testEntrypoints.includes(local)) {
        findings.push(finding(
          'FEARCH-PUBLIC-001', 'block', target.id, source.path, destination.path,
          'production source imports a test-only entrypoint',
          { sourceModule: source.module.id, targetModule: destination.module.id, edgeKind: edge.kind },
        ));
      }
    }
  }
  return findings;
}

function checkCycles(target, files) {
  const graph = new Map();
  const byPath = new Map(files.map((file) => [file.path, file]));
  for (const file of files) if (file.module) graph.set(file.module.id, graph.get(file.module.id) ?? new Set());
  for (const file of files) {
    if (!file.module) continue;
    for (const edge of file.edges) {
      const destination = byPath.get(edge.targetPath);
      if (destination?.module && destination.module.id !== file.module.id) graph.get(file.module.id).add(destination.module.id);
    }
  }
  return stronglyConnected(graph)
    .filter((component) => component.length > 1)
    .map((component) => finding(
      'FEARCH-CYCLE-001', 'block', target.id, component[0], component[component.length - 1],
      `cross-module dependency cycle: ${component.join(' -> ')} -> ${component[0]}`,
      { modules: component },
    ));
}

function checkBudgets(target, files, modules, budgets) {
  const findings = [];
  const severity = budgets.enforcement === 'hard' ? 'block' : 'review';
  const byModule = new Map(modules.map((module) => [module.id, []]));
  for (const file of files) if (file.module) byModule.get(file.module.id)?.push(file);
  for (const module of modules) {
    const owned = byModule.get(module.id) ?? [];
    if (owned.length > budgets.filesPerModule) findings.push(budgetFinding(target, module, severity, 'filesPerModule', owned.length, budgets.filesPerModule));
    const publicSymbols = owned
      .filter((file) => module.publicEntrypoints.includes(file.path.slice(module.root.length + 1)))
      .reduce((sum, file) => sum + file.metrics.exports, 0);
    if (publicSymbols > budgets.publicSymbols) findings.push(budgetFinding(target, module, severity, 'publicSymbols', publicSymbols, budgets.publicSymbols));
    const dependencies = new Set();
    const byPath = new Map(files.map((file) => [file.path, file]));
    for (const file of owned) for (const edge of file.edges) {
      const other = byPath.get(edge.targetPath)?.module;
      if (other && other.id !== module.id) dependencies.add(other.id);
    }
    if (dependencies.size > budgets.directModuleDependencies) findings.push(budgetFinding(target, module, severity, 'directModuleDependencies', dependencies.size, budgets.directModuleDependencies));
    for (const file of owned) {
      const local = file.path.slice(module.root.length + 1);
      const depth = local.split('/').length;
      if (depth > budgets.directoryDepth) findings.push(finding('FEARCH-BUDGET-001', severity, target.id, file.path, file.path, 'directory depth exceeds review budget', { metric: 'directoryDepth', actual: depth, limit: budgets.directoryDepth, module: module.id }));
    }
  }
  return findings;
}

function budgetFinding(target, module, severity, metric, actual, limit) {
  return finding('FEARCH-BUDGET-001', severity, target.id, module.root, module.root, `${metric} exceeds configured budget`, { metric, actual, limit, module: module.id });
}

function checkGodSignals(target, files, budgets) {
  const findings = [];
  for (const file of files) {
    const m = file.metrics;
    const families = [];
    if (m.physicalLines > budgets.physicalLines) families.push({ family: 'size', metric: 'physicalLines', actual: m.physicalLines, limit: budgets.physicalLines });
    if (m.topLevelDeclarations > budgets.topLevelDeclarations || m.imports > budgets.imports || m.exports > budgets.exports) families.push({ family: 'surface', metrics: { topLevelDeclarations: m.topLevelDeclarations, imports: m.imports, exports: m.exports } });
    if (m.stateCalls > budgets.stateCalls) families.push({ family: 'state', metric: 'stateCalls', actual: m.stateCalls, limit: budgets.stateCalls });
    if (m.effectCalls > budgets.effectCalls) families.push({ family: 'effects', metric: 'effectCalls', actual: m.effectCalls, limit: budgets.effectCalls });
    if (m.eventHandlers > budgets.eventHandlers) families.push({ family: 'handlers', metric: 'eventHandlers', actual: m.eventHandlers, limit: budgets.eventHandlers });
    if (m.branchPoints > budgets.branchPoints) families.push({ family: 'branching', metric: 'branchPoints', actual: m.branchPoints, limit: budgets.branchPoints });
    if (families.length >= budgets.godSignalFamilies) findings.push(finding(
      'FEARCH-GOD-001', 'review', target.id, file.path, '',
      'multiple static signal families require semantic responsibility review',
      { signalFamilies: families, threshold: budgets.godSignalFamilies },
    ));
  }
  return findings;
}

export function applyGovernance(findings, baseline, waivers, nonWaivableRules = []) {
  const baselineMap = new Map((baseline.findings ?? []).map((item) => [item.findingFingerprint, item.ruleId]));
  const nonWaivable = new Set(nonWaivableRules);
  const waiverMap = new Map();
  for (const item of waivers.waivers ?? []) {
    const entries = waiverMap.get(item.findingFingerprint) ?? [];
    entries.push(item);
    waiverMap.set(item.findingFingerprint, entries);
  }
  const occurrences = new Map();
  return findings.map((item) => {
    if (!nonWaivable.has(item.ruleId) && baselineMap.get(item.fingerprint) === item.ruleId) {
      return { ...item, disposition: 'baselined' };
    }
    const matches = waiverMap.get(item.fingerprint) ?? [];
    for (const waiver of matches) {
      const used = occurrences.get(waiver.id) ?? 0;
      if (used < waiver.maxOccurrences && waiver.ruleId === item.ruleId
          && waiver.source === item.source && waiver.target === item.target) {
        occurrences.set(waiver.id, used + 1);
        return { ...item, disposition: 'waived', waiverId: waiver.id };
      }
    }
    return { ...item, disposition: 'open' };
  });
}

function finding(ruleId, severity, targetId, source, target, message, evidence) {
  return { ruleId, severity, targetId, source, target, message, evidence };
}

function withFingerprint(item) {
  const stable = [item.ruleId, item.targetId, item.source, item.target, item.message, JSON.stringify(item.evidence)];
  const digest = createHash('sha256').update(stable.join('\0')).digest('hex');
  return { ...item, fingerprint: `sha256:${digest}` };
}

function findingOrder(a, b) {
  return [a.ruleId, a.targetId, a.source, a.target, a.message].join('\0')
    .localeCompare([b.ruleId, b.targetId, b.source, b.target, b.message].join('\0'));
}

function stronglyConnected(graph) {
  let index = 0;
  const stack = [];
  const meta = new Map();
  const components = [];
  for (const node of [...graph.keys()].sort()) if (!meta.has(node)) visit(node);
  return components.map((component) => component.sort()).sort((a, b) => a[0].localeCompare(b[0]));

  function visit(node) {
    const record = { index, low: index, onStack: true };
    index += 1;
    meta.set(node, record);
    stack.push(node);
    for (const next of [...(graph.get(node) ?? [])].sort()) {
      if (!meta.has(next)) { visit(next); record.low = Math.min(record.low, meta.get(next).low); }
      else if (meta.get(next).onStack) record.low = Math.min(record.low, meta.get(next).index);
    }
    if (record.low !== record.index) return;
    const component = [];
    let current;
    do {
      current = stack.pop();
      meta.get(current).onStack = false;
      component.push(current);
    } while (current !== node);
    components.push(component);
  }
}

function pathContains(parent, child) {
  return parent === child || child.startsWith(`${parent}/`);
}

function inside(path, root) {
  const full = resolve(path);
  const base = resolve(root);
  return full === base || full.startsWith(base + sep);
}

function clean(path) {
  return posix.normalize(path.replaceAll('\\', '/')).replace(/^\.\//, '').replace(/\/$/, '');
}
