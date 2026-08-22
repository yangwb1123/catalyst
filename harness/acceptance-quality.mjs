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
import {
  closeSync,
  constants as fsConstants,
  existsSync,
  fstatSync,
  fsyncSync,
  linkSync,
  lstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  rmdirSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { PASS, FAIL, NA, ROOT, run, result, splitCmd } from './acceptance-kernel.mjs';
import {
  detectLanguages,
  loadAdapter,
  loadAdapterDocument,
  lintBinary,
  coverageBinary,
  judgeCoverage,
  resolveCoverageThreshold,
  versionProbeArgs,
  coverageArtifacts,
} from './adapters.mjs';
import {
  assertCoveragePathParents,
  readCoverageReportPercent,
  resolveCoverageLayout,
} from './adapters/coverage-report.mjs';
import {
  PROJECT_LINT_LANGS,
  probeProjectOperations,
} from './adapters/project.mjs';

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
// The criterion is N/A if ANY detected language is N/A: a working tool in one
// ecosystem must not hide a missing/unconfigured tool in another. Install and
// configure every detected linter to make the aggregate a real PASS/FAIL verdict.

// linterInstalled: true iff `<bin> --version` exits 0. The cheap, side-effect-
// free probe for "is this tool on PATH" before running the heavier lint command.
function linterInstalled(bin, root, exec) {
  return exec(bin, ['--version'], {}, root).ok;
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
function probeLintLang(lang, root, exec) {
  const { lint } = loadAdapter(lang);
  const bin = lintBinary(lint);
  if (!bin) return judgeLint(lang, null, false, null);
  const installed = linterInstalled(bin, root, exec);
  const [cmd, args] = splitCmd(lint);
  const r = installed ? exec(cmd, args, {}, root) : null;
  return judgeLint(lang, bin, installed, r);
}

// probeLint: aggregate per-language lint into the single `lint` criterion.
// A PASS requires every detected language to produce a real clean verdict:
// one missing/unconfigured tool keeps the aggregate N/A so a green ecosystem
// cannot mask a production-blocking tool gap in another.
export function probeLint(root = ROOT, exec = run) {
  const langs = detectLanguages(root);
  if (langs.length === 0) return result('lint', NA, 'no source languages detected');
  // Manifest-aware languages are planned below so each command receives the
  // project root as cwd instead of an arbitrary repository root.
  const per = langs
    .filter((lang) => !PROJECT_LINT_LANGS.includes(lang))
    .map((lang) => probeLintLang(lang, root, exec));
  per.push(...probeProjectOperations(root, 'lint', exec));
  const detail = per.map((p) => p.detail).join('; ');
  if (per.some((p) => p.status === FAIL)) return result('lint', FAIL, detail);
  if (per.some((p) => p.status === NA)) return result('lint', NA, detail);
  return result('lint', PASS, detail);
}

// coverage >= threshold  <-  per-language coverage tools from the adapters (go
// test -coverprofile / pytest --cov / vitest --coverage). Like probeLint, this
// upgrades coverage from a STATIC N/A into an executable, framework-backed
// criterion. HONESTY/FAIL-SAFE identical to lint: a missing OR can't-run tool
// (no module/tests/config) is N/A, never FAIL; only a real run below threshold
// FAILs. The pure decision is adapters.judgeCoverage (unit-testable with no tool
// installed). N/A if ANY detected language is N/A, so one configured ecosystem
// cannot mask a missing coverage command/tool in another; only all-real results
// aggregate to PASS or FAIL.

// probeCoverageLang: I/O wrapper around judgeCoverage for one language. Loads the
// adapter's coverage command, probes whether its tool is installed (`<bin>
// --version`), and (only then) shells the coverage command — deferring the
// verdict to the pure judgeCoverage. Reuses splitCmd + the same no-shell run().
//
const COVERAGE_LOCK_API = 'forgeos.coverage-artifact-transaction-lock/v1';
const COVERAGE_LOCK_FILE = '.forge-coverage-artifact.lock';
const COVERAGE_LOCK_WAIT_MS = 30_000;
const COVERAGE_LOCK_POLL_MS = 25;
const COVERAGE_LOCK_MAX_BYTES = 1_024;
const COVERAGE_LOCK_FIELDS = new Set([
  'api_version', 'created_at_unix_ms', 'pid', 'token',
]);
const coverageLockWaitCell = new Int32Array(new SharedArrayBuffer(4));

function createCoverageLockCandidate(root) {
  if (!Number.isInteger(fsConstants.O_NOFOLLOW)) {
    throw new Error('coverage lock requires no-follow file support');
  }
  const token = randomUUID().replaceAll('-', '');
  const candidate = join(root, `.forge-coverage-lock-candidate-${process.pid}-${token}`);
  const record = {
    api_version: COVERAGE_LOCK_API,
    created_at_unix_ms: Date.now(),
    pid: process.pid,
    token,
  };
  const flags = fsConstants.O_WRONLY | fsConstants.O_CREAT
    | fsConstants.O_EXCL | fsConstants.O_NOFOLLOW;
  const descriptor = openSync(candidate, flags, 0o600);
  const errors = [];
  try {
    writeFileSync(descriptor, JSON.stringify(record), 'utf8');
    fsyncSync(descriptor);
  } catch (error) { errors.push(error); }
  try { closeSync(descriptor); } catch (error) { errors.push(error); }
  if (errors.length > 0) {
    const cleanupError = removeCandidate(candidate);
    if (cleanupError) errors.push(cleanupError);
    throw new AggregateError(errors, 'coverage lock candidate creation failed closed');
  }
  return { candidate, record };
}

function parseCoverageLock(raw, path) {
  let value;
  try { value = JSON.parse(raw); } catch (error) {
    throw new Error(`invalid coverage lock JSON at ${path}: ${error.message}`);
  }
  if (!value || Array.isArray(value) || typeof value !== 'object'
      || value.api_version !== COVERAGE_LOCK_API
      || !Number.isSafeInteger(value.created_at_unix_ms) || value.created_at_unix_ms < 0
      || !Number.isSafeInteger(value.pid) || value.pid <= 0
      || typeof value.token !== 'string' || !/^[a-f0-9]{32}$/.test(value.token)
      || Object.keys(value).length !== COVERAGE_LOCK_FIELDS.size
      || Object.keys(value).some((field) => !COVERAGE_LOCK_FIELDS.has(field))) {
    throw new Error(`invalid coverage lock metadata at ${path}; manual recovery required`);
  }
  return value;
}

function readCoverageLock(path) {
  const before = lstatSync(path);
  if (!before.isFile() || before.isSymbolicLink()) {
    throw new Error(`unsafe coverage lock path is not a regular file: ${path}`);
  }
  const descriptor = openSync(path, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
  try {
    const actual = fstatSync(descriptor);
    if (!actual.isFile() || actual.dev !== before.dev || actual.ino !== before.ino
        || actual.size <= 0 || actual.size > COVERAGE_LOCK_MAX_BYTES) {
      throw new Error(`unsafe or oversized coverage lock at ${path}`);
    }
    return parseCoverageLock(readFileSync(descriptor, 'utf8'), path);
  } finally {
    closeSync(descriptor);
  }
}

function coverageLockOwnerAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === 'ESRCH') return false;
    if (error?.code === 'EPERM') return true;
    throw error;
  }
}

