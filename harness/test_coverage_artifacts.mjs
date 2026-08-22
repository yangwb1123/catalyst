import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { FAIL, NA, PASS, probeCoverage } from './acceptance.mjs';
import {
  COVERAGE_REPORT_MAX_BYTES,
  resolveCoverageLayout,
} from './adapters/coverage-report.mjs';
import {
  coverageArtifacts,
  judgeCoverage,
  parseCoveragePercent,
} from './adapters.mjs';
const COVERAGE_LOCK = '.forge-coverage-artifact.lock';
const ACCEPTANCE_URL = new URL('./acceptance.mjs', import.meta.url).href;
const pythonReport = (percent) => JSON.stringify({ totals: { percent_covered: percent } });
const typescriptReport = (percent) => JSON.stringify({
  total: { lines: { pct: percent } },
});
function writePythonReport(root, percent) {
  writeFileSync(join(root, 'coverage.json'), pythonReport(percent));
}
function writeTypeScriptReport(root, percent) {
  mkdirSync(join(root, 'coverage'), { recursive: true });
  writeFileSync(join(root, 'coverage', 'coverage-summary.json'), typescriptReport(percent));
}
async function waitForPath(path, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs;
  while (!existsSync(path)) {
    if (Date.now() >= deadline) throw new Error(`timed out waiting for ${path}`);
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
function probeChildSource(root, id) {
  return `
    import { existsSync, writeFileSync } from 'node:fs';
    import { join } from 'node:path';
    import { probeCoverage } from ${JSON.stringify(ACCEPTANCE_URL)};
    const root = ${JSON.stringify(root)};
    const id = ${JSON.stringify(id)};
    const waitCell = new Int32Array(new SharedArrayBuffer(4));
    writeFileSync(join(root, 'started-' + id), 'started');
    const exec = (_cmd, args, environment) => {
      assert.deepEqual(environment, { PYTHONDONTWRITEBYTECODE: '1' });
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      writeFileSync(join(root, '.coverage'), 'probe-' + id);
      writeFileSync(join(root, 'coverage.json'), '{"totals":{"percent_covered":100}}');
      writeFileSync(join(root, 'ready-' + id), 'ready');
      const deadline = Date.now() + 10_000;
      while (!existsSync(join(root, 'release-' + id))) {
        if (Date.now() >= deadline) throw new Error('release timeout ' + id);
        Atomics.wait(waitCell, 0, 0, 10);
      }
      return { ok: true, code: 0, out: 'TOTAL 10 0 100%' };
    };
    const result = probeCoverage(root, exec);
    if (result.status !== 'PASS') throw new Error(JSON.stringify(result));
  `;
}
function spawnProbe(root, id) {
  const child = spawn(process.execPath, [
    '--input-type=module', '-e', probeChildSource(root, id),
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', (chunk) => { stdout += chunk; });
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  const done = new Promise((resolve) => {
    child.once('error', (error) => resolve({ code: null, error, stderr, stdout }));
    child.once('close', (code) => resolve({ code, stderr, stdout }));
  });
  return { child, done };
}

function inPythonRepo(run) {
  const root = mkdtempSync(join(tmpdir(), 'forge-coverage-artifacts-'));
  try {
    writeFileSync(join(root, 'main.py'), 'VALUE = 1\n');
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function inTypeScriptRepo(run) {
  const root = mkdtempSync(join(tmpdir(), 'forge-typescript-coverage-'));
  try {
    writeFileSync(join(root, 'main.ts'), 'export const VALUE = 1;\n');
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('coverage artifact discovery includes pytest data and report files', () => {
  assert.deepEqual(coverageArtifacts('pytest --cov --cov-report=json'), [
    '.coverage',
    'coverage.json',
  ]);
  assert.deepEqual(coverageArtifacts('go test -coverprofile=coverage.out ./...'), [
    'coverage.out',
  ]);
  assert.deepEqual(coverageArtifacts('pytest --cov=forge'), ['.coverage']);
  assert.deepEqual(coverageArtifacts('pytest --cov --cov-report=json:reports/cov.json'), [
    '.coverage',
    'reports/cov.json',
  ]);
  assert.deepEqual(
    coverageArtifacts('vitest run --coverage --coverage.reporter=json-summary'), ['coverage']);
});

test('coverage artifact and report paths stay canonical and inside the project', () => {
  inPythonRepo((root) => {
    for (const path of ['', '.', '..', '../outside', 'a/../b', 'a//b', '/tmp/report',
      'C:/report', 'reports\\report.json']) {
      assert.throws(() => resolveCoverageLayout(root, [path], null), /canonical|segments/);
    }
    assert.throws(() => resolveCoverageLayout(root, ['coverage'], 'other/report.json'),
      /contained by a declared artifact/);
    for (const command of ['pytest --cov --cov-report=json:/tmp/out.json',
      'pytest --cov --cov-report=json:../out.json']) {
      const artifacts = coverageArtifacts(command);
      assert.throws(() => resolveCoverageLayout(root, artifacts, artifacts.at(-1)),
        /canonical|segments/);
    }
    const outside = mkdtempSync(join(tmpdir(), 'forge-coverage-outside-'));
    try {
      symlinkSync(outside, join(root, 'reports'), 'dir');
      assert.throws(() => resolveCoverageLayout(
        root, ['reports/coverage.json'], 'reports/coverage.json'), /symlink ancestor/);
    } finally {
      rmSync(outside, { recursive: true, force: true });
    }
  });
});

test('multiple Go package percentages are not mistaken for an aggregate', () => {
  const out = 'ok example/a coverage: 20.0% of statements\n'
    + 'ok example/b coverage: 100.0% of statements';
  assert.equal(parseCoveragePercent(out), null);
});

test('mixed Go no-test packages do not hide real low coverage', () => {
  const out = '?\texample/empty\t[no test files]\n'
    + 'ok\texample/tested\tcoverage: 20.0% of statements';
  assert.equal(judgeCoverage('go', 'go', true, { ok: true, code: 0, out }, 60).status,
    FAIL);
  const broken = `${out}\nFAIL\texample/broken [setup failed]`;
  assert.equal(judgeCoverage('go', 'go', true, { ok: false, code: 1, out: broken }, 60).status,
    NA, 'a real setup failure is still not a complete coverage verdict');
});

test('known no-test outcomes remain N/A when no machine report exists', () => {
  for (const [lang, bin, out] of [
    ['python', 'pytest', 'no tests ran'],
    ['typescript', 'vitest', 'No test files found'],
  ]) {
    assert.equal(judgeCoverage(lang, bin, true,
      { ok: false, code: 5, out, coverageError: 'report missing' }, 60).status, NA);
  }
});

test('a high machine percentage cannot hide a failing coverage command', () => {
  for (const [lang, bin] of [['python', 'python3'], ['typescript', 'vitest']]) {
    const result = judgeCoverage(lang, bin, true,
      { ok: false, code: 1, out: 'one test failed', coveragePercent: 99 }, 80);
    assert.equal(result.status, FAIL);
    assert.match(result.detail, /coverage command failed \(exit 1\)/);
  }
});
test('probeCoverage derives the real Go aggregate from the profile', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-go-coverage-aggregate-'));
  try {
    writeFileSync(join(root, 'main.go'), 'package main\n');
    const calls = [];
    const exec = (cmd, args) => {
      calls.push([cmd, args]);
      if (args[0] === 'version') return { ok: true, code: 0, out: 'go version' };
      if (args[0] === 'test') {
        writeFileSync(join(root, 'coverage.out'), 'mode: set\n');
        return { ok: true, code: 0, out:
          'ok example/a coverage: 20.0% of statements\n'
            + 'ok example/b coverage: 100.0% of statements' };
      }
      return { ok: true, code: 0, out: 'total: (statements) 50.0%' };
    };
    assert.equal(probeCoverage(root, exec).status, FAIL);
    assert.deepEqual(calls.at(-1), ['go', ['tool', 'cover', '-func=coverage.out']]);
    assert.equal(existsSync(join(root, 'coverage.out')), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('pytest trusts coverage.json, never its real-shaped [100%] progress output', () => {
  inPythonRepo((root) => {
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      writeFileSync(join(root, '.coverage'), 'probe data');
      writePythonReport(root, 20);
      return { ok: true, code: 0, out:
        'test_main.py . [100%]\nCoverage JSON written to file coverage.json\n1 passed' };
    };
    const result = probeCoverage(root, exec);
    assert.equal(result.status, FAIL);
    assert.match(result.detail, /20% < 60%/);
    assert.equal(existsSync(join(root, '.coverage')), false);
    assert.equal(existsSync(join(root, 'coverage.json')), false);
  });
});

test('missing, malformed, or oversized pytest reports fail and are cleaned', () => {
  const cases = [
    ['missing', () => {}],
    ['malformed', (root) => writeFileSync(join(root, 'coverage.json'), '{bad json')],
    ['numeric field required', (root) => writeFileSync(
      join(root, 'coverage.json'), pythonReport('100'))],
    ['range checked', (root) => writeFileSync(join(root, 'coverage.json'), pythonReport(101))],
    ['oversized', (root) => writeFileSync(
      join(root, 'coverage.json'), Buffer.alloc(COVERAGE_REPORT_MAX_BYTES + 1))],
  ];
  for (const [name, writeReport] of cases) {
    inPythonRepo((root) => {
      const exec = (_cmd, args) => {
        if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
        writeReport(root);
        return { ok: true, code: 0, out: 'test_main.py . [100%]\n1 passed' };
      };
      const result = probeCoverage(root, exec);
      assert.equal(result.status, FAIL, name);
      assert.match(result.detail, /machine coverage report invalid or missing/, name);
      assert.equal(existsSync(join(root, 'coverage.json')), false, name);
    });
  }
});

test('vitest trusts total.lines.pct and cleans its json-summary directory', () => {
  inTypeScriptRepo((root) => {
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'vitest 3' };
      writeTypeScriptReport(root, 28.57);
      return { ok: true, code: 0, out:
        'Tests 1 passed (1)\nAll files: 100%\nAll files | 100 | 100 | 100 | 100' };
    };
    const result = probeCoverage(root, exec);
    assert.equal(result.status, FAIL);
    assert.match(result.detail, /28\.57% < 60%/);
    assert.equal(existsSync(join(root, 'coverage')), false);
  });
});

test('missing or malformed vitest summaries fail closed', () => {
  for (const malformed of [false, true]) {
    inTypeScriptRepo((root) => {
      const exec = (_cmd, args) => {
        if (args[0] === '--version') return { ok: true, code: 0, out: 'vitest 3' };
        mkdirSync(join(root, 'coverage'));
        if (malformed) writeFileSync(join(root, 'coverage', 'coverage-summary.json'),
          JSON.stringify({ total: { statements: { pct: 100 } } }));
        return { ok: true, code: 0, out: 'Tests 1 passed (1)' };
      };
      assert.equal(probeCoverage(root, exec).status, FAIL);
      assert.equal(existsSync(join(root, 'coverage')), false);
    });
  }
});

test('a pre-existing vitest coverage symlink is restored without following it', () => {
  inTypeScriptRepo((root) => {
    const target = join(root, 'user-coverage');
    mkdirSync(target);
    writeFileSync(join(target, 'user.txt'), 'USER');
    symlinkSync('user-coverage', join(root, 'coverage'), 'dir');
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'vitest 3' };
      writeTypeScriptReport(root, 100);
      return { ok: true, code: 0, out: 'Tests 1 passed (1)' };
    };
    assert.equal(probeCoverage(root, exec).status, PASS);
    assert.equal(readlinkSync(join(root, 'coverage')), 'user-coverage');
    assert.equal(readFileSync(join(target, 'user.txt'), 'utf8'), 'USER');
    assert.equal(existsSync(join(target, 'coverage-summary.json')), false);
  });
});

test('a generated machine-report symlink is rejected and only the link is cleaned', () => {
  inPythonRepo((root) => {
    const target = join(root, 'outside-report');
    writeFileSync(target, pythonReport(100));
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      symlinkSync('outside-report', join(root, 'coverage.json'));
      return { ok: true, code: 0, out: 'test_main.py . [100%]\n1 passed' };
    };
    assert.equal(probeCoverage(root, exec).status, FAIL);
    assert.equal(readFileSync(target, 'utf8'), pythonReport(100));
    assert.equal(existsSync(join(root, 'coverage.json')), false);
  });
});

test('pytest coverage removes both artifacts created by the probe', () => {
  inPythonRepo((root) => {
    let coverageRuns = 0;
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      coverageRuns += 1;
      writeFileSync(join(root, '.coverage'), 'data');
      writeFileSync(join(root, 'coverage.json'), '{}');
      return { ok: false, code: 5, out: 'no tests ran' };
    };
    assert.equal(probeCoverage(root, exec).status, NA);
    assert.equal(coverageRuns, 1);
    assert.equal(existsSync(join(root, '.coverage')), false);
    assert.equal(existsSync(join(root, 'coverage.json')), false);
  });
});

test('pre-existing pytest artifacts are preserved while real low coverage fails', () => {
  for (const artifact of ['.coverage', 'coverage.json']) {
    inPythonRepo((root) => {
      const path = join(root, artifact);
      writeFileSync(path, 'user report');
      let coverageRuns = 0;
      const exec = (_cmd, args) => {
        if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
        coverageRuns += 1;
        writeFileSync(join(root, '.coverage'), 'probe data');
        writePythonReport(root, 20);
        return { ok: true, code: 0, out: 'test_main.py . [100%]\n1 passed' };
      };
      const result = probeCoverage(root, exec);
      assert.equal(result.status, FAIL);
      assert.match(result.detail, /20% < 60%/);
      assert.equal(coverageRuns, 1);
      assert.equal(readFileSync(path, 'utf8'), 'user report');
      const leftovers = readdirSync(root).filter((name) => name.startsWith('.forge-coverage-backup-'));
      assert.deepEqual(leftovers, []);
    });
  }
});

test('a pre-existing coverage directory is restored after the real probe', () => {
  inPythonRepo((root) => {
    const artifact = join(root, 'coverage.json');
    mkdirSync(artifact);
    writeFileSync(join(artifact, 'user-report.txt'), 'directory report');
    let coverageRuns = 0;
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      coverageRuns += 1;
      writeFileSync(join(root, '.coverage'), 'probe data');
      writePythonReport(root, 100);
      return { ok: true, code: 0, out: 'test_main.py . [100%]\n1 passed' };
    };
    assert.equal(probeCoverage(root, exec).status, PASS);
    assert.equal(coverageRuns, 1);
    assert.equal(readFileSync(join(artifact, 'user-report.txt'), 'utf8'), 'directory report');
  });
});

test('a dangling machine-report symlink is preserved without following it', () => {
  inPythonRepo((root) => {
    const artifact = join(root, 'coverage.json');
    symlinkSync('missing-user-report', artifact);
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      writeFileSync(join(root, '.coverage'), 'probe data');
      writePythonReport(root, 10);
      return { ok: true, code: 0, out: 'test_main.py . [100%]\n1 passed' };
    };
    assert.equal(probeCoverage(root, exec).status, FAIL);
    assert.equal(readlinkSync(artifact), 'missing-user-report');
    assert.equal(existsSync(join(root, 'missing-user-report')), false);
  });
});

test('a user artifact is restored when the coverage runner throws', () => {
  inPythonRepo((root) => {
    writeFileSync(join(root, '.coverage'), 'user data');
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      writeFileSync(join(root, '.coverage'), 'probe data');
      writeFileSync(join(root, 'coverage.json'), '{}');
      throw new Error('synthetic runner failure');
    };
    assert.throws(() => probeCoverage(root, exec), /synthetic runner failure/);
    assert.equal(readFileSync(join(root, '.coverage'), 'utf8'), 'user data');
    assert.equal(existsSync(join(root, 'coverage.json')), false);
  });
});

test('a restoration failure blocks and leaves the user artifact recoverable', () => {
  inPythonRepo((root) => {
    writeFileSync(join(root, '.coverage'), 'user data');
    let recoveryPath;
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      const backup = readdirSync(root).find((name) => name.startsWith('.forge-coverage-backup-'));
      recoveryPath = join(root, backup, 'recoverable-user-artifact');
      renameSync(join(root, backup, 'artifact-0'), recoveryPath);
      writeFileSync(join(root, '.coverage'), 'probe data');
      return { ok: true, code: 0, out: 'TOTAL 10 0 100%' };
    };
    assert.throws(() => probeCoverage(root, exec), /restoration failed closed/);
    assert.equal(existsSync(join(root, '.coverage')), false);
    assert.equal(readFileSync(recoveryPath, 'utf8'), 'user data');
  });
});

