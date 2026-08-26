// Security and freshness tests for the explicitly advisory acceptance cache.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  chmodSync, existsSync, linkSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync,
  renameSync, symlinkSync, truncateSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import {
  CACHE_SCHEMA, CACHEABLE_PLANS, PROBE_INPUTS, cacheDisabled, cachedDecide,
  fingerprintProbe, fingerprintProbes, loadCache, saveRows, storePath,
} from './acceptance-cache.mjs';
import { candidateFingerprint } from './acceptance-candidate.mjs';

// Missing tools are still a stable identity and make these unit tests fast.
const CACHE_ENV = { PATH: '' };
const SOURCE_ONLY_CACHE_ROWS = [
  'arch_violations', 'architecture', 'complexity_violations', 'security_findings',
];

function makeRoot() {
  const root = mkdtempSync(join(tmpdir(), 'accept-cache-test-'));
  for (const dir of ['harness', 'forge-core', 'forge-runtime', 'examples', '.agent', 'docs']) {
    mkdirSync(join(root, dir), { recursive: true });
  }
  writeFileSync(join(root, 'harness', 'a.mjs'), 'export const a = 1;\n');
  writeFileSync(join(root, 'harness', 'b.py'), 'def b():\n    return 1\n');
  writeFileSync(join(root, 'forge-core', 'go.mod'), 'module x\n');
  writeFileSync(join(root, 'forge-runtime', 'Cargo.toml'), '[package]\nname = "x"\n');
  writeFileSync(join(root, 'examples', 'app.test.js'), 'test("x", () => {});\n');
  writeFileSync(join(root, '.agent', 'project.yml'), 'lifecycle: mvp\n');
  writeFileSync(join(root, 'docs', 'contract.md'), 'contract-v1\n');
  writeFileSync(join(root, 'README.md'), 'read me\n');
  return root;
}

function removeRoot(root) {
  rmSync(root, { recursive: true, force: true });
}

function fingerprint(name, root) {
  return fingerprintProbe(name, root, CACHE_ENV);
}

function pass(detail = 'clean') {
  return { status: 'PASS', detail, category: 'applicable' };
}

function saveComplete(root, detail = 'clean') {
  const names = Object.keys(CACHEABLE_PLANS);
  const fingerprints = fingerprintProbes(names, root, CACHE_ENV);
  return saveRows(root, names.map((name) => ({
    name, fingerprint: fingerprints.get(name), result: pass(`${detail}:${name}`),
  })));
}

test('fingerprints are stable and every cached probe covers repository source', () => {
  const root = makeRoot();
  try {
    assert.deepEqual(Object.keys(CACHEABLE_PLANS).sort(), SOURCE_ONLY_CACHE_ROWS);
    assert.equal(fingerprint('architecture', root), fingerprint('architecture', root));
    assert.equal(fingerprint('architecture', root).length, 64);
    for (const [name, input] of Object.entries(PROBE_INPUTS)) {
      assert.ok(CACHEABLE_PLANS[name]);
      assert.equal(input.scope, 'repository-source');
    }
  } finally { removeRoot(root); }
});

test('PyYAML package identity participates in arch cache fingerprints', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  const tools = mkdtempSync(join(tmpdir(), 'accept-cache-tools-'));
  const identity = join(tools, 'pyyaml-identity');
  try {
    writeFileSync(identity, 'pyyaml-v1\n');
    const python = join(tools, 'python3');
    writeFileSync(python, `#!/bin/sh
if [ "$1" = "--version" ]; then printf 'Python 3.12.0\n'; else cat ${JSON.stringify(identity)}; fi
`);
    chmodSync(python, 0o700);
    const env = { PATH: `${tools}:${process.env.PATH}` };
    const before = fingerprintProbe('arch_violations', root, env);
    writeFileSync(identity, 'pyyaml-v2\n');
    assert.notEqual(fingerprintProbe('arch_violations', root, env), before);
  } finally {
    removeRoot(root);
    removeRoot(tools);
  }
});

test('a non-repository candidate never inherits Git discovery from a parent', {
  skip: process.platform === 'win32',
}, () => {
  const parent = mkdtempSync(join(tmpdir(), 'accept-candidate-parent-'));
  const root = join(parent, 'child');
  const bin = join(parent, 'bin');
  const called = join(parent, 'git-called');
  try {
    mkdirSync(root);
    mkdirSync(bin);
    writeFileSync(join(root, 'README.md'), 'child\n');
    const fakeGit = join(bin, 'git');
    writeFileSync(fakeGit, `#!/bin/sh\n: > "$FORGE_GIT_CALLED"\nprintf 'README.md\\0'\n`);
    chmodSync(fakeGit, 0o700);
    assert.equal(candidateFingerprint(root, { PATH: bin, FORGE_GIT_CALLED: called }).length, 64);
    assert.equal(existsSync(called), false);
  } finally { removeRoot(parent); }
});

