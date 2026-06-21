#!/usr/bin/env node
// ForgeOS acceptance gate — the executable Stop gate for the Evaluation layer.
// Maps `.agent/eval/acceptance.schema.yml` criteria onto THIS repo's existing
// out-of-band checks and renders an ACCEPTED / REJECTED verdict.
//
// Honesty first: a criterion is only PASS/FAIL when a real check backs it.
// Criteria with no executable check in this repo (coverage / lint / typecheck
// / build / security_findings) are reported N/A with a reason — never faked
// into a pass, and N/A is never counted as satisfied.
//
// Design: one criterion == one probe function (shells its command, judges the
// exit code); a runner collects results; `decide()` is a PURE function over the
// results array so the verdict is unit-testable without spawning anything.
//
// CLI: `node harness/acceptance.mjs`         ->  exit 0 ACCEPTED · exit 1 REJECTED.
//      `node harness/acceptance.mjs --json`  ->  per-criterion JSON to stdout.
import { readdirSync, statSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { appTestPlan, ADAPTER_LANG_BY_RUNNER, loadAdapter } from './adapters.mjs';
import { scanRepo as scaScanRepo } from './sca.mjs';
import { PASS, FAIL, NA, ROOT, HARNESS_DIR, run, result, withCategory } from './acceptance-kernel.mjs';
import { probeLint, probeCoverage } from './acceptance-quality.mjs';

// Re-export the verdict statuses (defined in the kernel) so importers — chiefly
// test_acceptance.mjs — keep getting PASS/FAIL/NA from acceptance.mjs unchanged.
export { PASS, FAIL, NA };
// Re-export the adapter-backed quality probes (moved to acceptance-quality.mjs):
// collect() below calls them, and test_acceptance.mjs imports probeLint/
// probeCoverage / the pure judgeLint+unconfigured from acceptance.mjs — keep that
// surface intact across the split.
export { judgeLint, unconfigured, probeLint, probeCoverage } from './acceptance-quality.mjs';

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

// test_pass == true  <-  ALL committed harness suites must be green (self-
// governance: the harness runs its OWN tests, not a curated subset, so a
// regression in verdict logic — or a silently-green arch-check faking a pass —
// can't ship). Suites: test_check.py + test_yaml2json.py, plus two Node globs —
// 'harness/test_*.mjs' (test_gate/test_acceptance/test_scorecard/test_enforce/…)
// and 'harness/arch/test_*.mjs' (test_arch-check's negative fixtures; the first
// glob is non-recursive so this second entry is required, else those vanish).
// BOTH Node globs are fail-CLOSED on discovery via runCountedTest: a zero-match
// glob exits 0 ("# tests 0"), so requiring N>0 stops a moved/renamed suite from
// reporting green while running nothing — the symmetric guard the arch glob
// always had but harness/test_*.mjs previously lacked (fail-OPEN: judged `.ok`).
export function probeTests() {
  const mjsRun = runCountedTest('harness/test_*.mjs', { FORGE_ACCEPT_INNER: '1' });
  const archRun = runCountedTest('harness/arch/test_*.mjs', { FORGE_ACCEPT_INNER: '1' });
  const suites = [
    ['test_check.py', run('python3', [join(HARNESS_DIR, 'test_check.py')]).ok],
    ['test_yaml2json.py', run('python3', [join(HARNESS_DIR, 'test_yaml2json.py')]).ok],
    ['harness/test_*.mjs', mjsRun.ok],
    ['harness/arch/test_*.mjs', archRun.ok],
  ];
  const failed = suites.filter(([, ok]) => !ok).map(([name]) => name);
  const ok = failed.length === 0;
  const detail = ok
    ? `test_check.py + test_yaml2json.py + harness/test_*.mjs (${mjsRun.count}) + harness/arch/test_*.mjs (${archRun.count}): all green`
    : `failed: ${failed.join(', ')}`;
  return result('test_pass', ok ? PASS : FAIL, detail);
}

// app_test_pass == true  <-  EVERY discovered example app's test suite is green.
// Without this probe, `forge accept` would happily ACCEPT a regressed app: no
// automated gate ran the app's tests. P21: discover all examples/<app>/ dirs
// that ship a test/ subdir (not just the hardcoded url-shortener), infer the
// runner per app from its test-file extensions, and FAIL if ANY app fails.

// Discover example apps: examples/<app>/ dirs that contain a test/ subdir.
// Returns [{name, testDir}] sorted by name (deterministic ordering).
function discoverApps() {
  const examples = join(ROOT, 'examples');
  if (!existsSync(examples)) return [];
  const apps = [];
  for (const name of readdirSync(examples).sort()) {
    const testDir = join(examples, name, 'test');
    let st;
    try { st = statSync(testDir); } catch { continue; }
    if (st.isDirectory()) apps.push({ name, testDir });
  }
  return apps;
}

// Infer a runner from the test files present. Order matters: a single app dir
// is assumed homogeneous; first matching language wins. null -> unknown layout.
function inferRunner(app) {
  const files = readdirSync(app.testDir);
  if (files.some((f) => f.endsWith('.test.mjs'))) return 'node';
  if (files.some((f) => f.endsWith('_test.py') || (f.startsWith('test_') && f.endsWith('.py')))) return 'python';
  if (files.some((f) => f.endsWith('_test.go'))) return 'go';
  return null;
}

// Run one app's suite; return {name, ok, detail}. DECLARATION-DRIVEN: the pure
// appTestPlan picks the app's adapter `test:` command when its runner fits the
// on-disk layout (e.g. go-taskd's `go test ./...`, from its own module dir via
// plan.cwd), else the hardcoded fallback (e.g. url-shortener's node:test, since
// the typescript adapter's `vitest run` does not discover *.test.mjs) — annotating
// WHY (honesty). plan.countCheck routes node to the fail-closed counting runner;
// adapter/python/go judge on exit code. (ADAPTER_LANG_BY_RUNNER maps the runner
// kind onto a <lang>.yml.)
function runApp(app) {
  const runner = inferRunner(app);
  if (!runner) return { name: app.name, ok: false, detail: `${app.name}: FAIL (no recognized test runner)` };
  const { test: testCmd } = loadAdapter(ADAPTER_LANG_BY_RUNNER[runner]);
  const plan = appTestPlan(testCmd, readdirSync(app.testDir), app.name, runner);
  if (plan.countCheck) return runNodeApp(app, plan.tag);
  const r = run(plan.cmd, plan.args, {}, plan.cwd ? join(ROOT, plan.cwd) : ROOT);
  const detail = r.ok ? `${app.name}: PASS (${plan.tag})` : `${app.name}: FAIL (${plan.tag}, exit ${r.code})`;
  return { name: app.name, ok: r.ok, detail };
}

// Run a node app's *.test.mjs suite (always node here — the typescript adapter's
// `vitest run` never discovers node:test *.test.mjs, so plan.countCheck is set).
// Fail-closed: exit 0 is necessary but NOT sufficient — an empty glob still exits
// 0 ("tests 0"); require the `tests N` summary with N > 0. Quoted glob dodges Node
// 26's broken `--test <dir>`; the TAP reporter pins the format. `tag` carries the
// honest "(node fallback: …)" annotation from appTestPlan.
function runNodeApp(app, tag) {
  const glob = `examples/${app.name}/test/*.test.mjs`;
  const r = run('node', ['--test', '--test-reporter=tap', glob]);
  const m = r.out.match(/(?:^|\n)# tests (\d+)/);
  const count = m ? Number(m[1]) : null;
  const ok = r.ok && count !== null && count > 0;
  const detail = ok
    ? `${app.name}: PASS (${tag}, ${count} tests)`
    : count === 0 || count === null
      ? `${app.name}: FAIL (no tests discovered, expected >=1)`
      : `${app.name}: FAIL (node exit ${r.code})`;
  return { name: app.name, ok, detail };
}

export function probeAppTests() {
  const apps = discoverApps();
  if (apps.length === 0) {
    // Never a silent pass: zero apps is reported N/A with a reason.
    return result('app_test_pass', NA, 'no example apps with a test/ dir discovered under examples/');
  }
  const runs = apps.map(runApp);
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

// Criteria with NO executable check in this repo. Surfaced as N/A with an honest
// reason; NEVER asserted as a pass (see decide()). security_findings, lint,
// coverage, and dependency_vulnerabilities are NO LONGER here — each is now a
// real probe (secret-scan / adapter linters / adapter coverage / sca) that
// reports PASS/FAIL when its tool/data is present and N/A only when it is not.
// typecheck/build stay N/A: no type-checker / build step is wired for this repo.
export function probeNotApplicable() {
  return [
    result('typecheck', NA, 'no TS sources / type-checker in this repo'),
    result('build', NA, 'no build step (declarative + zero-dep harness)'),
  ];
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
// -> N/A, scan NOT run and NOT faked. NOT in LOAD_BEARING, so the no-DB N/A does
// not block accept; a DB-present FAIL blocks via decide()'s normal fail path.
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
// annotated with its lifecycle-aware category (withCategory) — an ADDITIVE field
// that decide()/LOAD_BEARING deliberately ignore (the verdict logic is unchanged),
// carried so the --json bridge can hand forge-core WHY an N/A is N/A (a missing
// tool vs a concept the language lacks) for its lifecycle-aware exemption matrix.
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
    ...probeNotApplicable(),
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
//   (2) the four load-bearing criteria are each PRESENT and PASS (not merely
//       not-FAIL); a missing/NA/unknown load-bearing criterion blocks accept;
//   (3) the results are non-empty (zero criteria prove nothing).
// N/A is explicitly NOT a pass for the remaining criteria: it neither blocks nor
// counts toward satisfaction — surfaced so missing coverage stays honest.
export function decide(results) {
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
  process.exit(decide(results).accepted ? 0 : 1);
}

function main() {
  if (process.argv.slice(2).includes('--json')) {
    emitJson();
    return;
  }
  const results = collect();
  const verdict = decide(results);
  console.log(render(results, verdict));
  process.exit(verdict.accepted ? 0 : 1);
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
