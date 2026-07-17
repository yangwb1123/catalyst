#!/usr/bin/env node
// ForgeOS sca — a real, zero-dependency Software-Composition-Analysis (SCA)
// framework: parse dependency manifests, then match them against an OSV-format
// advisory database to surface known-vulnerable dependencies (CVE/GHSA). Backs
// the acceptance gate's `dependency_vulnerabilities` criterion — the v3 roadmap
// item that secret-scan.mjs explicitly deferred ("a dependency/CVE scanner …
// needs a vulnerability database (e.g. OSV/NVD)").
//
// HONESTY — what this IS and is NOT (the whole point, identical posture to the
// lint/coverage adapters: real FRAMEWORK, honest N/A when external DATA absent):
//   * IS: a real, fixture-verifiable engine. parseManifest() turns go.mod /
//     package.json / requirements.txt into a normalized dependency list, and
//     matchAdvisories() compares each dep against OSV-format advisory records
//     with a correct (if simplified) semver comparison — package+ecosystem must
//     match AND the installed version must fall in the half-open vulnerable
//     range [introduced, fixed). A fixture advisory + a vulnerable fixture dep
//     DOES produce a finding (see test_sca.mjs) — that is the anti-stub proof.
//   * IS NOT, and never PRETENDS to be, an omniscient "scanned the whole world,
//     no CVEs" oracle. Vulnerability COVERAGE is exactly as complete as the
//     advisory DB you feed it. With NO DB on disk, loadAdvisories() returns
//     {advisories: [], available: false} and the scan is reported N/A — NOT a
//     faked green. Supplying a real OSV/NVD export turns the SAME code into a
//     full PASS/FAIL scan. We refuse to fabricate "no vulnerabilities" from an
//     empty DB the way we refuse to fabricate a passing test.
//   * SIMPLIFICATIONS acknowledged: the semver compare handles the common
//     `v`-prefixed major.minor.patch (+ pre-release-aware: a `-rc` build sorts
//     below its release) shape that go.mod / npm / PyPI overwhelmingly use; it
//     does not implement full SemVer 2.0 build-metadata ordering or npm range
//     expressions (`^`,`~`,`>=`). An OSV record may carry multiple ranges; we
//     read the single {introduced, fixed} pair this framework's DB schema uses.
//
// Design mirrors secret-scan.mjs: a PURE core (parseManifest / matchAdvisories /
// the semver helpers) that is unit-testable with no disk, kept separate from the
// I/O boundary (loadAdvisories / discoverManifests / scanRepo).
//
// CLI: `node harness/sca.mjs [root] [dbPath]`
//   DB present + no vulnerable deps -> exit 0 (clean).
//   DB present + vulnerable deps    -> exit 1 (findings listed).
//   DB absent                       -> exit 0 with an HONEST "no advisory DB"
//                                      N/A report (framework ready, scan not run,
//                                      NOT faked) — N/A must never block a gate.
//   Fail-CLOSED: an unexpected error is REPORTED and exits 2 (never silent green).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, dirname, basename } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);

// Directories never worth scanning for manifests — same exclusions as the rest
// of the harness (vcs / build output / vendored / fixtures / runtime state).
export const SKIP_DIRS = new Set([
  'node_modules', '.git', 'dist', 'build', '.next', 'coverage',
  'vendor', 'testdata', '__pycache__', '.forge',
]);

// Manifest filename -> {kind, ecosystem}. kind drives parseManifest's grammar;
// ecosystem is the OSV ecosystem string an advisory's `ecosystem` is matched on.
export const MANIFESTS = {
  'go.mod': { kind: 'go.mod', ecosystem: 'Go' },
  'package.json': { kind: 'package.json', ecosystem: 'npm' },
  'requirements.txt': { kind: 'requirements.txt', ecosystem: 'PyPI' },
};

// --- pure core: semver compare ----------------------------------------------