test('Git discovery scrubs location overrides, includes ignored inputs, and omits deletions', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  const bin = join(root, 'fake-bin');
  try {
    mkdirSync(join(root, '.git'));
    mkdirSync(bin);
    const fakeGit = join(bin, 'git');
    writeFileSync(join(root, 'ignored.js'), 'ignored-v1\n');
    writeFileSync(fakeGit, `#!/bin/sh
if [ -n "$GIT_DIR" ]; then exit 91; fi
case "$*" in
  *rev-parse*) printf '%s\n' "$FORGE_EXPECTED_ROOT" ;;
  *--deleted*) printf 'deleted.txt\\0' ;;
  *ls-files*) printf 'README.md\\0deleted.txt\\0' ;;
  *) exit 92 ;;
esac
`);
    chmodSync(fakeGit, 0o700);
    const env = {
      PATH: bin, GIT_DIR: join(root, 'elsewhere'), FORGE_EXPECTED_ROOT: root,
    };
    const value = candidateFingerprint(root, env);
    assert.equal(value.length, 64, 'a deleted tracked path must not fail fingerprinting');
    writeFileSync(join(root, 'ignored.js'), 'ignored-v2\n');
    assert.notEqual(candidateFingerprint(root, env), value);
  } finally { removeRoot(root); }
});

test('docs/contracts invalidate every cacheable advisory hint', () => {
  const root = makeRoot();
  const names = Object.keys(CACHEABLE_PLANS);
  try {
    const before = fingerprintProbes(names, root, CACHE_ENV);
    writeFileSync(join(root, 'docs', 'contract.md'), 'contract-v2\n');
    const after = fingerprintProbes(names, root, CACHE_ENV);
    for (const name of names) assert.notEqual(before.get(name), after.get(name), name);
  } finally { removeRoot(root); }
});

test('an identical-byte source replacement still invalidates candidate identity', () => {
  const root = makeRoot();
  try {
    const path = join(root, 'README.md');
    const replacement = join(root, 'README.replacement');
    const before = fingerprint('architecture', root);
    writeFileSync(replacement, readFileSync(path));
    renameSync(replacement, path);
    assert.notEqual(before, fingerprint('architecture', root));
  } finally { removeRoot(root); }
});

test('root-relative prefixes prevent cross-root content-swap collisions', () => {
  const root = makeRoot();
  try {
    mkdirSync(join(root, 'alpha'));
    mkdirSync(join(root, 'beta'));
    writeFileSync(join(root, 'alpha', 'same.txt'), 'A');
    writeFileSync(join(root, 'beta', 'same.txt'), 'B');
    const before = fingerprint('architecture', root);
    writeFileSync(join(root, 'alpha', 'same.txt'), 'B');
    writeFileSync(join(root, 'beta', 'same.txt'), 'A');
    assert.notEqual(before, fingerprint('architecture', root));
  } finally { removeRoot(root); }
});

test('empty candidate directories participate in the fingerprint', () => {
  const root = makeRoot();
  try {
    const before = fingerprint('architecture', root);
    mkdirSync(join(root, 'docs', 'empty-contract-dir'));
    assert.notEqual(before, fingerprint('architecture', root));
  } finally { removeRoot(root); }
});

test('vendored source participates in source-only scanner fingerprints', () => {
  const root = makeRoot();
  try {
    mkdirSync(join(root, 'vendor'));
    const dependency = join(root, 'vendor', 'dependency.js');
    writeFileSync(dependency, 'export const value = 1;\n');
    const before = fingerprint('architecture', root);
    writeFileSync(dependency, 'export const value = 2;\n');
    assert.notEqual(before, fingerprint('architecture', root));
  } finally { removeRoot(root); }
});

test('candidate enumeration fails closed when a source entry appears during hashing', {
  skip: process.platform === 'win32',
}, async () => {
  const root = makeRoot();
  const large = join(root, 'aaa-large.bin');
  const added = join(root, 'zz-added.txt');
  try {
    writeFileSync(large, '');
    truncateSync(large, 512 * 1024 * 1024);
    const writer = spawn('/bin/sh', [
      '-c', 'sleep 0.02; printf added > "$1"', 'candidate-writer', added,
    ], { stdio: 'ignore' });
    assert.throws(
      () => candidateFingerprint(root, CACHE_ENV),
      /candidate directory changed during fingerprint/,
    );
    await new Promise((resolve) => writer.once('close', resolve));
    assert.equal(readFileSync(added, 'utf8'), 'added');
  } finally { removeRoot(root); }
});

