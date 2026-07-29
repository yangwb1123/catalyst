#!/usr/bin/env node
// ForgeOS arch-check — runs .arch/rules.yaml against the repo and FAILS on any
// violation. This makes ForgeOS's clean-architecture rules EXECUTABLE: until now
// the clean-architecture skill only DECLARED dependency direction; nothing
// checked it. Checks: layering (dependency direction — the headline rule),
// package size + exports, fan-in (coupling), cognitive load (top-level modules),
// and a drift-guard that .arch limits equal harness/policies.yml.
//
// Zero third-party deps. Reuses the pure model from harness/arch/scan.mjs.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { scan, parseRules } from './scan.mjs';

const ARCH_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(dirname(ARCH_DIR)); // harness/arch -> harness -> repo root

function relOf(dir) {
  return dir.startsWith(ROOT) ? dir.slice(ROOT.length + 1) || '.' : dir;
}

// layering: a non-test file in layer F must NOT import an internal target in
// layer T when "F -> T" is forbidden (inner layers never import outer ones).
export function checkLayering(model, rules) {
  const forbidden = new Set(rules.architecture?.forbidden ?? []);
  const v = [];
  for (const f of model.files) {
    if (f.isTest || !f.layer) continue;
    for (const imp of f.imports) {
      if (imp.kind !== 'internal' || !imp.layer) continue;
      const edge = `${f.layer} -> ${imp.layer}`;
      if (forbidden.has(edge)) v.push(`${f.rel}: imports ${imp.rel} — forbidden ${edge}`);
    }
  }
  return v;
}

// package: per directory, total NON-TEST code files <= max_files and exported
// (Go) identifiers from non-test files <= max_exports. Tests are excluded from
// both budgets — a package's "god object" smell is about its production code,
// and tests legitimately scale with it (this mirrors the export treatment).
export function checkPackage(model, rules) {
  const max = rules.package ?? {};
  const byDir = new Map();
  for (const f of model.files) {
    const d = byDir.get(f.dir) ?? { count: 0, exports: 0 };
    if (!f.isTest) {
      d.count += 1;
      d.exports += f.exports;
    }
    byDir.set(f.dir, d);
  }
  const v = [];
  for (const [dir, d] of byDir) {
    if (d.count > max.max_files) v.push(`${relOf(dir)}: ${d.count} files (max ${max.max_files})`);
    if (d.exports > max.max_exports) v.push(`${relOf(dir)}: ${d.exports} exports (max ${max.max_exports})`);
  }
  return v;
}

// fanin: no internal target dir may be imported by more than max_importers
// PRODUCTION files. Coupling is a production concern — a test importing the data
// structure it exercises is not architectural coupling — so test files are
// EXCLUDED, matching checkLayering/checkPackage (which also skip tests). Counting
// test importers previously over-reported a healthy data-model package (e.g.
// internal/asset: 7 production importers but ~13 test files) as near-limit.
export function checkFanin(model, rules) {
  const max = rules.fanin?.max_importers;
  const importers = new Map();
  for (const f of model.files) {
    if (f.isTest) continue;
    for (const imp of f.imports) {
      if (imp.kind !== 'internal' || !imp.dir) continue;
      if (isRustSelfImport(f, imp)) continue;
      const set = importers.get(imp.dir) ?? new Set();
      set.add(f.rel);
      importers.set(imp.dir, set);
    }
  }
  const v = [];
  for (const [dir, set] of importers) {
    if (set.size > max) v.push(`${relOf(dir)}: ${set.size} importers (max ${max})`);
  }
  return v;
}

// `use crate::...`, `self::...`, and `super::...` are cohesion inside one Rust
// crate, not inbound coupling to that crate. Resolution deliberately maps them
// to the crate root for layering/cycle checks; fan-in must not count every
// split module as if it were an external consumer.
function isRustSelfImport(file, imported) {
  if (file.lang !== 'rust') return false;
  const head = imported.spec.split('::')[0];
  return head === 'crate' || head === 'self' || head === 'super';
}

// cognitive: number of distinct top-level source modules <= max_root_modules.
export function checkCognitive(model, rules) {
  const max = rules.cognitive?.max_root_modules;
  const mods = new Set(model.files.map((f) => f.rel.split(/[\\/]/)[0]));
  if (mods.size > max) {
    return [`${mods.size} top-level source modules (max ${max}): ${[...mods].sort().join(', ')}`];
  }
  return [];
}

// anti-pattern naming: a source directory whose basename is a known technical-
// role grab-bag (utils/common/manager/handler/...) signals "organized by tech
// detail, not architecture". A name blessed as a real layer in dir_aliases
// (e.g. service -> application) is EXEMPT — it is a deliberate layer, not a junk
// drawer. CHEAP PROXY for the cognitive-architecture skill (principle 1/2): it
// catches obvious smells; it cannot prove a structure is good (that stays the
// architect's judgment + the 30-second tree litmus).
export function checkAntiPatterns(model, rules) {
  const bad = new Set(rules.naming?.anti_patterns ?? []);
  const blessed = new Set(Object.keys(rules.architecture?.dir_aliases ?? {}));
  const seen = new Set();
  const v = [];
  for (const f of model.files) {
    for (const seg of f.rel.split(/[\\/]/).slice(0, -1)) {
      if (bad.has(seg) && !blessed.has(seg) && !seen.has(seg)) {
        seen.add(seg);
        v.push(`"${seg}/" is a technical-role grab-bag name (e.g. ${f.rel}) — organize by capability/layer, or bless "${seg}" as a layer in dir_aliases`);
      }
    }
  }
  return v;
}

