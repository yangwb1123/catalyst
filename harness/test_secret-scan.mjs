// Tests for harness/secret-scan.mjs (node:test, zero external deps).
// Run: node --test harness/test_secret-scan.mjs   (or: node --test harness/)
//
// Layers, mirroring the rest of the harness:
//   1. unit — the PURE matcher (scanText/scanLines/matchLine) over crafted
//      fixtures: POSITIVE (a known secret shape IS detected) + NEGATIVE
//      (ordinary code is NOT) + the `secret-scan:ignore` suppression.
//   2. integration — drive the real CLI against the real repo and assert it is
//      CLEAN (exit 0, 0 findings): the repo must ship no hardcoded secret.
//
// SELF-SCAN SAFETY: every secret-shaped fixture below is BUILT FROM STRING
// PIECES (concatenation), so NO full secret literal exists in THIS file — the
// live `node harness/secret-scan.mjs` therefore stays clean against its own
// test. Where a test needs an actual inline literal (the ignore test), the line
// carries the `secret-scan:ignore` marker so the live scan also exempts it.
// Belt and suspenders: concatenation defeats the matcher, the marker proves the
// suppression path, and neither lets this file trip the repo scan.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import * as scanner from './secret-scan.mjs';
const { scanText, scanLines, matchLine, walkFiles, scanRepo, PATTERNS } = scanner;

const SCAN_PATH = join(dirname(fileURLToPath(import.meta.url)), 'secret-scan.mjs');

// Secret-shaped fixtures, assembled from pieces so the assembled string only
// exists at RUNTIME (never as a source literal the live scanner could read).
const AWS_KEY = 'AKIA' + 'ABCDEFGHIJ123456'; // AKIA + 16 upper-alnum
const GH_TOKEN = 'ghp_' + 'aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789'; // ghp_ + 36
const PEM_HEADER = '-----BEGIN ' + 'RSA PRIVATE KEY-----';
const SLACK = 'xoxb' + '-1234567890-abcdefghij';
const HI_ENTROPY = 'A1b2C3d4E5f6G7h8I9j0KLMN'; // 24 base64-ish chars

// --- smoke: importing secret-scan.mjs must have no side effects --------------
test('importing secret-scan.mjs produces no output and exits 0 (no side effects)', () => {
  const specifier = pathToFileURL(SCAN_PATH).href;
  const res = spawnSync(
    process.execPath,
    ['-e', `import(${JSON.stringify(specifier)})`],
    { encoding: 'utf8' },
  );
  assert.equal(res.status, 0, `exit 0 expected; stderr:\n${res.stderr}`);
  assert.equal(res.stdout, '', `import must print nothing; got:\n${res.stdout}`);
  assert.equal(typeof scanText, 'function');
  assert.equal(typeof scanRepo, 'function');
});

// --- POSITIVE: each known pattern is detected --------------------------------
test('POSITIVE: an AWS access key id (AKIA…) is detected', () => {
  const f = scanText(`const id = "${AWS_KEY}";`);
  assert.equal(f.length, 1);
  assert.equal(f[0].type, 'aws-access-key-id');
  assert.equal(f[0].line, 1);
  assert.equal(f[0].match, AWS_KEY);
});

test('POSITIVE: a PEM private-key header is detected', () => {
  const f = scanText(`line one\n${PEM_HEADER}\nMIIEv...`);
  assert.equal(f.length, 1);
  assert.equal(f[0].type, 'private-key');
  assert.equal(f[0].line, 2, 'reports the 1-based line of the header');
});

test('POSITIVE: a GitHub token (ghp_…) is detected', () => {
  const f = scanText(`token=${GH_TOKEN}`);
  assert.equal(f.length, 1);
  assert.equal(f[0].type, 'github-token');
});

test('POSITIVE: a Slack token (xoxb-…) is detected', () => {
  const f = scanText(`SLACK="${SLACK}"`);
  assert.equal(f.length, 1);
  assert.equal(f[0].type, 'slack-token');
});

test('POSITIVE: a high-entropy secret assignment is detected', () => {
  const f = scanText(`api_key = "${HI_ENTROPY}"`);
  assert.equal(f.length, 1);
  assert.equal(f[0].type, 'generic-secret-assignment');
});

test('POSITIVE: multiple distinct secrets across lines are each reported', () => {
  const f = scanText([`a = "${AWS_KEY}"`, 'ordinary();', `t = ${GH_TOKEN}`].join('\n'));
  assert.equal(f.length, 2);
  assert.deepEqual(f.map((x) => x.line), [1, 3]);
  assert.deepEqual(f.map((x) => x.type).sort(), ['aws-access-key-id', 'github-token']);
});