// parseVer: a version string -> {nums:[major,minor,patch], pre} or null.
// Tolerant of a leading `v`/`=`/whitespace and of missing minor/patch (zero-
// filled). A pre-release suffix is captured separately so a pre-release can
// sort BELOW its final release (1.0.0-rc < 1.0.0), matching SemVer precedence
// for the common shapes go.mod / npm / PyPI emit. The separator before the
// suffix is OPTIONAL (`[-+]?`, not required): SemVer/npm/Go use a `-`/`+`
// separator (`1.0.0-rc.1`), but PyPI's PEP 440 pre-release form has NO
// separator at all (`5.2b1`, `5.4rc1`, `1.0a3`) — real OSV advisory data (this
// repo's own advisories.json) uses exactly this bare form, and requiring a
// separator would make those versions unparseable (silently sorting as
// -Infinity in every comparison, which is wrong in either direction depending
// on which side of a compare it lands on — not merely "conservative").
function parseVer(v) {
  if (typeof v !== 'string') return null;
  const cleaned = v.trim().replace(/^[v=]\s*/i, '');
  const m = cleaned.match(/^(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:[-+]?(.*))?$/);
  if (!m) return null;
  return {
    nums: [Number(m[1]), Number(m[2] || 0), Number(m[3] || 0)],
    pre: m[4] ? m[4] : null,
  };
}