// function-length: every DECLARED function's body must be <= max_function_lines
// (read from harness/policies.yml — the single source of truth this check finally
// ENFORCES). Spans come from the scan model's heuristic extractor; see
// scan.extractFunctions for the honesty note on its limits. Tests are included:
// an over-long test is just as much a maintainability smell as production code.
export function checkFunctionLength(model, limit) {
  if (!Number.isFinite(limit)) {
    // fail-closed: a missing/garbled limit is a config error, not a silent pass.
    return ['max_function_lines missing or non-numeric in harness/policies.yml'];
  }
  const v = [];
  for (const f of model.files) {
    for (const fn of f.functions ?? []) {
      if (fn.lines > limit) {
        v.push(`${f.rel}:${fn.line} ${fn.name} ${fn.lines} lines (max ${limit})`);
      }
    }
  }
  return v;
}

// circular-dependency: build the internal module-import graph (dir -> dir from
// resolved internal imports) and FAIL on any cycle, reporting the cycle path.
// Corresponds to policies.yml circular_dependency_count: 0.
// HONESTY: Go package-level import cycles are already forbidden by the Go
// compiler (so a Go cycle never reaches here); this check earns its keep on the
// JS/Python module graphs, where nothing else forbids A->B->A.
export function checkCircular(model) {
  const graph = buildImportGraph(model);
  const cycles = findCycles(graph);
  return cycles.map((cyc) => cyc.map(relOf).join(' -> '));
}

// buildImportGraph: dir -> Set(dir) over INTERNAL imports only, excluding
// self-edges (a file importing a sibling in its own dir is not a module cycle).
function buildImportGraph(model) {
  const graph = new Map();
  for (const f of model.files) {
    const from = f.dir;
    if (!graph.has(from)) graph.set(from, new Set());
    for (const imp of f.imports) {
      if (imp.kind !== 'internal' || !imp.dir || imp.dir === from) continue;
      graph.get(from).add(imp.dir);
    }
  }
  return graph;
}

// findCycles: DFS with a recursion stack; on a back-edge to a node on the stack,
// record the cycle slice. De-duplicates cycles by their normalized rotation so
// the same loop is reported once regardless of entry point.
function findCycles(graph) {
  const color = new Map(); // undefined=white, 1=gray(on stack), 2=black(done)
  const stack = [];
  const found = new Map();
  const visit = (node) => {
    color.set(node, 1);
    stack.push(node);
    for (const next of graph.get(node) ?? []) {
      if (color.get(next) === 1) {
        const cyc = stack.slice(stack.indexOf(next));
        const key = canonCycle(cyc);
        if (!found.has(key)) found.set(key, [...cyc, next]);
      } else if (!color.get(next)) {
        visit(next);
      }
    }
    stack.pop();
    color.set(node, 2);
  };
  for (const node of graph.keys()) if (!color.get(node)) visit(node);
  return [...found.values()];
}

// canonCycle: rotation-invariant key for a cycle, so A->B->A and B->A->B dedup.
function canonCycle(cyc) {
  const i = cyc.indexOf([...cyc].sort()[0]);
  return [...cyc.slice(i), ...cyc.slice(0, i)].join(' ');
}

// drift-guard: .arch/rules.yaml file/root limits MUST equal harness/policies.yml
// so the two sources of truth cannot silently diverge.
export function checkDrift(rules) {
  const pol = readPolicies();
  if (pol === null) return [];
  const pairs = [
    ['file.max_lines', rules.file?.max_lines, policyNum(pol, 'max_file_lines')],
    ['root.max_files', rules.root?.max_files, policyNum(pol, 'max_root_files')],
  ];
  return pairs
    .filter(([, a, b]) => a !== b)
    .map(([name, a, b]) => `${name}: .arch=${a} != harness/policies.yml=${b}`);
}

// readPolicies / policyNum: shared access to harness/policies.yml numeric keys,
// used by both the drift-guard and the function-length limit lookup.
function readPolicies() {
  try {
    return readFileSync(join(ROOT, 'harness', 'policies.yml'), 'utf8');
  } catch {
    return null;
  }
}

function policyNum(pol, key) {
  const m = pol.match(new RegExp(`^${key}\\s*:\\s*(\\d+)`, 'm'));
  return m ? Number(m[1]) : null;
}

function main() {
  const rules = parseRules(readFileSync(join(ROOT, '.arch', 'rules.yaml'), 'utf8'));
  const model = scan(ROOT, rules);
  const pol = readPolicies();
  const fnLimit = pol === null ? NaN : policyNum(pol, 'max_function_lines');
  const checks = [
    ['layering', checkLayering(model, rules)],
    ['package', checkPackage(model, rules)],
    ['fanin', checkFanin(model, rules)],
    ['cognitive', checkCognitive(model, rules)],
    ['anti-pattern-naming', checkAntiPatterns(model, rules)],
    ['function-length', checkFunctionLength(model, fnLimit)],
    ['circular-dependency', checkCircular(model)],
    ['drift-guard', checkDrift(rules)],
  ];
  let failed = 0;
  for (const [name, v] of checks) {
    if (v.length === 0) {
      console.log(`forge-arch: [PASS] ${name}`);
      continue;
    }
    failed += 1;
    console.log(`forge-arch: [FAIL] ${name} — ${v.length} violation(s):`);
    for (const line of v) console.log(`    ${line}`);
  }
  console.log(
    failed === 0
      ? `forge-arch: PASS (${model.files.length} source files, ${checks.length} checks)`
      : `forge-arch: FAIL — ${failed} check(s) violated`,
  );
  process.exit(failed === 0 ? 0 : 1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
