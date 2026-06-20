// ForgeOS acceptance — adapter-backed QUALITY probes (lint + coverage).
//
// Split out of acceptance.mjs alongside the kernel: the lint and coverage
// criteria form one cohesive family (both upgrade a STATIC N/A into an
// executable, framework-backed criterion by reading harness/adapters/<lang>.yml,
// probing whether the tool is INSTALLED, and only then shelling it — with the
// same honesty/fail-safe contract). Grouping them here drops acceptance.mjs's
// responsibility density and lifts it off the 500-line cap.
//
// Dependency direction (acyclic): this module imports DOWN from the kernel
// (run/result/splitCmd/PASS/FAIL/NA/ROOT) and from adapters.mjs (the declarative
// command maps + their pure judges) — never from acceptance.mjs, which imports
// these probes. Behaviour is byte-for-byte identical to the pre-split code.
import { existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { PASS, FAIL, NA, ROOT, run, result, splitCmd } from './acceptance-kernel.mjs';
import {
  detectLanguages,
  loadAdapter,
  lintBinary,
  coverageBinary,
  judgeCoverage,
  resolveCoverageThreshold,
  versionProbeArgs,
  coverageArtifact,
} from './adapters.mjs';

// lint == clean  <-  per-language linters from harness/adapters/<lang>.yml.
// This upgrades lint from a STATIC N/A (the adapters used to be pure
// declarations with no consumer) to an executable, framework-backed criterion:
// detect the project's languages, load each adapter's lint command, and — only
// when that linter is actually INSTALLED — shell it out.
//
// HONESTY + FAIL-SAFE (the whole point): a missing linter is NOT a failure and
// NOT a faked pass. Each contributing helper returns one of:
//   PASS  — linter installed AND the lint command exited 0 (a real clean run);
//   FAIL  — linter installed AND it reported real lint violations;
//   N-A   — linter NOT installed, OR installed but unconfigured for this project
//           (e.g. eslint with no eslintrc: it can't run, so its result is not a
//           verdict on the code). N/A is the honest outcome, never a FAIL.
// The criterion is N/A iff every language is N/A (a repo with no installed linter
// stays N/A and therefore ACCEPTED, keeping lint NON-load-bearing); install
// eslint/golangci-lint/ruff with a config and the SAME code auto-enforces PASS/FAIL.

// linterInstalled: true iff `<bin> --version` exits 0. The cheap, side-effect-
// free probe for "is this tool on PATH" before running the heavier lint command.
function linterInstalled(bin) {
  return run(bin, ['--version']).ok;
}

// unconfigured: did the lint command fail because the linter could not actually
// RUN (no project config), as opposed to finding real violations? Such a result
// is not a verdict on the code, so we map it to N/A rather than FAIL. Detected
// generically from the tool's own "couldn't find a configuration" wording (and
// eslint's exit-2 "fatal/config" code, distinct from its exit-1 "found lint
// errors"). Conservative: only the clear can't-run signals match here.
export function unconfigured(out) {
  return /no\s+configuration|couldn'?t\s+find\s+a?\s*config|configuration\s+file|unable\s+to\s+(?:find|locate)\s+config/i.test(out ?? '');
}

// judgeLint: PURE per-language lint decision (no I/O) so the honesty/fail-safe
// branches are directly unit-testable without any linter installed. Inputs:
//   lang      — adapter language tag (for the detail string);
//   bin       — the linter binary, or null when the adapter has no lint command;
//   installed — did `<bin> --version` exit 0 (boolean);
//   r         — the lint command's run result {ok,code,out}, or null if not run.
// Order: no bin / not installed -> N/A; exit 0 -> PASS; could-not-run
// (unconfigured) -> N/A; otherwise (real violations) -> FAIL. A missing linter is
// NEVER a FAIL and an unconfigured one is NEVER a faked PASS.
export function judgeLint(lang, bin, installed, r) {
  if (!bin) return { lang, status: NA, detail: `${lang}: adapter has no lint command` };
  if (!installed) return { lang, status: NA, detail: `${lang}: ${bin} not installed` };
  if (r && r.ok) return { lang, status: PASS, detail: `${lang}: ${bin} clean` };
  if (r && unconfigured(r.out)) return { lang, status: NA, detail: `${lang}: ${bin} installed but unconfigured (no project config) — not run` };
  return { lang, status: FAIL, detail: `${lang}: ${bin} exit ${r ? r.code : 'n/a'}` };
}

// probeLintLang: I/O wrapper around judgeLint for one language. Loads the
// adapter, probes whether the linter is installed, and (only then) shells the
// lint command — deferring the verdict to the pure judgeLint.
function probeLintLang(lang) {
  const { lint } = loadAdapter(lang);
  const bin = lintBinary(lint);
  if (!bin) return judgeLint(lang, null, false, null);
  const installed = linterInstalled(bin);
  const r = installed ? run(...splitCmd(lint)) : null;
  return judgeLint(lang, bin, installed, r);
}

// probeLint: aggregate per-language lint into the single `lint` criterion.
// N/A when no source languages are detected OR every detected language is N/A
// (no installed/configured linter). FAIL if any language FAILs. Otherwise PASS.
export function probeLint() {
  const langs = detectLanguages(ROOT);
  if (langs.length === 0) return result('lint', NA, 'no source languages detected');
  const per = langs.map(probeLintLang);
  const detail = per.map((p) => p.detail).join('; ');
  if (per.some((p) => p.status === FAIL)) return result('lint', FAIL, detail);
  if (per.every((p) => p.status === NA)) return result('lint', NA, detail);
  return result('lint', PASS, detail);
}

// coverage >= threshold  <-  per-language coverage tools from the adapters (go
// test -coverprofile / pytest --cov / vitest --coverage). Like probeLint, this
// upgrades coverage from a STATIC N/A into an executable, framework-backed
// criterion. HONESTY/FAIL-SAFE identical to lint: a missing OR can't-run tool
// (no module/tests/config) is N/A, never FAIL; only a real run below threshold
// FAILs. The pure decision is adapters.judgeCoverage (unit-testable with no tool
// installed). N/A iff every language is N/A — so this repo stays N/A and ACCEPTED,
// keeping coverage NON-load-bearing; install+configure a tool and PASS/FAIL auto-
// enforce.

// probeCoverageLang: I/O wrapper around judgeCoverage for one language. Loads the
// adapter's coverage command, probes whether its tool is installed (`<bin>
// --version`), and (only then) shells the coverage command — deferring the
// verdict to the pure judgeCoverage. Reuses splitCmd + the same no-shell run().
//
// Side-effect discipline: coverage commands WRITE a report artifact into the
// working tree (e.g. `go test -coverprofile=coverage.out` drops coverage.out even
// when the run fails on "not a module"). A gate must not pollute the repo it
// judges, so we snapshot whether that artifact pre-existed and remove ONLY one we
// ourselves created — never a user's pre-existing coverage report.
function probeCoverageLang(lang, threshold) {
  const { coverage } = loadAdapter(lang);
  const bin = coverageBinary(coverage);
  if (!bin) return judgeCoverage(lang, null, false, null, threshold);
  // Tool-aware install probe: `go version`, else `<bin> --version` (see
  // versionProbeArgs — `go --version` would falsely read as "not installed").
  const installed = run(bin, versionProbeArgs(bin)).ok;
  if (!installed) return judgeCoverage(lang, bin, false, null, threshold);
  const artifact = coverageArtifact(coverage);
  const path = artifact ? join(ROOT, artifact) : null;
  const preexisted = path ? existsSync(path) : false;
  const r = run(...splitCmd(coverage));
  if (path && !preexisted && existsSync(path)) rmSync(path, { recursive: true, force: true });
  return judgeCoverage(lang, bin, true, r, threshold);
}

// probeCoverage: aggregate per-language coverage into the single `coverage`
// criterion (mirrors probeLint). N/A when no source languages are detected OR
// every detected language is N/A (no installed/runnable coverage tool). FAIL if
// any language is below threshold. Otherwise PASS.
export function probeCoverage() {
  const langs = detectLanguages(ROOT);
  if (langs.length === 0) return result('coverage', NA, 'no source languages detected');
  // The line-coverage floor is the project's mode×lifecycle threshold (central knob:
  // .agent/project.yml × modes.yml — see resolveCoverageThreshold for the resolution,
  // its missing-file FAIL-SAFE, and why an N/A here is unaffected), NOT a hardcoded 60.
  const threshold = resolveCoverageThreshold(ROOT);
  const per = langs.map((lang) => probeCoverageLang(lang, threshold));
  const detail = per.map((p) => p.detail).join('; ');
  if (per.some((p) => p.status === FAIL)) return result('coverage', FAIL, detail);
  if (per.every((p) => p.status === NA)) return result('coverage', NA, detail);
  return result('coverage', PASS, detail);
}
