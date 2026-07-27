import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  discoverHarnessSuites, runCountedNodeFiles, runPythonSuites,
} from './acceptance-tests.mjs';

function tempHarness(fn) {
  const root = mkdtempSync(join(tmpdir(), 'recursive-harness-tests-'));
  const harness = join(root, 'harness');
  mkdirSync(harness);
  try {
    return fn(root, harness);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('recursive discovery returns Node and Python suites from arbitrary subpackages', () => {
  tempHarness((root, harness) => {
    mkdirSync(join(harness, 'new-subpackage'));
    writeFileSync(join(harness, 'test_root.mjs'), '');
    writeFileSync(join(harness, 'new-subpackage', 'test_nested.mjs'), '');
    writeFileSync(join(harness, 'new-subpackage', 'test_nested.py'), '');
    const found = discoverHarnessSuites(harness);
    assert.deepEqual(found.node, [
      join(harness, 'new-subpackage', 'test_nested.mjs'),
      join(harness, 'test_root.mjs'),
    ]);
    assert.deepEqual(found.python, [join(harness, 'new-subpackage', 'test_nested.py')]);
    assert.deepEqual(found.errors, []);

    const calls = [];
    const result = runCountedNodeFiles(found.node, root, (cmd, args, env, cwd) => {
      calls.push({ cmd, args, env, cwd });
      return { ok: true, code: 0, out: 'TAP version 13\n# tests 2\n' };
    });
    assert.deepEqual(result, { ok: true, count: 2, files: 2 });
    assert.deepEqual(calls[0], {
      cmd: 'node',
      args: [
        '--test', '--test-reporter=tap',
        'harness/new-subpackage/test_nested.mjs',
        'harness/test_root.mjs',
      ],
      env: { FORGE_ACCEPT_INNER: '1' },
      cwd: root,
    });
  });
});

test('recursive discovery reports unreadable subtrees and the suite result fails closed', () => {
  tempHarness((root, harness) => {
    const blocked = join(harness, 'blocked');
    mkdirSync(blocked);
    writeFileSync(join(harness, 'test_check.py'), 'def test_green(): pass\n');
    writeFileSync(join(harness, 'test_yaml2json.py'), 'def test_green(): pass\n');
    writeFileSync(join(blocked, 'test_hidden.py'), 'def test_hidden(): pass\n');
    const found = discoverHarnessSuites(harness, (path, options) => {
      if (path === blocked) throw new Error('EACCES synthetic unreadable directory');
      return readdirSync(path, options);
    });
    assert.equal(found.errors.length, 1);
    assert.equal(found.errors[0].path, 'blocked');
    const result = runPythonSuites(
      harness,
      () => ({ ok: true, code: 0, out: 'FORGE_PY_TESTS=1\n' }),
      found,
    );
    assert.ok(result.entries.some(([name, ok]) => (
      !ok && name.includes('recursive harness discovery unreadable: blocked')
    )));
  });
});

test('Python runner executes pytest-style functions and rejects zero-test files', () => {
  tempHarness((root, harness) => {
    mkdirSync(join(harness, 'nested'));
    writeFileSync(join(harness, 'test_check.py'), 'def test_green(): pass\n');
    writeFileSync(join(harness, 'test_yaml2json.py'), 'def test_green(): pass\n');
    writeFileSync(join(harness, 'nested', 'test_red.py'),
      'def test_failure(): raise AssertionError("executed")\n');
    writeFileSync(join(harness, 'nested', 'test_zero.py'), 'def helper(): pass\n');

    const result = runPythonSuites(harness);
    assert.equal(result.files, 4);
    assert.equal(result.count, 3, 'two green + one red test function must actually execute');
    assert.deepEqual(
      result.entries.find(([name]) => name === 'nested/test_red.py'),
      ['nested/test_red.py', false],
    );
    assert.deepEqual(
      result.entries.find(([name]) => name === 'nested/test_zero.py'),
      ['nested/test_zero.py', false],
    );
  });
});
