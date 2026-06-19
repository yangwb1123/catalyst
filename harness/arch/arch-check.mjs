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

// fanin: no internal target dir may be imported by more than max_importers files.
export function checkFanin(model, rules) {
  const max = rules.fanin?.max_importers;
  const importers = new Map();
  for (const f of model.files) {
    for (const imp of f.imports) {
      if (imp.kind !== 'internal' || !imp.dir) continue;
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

// drift-guard: .arch/rules.yaml file/root limits MUST equal harness/policies.yml
// so the two sources of truth cannot silently diverge.
export function checkDrift(rules) {
  let pol;
  try {
    pol = readFileSync(join(ROOT, 'harness', 'policies.yml'), 'utf8');
  } catch {
    return [];
  }
  const polNum = (key) => {
    const m = pol.match(new RegExp(`^${key}\\s*:\\s*(\\d+)`, 'm'));
    return m ? Number(m[1]) : null;
  };
  const pairs = [
    ['file.max_lines', rules.file?.max_lines, polNum('max_file_lines')],
    ['root.max_files', rules.root?.max_files, polNum('max_root_files')],
  ];
  return pairs
    .filter(([, a, b]) => a !== b)
    .map(([name, a, b]) => `${name}: .arch=${a} != harness/policies.yml=${b}`);
}

function main() {
  const rules = parseRules(readFileSync(join(ROOT, '.arch', 'rules.yaml'), 'utf8'));
  const model = scan(ROOT, rules);
  const checks = [
    ['layering', checkLayering(model, rules)],
    ['package', checkPackage(model, rules)],
    ['fanin', checkFanin(model, rules)],
    ['cognitive', checkCognitive(model, rules)],
    ['anti-pattern-naming', checkAntiPatterns(model, rules)],
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
