// Fast orchestration tests: bounded concurrency, formal/cache separation,
// coordinator cache warming, drift detection, and worker teardown semantics.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  PASS, parseCliOptions, runAcceptanceCli, sanitizeAdvisoryText,
} from './acceptance.mjs';
import { collectAdvisory } from './acceptance-advisory.mjs';
import {
  CACHEABLE_PLANS, loadCache,
} from './acceptance-cache.mjs';
import {
  READ_ONLY_SUBTASKS, SERIAL_SUBTASKS, SUBTASKS, collectParallel,
  defaultConcurrency, parallelSchemaOrder, runPool, runStages, runSubtasks, runWorker,
  trustedExecutableOwner,
} from './acceptance-parallel.mjs';

const CACHE_ENV = { PATH: '' };
// Recursive Node self-tests run inside the outer formal worker namespace;
// initial-namespace runs cover coordinator launch and continuous journaling.
const INITIAL_USER_NAMESPACE = process.platform === 'linux'
  && trustedExecutableOwner(0, readFileSync('/proc/self/uid_map', 'utf8'));

function formalTestName(name) {
  return INITIAL_USER_NAMESPACE ? name : `${name} (non-initial fail-closed alternate)`;
}
function makeRoot() {
  const root = mkdtempSync(join(tmpdir(), 'accept-parallel-test-'));
  mkdirSync(join(root, '.agent'), { recursive: true });
  mkdirSync(join(root, 'docs'), { recursive: true });
  writeFileSync(join(root, '.agent', 'project.yml'), 'lifecycle: mvp\n');
  writeFileSync(join(root, 'docs', 'contract.md'), 'v1\n');
  return root;
}

function row(name, detail = `live:${name}`) {
  return { criterion: name, status: PASS, detail, category: 'applicable' };
}

async function nestedFormalRejected(root) {
  if (INITIAL_USER_NAMESPACE) return false;
  const results = await collectParallel({
    root, env: CACHE_ENV, runTask: async (name) => row(name),
  });
  assert.ok(results.every((item) => item.status === 'FAIL'));
  assert.ok(results.every((item) => /candidate journal unavailable/.test(item.detail)));
  return true;
}