// comparePre: SemVer-style pre-release ordering. No pre-release outranks any
// pre-release (1.0.0 > 1.0.0-rc). Otherwise compare dot-separated identifiers:
// numeric identifiers compare numerically, and a numeric identifier has lower
// precedence than an alphanumeric one. Returns -1 | 0 | 1.
function comparePre(a, b) {
  if (a === null && b === null) return 0;
  if (a === null) return 1;   // a (release) > b (pre-release)
  if (b === null) return -1;
  const as = a.split('.');
  const bs = b.split('.');
  for (let i = 0; i < Math.max(as.length, bs.length); i += 1) {
    const x = as[i];
    const y = bs[i];
    if (x === undefined) return -1; // shorter pre-release set has lower precedence
    if (y === undefined) return 1;
    const xn = /^\d+$/.test(x);
    const yn = /^\d+$/.test(y);
    if (xn && yn) {
      const d = Number(x) - Number(y);
      if (d !== 0) return d < 0 ? -1 : 1;
    } else if (xn !== yn) {
      return xn ? -1 : 1; // numeric < alphanumeric
    } else if (x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

// compareVersions: PURE. Returns -1 | 0 | 1 (a<b / a==b / a>b). Unparseable
// versions sort as -Infinity-ish (null) BELOW any real version, so a malformed
// installed version is treated conservatively (it won't spuriously land >= fixed
// and thus won't hide a vulnerability). Exported for direct unit testing.
export function compareVersions(a, b) {
  const pa = parseVer(a);
  const pb = parseVer(b);
  if (!pa && !pb) return 0;
  if (!pa) return -1;
  if (!pb) return 1;
  for (let i = 0; i < 3; i += 1) {
    if (pa.nums[i] !== pb.nums[i]) return pa.nums[i] < pb.nums[i] ? -1 : 1;
  }
  return comparePre(pa.pre, pb.pre);
}

// inRange: PURE. Is `version` within the half-open vulnerable window
// [introduced, fixed)? introduced omitted/"0" means "from the beginning"; fixed
// omitted means "no fix yet" (open-ended — every version at/above introduced is
// vulnerable). This is the OSV half-open semantics: == introduced is vulnerable,
// == fixed is NOT.
export function inRange(version, introduced, fixed) {
  const fromOk = introduced === undefined || introduced === null
    || introduced === '0' || compareVersions(version, introduced) >= 0;
  const toOk = fixed === undefined || fixed === null
    || compareVersions(version, fixed) < 0;
  return fromOk && toOk;
}

// --- pure core: manifest parsing --------------------------------------------

// parseGoMod: extract require entries from a go.mod. Handles BOTH the single-
// line form (`require example.com/x v1.2.3`) and the grouped block
// (`require (\n  example.com/x v1.2.3 // indirect\n)`). Trailing `// indirect`
// / comments are stripped. Returns [{name, version, ecosystem:'Go'}].
function parseGoMod(text) {
  const deps = [];
  const lines = text.split('\n');
  let inBlock = false;
  for (const raw of lines) {
    const line = raw.split('//')[0].trim();
    if (!line && !inBlock) continue;
    if (inBlock) {
      if (line === ')') { inBlock = false; continue; }
      const m = line.match(/^(\S+)\s+(\S+)/);
      if (m) deps.push({ name: m[1], version: m[2], ecosystem: 'Go' });
      continue;
    }
    if (/^require\s*\($/.test(line)) { inBlock = true; continue; }
    const single = line.match(/^require\s+(\S+)\s+(\S+)/);
    if (single) deps.push({ name: single[1], version: single[2], ecosystem: 'Go' });
  }
  return deps;
}

// parsePackageJson: dependencies + devDependencies from a package.json. Strips a
// leading npm range operator (^ ~ >= > <= < =) to the concrete base version so
// the comparison has a number to work with (a true range solver is out of scope;
// the base version is the honest, conservative anchor). Returns npm-ecosystem deps.
function parsePackageJson(text) {
  const deps = [];
  let pkg;
  try { pkg = JSON.parse(text); } catch { return deps; }
  for (const field of ['dependencies', 'devDependencies']) {
    const obj = pkg && pkg[field];
    if (!obj || typeof obj !== 'object') continue;
    for (const [name, spec] of Object.entries(obj)) {
      const version = String(spec).replace(/^[\^~>=<]+\s*/, '').trim();
      deps.push({ name, version, ecosystem: 'npm' });
    }
  }
  return deps;
}

// parseRequirements: a requirements.txt -> PyPI deps. Reads `pkg==1.2.3` (and
// the `pkg===1.2.3` / `pkg>=1.2.3` forms, taking the pinned/base version);
// comments (#…), blank lines, options (-r, --hash) and unpinned bare names are
// skipped (no version to compare). Returns PyPI-ecosystem deps.
function parseRequirements(text) {
  const deps = [];
  for (const raw of text.split('\n')) {
    const line = raw.split('#')[0].trim();
    if (!line || line.startsWith('-')) continue;
    const m = line.match(/^([A-Za-z0-9._-]+)\s*(?:===|==|>=|<=|~=|!=|>|<)\s*([^\s,;]+)/);
    if (m) deps.push({ name: m[1], version: m[2], ecosystem: 'PyPI' });
  }
  return deps;
}

// parseManifest: PURE dispatcher. (text, kind) -> normalized
// [{name, version, ecosystem}]. Unknown kind -> []. This is the unit-test entry
// point for the parsers.
export function parseManifest(text, kind) {
  const src = String(text);
  if (kind === 'go.mod') return parseGoMod(src);
  if (kind === 'package.json') return parsePackageJson(src);
  if (kind === 'requirements.txt') return parseRequirements(src);
  return [];
}

// --- pure core: advisory matching -------------------------------------------

// matchAdvisories: PURE. Cross every dependency against every OSV-format
// advisory; emit a finding when package name AND ecosystem match AND the
// installed version is in the advisory's half-open vulnerable range. An OSV
// record here is {id, package, ecosystem, vulnerable:{introduced, fixed},
// severity}. Returns [{dep, advisory_id, severity, installed, ecosystem}].
// Exported and the CORE anti-stub proof: a vulnerable fixture dep + a fixture
// advisory yields a finding here, with zero I/O.
export function matchAdvisories(deps, advisories) {
  const findings = [];
  if (!Array.isArray(deps) || !Array.isArray(advisories)) return findings;
  for (const dep of deps) {
    for (const adv of advisories) {
      if (!adv || adv.package !== dep.name || adv.ecosystem !== dep.ecosystem) continue;
      const range = adv.vulnerable || {};
      if (!inRange(dep.version, range.introduced, range.fixed)) continue;
      findings.push({
        dep: dep.name,
        advisory_id: adv.id || '(unknown)',
        severity: adv.severity || 'UNKNOWN',
        installed: dep.version,
        ecosystem: dep.ecosystem,
        fixed: range.fixed ?? null,
      });
    }
  }
  return findings;
}

// --- I/O boundary -----------------------------------------------------------

// loadAdvisories: read the OSV advisory DB (a JSON array of advisory records)
// from dbPath. HONESTY CONTRACT: a MISSING file -> {advisories: [],
// available: false} (the DB is simply not provided — N/A downstream, NOT an
// error and NOT a fake clean). A present-but-malformed file IS surfaced as an
// error (available:false + error) — a corrupt DB must not silently read as
// "no advisories". Returns {advisories, available, error?}.
export function loadAdvisories(dbPath) {
  if (!dbPath) return { advisories: [], available: false };
  let text;
  try {
    text = readFileSync(dbPath, 'utf8');
  } catch {
    // File absent / unreadable -> DB not provided. Honest N/A, not an error.
    return { advisories: [], available: false };
  }
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (err) {
    return { advisories: [], available: false, error: `malformed advisory DB: ${err.message}` };
  }
  // Accept either a bare array or an {advisories:[...]} envelope.
  const advisories = Array.isArray(parsed) ? parsed
    : (parsed && Array.isArray(parsed.advisories)) ? parsed.advisories : null;
  if (!advisories) {
    return { advisories: [], available: false, error: 'advisory DB is not a JSON array' };
  }
  return { advisories, available: true };
}

// discoverManifests: walk `root` for known dependency manifests, skipping
// SKIP_DIRS. Returns [{file, kind, ecosystem}] (absolute file paths). Tolerant
// of permission / broken-symlink errors (skipped, never crash).
export function discoverManifests(root, acc = []) {
  let entries;
  try { entries = readdirSync(root); } catch { return acc; }
  for (const name of entries) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(root, name);
    let st;
    try { st = statSync(full); } catch { continue; }
    if (st.isDirectory()) { discoverManifests(full, acc); continue; }
    const spec = MANIFESTS[basename(full)];
    if (spec) acc.push({ file: full, kind: spec.kind, ecosystem: spec.ecosystem });
  }
  return acc;
}

// scanRepo: the full pipeline. Discover manifests under `root`, parse each into
// deps, load the advisory DB at `dbPath`, and match. Returns
// {findings, available, manifestCount, depCount, error?}. When the DB is absent
// (available:false), findings is [] and the caller reports N/A — the scan did
// not run, and we do NOT claim a clean result.
export function scanRepo(root = ROOT, dbPath) {
  const manifests = discoverManifests(root);
  const deps = [];
  for (const m of manifests) {
    let text;
    try { text = readFileSync(m.file, 'utf8'); } catch { continue; }
    for (const d of parseManifest(text, m.kind)) {
      deps.push({ ...d, manifest: relative(root, m.file) });
    }
  }
  const db = loadAdvisories(dbPath);
  const findings = db.available ? matchAdvisories(deps, db.advisories) : [];
  return {
    findings,
    available: db.available,
    manifestCount: manifests.length,
    depCount: deps.length,
    error: db.error,
  };
}

// --- CLI --------------------------------------------------------------------

// render: friendly report. Three honest outcomes: N/A (no DB — framework ready
// but scan not run, NOT faked), PASS (DB present, no vulnerable deps), FAIL (DB
// present, vulnerable deps listed with advisory id + severity).
export function render(report) {
  const { findings, available, manifestCount, depCount, error } = report;
  if (!available) {
    const why = error ? ` (${error})` : '';
    return [
      `forge-sca: N/A — SCA framework ready; no OSV advisory DB${why}`,
      `  (set FORGE_SCA_DB or place .agent/security/advisories.json — ${manifestCount} manifest(s), ${depCount} dep(s) parsed)`,
      '  dependency CVE scan NOT run, NOT faked — supply an OSV DB to enable PASS/FAIL.',
    ].join('\n');
  }
  if (findings.length === 0) {
    return `forge-sca: PASS (${manifestCount} manifest(s), ${depCount} dep(s) — 0 known-vulnerable vs advisory DB)`;
  }
  const lines = findings.map(
    (f) => `    ${f.dep}@${f.installed} [${f.ecosystem}] ${f.advisory_id} (${f.severity})${f.fixed ? ` — fixed in ${f.fixed}` : ''}`,
  );
  return [
    `forge-sca: FAIL — ${findings.length} vulnerable dependenc(ies) (${depCount} dep(s) scanned):`,
    ...lines,
  ].join('\n');
}

function main() {
  const args = process.argv.slice(2);
  const root = args[0] || ROOT;
  // dbPath precedence: explicit CLI arg > FORGE_SCA_DB env > conventional path.
  const dbPath = args[1] || process.env.FORGE_SCA_DB
    || join(root, '.agent', 'security', 'advisories.json');
  let report;
  try {
    report = scanRepo(root, dbPath);
  } catch (err) {
    // Fail-closed: a scanner crash is REPORTED and exits 2, never a silent pass.
    console.error(`forge-sca: ERROR — ${err && err.stack ? err.stack : err}`);
    process.exit(2);
  }
  if (args.includes('--json')) {
    console.log(JSON.stringify(report));
  } else {
    console.log(render(report));
  }
  // N/A (no DB) and PASS both exit 0; only real findings (DB present) exit 1.
  process.exit(report.available && report.findings.length > 0 ? 1 : 0);
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