function removeCandidate(path) {
  try {
    unlinkSync(path);
    return null;
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    return error;
  }
}

function releaseCoverageLock(lock) {
  const current = readCoverageLock(lock.path);
  if (current.pid !== lock.record.pid || current.token !== lock.record.token) {
    throw new Error(`coverage lock ownership changed; refusing removal: ${lock.path}`);
  }
  unlinkSync(lock.path);
}

function acquireCoverageLock(root) {
  const path = join(root, COVERAGE_LOCK_FILE);
  const owner = createCoverageLockCandidate(root);
  const deadline = Date.now() + COVERAGE_LOCK_WAIT_MS;
  let acquired = false;
  try {
    while (!acquired) {
      try {
        linkSync(owner.candidate, path);
        acquired = true;
      } catch (error) {
        if (error?.code !== 'EEXIST') throw error;
        let current;
        try { current = readCoverageLock(path); } catch (readError) {
          if (readError?.code === 'ENOENT') continue;
          throw readError;
        }
        if (!coverageLockOwnerAlive(current.pid)) {
          throw new Error(`stale coverage lock retained for manual recovery: ${path}`);
        }
        if (Date.now() >= deadline) {
          throw new Error(`timed out waiting for live coverage lock owner ${current.pid}`);
        }
        Atomics.wait(coverageLockWaitCell, 0, 0, COVERAGE_LOCK_POLL_MS);
      }
    }
    const cleanupError = removeCandidate(owner.candidate);
    if (cleanupError) throw cleanupError;
    return { path, record: owner.record };
  } catch (error) {
    const cleanupError = removeCandidate(owner.candidate);
    if (acquired) {
      try { releaseCoverageLock({ path, record: owner.record }); }
      catch (releaseError) { throw new AggregateError([error, releaseError], error.message); }
    }
    if (cleanupError) throw new AggregateError([error, cleanupError], error.message);
    throw error;
  }
}

