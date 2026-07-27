#!/usr/bin/env node
// ForgeOS secret-scan — a real, zero-dependency hardcoded-secret scanner.
// Backs the acceptance gate's `security_findings` criterion (direction 5,
// "security/compliance gate"), aligned with OWASP Agentic Top-10 2025-12
// "sensitive information disclosure". Until now `security_findings` was N/A
// (no scanner existed); this turns it into a real check.
//
// HONESTY — what this IS and is NOT:
//   * IS: a PATTERN matcher (regex + a light shape/entropy heuristic) for
//     HARDCODED secrets — committed credentials a grep could find (AWS keys,
//     private-key PEM headers, GitHub/Slack tokens, high-entropy api_key/secret
//     assignments). This is the class OWASP flags as accidental disclosure.
//   * IS NOT: a dependency/CVE scanner. Software-composition analysis (SCA) needs
//     a vulnerability database (e.g. OSV/NVD) and is explicitly OUT of scope here
//     — that is the v3 roadmap item, not this gate.
//   * FALSE NEGATIVES are expected and acknowledged: an OBFUSCATED or
//     base64/rot-encoded secret, a secret split across concatenated string
//     pieces, or a credential in a format we don't pattern WILL be missed. We
//     bias hard toward LOW FALSE POSITIVES (a noisy scanner that cries wolf gets
//     muted, drowning the real signal) — so we'd rather miss a cleverly-hidden
//     secret than flag every long string. A fuller entropy/AST scanner is a
//     later enhancement; this is the honest v0.
//
// Design mirrors the rest of the harness: a PURE core (scanText / scanLines)
// that takes text and returns findings, kept separate from the I/O boundary
// (walkFiles / scanRepo) so the matching logic is unit-testable without a disk.
//
// Suppression: a line carrying `secret-scan:ignore` (inline) OR whose PREVIOUS
// line carries it is exempt — this is how a test FIXTURE's example secret (which
// must exist as a literal to prove the matcher fires) is kept from tripping the
// scanner against its own test file. Use sparingly; every ignore is a hole.
//
// CLI: `node harness/secret-scan.mjs`        -> exit 0 clean · exit 1 findings.
//      `node harness/secret-scan.mjs --json` -> findings as JSON to stdout.
//   Fail-CLOSED: an unexpected scanner error is REPORTED and exits 2 (never a
//   silent green) so a broken scanner cannot masquerade as "no secrets".
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname, basename, relative, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(HARNESS_DIR);

// Directories never worth scanning — same exclusions as arch/scan.mjs
// (vcs / build output / vendored / fixtures / forge-core runtime state).
export const SKIP_DIRS = new Set([
  'node_modules', '.git', 'dist', 'build', 'target', '.next', 'coverage',
  'vendor', 'testdata', '__pycache__', '.forge',
]);

// File extensions worth scanning for embedded secrets: source + config/docs
// where a credential is plausibly pasted. Binary/asset types are skipped.
// NOTE: NO '.env' here — `extname('.env') === ''` (it is a basename, not an
// extension), so listing it never matched anything. Extensionless credential
// files are gated by SCAN_FILENAMES below instead.
const SCAN_EXTS = new Set([
  '.go', '.mjs', '.cjs', '.js', '.jsx', '.ts', '.tsx', '.py', '.rb', '.rs',
  '.java', '.php', '.sh', '.bash', '.zsh', '.yml', '.yaml', '.json',
  '.toml', '.ini', '.cfg', '.conf', '.txt', '.md', '.xml', '.properties',
]);

// Credential-bearing files with NO scannable extension — `extname()` returns ''
// for these, so the SCAN_EXTS gate skipped them entirely (a structural false
// negative: `.env` is the FIRST place a leaked secret lands). Matched by BASENAME
// instead. `.env.local` / `.env.production` etc. are caught by the startsWith
// check in scannableName, so only the exact extras need listing here.
const SCAN_FILENAMES = new Set([
  '.env', '.npmrc', 'Dockerfile', 'credentials', 'id_rsa',
]);

// scannableName: a file is scanned when its extension is in SCAN_EXTS OR its
// basename is a known credential file (exact match in SCAN_FILENAMES, or any
// `.env*` variant). Pure over the path so the gate is unit-testable.
export function scannableName(full) {
  const base = basename(full);
  return SCAN_EXTS.has(extname(full)) || SCAN_FILENAMES.has(base) || base.startsWith('.env');
}

// Inline / previous-line suppression marker.
const IGNORE_MARKER = 'secret-scan:ignore';

