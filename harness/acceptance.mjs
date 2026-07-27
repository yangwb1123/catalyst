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
// CLI: `node harness/acceptance.mjs`         ->  exit 0 ACCEPTED · exit 1 REJECTED.
//      `node harness/acceptance.mjs --json`  ->  per-criterion JSON to stdout.
import { readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { probeDiscoveredProjectTests } from './adapters/project.mjs';
import { scanRepo as scaScanRepo } from './sca.mjs';
import {
  PASS, FAIL, NA, ROOT, HARNESS_DIR, INAPPLICABLE, run, result, withCategory,
} from './acceptance-kernel.mjs';
import { probeLint, probeCoverage } from './acceptance-quality.mjs';
import {
  runHarnessSuites, runPythonSuites,
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

// test_pass recursively discovers every harness Node/Python suite. Node runs the
// concrete discovered file list and requires TAP `# tests N>0`; Python imports
// each file, runs unittest plus module-level pytest-style tests, and likewise
// requires a real positive count. New nested subpackages cannot vanish behind a
// curated glob. Project manifests are independently load-bearing below.
export function probeTests() {
  const harness = runHarnessSuites();
  const suites = [...harness.entries];
  // Project test targets are load-bearing when present. A missing Rust/Java
  // tool is N/A/no_tool at the project-probe boundary, but cannot be silently
  // skipped inside test_pass; only a truly inapplicable "no project" is omitted.
  const projectTests = withCategory(probeProjectTests());
  if (projectTests.category !== INAPPLICABLE) {
    suites.push(['project tests', projectTests.status === PASS]);
  }
  const failed = suites.filter(([, ok]) => !ok).map(([name]) => name);
  const ok = failed.length === 0;
  const detail = ok
    ? `recursive Python suites (${harness.python.files} files/${harness.python.count} tests)`
      + ` + recursive Node suites (${harness.node.files} files/${harness.node.count} tests)`
      + (projectTests.status === PASS ? ` + ${projectTests.detail}` : '')
      + ': all green'
    : `failed: ${failed.join(', ')}${projectTests.status !== PASS ? ` — ${projectTests.detail}` : ''}`;
  return result('test_pass', ok ? PASS : FAIL, detail);
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
export const LOAD_BEARING = ['test_pass', 'app_test_pass', 'complexity_violations', 'arch_violations', 'architecture', 'security_findings'];

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
  const failed = results.filter((r) => r.status === FAIL);
  const passed = results.filter((r) => r.status === PASS);
  const na = results.filter((r) => r.status === NA);
  const unknown = results.filter((r) => r.status !== PASS && r.status !== FAIL && r.status !== NA);
  const byName = new Map(results.map((r) => [r.criterion, r.status]));
  const missing = LOAD_BEARING.filter((name) => byName.get(name) !== PASS);
  const reasons = [
    ...failed.map((r) => `${r.criterion} failed`),
    ...unknown.map((r) => `${r.criterion} bad status ${JSON.stringify(r.status)}`),
    ...missing
      .filter((name) => byName.get(name) === undefined || byName.get(name) === NA)
      .map((name) => `${name} not satisfied (${byName.get(name) ?? 'absent'})`),
    ...criticalNAReasons(results, lifecycle),
  ];
  const empty = results.length === 0;
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

export function render(results, verdict) {
  const lines = results.map((r) => `  [${ICON[r.status]}] ${r.criterion} — ${r.detail}`);
  return [
    'forge-accept: acceptance gate (.agent/eval/acceptance.schema.yml)',
    ...lines,
    `  (${verdict.passed} pass · ${verdict.failed} fail · ${verdict.na} n/a — n/a is NOT counted as satisfied)`,
    verdict.line,
  ].join('\n');
}

// emitJson prints the per-criterion results as a JSON array
// [{criterion,status,detail}] to stdout — the structured mode consumed by
// forge-core's gate.ProbeAll. Reuses the same pure collect(); the exit code
// still reflects the verdict so `--json` stays usable as a gate too.
function emitJson() {
  const results = collect();
  console.log(JSON.stringify(results));
  process.exit(decide(results, readProjectLifecycle()).accepted ? 0 : 1);
}

function main() {
  if (process.argv.slice(2).includes('--json')) {
    emitJson();
    return;
  }
  const results = collect();
  const verdict = decide(results, readProjectLifecycle());
  console.log(render(results, verdict));
  process.exit(verdict.accepted ? 0 : 1);
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