function restoreCoverageArtifacts(entries, root) {
  const errors = [];
  for (const entry of [...entries].reverse()) {
    try {
      assertCoveragePathParents(root, entry.path, 'coverage artifact path');
      if (existsSync(entry.path)) throw new Error(`restore target exists: ${entry.path}`);
      renameSync(entry.backup, entry.path);
    } catch (error) {
      errors.push(error);
    }
  }
  return errors;
}

function coverageArtifactExists(path, root) {
  try {
    assertCoveragePathParents(root, path, 'coverage artifact path');
    lstatSync(path);
    return true;
  } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
}

function preserveCoverageArtifacts(paths, root) {
  const present = paths.filter((path) => coverageArtifactExists(path, root));
  if (present.length === 0) return { directory: null, entries: [] };
  const directory = mkdtempSync(join(root, '.forge-coverage-backup-'));
  const entries = [];
  try {
    for (const [index, path] of present.entries()) {
      const backup = join(directory, `artifact-${index}`);
      renameSync(path, backup);
      entries.push({ path, backup });
    }
  } catch (error) {
    const errors = [error, ...restoreCoverageArtifacts(entries, root)];
    try { rmdirSync(directory); } catch (cleanupError) { errors.push(cleanupError); }
    throw new AggregateError(errors, `coverage artifact preservation failed; inspect ${directory}`);
  }
  return { directory, entries };
}

function finalizeCoverageArtifacts(paths, preserved, root) {
  const errors = [];
  for (const path of paths) {
    try {
      assertCoveragePathParents(root, path, 'coverage artifact path');
      rmSync(path, { recursive: true, force: true });
    } catch (error) { errors.push(error); }
  }
  errors.push(...restoreCoverageArtifacts(preserved.entries, root));
  if (preserved.directory && errors.length === 0) {
    try { rmdirSync(preserved.directory); } catch (error) { errors.push(error); }
  }
  return errors;
}

function runCoverageArtifactTransaction(paths, root, operation) {
  const preserved = preserveCoverageArtifacts(paths, root);
  let value;
  let operationError;
  try {
    value = operation();
  } catch (error) {
    operationError = error;
  }
  const errors = finalizeCoverageArtifacts(paths, preserved, root);
  if (!operationError && errors.length === 0) return value;
  if (operationError && errors.length === 0) throw operationError;
  if (operationError) errors.unshift(operationError);
  const location = preserved.directory ? `; inspect ${preserved.directory}` : '';
  throw new AggregateError(errors, `coverage artifact restoration failed closed${location}`);
}

function runPreservingCoverageArtifacts(paths, root, operation) {
  const lock = acquireCoverageLock(root);
  let value;
  let transactionError;
  try {
    value = runCoverageArtifactTransaction(paths, root, operation);
  } catch (error) {
    transactionError = error;
  }
  let releaseError;
  try { releaseCoverageLock(lock); } catch (error) { releaseError = error; }
  if (transactionError && releaseError) {
    throw new AggregateError(
      [transactionError, releaseError], 'coverage transaction and lock release failed closed');
  }
  if (transactionError) throw transactionError;
  if (releaseError) throw releaseError;
  return value;
}