// --- detection patterns ------------------------------------------------------
// Each rule: { type, re }. Bias to LOW FALSE POSITIVE — anchored, specific
// shapes for known providers; the one generic rule requires BOTH a
// secret-ish KEY name AND a quoted high-entropy-LENGTH value, so an ordinary
// `const name = "hello"` never matches.
//
// NOTE: the patterns below are described in comments using SPLIT/!-broken forms
// (e.g. "AKIA" + 16 chars) on purpose, so this very source file contains no
// literal that matches its own rules — the scanner stays clean against itself.
export const PATTERNS = [
  // AWS access key id: literal "AKIA" + exactly 16 uppercase-alnum chars.
  { type: 'aws-access-key-id', re: /\bAKIA[0-9A-Z]{16}\b/ },
  // PEM private-key header: "-----BEGIN [...]PRIVATE KEY-----".
  { type: 'private-key', re: /-----BEGIN (?:[A-Z0-9]+ )?PRIVATE KEY-----/ },
  // GitHub personal access token: "ghp_" + 36 url-safe chars.
  { type: 'github-token', re: /\bghp_[A-Za-z0-9]{36}\b/ },
  // Slack token: "xox" + one of b/a/p/r/s + "-".
  { type: 'slack-token', re: /\bxox[baprs]-[A-Za-z0-9-]{8,}/ },
  // Generic high-entropy assignment: a secret-ish key name assigned a quoted
  // value of >=20 base64-ish chars. Requires the KEY name to gate false
  // positives (an arbitrary long quoted string alone is NOT flagged).
  {
    type: 'generic-secret-assignment',
    re: /(?:api[_-]?key|secret|token|password|passwd|pwd)["']?\s*[:=]\s*["'][A-Za-z0-9+/_-]{20,}={0,2}["']/i,
  },
];

// --- pure core (no I/O) ------------------------------------------------------

// hasIgnore: does this line carry the inline suppression marker?
function hasIgnore(line) {
  return typeof line === 'string' && line.includes(IGNORE_MARKER);
}

// matchLine: return the FIRST pattern that hits this line, as {type, match}, or
// null. First-match-wins keeps a line to one finding (low noise); order in
// PATTERNS therefore puts specific provider shapes before the generic rule.
export function matchLine(line) {
  for (const { type, re } of PATTERNS) {
    const m = re.exec(line);
    if (m) return { type, match: m[0] };
  }
  return null;
}

// scanLines: PURE. Given an array of source lines, return findings
// [{line, type, match}] (1-based line numbers). A line is EXEMPT when it OR the
// line directly above it carries the ignore marker — the previous-line form lets
// a fixture annotate the secret on its own comment line.
export function scanLines(lines) {
  const findings = [];
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (hasIgnore(line)) continue;
    if (i > 0 && hasIgnore(lines[i - 1])) continue;
    const hit = matchLine(line);
    if (hit) findings.push({ line: i + 1, type: hit.type, match: hit.match });
  }
  return findings;
}

// scanText: PURE convenience wrapper — split text into lines and scan. This is
// the unit-test entry point: `scanText(fixtureWithSecret)` -> findings.
export function scanText(text) {
  return scanLines(String(text).split('\n'));
}

// --- I/O boundary ------------------------------------------------------------

// walkFiles: recursively collect scannable file paths under root, skipping
// SKIP_DIRS and non-scannable extensions. Tolerant of broken symlinks /
// permission errors (skipped, never crash) — same shape as arch/scan.walkSource.
export function walkFiles(root, acc = []) {
  let entries;
  try {
    entries = readdirSync(root);
  } catch {
    return acc;
  }
  for (const name of entries) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(root, name);
    let st;
    try { st = statSync(full); } catch { continue; }
    if (st.isDirectory()) walkFiles(full, acc);
    else if (scannableName(full)) acc.push(full);
  }
  return acc;
}

// scanFile: read one file and return its findings tagged with the relative path.
// Unreadable files are reported as a scan ERROR (fail-closed) rather than
// silently skipped — a file we cannot read might be the one hiding a secret.
function scanFile(file, root) {
  const rel = relative(root, file);
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch (err) {
    return [{ file: rel, line: 0, type: 'scan-error', match: String(err.message) }];
  }
  return scanText(text).map((f) => ({ file: rel, ...f }));
}

// scanRepo: walk `root` and aggregate findings across every scannable file.
// Returns { findings, fileCount }.
export function scanRepo(root = ROOT) {
  const files = walkFiles(root);
  const findings = [];
  for (const file of files) findings.push(...scanFile(file, root));
  return { findings, fileCount: files.length };
}

// --- CLI ---------------------------------------------------------------------

function render(findings, fileCount) {
  if (findings.length === 0) {
    return `forge-secret-scan: PASS (${fileCount} files scanned, 0 hardcoded secrets)`;
  }
  const lines = findings.map((f) => `    ${f.file}:${f.line} ${f.type}`);
  return [
    `forge-secret-scan: FAIL — ${findings.length} potential secret(s):`,
    ...lines,
    '  (hardcoded-secret patterns; suppress a true fixture with `secret-scan:ignore`)',
  ].join('\n');
}

function main() {
  let report;
  try {
    report = scanRepo(ROOT);
  } catch (err) {
    // Fail-closed: a scanner crash is REPORTED and exits 2, never a silent pass.
    console.error(`forge-secret-scan: ERROR — ${err && err.stack ? err.stack : err}`);
    process.exit(2);
  }
  const { findings, fileCount } = report;
  if (process.argv.slice(2).includes('--json')) {
    console.log(JSON.stringify(findings));
    process.exit(findings.length === 0 ? 0 : 1);
  }
  console.log(render(findings, fileCount));
  process.exit(findings.length === 0 ? 0 : 1);
}

// Run only when executed directly, not on import (keeps the module test-safe).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