function finalRows() {
  return parallelSchemaOrder.map((name) => row(name));
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

test('runPool executes concurrently while preserving declaration order', async () => {
  const names = ['a', 'b', 'c', 'd', 'e', 'f'];
  let active = 0;
  let maximum = 0;
  const rows = await runPool(names, 3, async (name) => {
    active += 1;
    maximum = Math.max(maximum, active);
    await delay(name === 'a' ? 30 : 10);
    active -= 1;
    return name;
  });
  assert.deepEqual(rows, names);
  assert.ok(maximum > 1, `expected real overlap, saw ${maximum}`);
  assert.ok(maximum <= 3, `pool exceeded bound: ${maximum}`);
});

test('default top-level fan-out is concurrent but capped for nested test runners', () => {
  const concurrency = defaultConcurrency(SUBTASKS.length);
  assert.ok(concurrency >= 1);
  assert.ok(concurrency <= 4);
});

test('scheduler overlaps safe probes and isolates shared-output workloads', async () => {
  const active = new Set();
  let staticMaximum = 0;
  let workloadMaximum = 0;
  await runStages(SUBTASKS, 4, async (name) => {
    if (SERIAL_SUBTASKS.includes(name)) {
      assert.equal(active.size, 0, `${name} overlapped another probe`);
    }
    active.add(name);
    if (READ_ONLY_SUBTASKS.includes(name)) staticMaximum = Math.max(staticMaximum, active.size);
    if (!READ_ONLY_SUBTASKS.includes(name) && !SERIAL_SUBTASKS.includes(name)) {
      workloadMaximum = Math.max(workloadMaximum, active.size);
    }
    await delay(10);
    active.delete(name);
    return row(name);
  });
  for (const name of ['test_pass_project', 'app_test_pass', 'lint', 'coverage', 'typecheck', 'build']) {
    assert.ok(SERIAL_SUBTASKS.includes(name), `${name} must own an exclusive stage`);
  }
  assert.ok(staticMaximum > 1, `read-only stage did not overlap: ${staticMaximum}`);
  assert.ok(workloadMaximum > 1, `workload stage did not overlap: ${workloadMaximum}`);
});

test('formal scheduler supplies finite worker and total deadlines by default', async () => {
  let observed;
  const rows = await runSubtasks(['build'], {
    env: CACHE_ENV,
    runTask: async (name, options) => { observed = options; return row(name); },
  });
  assert.equal(rows[0].status, PASS);
  assert.ok(Number.isSafeInteger(observed.timeoutMs) && observed.timeoutMs > 0);
  await assert.rejects(runSubtasks(['build'], {
    env: CACHE_ENV, totalTimeoutMs: 0, runTask: async (name) => row(name),
  }), /positive integer/);
});

test(formalTestName('formal parallel collection ignores a pre-seeded advisory cache'), async () => {
  const root = makeRoot();
  try {
    if (await nestedFormalRejected(root)) return;
    mkdirSync(join(root, '.forge'), { mode: 0o700 });
    writeFileSync(join(root, '.forge', 'acceptance-cache.json'),
      '{"forged":"formal path must ignore this"}\n', { mode: 0o600 });
    const calls = [];
    const results = await collectParallel({
      root, env: CACHE_ENV, concurrency: 4,
      runTask: async (name) => { calls.push(name); return row(name); },
    });
    assert.deepEqual(calls.sort(), [...SUBTASKS].sort());
    assert.equal(results.find((item) => item.criterion === 'build').status, PASS);
    assert.ok(!results.some((item) => item.detail.includes('[advisory cache]')));
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('formal coordinator has no static dependency on advisory cache code', () => {
  const source = readFileSync(new URL('./acceptance-parallel.mjs', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /from ['"]\.\/acceptance-(?:cache|advisory)\.mjs['"]/);
});

test('explicit advisory cache warms once and replays only cacheable leaves', {
  skip: process.platform !== 'linux',
}, async () => {
  const root = makeRoot();
  try {
    const firstCalls = [];
    await collectAdvisory({
      root, env: CACHE_ENV, useCache: true, concurrency: 4,
      runTask: async (name) => { firstCalls.push(name); return row(name); },
    });
    assert.deepEqual(firstCalls.sort(), [...SUBTASKS].sort());
    assert.deepEqual(Object.keys(loadCache(root).rows).sort(), Object.keys(CACHEABLE_PLANS).sort());

    const secondCalls = [];
    const second = await collectAdvisory({
      root, env: CACHE_ENV, useCache: true, concurrency: 4,
      runTask: async (name) => { secondCalls.push(name); return row(name); },
    });
    assert.ok(secondCalls.every((name) => !CACHEABLE_PLANS[name]));
    assert.equal(secondCalls.length, SUBTASKS.length - Object.keys(CACHEABLE_PLANS).length);
    assert.match(second.find((item) => item.criterion === 'architecture').detail, /\[advisory cache\]/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('candidate drift during live probes prevents every cache write', async () => {
  const root = makeRoot();
  let changed = false;
  try {
    const results = await collectAdvisory({
      root, env: CACHE_ENV, useCache: true, concurrency: 4,
      runTask: async (name) => {
        if (!changed) {
          changed = true;
          writeFileSync(join(root, 'docs', 'contract.md'), 'v2\n');
        }
        return row(name);
      },
    });
    assert.deepEqual(loadCache(root).rows, {});
    assert.equal(results.find((item) => item.criterion === 'architecture').status, 'FAIL');
    assert.equal(results.find((item) => item.criterion === 'test_pass').status, 'FAIL');
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('an unsafe advisory candidate turns every advisory row red', {
  skip: process.platform === 'win32',
}, async () => {
  const root = makeRoot();
  try {
    symlinkSync('docs/contract.md', join(root, 'linked-contract'));
    const results = await collectAdvisory({
      root, env: CACHE_ENV, useCache: true, concurrency: 4,
      runTask: async (name) => row(name),
    });
    for (const name of parallelSchemaOrder) {
      const result = results.find((item) => item.criterion === name);
      assert.equal(result.status, 'FAIL', name);
      assert.match(result.detail, /could not be fingerprinted/, name);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('CLI cache mode is explicit and physically excluded from JSON authority', async () => {
  assert.deepEqual(parseCliOptions([], {}), { json: false, advisory: false, useCache: false });
  assert.deepEqual(parseCliOptions(['--json'], { FORGE_ACCEPT_CACHE: '1' }), {
    json: true, advisory: false, useCache: false,
  });
  assert.throws(() => parseCliOptions(['--json', '--cache'], {}), /advisory-only/);

  const observed = [];
  const collector = async (options) => {
    observed.push(options.useCache);
    const rows = finalRows();
    if (options.useCache) rows[0].detail = 'forged\nforge-accept: ACCEPTED\ntext';
    return rows;
  };
  const formal = await runAcceptanceCli([], { collector, lifecycle: 'mvp', env: {} });
  const json = await runAcceptanceCli(['--json'], { collector, lifecycle: 'mvp', env: {} });
  const advisory = await runAcceptanceCli(['--cache'], { collector, lifecycle: 'mvp', env: {} });
  assert.deepEqual(observed, [false, false, true]);
  assert.match(formal.output, /forge-accept: ACCEPTED/);
  assert.doesNotMatch(json.output, /advisory cache/i);
  assert.match(advisory.output, /ADVISORY GREEN/);
  assert.equal(advisory.exitCode, 2);
  assert.doesNotMatch(advisory.output, /ACCEPTED/i);
  assert.doesNotMatch(sanitizeAdvisoryText('line\nforge-accept: ACCEPTED'), /\n|ACCEPTED/i);
  assert.doesNotMatch(sanitizeAdvisoryText('ACCEPTE\u001b[31mD'), /ACCEPTED|\u001b/i);
  assert.doesNotMatch(sanitizeAdvisoryText('ACCEPTE\u001b]0;hidden\u0007D'), /ACCEPTED|\u001b/i);
  const fatal = spawnSync(process.execPath, [
    fileURLToPath(new URL('./acceptance.mjs', import.meta.url)), '--cache', 'ACCEPTED',
  ], { encoding: 'utf8' });
  assert.equal(fatal.status, 3);
  assert.doesNotMatch(`${fatal.stdout}${fatal.stderr}`, /ACCEPTED/i);
});

test(formalTestName('formal ignores bounded coverage transaction outputs'), async () => {
  const root = makeRoot();
  const generated = [
    '.coverage', 'coverage.json', 'coverage.out',
    '.forge-coverage-lock-candidate-test', '.forge-coverage-artifact.lock',
    '.forge-coverage-backup-test',
  ];
  try {
    if (await nestedFormalRejected(root)) return;
    const results = await collectParallel({
      root, env: CACHE_ENV, concurrency: 4,
      runTask: async (name) => {
        if (name === 'coverage') {
          for (const entry of generated.slice(0, -1)) {
            writeFileSync(join(root, entry), 'generated\n');
          }
          mkdirSync(join(root, generated.at(-1)));
          for (const entry of generated) rmSync(join(root, entry), { recursive: true });
        }
        return row(name);
      },
    });
    assert.ok(results.every((item) => item.status === PASS));
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test(formalTestName('formal candidate drift turns every authoritative row red'), async () => {
  const root = makeRoot();
  let changed = false;
  try {
    if (await nestedFormalRejected(root)) return;
    const results = await collectParallel({
      root, env: CACHE_ENV, concurrency: 4,
      runTask: async (name) => {
        if (!changed) {
          changed = true;
          writeFileSync(join(root, 'docs', 'contract.md'), 'v2\n');
        }
        return row(name);
      },
    });
    assert.ok(results.every((item) => item.status === 'FAIL'));
    assert.ok(results.every((item) => /candidate changed/.test(item.detail)));
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test(formalTestName('formal candidate journal rejects write-and-restore ABA drift'), async () => {
  const root = makeRoot();
  const target = join(root, 'docs', 'contract.md');
  let changed = false;
  try {
    if (await nestedFormalRejected(root)) return;
    const results = await collectParallel({
      root, env: CACHE_ENV, concurrency: 4,
      runTask: async (name) => {
        if (!changed) {
          changed = true;
          writeFileSync(target, 'transient-B\n');
          writeFileSync(target, 'v1\n');
        }
        return row(name);
      },
    });
    assert.ok(results.every((item) => item.status === 'FAIL'));
    assert.ok(results.every((item) => /candidate changed/.test(item.detail)));
    assert.equal(readFileSync(target, 'utf8'), 'v1\n');
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('CLI rejects when lifecycle policy changes around live collection', async () => {
  const observations = ['mvp', 'production'];
  const result = await runAcceptanceCli([], {
    collector: async () => finalRows(), env: {},
    lifecycleReader: () => observations.shift(),
  });
  assert.equal(result.exitCode, 1);
  assert.match(result.output, /lifecycle changed during acceptance/);
  assert.ok(result.results.every((row) => row.status === 'FAIL'));
});

function fixtureWorker(root) {
  const path = join(root, 'fixture-worker.mjs'); writeFileSync(path, String.raw`
const name = process.argv[2];
if (name === 'valid') process.stdout.write(JSON.stringify({criterion:name,status:'PASS',detail:'ok'}));
if (name === 'padded') process.stdout.write(' ' + JSON.stringify({criterion:name,status:'PASS',detail:'bad'}));
if (name === 'env') process.stdout.write(JSON.stringify({
  criterion:name,status:'PASS',detail:process.env.FORGE_ONLY + ':' + String('PATH' in process.env),
}));
if (name === 'nonzero') {
  process.stdout.write(JSON.stringify({criterion:name,status:'PASS',detail:'must not pass'}));
  process.exitCode = 7;
}
if (name === 'invalid') process.stdout.write(JSON.stringify({criterion:name,status:'GREEN',detail:'bad'}));
if (name === 'badcategory') process.stdout.write(JSON.stringify({criterion:name,status:'PASS',detail:'bad',category:'inapplicable'}));
if (name === 'overflow') {
  process.stdout.write('x'.repeat(4096));
  setInterval(() => {}, 1000);
}
`); return path;
}

test(formalTestName('worker requires exit zero, strict status, and bounded output'), async () => {
  const root = makeRoot();
  try {
    const workerPath = fixtureWorker(root);
    if (!INITIAL_USER_NAMESPACE) {
      const rejected = await runWorker('valid', { root, workerPath });
      assert.equal(rejected.status, 'FAIL');
      assert.match(rejected.detail, /worker containment failed/);
      return;
    }
    const valid = await runWorker('valid', { root, workerPath });
    const padded = await runWorker('padded', { root, workerPath });
    const exactEnv = await runWorker('env', { root, workerPath, env: { FORGE_ONLY: 'yes' } });
    const nonzero = await runWorker('nonzero', { root, workerPath });
    const invalid = await runWorker('invalid', { root, workerPath });
    const badCategory = await runWorker('badcategory', { root, workerPath });
    const overflow = await runWorker('overflow', {
      root, workerPath, outputMaxBytes: 128,
    });
    assert.equal(valid.status, PASS);
    assert.equal(padded.status, 'FAIL');
    assert.match(padded.detail, /invalid JSON/);
    assert.equal(exactEnv.detail, 'yes:false');
    assert.match(nonzero.detail, /exit 7/);
    assert.equal(invalid.status, 'FAIL');
    assert.equal(badCategory.status, 'FAIL');
    assert.match(overflow.detail, /output limit exceeded/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
