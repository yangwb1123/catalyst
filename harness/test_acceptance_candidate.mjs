// Focused regressions for portable candidate observation and the final
// post-rehash identity pass.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, truncateSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import {
  candidateFingerprint, candidateFingerprintPortable,
} from './acceptance-candidate.mjs';

const NO_GIT_ENV = { PATH: '' };

function makeRoot() {
  const root = mkdtempSync(join(tmpdir(), 'accept-candidate-test-'));
  mkdirSync(join(root, 'nested'));
  writeFileSync(join(root, 'nested', 'source.txt'), 'source-v1\n');
  return root;
}

test('portable fallback preserves the fingerprint contract and detects byte drift', () => {
  const root = makeRoot();
  try {
    const portable = candidateFingerprintPortable(root, NO_GIT_ENV);
    assert.equal(portable, candidateFingerprint(root, NO_GIT_ENV));
    assert.equal(portable.length, 64);
    writeFileSync(join(root, 'nested', 'source.txt'), 'source-v2\n');
    assert.notEqual(candidateFingerprintPortable(root, NO_GIT_ENV), portable);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('coverage artifacts are generated outputs, not candidate source', () => {
  const root = makeRoot();
  try {
    const before = candidateFingerprint(root, NO_GIT_ENV);
    writeFileSync(join(root, '.coverage'), 'generated data');
    writeFileSync(join(root, 'coverage.json'), '{"totals":{}}\n');
    writeFileSync(join(root, 'coverage.out'), 'mode: atomic\n');
    writeFileSync(join(root, '.forge-coverage-artifact.lock'), 'lock\n');
    mkdirSync(join(root, '.forge-coverage-backup-test'));
    assert.equal(candidateFingerprint(root, NO_GIT_ENV), before);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('portable fallback rejects a persistent symlink candidate', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  try {
    symlinkSync('nested/source.txt', join(root, 'linked-source'));
    assert.throws(
      () => candidateFingerprintPortable(root, NO_GIT_ENV),
      /candidate symlinks are unsupported/,
    );
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('final identity pass catches an early file changed during the second large read', {
  skip: process.platform !== 'linux', timeout: 10_000,
}, async () => {
  const root = makeRoot();
  const target = join(root, '000-target.txt');
  const large = join(root, '001-large.bin');
  const ready = join(root, 'watcher-ready');
  writeFileSync(target, 'before!');
  writeFileSync(large, '');
  truncateSync(large, 512 * 1024 * 1024);
  const watcher = spawn(process.execPath, ['-e', String.raw`
const { readdirSync, readlinkSync, writeFileSync } = require('node:fs');
const [pid, observed, target, ready] = process.argv.slice(1);
writeFileSync(ready, '');
let seen = false;
let closed = false;
const deadline = Date.now() + 8_000;
while (Date.now() < deadline) {
  let found = false;
  for (const fd of readdirSync('/proc/' + pid + '/fd')) {
    try {
      if (readlinkSync('/proc/' + pid + '/fd/' + fd) === observed) {
        found = true;
        break;
      }
    } catch { /* descriptor closed between listing and inspection */ }
  }
  if (found && closed) {
    writeFileSync(target, 'changed');
    process.exit(0);
  }
  if (found) seen = true;
  else if (seen) closed = true;
}
process.exit(9);
`, String(process.pid), large, target, ready], { stdio: 'ignore' });
  const closed = new Promise((resolve) => watcher.once('close', resolve));
  const waitCell = new Int32Array(new SharedArrayBuffer(4));
  for (let attempt = 0; attempt < 1_000 && !existsSync(ready); attempt += 1) {
    Atomics.wait(waitCell, 0, 0, 1);
  }
  assert.equal(existsSync(ready), true, 'watcher must be ready before fingerprinting');
  let thrown = null;
  try { candidateFingerprint(root, NO_GIT_ENV); }
  catch (error) { thrown = error; }
  let code = await Promise.race([
    closed,
    new Promise((resolve) => setTimeout(() => resolve('timeout'), 1_000).unref()),
  ]);
  if (code === 'timeout') {
    watcher.kill('SIGKILL');
    code = await closed;
  }
  try {
    assert.equal(code, 0, 'watcher must change the target during the second large read');
    assert.match(thrown?.message ?? '', /candidate entry changed after revalidation/);
    assert.equal(readFileSync(target, 'utf8'), 'changed');
  } finally { rmSync(root, { recursive: true, force: true }); }
});
