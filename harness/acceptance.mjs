#!/usr/bin/env node
// ForgeOS acceptance gate — the executable Stop gate for the Evaluation layer.
// Maps `.agent/eval/acceptance.schema.yml` criteria onto THIS repo's existing
// out-of-band checks and renders an ACCEPTED / REJECTED verdict.
//
// Honesty first: a criterion is only PASS/FAIL when a real check backs it. The
// Build/typecheck are project-aware: forge-core is test/build/vet gated, while a
// copied project with no such target is honestly N/A. lint / coverage /
// dependency_vulnerabilities are REAL probes that report N/A only when their
// tool/DB is absent here. security_findings is a real load-bearing PASS/FAIL
// probe. N/A is never faked into a pass or counted as satisfied; production
// blocks missing tools for critical criteria while exempting true inapplicability.
//
// Design: one criterion == one probe function (shells its command, judges the
// exit code); a runner collects results; `decide()` is a PURE function over the
// results array so the verdict is unit-testable without spawning anything.
//
// CLI: `node harness/acceptance.mjs`         ->  live exit 0 ACCEPTED · exit 1 REJECTED.
//      `node harness/acceptance.mjs --json`  ->  live per-criterion JSON to stdout.
//      `node harness/acceptance.mjs --cache` ->  explicitly advisory acceleration.
import { readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { probeDiscoveredProjectTests } from './adapters/project.mjs';
import { scanRepo as scaScanRepo } from './sca.mjs';
import {
  APPLICABLE, INAPPLICABLE, NO_TOOL, PASS, FAIL, NA, ROOT, HARNESS_DIR,
  run, result, withCategory,
} from './acceptance-kernel.mjs';
import { probeLint, probeCoverage } from './acceptance-quality.mjs';
import {
  runNodeHarness, runPythonHarness, runPythonHarnessParallel,
  runCountedNodeFiles, runPythonSuites,
} from './acceptance-tests.mjs';
import {
  criticalNAReasons,
  probeBuild,
  probeForgeCoreTests,
  probeProjectTests,
  probeTypecheck,
  readProjectLifecycle,
} from './acceptance-project.mjs';

// Re-export the verdict statuses (defined in the kernel) so importers — chiefly
// test_acceptance.mjs — keep getting PASS/FAIL/NA from acceptance.mjs unchanged.
export { PASS, FAIL, NA };
// Dynamic import keeps the dependency graph one-way: the parallel coordinator
// imports these probe functions, while this module loads it only after module
// evaluation. Importing acceptance.mjs therefore remains side-effect free and
// cannot form acceptance <-> parallel static-initialisation cycles.
export async function collectParallel(options = {}) {
  const runner = await import('./acceptance-parallel.mjs');
  return runner.collectParallel(options);
}

export async function collectCliRows(options = {}) {
  if (options.useCache === true) {
    const advisory = await import('./acceptance-advisory.mjs');
    return advisory.collectAdvisory(options);
  }
  return collectParallel(options);
}
// Re-export the adapter-backed quality probes (moved to acceptance-quality.mjs):
// collect() below calls them, and test_acceptance.mjs imports probeLint/
// probeCoverage / the pure judgeLint+unconfigured from acceptance.mjs — keep that
// surface intact across the split.
export { judgeLint, unconfigured, probeLint, probeCoverage } from './acceptance-quality.mjs';
export {
  discoverHarnessSuites, runCountedNodeFiles, runPythonSuites,
} from './acceptance-tests.mjs';
export {
  criticalNAReasons,
  probeBuild,
  probeForgeCoreTests,
  probeProjectTests,
  probeTypecheck,
  readProjectLifecycle,
} from './acceptance-project.mjs';

// --- per-criterion probes (one criterion == one function) --------------------

// complexity_violations == 0  <-  structural gate (per-file line cap + root count).
export function probeComplexity() {
  const r = run('node', [join(HARNESS_DIR, 'gate.mjs')]);
  return result(
    'complexity_violations',
    r.ok ? PASS : FAIL,
    r.ok ? 'gate.mjs: structural caps clean' : `gate.mjs exit ${r.code}`,
  );
}

// arch_violations == 0  <-  governance integrity (broken agent/skill refs, tiers).
export function probeArch() {
  const r = run('python3', [join(HARNESS_DIR, 'check.py')]);
  return result(
    'arch_violations',
    r.ok ? PASS : FAIL,
    r.ok ? 'check.py: governance integrity clean' : `check.py exit ${r.code}`,
  );
}

// runCountedTest: a FAIL-CLOSED `node --test <glob>` run. Exit 0 alone is NOT
// proof a suite ran — `node --test` exits 0 on a glob that matches ZERO files
// ("# tests 0"), so a renamed/moved/restructured suite would report green while
// running nothing (load-bearing test_pass faking a pass; this really happened
// when forge-init dropped test_enforce.mjs). So every Node glob routes through
// here: run with the TAP reporter (pins the `# tests N` format; Node 26's
// `--test <dir>` is broken, hence a QUOTED glob), parse the count, and require
// BOTH exit 0 AND N > 0. extraEnv carries FORGE_ACCEPT_INNER=1 so a re-entered
// test_acceptance.mjs SKIPS its full-gate re-spawn (avoiding ~4x nested runs).
// Returns {ok, count} so callers can surface the discovered count honestly.
// Exported so test_acceptance.mjs can pin the fail-closed contract directly
// (a zero-match glob -> ok:false) without spawning the whole probeTests run.
export function runCountedTest(glob, extraEnv = {}) {
  const r = run('node', ['--test', '--test-reporter=tap', glob], extraEnv);
  const count = Number((r.out.match(/(?:^|\n)# tests (\d+)/) ?? [])[1]);
  return { ok: r.ok && count > 0, count: Number.isNaN(count) ? null : count };
}

// Sub-domain row builders used by the parallel workers. The synchronous
// collect() compatibility path calls the same live probes directly.
export function nodeTestRow() {
  const r = runNodeHarness();
  return result('test_pass_node', r.ok ? PASS : FAIL,
    `recursive Node suites (${r.files} files/${r.count} tests/${r.skipped} skipped)`);
}

export function pythonTestRow() {
  const r = runPythonHarness();
  return result('test_pass_python', r.ok ? PASS : FAIL,
    `recursive Python suites (${r.files} files/${r.count} tests)`);
}

// Parallel-python row used by the worker path: the 60+ suite files run as
// independent processes with a bounded pool, so the python wall-clock drops to
// ~the slowest file. Same verdict semantics and detail shape as pythonTestRow.
export async function pythonTestRowParallel() {
  const r = await runPythonHarnessParallel();
  return result('test_pass_python', r.ok ? PASS : FAIL,
    `recursive Python suites (${r.files} files/${r.count} tests)`);
}

// Fold the three test sub-rows (Node / Python / project) into the single
// test_pass criterion the acceptance schema declares. Pure over the rows, so
// the parallel runner can rebuild the test_pass row from cached sub-rows and
// live worker rows without duplicating the aggregation semantics.
export function aggregateTestPass(node, python, project) {
  const sub = [node, python];
  if (project.category !== INAPPLICABLE) sub.push(project);
  const failed = sub.filter((r) => r.status !== PASS).map((r) => r.criterion);
  const ok = failed.length === 0;
  const parts = [
    `${python.detail}`, `${node.detail}`,
    ...(project.category !== INAPPLICABLE ? [project.detail] : []),
  ];
  const detail = ok
    ? `${parts.join(' + ')}: all green`
    : `failed: ${failed.join(', ')}${project.status !== PASS && project.category !== INAPPLICABLE ? ` — ${project.detail}` : ''}`;
  return result('test_pass', ok ? PASS : FAIL, detail);
}

// test_pass recursively discovers every harness Node/Python suite and every
// project test target. The three sub-domains (Node family / Python family /
// project tests) are all live on this formal/synchronous path. Optional cached
// replay belongs only to the explicitly advisory parallel CLI mode; it can
// never substitute for this load-bearing criterion.
export function probeTests() {
  const node = nodeTestRow();
  const python = pythonTestRow();
  // Project test targets are load-bearing when present. A missing Rust/Java
  // tool is N/A/no_tool at the project-probe boundary, but cannot be silently
  // skipped inside test_pass; only a truly inapplicable "no project" is omitted.
  const project = withCategory({ ...probeProjectTests(), criterion: 'test_pass_project' });
  return aggregateTestPass(node, python, project);
}

// app_test_pass == true  <-  EVERY discovered example app's test suite is green.
// Without this probe, `forge accept` would happily ACCEPT a regressed app: no
// automated gate ran the app's tests. Every direct examples/<app>/ directory is
// a required app; its recursively discovered Go/Node/Python/Rust/Java project
// plans must execute at least one observable test. This covers nested tests/,
// language-native layouts, and mixed/nested manifest roots without a shallow
// extension heuristic.
function discoverApps(root) {
  const examples = join(root, 'examples');
  if (!existsSync(examples)) return [];
  return readdirSync(examples, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({ name: entry.name, root: join(examples, entry.name) }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function runApp(app, exec) {
  const rows = probeDiscoveredProjectTests(app.root, exec);
  if (rows.length === 0) {
    return {
      name: app.name,
      ok: false,
      detail: `${app.name}: FAIL (no configured project test target)`,
    };
  }
  const failed = rows.filter((row) => row.status !== PASS);
  const evidence = rows.map((row) => row.detail).join(', ');
  return {
    name: app.name,
    ok: failed.length === 0,
    detail: `${app.name}: ${failed.length === 0 ? 'PASS' : 'FAIL'} (${evidence})`,
  };
}

export function probeAppTests(root = ROOT, exec = run) {
  const apps = discoverApps(root);
  if (apps.length === 0) {
    // Never a silent pass: zero apps is reported N/A with a reason.
    return result('app_test_pass', NA, 'no example app directories discovered under examples/');
  }
  const runs = apps.map((app) => runApp(app, exec));
  const failed = runs.filter((r) => !r.ok);
  const ok = failed.length === 0;
  const detail = runs.map((r) => r.detail).join('; ');
  return result('app_test_pass', ok ? PASS : FAIL, detail);
}

// NOTE: the adapter-backed quality probes — lint (linterInstalled / unconfigured
// / judgeLint / probeLintLang / probeLint) and coverage (probeCoverageLang /
// probeCoverage), plus the shared splitCmd — live in ./acceptance-quality.mjs and
// ./acceptance-kernel.mjs. collect() calls probeLint/probeCoverage (imported at
// the top); test_acceptance.mjs's surface (probeLint/probeCoverage/judgeLint/
// unconfigured) is preserved via the re-export there.

// Kept as a compatibility surface for callers/tests that imported the former
// static-N/A list. Build and typecheck are now project probes: forge-core makes
// them real Go build/vet verdicts; a copied project without such a target gets an
// honest inapplicable N/A from acceptance-project.mjs.
export function probeNotApplicable() {
  return [];
}

// security_findings == 0  <-  hardcoded-secret scan (harness/secret-scan.mjs).
// Direction-5 security/compliance gate, aligned with OWASP Agentic Top-10 2025-12
// (sensitive information disclosure). PASS when the scanner exits 0 (no committed
// AWS keys / private-key headers / GitHub-Slack tokens / high-entropy credential
// assignments), FAIL otherwise. HONESTY: this is a PATTERN scanner for hardcoded
// secrets ONLY — dependency/CVE (SCA) scanning is the separate probeSCA below. It
// is load-bearing (see LOAD_BEARING): a gate that found a leaked credential yet
// still ACCEPTED would defeat its purpose.
export function probeSecurity() {
  const r = run('node', [join(HARNESS_DIR, 'secret-scan.mjs')]);
  return result(
    'security_findings',
    r.ok ? PASS : FAIL,
    r.ok ? 'secret-scan: no hardcoded secrets (pattern scan; SCA/CVE out of scope)' : `secret-scan exit ${r.code}: ${r.out}`,
  );
}

// dependency_vulnerabilities == 0  <-  Software-Composition-Analysis (sca.mjs):
// parse manifests (go.mod/package.json/requirements.txt) and match them vs an
// OSV-format advisory DB. The v3 SCA/CVE item secret-scan.mjs deferred; distinct
// from security_findings (hardcoded-secret scan). HONESTY (full rationale in
// sca.mjs's header): DB present (FORGE_SCA_DB env, else .agent/security/
// advisories.json) -> PASS / FAIL (detail lists advisory_id+severity); DB ABSENT
// -> N/A, scan NOT run and NOT faked. It stays outside the unconditional
// LOAD_BEARING set so an immature project may honestly lack a repo-specific DB;
// criticalNAReasons requires the tool at production. A DB-present FAIL always
// blocks via decide()'s normal fail path.
export function probeSCA(root = ROOT) {
  const dbPath = process.env.FORGE_SCA_DB || join(root, '.agent', 'security', 'advisories.json');
  const report = scaScanRepo(root, dbPath);
  const counts = `${report.manifestCount} manifest(s), ${report.depCount} dep(s)`;
  if (!report.available) {
    return result('dependency_vulnerabilities', NA,
      `SCA framework ready; no OSV advisory DB (set FORGE_SCA_DB or ${join('.agent', 'security', 'advisories.json')}) — dependency CVE scan not run, not faked (${counts} parsed)`);
  }
  if (report.findings.length === 0) {
    return result('dependency_vulnerabilities', PASS, `sca: 0 known-vulnerable dependencies (${counts} vs OSV advisory DB)`);
  }
  const listed = report.findings.map((f) => `${f.dep}@${f.installed} ${f.advisory_id} (${f.severity})`).join('; ');
  return result('dependency_vulnerabilities', FAIL, `sca: ${report.findings.length} vulnerable dependenc(ies): ${listed}`);
}

// architecture == clean  <-  clean-architecture DEPENDENCY DIRECTION + size
// budgets (harness/arch/arch-check.mjs, all 8 checks: layering / package /
// fan-in / cognitive / anti-pattern-naming / function-length /
// circular-dependency / drift-guard). Distinct from arch_violations (check.py
// governance integrity): this FAILs when e.g. a domain package imports
// infrastructure (an inner layer imports an outer) or a function exceeds budget.
export function probeArchitecture() {
  const r = run('node', [join(HARNESS_DIR, 'arch', 'arch-check.mjs')]);
  return result(
    'architecture',
    r.ok ? PASS : FAIL,
    r.ok
      ? 'arch-check: layering/package/fan-in/cognitive/anti-pattern-naming/function-length/circular-dependency/drift-guard clean'
      : `arch-check exit ${r.code}`,
  );
}

// --- aggregation + verdict ---------------------------------------------------

// Gather every criterion result. Order mirrors the acceptance schema. Each row is
// annotated with its lifecycle-aware category (withCategory). The category lets
// both direct decide() and the --json forge-core bridge distinguish a missing
// tool from a concept that does not apply.
// collect() is the synchronous compatibility surface used by scorecard and
// tests. It is deliberately all-live: formal acceptance, imported consumers,
// and the --json authority bridge can never obtain a verdict from a cache.
export function collect() {
  return [
    probeTests(),
    probeAppTests(),
    probeComplexity(),
    probeArch(),
    probeArchitecture(),
    probeSecurity(),
    probeSCA(),
    probeLint(),
    probeCoverage(),
    probeTypecheck(),
    probeBuild(),
  ].map(withCategory);
}

// The executable, load-bearing criteria: each MUST be present AND status===PASS
// for acceptance. Unlike the N/A criteria, these are backed by a real check, so
// "not FAIL" is not enough — a missing or non-PASS load-bearing criterion is a
// hard reject (e.g. a probe silently dropped, or surfaced as N/A by mistake).
export const ACCEPTANCE_CRITERIA = [
  'test_pass', 'app_test_pass', 'complexity_violations', 'arch_violations',
  'architecture', 'security_findings', 'dependency_vulnerabilities', 'lint',
  'coverage', 'typecheck', 'build',
];
export const LOAD_BEARING = [
  'test_pass', 'app_test_pass', 'complexity_violations', 'arch_violations',
  'architecture', 'security_findings',
];
const ROW_FIELDS = new Set(['criterion', 'status', 'detail', 'category']);
const ROW_STATUSES = new Set([PASS, FAIL, NA]);
const ROW_CATEGORIES = new Set([APPLICABLE, INAPPLICABLE, NO_TOOL]);

function acceptanceSchemaReasons(results) {
  if (!Array.isArray(results)) return ['acceptance result is not an array'];
  const seen = new Map();
  const reasons = [];
  for (const [index, row] of results.entries()) {
    if (!row || Array.isArray(row) || typeof row !== 'object') {
      reasons.push(`row ${index} is not an object`);
      continue;
    }
    const fields = Object.keys(row);
    if (fields.length !== ROW_FIELDS.size || fields.some((field) => !ROW_FIELDS.has(field))) {
      reasons.push(`row ${index} has non-exact fields`);
    }
    if (!ACCEPTANCE_CRITERIA.includes(row.criterion)) reasons.push(`unknown criterion ${JSON.stringify(row.criterion)}`);
    else seen.set(row.criterion, (seen.get(row.criterion) ?? 0) + 1);
    if (!ROW_STATUSES.has(row.status)) reasons.push(`${row.criterion} bad status ${JSON.stringify(row.status)}`);
    if (typeof row.detail !== 'string') reasons.push(`${row.criterion} detail is not a string`);
    if (!ROW_CATEGORIES.has(row.category)) reasons.push(`${row.criterion} bad category ${JSON.stringify(row.category)}`);
    if (row.status === NA ? row.category === APPLICABLE : row.category !== APPLICABLE) {
      reasons.push(`${row.criterion} status/category mismatch`);
    }
  }
  for (const criterion of ACCEPTANCE_CRITERIA) {
    if (!seen.has(criterion)) {
      reasons.push(LOAD_BEARING.includes(criterion)
        ? `${criterion} not satisfied (absent)` : `${criterion} absent`);
    } else if (seen.get(criterion) !== 1) reasons.push(`${criterion} duplicated`);
  }
  return reasons;
}

// PURE verdict function (no I/O) so it is directly unit-testable.
//
// Hardened (P10): ACCEPTED requires ALL of —
//   (1) every status is a known verdict {PASS,FAIL,N-A} (unknown -> hard reject);
//   (2) the six load-bearing criteria (see LOAD_BEARING) are each PRESENT and PASS
//       (not merely not-FAIL); a missing/NA/unknown load-bearing one blocks accept;
//   (3) the results are non-empty (zero criteria prove nothing).
// N/A is explicitly NOT a pass. Before production a critical NO_TOOL N/A is
// exempt; production blocks it. INAPPLICABLE remains exempt at every lifecycle.
export function decide(results, lifecycle = 'mvp') {
  const rows = Array.isArray(results) ? results.filter((r) => r && typeof r === 'object') : [];
  const schema = acceptanceSchemaReasons(results);
  const failed = rows.filter((r) => r.status === FAIL);
  const passed = rows.filter((r) => r.status === PASS);
  const na = rows.filter((r) => r.status === NA);
  const byName = new Map(rows.map((r) => [r.criterion, r.status]));
  const missing = LOAD_BEARING.filter((name) => byName.get(name) !== PASS);
  const reasons = [
    ...schema,
    ...failed.map((r) => `${r.criterion} failed`),
    ...missing
      .filter((name) => byName.get(name) === undefined || byName.get(name) === NA)
      .map((name) => `${name} not satisfied (${byName.get(name) ?? 'absent'})`)
      .filter((reason) => !schema.includes(reason)),
    ...(schema.length === 0 ? criticalNAReasons(rows, lifecycle) : []),
  ];
  const empty = rows.length === 0;
  const accepted = reasons.length === 0 && !empty;
  return {
    accepted,
    failed: failed.length,
    passed: passed.length,
    na: na.length,
    line: accepted
      ? 'forge-accept: ACCEPTED'
      : `forge-accept: REJECTED — ${empty ? 'no criteria evaluated' : reasons.join('; ')}`,
  };
}

const ICON = { [PASS]: 'PASS', [FAIL]: 'FAIL', [NA]: 'N-A ' };
const TERMINAL_ESCAPE_RE = /\x1B(?:\][\s\S]*?(?:\x07|\x1B\\)|[PX^_][\s\S]*?\x1B\\|\[[0-?]*[ -/]*[@-~]|[@-_])/g;

function terminalSafeText(value) {
  return String(value).replace(TERMINAL_ESCAPE_RE, '').replace(/\p{Cf}/gu, '')
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F]/g, '')
    .replaceAll('\t', '\\t');
}

function singleLineText(value) {
  return terminalSafeText(value).replaceAll('\r', '\\r').replaceAll('\n', '\\n');
}

export function sanitizeAdvisoryText(value) {
  return singleLineText(value).replace(/accepted/gi, 'A_C_C_E_P_T_E_D');
}

export function render(results, verdict) {
  const lines = results.map((r) => `  [${ICON[r.status]}] ${r.criterion} — ${singleLineText(r.detail)}`);
  return [
    'forge-accept: acceptance gate (.agent/eval/acceptance.schema.yml)',
    ...lines,
    `  (${verdict.passed} pass · ${verdict.failed} fail · ${verdict.na} n/a — n/a is NOT counted as satisfied)`,
    verdict.line,
  ].join('\n');
}

export function parseCliOptions(args = process.argv.slice(2), env = process.env) {
  const supported = new Set(['--json', '--cache', '--no-cache']);
  const unknown = args.filter((arg) => !supported.has(arg));
  if (unknown.length > 0) throw new Error(`unsupported argument(s): ${unknown.join(', ')}`);
  const json = args.includes('--json');
  const advisory = args.includes('--cache');
  if (json && advisory) throw new Error('--cache is advisory-only and cannot be combined with --json');
  if (advisory && args.includes('--no-cache')) throw new Error('--cache and --no-cache are mutually exclusive');
  return { json, advisory, useCache: advisory && env.FORGE_ACCEPT_NO_CACHE !== '1' };
}

export function renderAdvisory(results, verdict, cacheEnabled) {
  const lines = results.map((r) => `  [${ICON[r.status]}] ${r.criterion} — ${sanitizeAdvisoryText(r.detail)}`);
  const state = verdict.accepted ? 'GREEN' : 'RED';
  const source = cacheEnabled ? 'explicit advisory cache' : 'live advisory run (cache bypassed)';
  return [
    'forge-accept: advisory acceleration (NOT the acceptance authority)',
    ...lines,
    `  (${verdict.passed} pass · ${verdict.failed} fail · ${verdict.na} n/a — n/a is NOT counted as satisfied)`,
    `forge-accept: ADVISORY ${state} — ${source}; run forge accept for an authoritative verdict`,
  ].join('\n');
}

export async function runAcceptanceCli(args = process.argv.slice(2), options = {}) {
  const env = options.env ?? process.env;
  const mode = parseCliOptions(args, env);
  const collector = options.collector ?? collectCliRows;
  const fixedLifecycle = options.lifecycle !== undefined;
  const lifecycleReader = options.lifecycleReader ?? readProjectLifecycle;
  const lifecycleBefore = fixedLifecycle ? options.lifecycle : lifecycleReader();
  let results = await collector({ useCache: mode.useCache, env });
  const lifecycleAfter = fixedLifecycle ? options.lifecycle : lifecycleReader();
  let verdict = decide(results, lifecycleBefore);
  if (lifecycleBefore !== lifecycleAfter) {
    results = Array.isArray(results) ? results.map((row) => withCategory({
      criterion: row?.criterion, status: FAIL,
      detail: 'project lifecycle changed during acceptance; rerun required',
    })) : [];
    verdict = decide(results, lifecycleBefore);
    verdict = {
      ...verdict, accepted: false,
      line: 'forge-accept: REJECTED — project lifecycle changed during acceptance; rerun required',
    };
  }
  const output = mode.json
    ? JSON.stringify(results)
    : (mode.advisory ? renderAdvisory(results, verdict, mode.useCache) : render(results, verdict));
  const exitCode = mode.advisory ? 2 : (verdict.accepted ? 0 : 1);
  return { exitCode, mode, output, results, verdict };
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runAcceptanceCli().then(({ exitCode, output }) => {
    process.stdout.write(`${output}\n`);
    process.exitCode = exitCode;
  }).catch((err) => {
    const raw = err?.stack ?? err;
    const detail = process.argv.slice(2).includes('--cache') ? sanitizeAdvisoryText(raw) : raw;
    process.stderr.write(`forge-accept: fatal: ${detail}\n`);
    process.exitCode = 3;
  });
}