// --- NEGATIVE: ordinary code must NOT be flagged (low false positive) --------
test('NEGATIVE: ordinary source code produces zero findings', () => {
  const code = [
    'import { readFileSync } from "node:fs";',
    'const greeting = "hello, world";',
    'function add(a, b) { return a + b; }',
    'const url = "https://example.com/path?q=1";',
    'const list = [1, 2, 3, 4, 5];',
  ].join('\n');
  assert.deepEqual(scanText(code), []);
});

test('NEGATIVE: a long NON-secret quoted string (no secret-ish key) is NOT flagged', () => {
  // The generic rule REQUIRES a secret-ish key name — an arbitrary long quoted
  // string assigned to a benign name must not trip it (this is the core
  // low-false-positive guard).
  const code = 'const description = "this is a fairly long human readable sentence value";';
  assert.deepEqual(scanText(code), []);
});

test('NEGATIVE: a short secret-key value (below the entropy length) is NOT flagged', () => {
  // `password = "short"` is too short for the >=20-char generic rule — we accept
  // the false negative rather than flag every `password =` line (low noise).
  assert.deepEqual(scanText('password = "hunter2"'), []);
});

test('NEGATIVE: a malformed AWS-ish string (wrong length) is NOT flagged', () => {
  assert.deepEqual(scanText('const x = "AKIATOOSHORT";'), []);
});

// --- suppression: secret-scan:ignore (inline + previous line) ----------------
test('IGNORE inline: a line carrying the marker is exempt', () => {
  // Real inline literal kept clean for the LIVE repo scan by the marker on the
  // same line (the previous-line form is covered by the next test).
  const id = 'AKIA' + 'IGNOREDKEY1234567'; // assembled at runtime
  const line = `const id = "${id}"; // ${'secret-scan'}:ignore`;
  assert.deepEqual(scanLines([line]), [], 'inline marker suppresses the finding');
});

test('IGNORE previous-line: a marker on the line ABOVE exempts the secret', () => {
  const marker = `// ${'secret-scan'}:ignore example fixture below`;
  const secret = `const k = "${GH_TOKEN}";`;
  assert.deepEqual(scanLines([marker, secret]), [], 'previous-line marker suppresses it');
});

test('IGNORE is scoped: the marker does NOT exempt OTHER lines', () => {
  const lines = [
    `// ${'secret-scan'}:ignore`,         // 1 — marker
    `a = "${AWS_KEY}"`,                    // 2 — exempt (prev line is marker)
    `b = "${GH_TOKEN}"`,                   // 3 — NOT exempt (line 2 is not a marker)
  ];
  const f = scanLines(lines);
  assert.equal(f.length, 1, 'only the secret two lines below the marker is reported');
  assert.equal(f[0].line, 3);
  assert.equal(f[0].type, 'github-token');
});

// --- matchLine first-match-wins (one finding per line, low noise) ------------
test('matchLine returns the FIRST matching pattern and null for clean lines', () => {
  assert.equal(matchLine('nothing to see here'), null);
  const hit = matchLine(`x = "${AWS_KEY}"`);
  assert.equal(hit.type, 'aws-access-key-id');
});

// --- PATTERNS sanity: rules are well-formed regexes --------------------------
test('every PATTERNS entry has a type and a RegExp', () => {
  assert.ok(PATTERNS.length >= 5, 'at least the five documented patterns');
  for (const p of PATTERNS) {
    assert.equal(typeof p.type, 'string');
    assert.ok(p.re instanceof RegExp, `${p.type} must carry a RegExp`);
  }
});

// --- I/O boundary: walkFiles skips SKIP_DIRS ---------------------------------
test('walkFiles excludes SKIP_DIRS (no node_modules/.git paths leak through)', () => {
  const files = walkFiles(dirname(SCAN_PATH));
  assert.ok(files.length > 0, 'finds at least the harness sources');
  assert.ok(
    !files.some((f) => /[\\/](node_modules|\.git|\.forge)[\\/]/.test(f)),
    'no skipped directory leaks into the walk',
  );
});

// --- integration: the REAL repo must be clean (exit 0, 0 findings) -----------
// This is the dogfood guard: if anyone commits a real hardcoded secret, this
// FAILS (and so does `forge accept`, since security_findings is load-bearing).
test('scanRepo over the real repo finds ZERO hardcoded secrets', () => {
  const { findings, fileCount } = scanRepo();
  assert.ok(fileCount > 0, 'the repo walk must scan at least one file');
  assert.deepEqual(
    findings, [],
    `repo must ship no hardcoded secret; got:\n${JSON.stringify(findings, null, 2)}`,
  );
});

test('the secret-scan CLI exits 0 with a PASS line on the clean repo', () => {
  const res = spawnSync(process.execPath, [SCAN_PATH], { encoding: 'utf8' });
  assert.equal(res.status, 0, `expected exit 0; stdout:\n${res.stdout}\nstderr:\n${res.stderr}`);
  assert.match(res.stdout, /forge-secret-scan: PASS/);
});