test('concurrent coverage probes serialize the complete artifact transaction', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'forge-coverage-concurrent-'));
  const children = [];
  t.after(() => {
    for (const child of children) if (child.exitCode === null) child.kill('SIGKILL');
    rmSync(root, { recursive: true, force: true });
  });
  writeFileSync(join(root, 'main.py'), 'VALUE = 1\n');
  writeFileSync(join(root, '.coverage'), 'USER');
  const first = spawnProbe(root, 'A');
  children.push(first.child);
  await waitForPath(join(root, 'ready-A'));
  const second = spawnProbe(root, 'B');
  children.push(second.child);
  await waitForPath(join(root, 'started-B'));
  await new Promise((resolve) => setTimeout(resolve, 250));
  assert.equal(existsSync(join(root, 'ready-B')), false,
    'second probe must not enter its artifact operation while the first owns the lock');
  writeFileSync(join(root, 'release-A'), 'release');
  await waitForPath(join(root, 'ready-B'));
  writeFileSync(join(root, 'release-B'), 'release');
  const outcomes = await Promise.all([first.done, second.done]);
  for (const outcome of outcomes) assert.equal(outcome.code, 0, outcome.stderr);
  assert.equal(readFileSync(join(root, '.coverage'), 'utf8'), 'USER');
  assert.equal(existsSync(join(root, 'coverage.json')), false);
  assert.deepEqual(readdirSync(root).filter((name) =>
    name.startsWith('.forge-coverage-')), []);
});