test('candidate revalidation catches same-path source replacement after its first hash', {
  skip: process.platform === 'win32',
}, async () => {
  const root = makeRoot();
  const target = join(root, '000-target.txt');
  const large = join(root, '001-large.bin');
  try {
    writeFileSync(target, 'before!');
    writeFileSync(large, '');
    truncateSync(large, 512 * 1024 * 1024);
    const writer = spawn('/bin/sh', [
      '-c', 'sleep 0.02; printf changed > "$1"', 'candidate-writer', target,
    ], { stdio: 'ignore' });
    assert.throws(
      () => candidateFingerprint(root, CACHE_ENV),
      /candidate entry changed during fingerprint: 000-target\.txt/,
    );
    await new Promise((resolve) => writer.once('close', resolve));
    assert.equal(readFileSync(target, 'utf8'), 'changed');
  } finally { removeRoot(root); }
});

test('candidate symlinks fail closed before scanners can dereference them', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  try {
    symlinkSync('README.md', join(root, 'linked-readme'));
    assert.throws(() => fingerprint('architecture', root), /symlinks are unsupported/);
  } finally { removeRoot(root); }
});

test('candidate hardlinks fail closed because an outside alias evades the change journal', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  try {
    linkSync(join(root, 'README.md'), join(root, 'README-copy.md'));
    assert.throws(
      () => fingerprint('architecture', root), /candidate hardlinks are unsupported/,
    );
  } finally { removeRoot(root); }
});

test('unreadable candidate bytes fail the fingerprint closed', () => {
  if (process.platform === 'win32' || process.getuid?.() === 0) {
    assert.ok(process.platform === 'win32' || process.getuid?.() === 0);
    return;
  }
  const root = makeRoot();
  const path = join(root, 'README.md');
  try {
    chmodSync(path, 0o000);
    assert.throws(() => fingerprint('architecture', root), /EACCES|permission denied/i);
  } finally {
    chmodSync(path, 0o600);
    removeRoot(root);
  }
});

test('unreadable candidate directories fail the fingerprint closed', () => {
  if (process.platform === 'win32' || process.getuid?.() === 0) {
    assert.ok(process.platform === 'win32' || process.getuid?.() === 0);
    return;
  }
  const root = makeRoot();
  const path = join(root, 'docs');
  try {
    chmodSync(path, 0o000);
    assert.throws(() => fingerprint('architecture', root), /EACCES|permission denied/i);
  } finally {
    chmodSync(path, 0o700);
    removeRoot(root);
  }
});

test('one complete snapshot writes a private atomic store and round-trips', {
  skip: process.platform !== 'linux',
}, () => {
  const root = makeRoot();
  try {
    assert.equal(saveComplete(root, 'static clean'), true);
    const store = loadCache(root);
    assert.equal(store.schema, CACHE_SCHEMA);
    assert.equal(store.rows.architecture.status, 'PASS');
    assert.equal(lstatSync(join(root, '.forge')).mode & 0o777, 0o700);
    assert.equal(lstatSync(storePath(root)).mode & 0o777, 0o600);
  } finally { removeRoot(root); }
});

test('coordinator replay requires an exact fingerprint and labels advisory bytes', {
  skip: process.platform !== 'linux',
}, () => {
  const root = makeRoot();
  try {
    const value = fingerprint('architecture', root);
    assert.equal(cachedDecide('architecture', 'architecture', root, value), null);
    saveComplete(root);
    const hit = cachedDecide('architecture', 'architecture', root, value);
    assert.deepEqual(hit, {
      criterion: 'architecture', status: 'PASS', category: 'applicable',
      detail: 'clean:architecture [advisory cache]',
    });
    assert.equal(cachedDecide('architecture', 'architecture', root, '0'.repeat(64)), null);
  } finally { removeRoot(root); }
});

test('invalid status/category rows are ignored and can never replay', () => {
  const root = makeRoot();
  try {
    const dir = join(root, '.forge');
    mkdirSync(dir, { mode: 0o700 });
    writeFileSync(storePath(root), `${JSON.stringify({
      schema: CACHE_SCHEMA,
      rows: {
        architecture: {
          fingerprint: 'a'.repeat(64), status: 'ACCEPTED', detail: 'forged',
          category: 'applicable',
        },
        security_findings: {
          fingerprint: 'b'.repeat(64), status: 'N-A', detail: 'forged',
          category: 'applicable',
        },
      },
    })}\n`, { mode: 0o600 });
    assert.deepEqual(loadCache(root).rows, {});
  } finally { removeRoot(root); }
});

