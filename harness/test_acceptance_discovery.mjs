import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  discoverHarnessSuites, runCountedNodeFiles, runPythonHarnessParallel, runPythonSuites,
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
      return { ok: true, code: 0, out: 'TAP version 13\n# tests 2\n# skipped 0\n' };
    });
    assert.deepEqual(result, { ok: true, count: 2, files: 2, skipped: 0 });
    assert.deepEqual(calls[0], {
      cmd: 'node',
      args: [
        '--test', '--test-reporter=tap', '--test-concurrency=4',
        'harness/new-subpackage/test_nested.mjs',
        'harness/test_root.mjs',
      ],
      env: { FORGE_ACCEPT_INNER: '1', PYTHONDONTWRITEBYTECODE: '1' },
      cwd: root,
    });
  });
});

test('recursive Node discovery fails closed when TAP reports a skipped test', () => {
  tempHarness((root, harness) => {
    const path = join(harness, 'test_skipped.mjs');
    writeFileSync(path, '');
    const result = runCountedNodeFiles([path], root, () => ({
      ok: true, code: 0,
      out: 'TAP version 13\n# tests 1\n# pass 0\n# skipped 1\n',
    }));
    assert.deepEqual(result, { ok: false, count: 1, files: 1, skipped: 1 });
  });
});

test('recursive discovery ignores non-test fixture support modules', () => {
  tempHarness((_root, harness) => {
    const helpers = join(harness, 'agent_engineering');
    mkdirSync(helpers);
    writeFileSync(join(helpers, 'support.py'), 'def make_fixture(): return {}\n');
    writeFileSync(join(helpers, 'test_contract.py'), 'def test_green(): pass\n');

    const found = discoverHarnessSuites(harness);
    assert.deepEqual(found.python, [join(helpers, 'test_contract.py')]);
    assert.equal(found.python.includes(join(helpers, 'support.py')), false);
  });
});

test('recursive discovery retires only the exact v30 helper path', () => {
  tempHarness((_root, harness) => {
    const retiredDir = join(harness, 'agent_engineering');
    const activeDir = join(harness, 'another_package');
    mkdirSync(retiredDir);
    mkdirSync(activeDir);
    const retired = join(retiredDir, 'test_support.py');
    const active = join(activeDir, 'test_support.py');
    writeFileSync(retired, 'def helper(): pass\n');
    writeFileSync(active, 'def helper(): pass\n');

    const found = discoverHarnessSuites(harness);
    assert.equal(found.python.includes(retired), false);
    assert.equal(found.python.includes(active), true);
    const calls = [];
    const result = runPythonSuites(
      harness,
      (cmd, args, env, cwd) => {
        calls.push({ cmd, args, env, cwd });
        return { ok: true, code: 0, out: 'FORGE_PY_TESTS=0\n' };
      },
      found,
    );
    assert.deepEqual(calls[0].env, { PYTHONDONTWRITEBYTECODE: '1' });
    assert.deepEqual(calls[0].args.slice(0, 2), ['-B', '-c']);
    assert.deepEqual(
      result.entries.find(([name]) => name === 'another_package/test_support.py'),
      ['another_package/test_support.py', false],
    );
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

test('recursive discovery reports symlinked entries instead of omitting tests', {
  skip: process.platform === 'win32',
}, () => {
  tempHarness((_root, harness) => {
    writeFileSync(join(harness, 'real_test.mjs'), '');
    symlinkSync('real_test.mjs', join(harness, 'test_link.mjs'));
    const found = discoverHarnessSuites(harness);
    assert.deepEqual(found.node, []);
    assert.deepEqual(found.errors, [{
      path: 'test_link.mjs', message: 'non-regular harness entry',
    }]);
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

test('parallel Python runner drains noisy stderr and fails bounded output closed', async () => {
  const root = mkdtempSync(join(tmpdir(), 'parallel-python-output-'));
  const harness = join(root, 'harness');
  mkdirSync(harness);
  try {
    writeFileSync(join(harness, 'test_check.py'), [
      'import os',
      'def test_noisy():',
      '    for _ in range(32):',
      '        os.write(2, b"x" * 65536)',
      '',
    ].join('\n'));
    const result = await runPythonHarnessParallel(harness, { workers: 1 });
    assert.deepEqual(
      result.entries.find(([name]) => name === 'test_check.py'),
      ['test_check.py', false],
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('parallel Python runner terminates a suite at the output limit', async () => {
  const root = mkdtempSync(join(tmpdir(), 'parallel-python-overflow-'));
  const harness = join(root, 'harness');
  const completed = join(root, 'completed');
  mkdirSync(harness);
  try {
    writeFileSync(join(harness, 'test_check.py'), [
      'import os, pathlib',
      'def test_noisy():',
      '    for _ in range(8192):',
      '        os.write(2, b"x" * 65536)',
      `    pathlib.Path(${JSON.stringify(completed)}).write_text("completed")`,
      '',
    ].join('\n'));
    const result = await runPythonHarnessParallel(harness, { workers: 1 });
    assert.equal(result.ok, false);
    assert.equal(existsSync(completed), false, 'overflowing suite must be killed, not drained to exit');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('parallel Python runner rejects a zero-width worker pool', async () => {
  await assert.rejects(
    runPythonHarnessParallel('/unused', { workers: 0 }),
    /workers must be a positive integer/,
  );
});