test('a dead-owner lock fails closed and remains for manual crash recovery', async () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-coverage-stale-lock-'));
  try {
    writeFileSync(join(root, 'main.py'), 'VALUE = 1\n');
    writeFileSync(join(root, '.coverage'), 'USER');
    const exited = spawn(process.execPath, ['-e', '']);
    const deadPid = exited.pid;
    await new Promise((resolve) => exited.once('close', resolve));
    writeFileSync(join(root, COVERAGE_LOCK), JSON.stringify({
      api_version: 'forgeos.coverage-artifact-transaction-lock/v1',
      created_at_unix_ms: 0,
      pid: deadPid,
      token: 'a'.repeat(32),
    }));
    let coverageRan = false;
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      coverageRan = true;
      return { ok: true, code: 0, out: 'TOTAL 10 0 100%' };
    };
    assert.throws(() => probeCoverage(root, exec), /stale coverage lock.*manual recovery/);
    assert.equal(coverageRan, false);
    assert.equal(readFileSync(join(root, '.coverage'), 'utf8'), 'USER');
    assert.equal(existsSync(join(root, COVERAGE_LOCK)), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('an untrusted lock symlink is never followed or removed', () => {
  inPythonRepo((root) => {
    const outside = join(root, 'outside-lock-target');
    writeFileSync(outside, 'DO NOT TOUCH');
    symlinkSync('outside-lock-target', join(root, COVERAGE_LOCK));
    let coverageRan = false;
    const exec = (_cmd, args) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'pytest 9' };
      coverageRan = true;
      return { ok: true, code: 0, out: 'TOTAL 10 0 100%' };
    };
    assert.throws(() => probeCoverage(root, exec), /unsafe coverage lock path/);
    assert.equal(coverageRan, false);
    assert.equal(readFileSync(outside, 'utf8'), 'DO NOT TOUCH');
    assert.equal(readlinkSync(join(root, COVERAGE_LOCK)), 'outside-lock-target');
  });
});