test('prototype-inherited cache plan names are rejected as unknown rows', {
  skip: process.platform !== 'linux',
}, () => {
  const root = makeRoot();
  try {
    mkdirSync(join(root, '.forge'), { mode: 0o700 });
    writeFileSync(storePath(root), `${JSON.stringify({
      schema: CACHE_SCHEMA,
      rows: {
        constructor: {
          fingerprint: 'c'.repeat(64), status: 'PASS', detail: 'forged',
          category: 'applicable',
        },
      },
    })}\n`, { mode: 0o600 });
    assert.deepEqual(loadCache(root).rows, {});
  } finally { removeRoot(root); }
});

test('corrupt JSON and schema drift degrade to an empty cache', () => {
  const root = makeRoot();
  try {
    mkdirSync(join(root, '.forge'), { mode: 0o700 });
    writeFileSync(storePath(root), '{ broken', { mode: 0o600 });
    assert.deepEqual(loadCache(root).rows, {});
    writeFileSync(storePath(root), JSON.stringify({ schema: CACHE_SCHEMA + 1, rows: {} }));
    assert.deepEqual(loadCache(root).rows, {});
  } finally { removeRoot(root); }
});

test('non-canonical duplicate-key stores are rejected as a whole', {
  skip: process.platform !== 'linux',
}, () => {
  const root = makeRoot();
  try {
    assert.equal(saveComplete(root), true);
    const body = readFileSync(storePath(root), 'utf8')
      .replace(`"schema":${CACHE_SCHEMA}`, `"schema":${CACHE_SCHEMA},"schema":${CACHE_SCHEMA}`);
    writeFileSync(storePath(root), body, { mode: 0o600 });
    assert.deepEqual(loadCache(root).rows, {});
  } finally { removeRoot(root); }
});

test('a symlinked cache target is neither followed nor replaced', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  const outside = join(root, 'outside-cache');
  try {
    mkdirSync(join(root, '.forge'), { mode: 0o700 });
    writeFileSync(outside, 'outside');
    symlinkSync(outside, storePath(root));
    assert.deepEqual(loadCache(root).rows, {});
    assert.equal(saveComplete(root), false);
    assert.equal(readFileSync(outside, 'utf8'), 'outside');
  } finally { removeRoot(root); }
});

test('a dangling cache symlink is refused instead of atomically replaced', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  try {
    mkdirSync(join(root, '.forge'), { mode: 0o700 });
    symlinkSync(join(root, 'missing-cache-target'), storePath(root));
    assert.equal(saveComplete(root), false);
    assert.equal(lstatSync(storePath(root)).isSymbolicLink(), true);
  } finally { removeRoot(root); }
});

test('a symlinked cache directory is neither followed nor populated', {
  skip: process.platform === 'win32',
}, () => {
  const root = makeRoot();
  const outside = mkdtempSync(join(tmpdir(), 'accept-cache-outside-'));
  try {
    symlinkSync(outside, join(root, '.forge'));
    assert.deepEqual(loadCache(root).rows, {});
    assert.equal(saveComplete(root), false);
    assert.equal(existsSync(join(outside, 'acceptance-cache.json')), false);
  } finally {
    removeRoot(root);
    removeRoot(outside);
  }
});

test('saveRows rejects duplicate or incomplete snapshots before publishing', () => {
  const root = makeRoot();
  try {
    const names = Object.keys(CACHEABLE_PLANS);
    const fingerprints = fingerprintProbes(names, root, CACHE_ENV);
    const entries = names.map((name) => ({
      name, fingerprint: fingerprints.get(name), result: pass(name),
    }));
    entries[entries.length - 1] = { ...entries[0] };
    assert.equal(saveRows(root, entries), false);
    assert.equal(existsSync(storePath(root)), false);
  } finally { removeRoot(root); }
});

test('saveRows publishes one complete coordinator snapshot', {
  skip: process.platform !== 'linux',
}, () => {
  const root = makeRoot();
  try {
    assert.equal(saveComplete(root), true);
    const store = loadCache(root);
    assert.deepEqual(Object.keys(store.rows).sort(), Object.keys(CACHEABLE_PLANS).sort());
    const leftovers = readFileSync(storePath(root), 'utf8');
    assert.match(leftovers, /"security_findings"/);
  } finally { removeRoot(root); }
});

test('store path is fixed under the candidate and environment can only disable cache', () => {
  const root = makeRoot();
  try {
    assert.equal(storePath(root), join(root, '.forge', 'acceptance-cache.json'));
    assert.equal(cacheDisabled({ FORGE_ACCEPT_NO_CACHE: '1' }), true);
    assert.equal(cacheDisabled({ FORGE_ACCEPT_CACHE: '/tmp/elsewhere' }), false);
  } finally { removeRoot(root); }
});