function withGoCoverageTotal(lang, runResult, artifactRels, paths, root, exec) {
  if (lang !== 'go' || !runResult?.ok || paths.length !== 1) return runResult;
  let info;
  try {
    assertCoveragePathParents(root, paths[0], 'coverage artifact path');
    info = lstatSync(paths[0]);
  } catch (error) {
    if (error?.code === 'ENOENT') return runResult;
    throw error;
  }
  if (!info.isFile() || info.isSymbolicLink()) return runResult;
  const total = exec('go', ['tool', 'cover', `-func=${artifactRels[0]}`], {}, root);
  if (!total.ok) return runResult;
  return { ...runResult, out: [runResult.out, total.out].filter(Boolean).join('\n') };
}

function withMachineCoverage(lang, runResult, report, root) {
  if (lang !== 'python' && lang !== 'typescript') return runResult;
  try {
    return { ...runResult, coveragePercent: readCoverageReportPercent(lang, root, report) };
  } catch (error) {
    return { ...runResult, coverageError: error.message };
  }
}

// Side-effect discipline: coverage commands WRITE report artifacts into the
// working tree (pytest-cov writes both .coverage and coverage.json). Existing
// files or directories are atomically moved into a private sibling directory,
// the real command runs, and only its known outputs are removed before the
// originals are renamed back. Any preservation/cleanup/restore error throws,
// preventing an incomplete probe from becoming an accepting N/A verdict.
function probeCoverageLang(lang, threshold, root, exec) {
  const { coverage } = loadAdapter(lang);
  const reportRel = loadAdapterDocument(lang)?.commands?.coverage?.report;
  const bin = coverageBinary(coverage);
  if (!bin) return judgeCoverage(lang, null, false, null, threshold);
  // Tool-aware install probe: `go version`, else `<bin> --version` (see
  // versionProbeArgs — `go --version` would falsely read as "not installed").
  const environment = lang === 'python' ? { PYTHONDONTWRITEBYTECODE: '1' } : {};
  const installed = exec(bin, versionProbeArgs(bin), environment, root).ok;
  if (!installed) return judgeCoverage(lang, bin, false, null, threshold);
  const artifactRels = coverageArtifacts(coverage);
  const machineReport = lang === 'python' || lang === 'typescript';
  const missingReport = machineReport && typeof reportRel !== 'string';
  const layout = resolveCoverageLayout(
    root, artifactRels, machineReport && !missingReport ? reportRel : null);
  const [cmd, args] = splitCmd(coverage);
  const r = runPreservingCoverageArtifacts(layout.artifacts, root, () => {
    const covered = exec(cmd, args, environment, root);
    if (missingReport) return { ...covered, coverageError: 'adapter has no report path' };
    const withTotal = withGoCoverageTotal(
      lang, covered, artifactRels, layout.artifacts, root, exec);
    return withMachineCoverage(lang, withTotal, layout.report, root);
  });
  return judgeCoverage(lang, bin, true, r, threshold);
}

// probeCoverage: aggregate per-language coverage into the single `coverage`
// criterion (mirrors probeLint). N/A when no source languages are detected OR
// every detected language is N/A (no installed/runnable coverage tool). FAIL if
// any language is below threshold. Otherwise PASS.
export function probeCoverage(root = ROOT, exec = run) {
  const langs = detectLanguages(root);
  if (langs.length === 0) return result('coverage', NA, 'no source languages detected');
  // The line-coverage floor is the project's mode×lifecycle threshold (central knob:
  // .agent/project.yml × modes.yml — see resolveCoverageThreshold for the resolution,
  // its missing-file FAIL-SAFE, and why an N/A here is unaffected), NOT a hardcoded 60.
  const threshold = resolveCoverageThreshold(root);
  const per = langs.map((lang) => probeCoverageLang(lang, threshold, root, exec));
  const detail = per.map((p) => p.detail).join('; ');
  if (per.some((p) => p.status === FAIL)) return result('coverage', FAIL, detail);
  if (per.some((p) => p.status === NA)) return result('coverage', NA, detail);
  return result('coverage', PASS, detail);
}
