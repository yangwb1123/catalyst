// Recursive, fail-closed harness self-test discovery/execution.
import { readdirSync } from 'node:fs';
import { join, relative } from 'node:path';

import {
  HARNESS_DIR, ROOT, run,
} from './acceptance-kernel.mjs';

const SKIP_DIRS = new Set(['__pycache__', 'node_modules', 'target']);
// v30 scaffold ledgers may retain this pre-rename helper when a safe plain
// upgrade runs without --prune. Exclude only that exact historical path; every
// other test_*.py remains fail-closed recursive test input.
const RETIRED_PYTHON_HELPERS = new Set([
  'agent_engineering/test_support.py',
]);
const REQUIRED_PYTHON = [
  'test_check.py',
  'test_agent_engineering_check.py',
  'test_engineering_check_support.py',
  'test_backend_decision_check.py',
  'test_frontend_design_adversarial.py',
  'test_frontend_business_ui_composition_boundaries.py',
  'test_frontend_business_ui_geometry.py',
  'test_frontend_geometry_coordinate_contract.py',
  'test_frontend_design_check.py',
  'test_legacy_ai_batch_contract.py',
  'test_yaml2json.py',
];
const PARALLEL_PYTHON_OUTPUT_MAX_BYTES = 1024 * 1024;
const NODE_TEST_CONCURRENCY = 4;

// Import a Python suite without triggering its __main__ block, run all unittest
// cases, then execute module-level pytest-style test_* callables. A file that
// defines zero tests exits non-zero and reports FORGE_PY_TESTS=0.
const PYTHON_SUITE_RUNNER = String.raw`
import asyncio, importlib.util, inspect, os, sys, traceback, unittest
path = os.path.abspath(sys.argv[1])
sys.path.insert(0, os.path.dirname(path))
name = "_forge_suite_" + str(abs(hash(path)))
spec = importlib.util.spec_from_file_location(name, path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
suite = unittest.defaultTestLoader.loadTestsFromModule(module)
count = suite.countTestCases()
result = unittest.TextTestRunner(verbosity=1).run(suite)
ok = result.wasSuccessful()
functions = [
    value for key, value in vars(module).items()
    if key.startswith("test_") and inspect.isfunction(value)
    and value.__module__ == module.__name__
]
for fn in functions:
    count += 1
    try:
        if inspect.iscoroutinefunction(fn):
            asyncio.run(fn())
        else:
            fn()
    except BaseException:
        ok = False
        traceback.print_exc()
print("FORGE_PY_TESTS=" + str(count))
raise SystemExit(0 if ok and count > 0 else 1)
`;

// Repository Python suites that live OUTSIDE harness/ are load-bearing source
// with no owning project manifest, so the acceptance lane owns and executes
// them instead of leaving them unowned by any runner. Two execution modes are
// required and neither may be substituted for the other:
//   - skills/<skill>/tests/test_*.py  fail closed unless Python is isolated,
//     so they run under -I;
//   - scripts/test_*.py                import their sibling modules from the
//     script directory, so they run with the harness lane's own sys.path
//     handling and no -I.
// Vendored copies under scripts/_vendor are the distribution payload itself
// and are excluded exactly as the harness lane excludes them.
export function discoverRepositoryPythonSuites(root = ROOT, readDir = readdirSync) {
  const isolated = [];
  const shared = [];
  const errors = [];
  function walk(current) {
    let entries;
    try {
      entries = readDir(current, { withFileTypes: true });
    } catch (err) {
      errors.push({
        path: relative(root, current).replace(/\\/g, '/') || '.',
        message: err?.message ?? String(err),
      });
      return;
    }
    for (const entry of entries) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name) && entry.name !== '_vendor') walk(path);
      } else if (entry.isFile() && /^test_.*\.py$/.test(entry.name)) {
        const inTests = current.split(/[\\/]/).pop() === 'tests';
        if (inTests) isolated.push(path);
        else if (current === join(root, 'scripts')) shared.push(path);
      }
    }
  }
  walk(join(root, 'skills'));
  walk(join(root, 'scripts'));
  const sort = (list) => list.sort();
  return {
    isolated: sort(isolated), shared: sort(shared), errors,
  };
}

export function discoverHarnessSuites(dir = HARNESS_DIR, readDir = readdirSync) {
  const node = [];
  const python = [];
  const errors = [];
  function walk(current) {
    let entries;
    try {
      entries = readDir(current, { withFileTypes: true });
    } catch (err) {
      errors.push({
        path: relative(dir, current).replace(/\\/g, '/') || '.',
        message: err?.message ?? String(err),
      });
      return;
    }
    for (const entry of entries) {
      const path = join(current, entry.name);
      const relativePath = relative(dir, path).replace(/\\/g, '/');
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) walk(path);
      } else if (entry.isFile() && /^test_.*\.(?:mjs|js|cjs)$/.test(entry.name)) {
        node.push(path);
      } else if (entry.isFile() && /^test_.*\.py$/.test(entry.name)
        && !RETIRED_PYTHON_HELPERS.has(relativePath)) {
        python.push(path);
      } else if (!entry.isFile()) {
        errors.push({ path: relativePath, message: 'non-regular harness entry' });
      }
    }
  }
  walk(dir);
  return { node: node.sort(), python: python.sort(), errors };
}

function tapMetric(out, label) {
  const pattern = new RegExp(`(?:^|\\n)# ${label} (\\d+)`);
  const value = Number((String(out ?? '').match(pattern) ?? [])[1]);
  return Number.isNaN(value) ? null : value;
}

export function runCountedNodeFiles(files, root = ROOT, exec = run) {
  if (files.length === 0) return { ok: false, count: 0, files: 0, skipped: null };
  const args = [
    '--test', '--test-reporter=tap', `--test-concurrency=${NODE_TEST_CONCURRENCY}`,
    ...files.map((path) => relative(root, path).replace(/\\/g, '/')),
  ];
  const r = exec('node', args, {
    FORGE_ACCEPT_INNER: '1', PYTHONDONTWRITEBYTECODE: '1',
  }, root);
  const count = tapMetric(r.out, 'tests');
  const skipped = tapMetric(r.out, 'skipped');
  return {
    ok: r.ok && count !== null && count > 0 && skipped === 0,
    count, files: files.length, skipped,
  };
}

export function runPythonSuites(dir = HARNESS_DIR, exec = run, discovery = discoverHarnessSuites(dir)) {
  const found = discovery.python;
  const entries = [];
  let count = 0;
  for (const path of found) {
    const r = exec('python3', ['-B', '-c', PYTHON_SUITE_RUNNER, path], {
      PYTHONDONTWRITEBYTECODE: '1',
    }, ROOT);
    const discovered = Number(
      (String(r.out ?? '').match(/(?:^|\n)FORGE_PY_TESTS=(\d+)/) ?? [])[1],
    );
    const tests = Number.isNaN(discovered) ? 0 : discovered;
    count += tests;
    entries.push([relative(dir, path).replace(/\\/g, '/'), r.ok && tests > 0]);
  }
  const relativeNames = found.map((path) => relative(dir, path).replace(/\\/g, '/'));
  const missing = REQUIRED_PYTHON.filter((name) => !relativeNames.includes(name));
  if (found.length === 0 || missing.length > 0) {
    entries.push([`recursive Python discovery (missing: ${missing.join(', ') || 'none found'})`, false]);
  }
  for (const failure of discovery.errors) {
    entries.push([
      `recursive harness discovery unreadable: ${failure.path} (${failure.message})`,
      false,
    ]);
  }
  return { entries, count, files: found.length };
}

// The python lane also executes the portable skill distributions, so the
// evidence string must name them: a silent extra lane would otherwise look
// identical to the original harness-only count.
export function pythonLaneDetail(result) {
  return `recursive Python suites (${result.files} files/${result.count} tests; `
    + `${result.portable ? result.portable.files : 0} outside harness/ `
    + `(skills/ isolated, scripts/ shared)`;
}

// Serial lane for the repository suites outside harness/. Each suite runs in
// the mode it demands, so a suite that refuses its interpreter still fails
// closed here instead of being skipped.
export function runPortableSkillSuites(
  exec = run, discovery = discoverRepositoryPythonSuites(),
) {
  const entries = [];
  let count = 0;
  const modes = [
    [discovery.isolated, ['-I', '-B']],
    [discovery.shared, ['-B']],
  ];
  for (const [files, flags] of modes) {
    for (const path of files) {
      const r = exec('python3', [...flags, '-c', PYTHON_SUITE_RUNNER, path], {
        PYTHONDONTWRITEBYTECODE: '1',
      }, ROOT);
      const discovered = Number(
        (String(r.out ?? '').match(/(?:^|\n)FORGE_PY_TESTS=(\d+)/) ?? [])[1],
      );
      const tests = Number.isNaN(discovered) ? 0 : discovered;
      count += tests;
      entries.push([relative(ROOT, path).replace(/\\/g, '/'), r.ok && tests > 0]);
    }
  }
  const files = discovery.isolated.length + discovery.shared.length;
  if (files === 0) {
    entries.push(['repository Python discovery (no suites outside harness/)', false]);
  }
  for (const failure of discovery.errors) {
    entries.push([
      `repository Python discovery unreadable: ${failure.path} (${failure.message})`,
      false,
    ]);
  }
  return { entries, count, files };
}

export function runNodeHarness(dir = HARNESS_DIR, root = ROOT, exec = run) {
  const found = discoverHarnessSuites(dir);
  const node = runCountedNodeFiles(found.node, root, exec);
  return {
    ok: node.ok && found.errors.length === 0,
    count: node.count,
    files: node.files,
    skipped: node.skipped,
    errors: found.errors,
  };
}

export function runPythonHarness(dir = HARNESS_DIR, exec = run) {
  const python = runPythonSuites(dir, exec);
  const portable = runPortableSkillSuites(exec);
  const entries = [...python.entries, ...portable.entries];
  return {
    ok: entries.every(([, ok]) => ok),
    count: python.count + portable.count,
    files: python.files + portable.files,
    entries,
    portable,
  };
}

// Parallel Python runner: one `python3 -B -c <runner>` per suite FILE, up to
// `workers` at once (the files are independent processes with no shared
// state, so concurrency is safe and the verdict is identical to the serial
// path — same runner text, same per-file exit semantics, counts summed).
// Kept as a separate async entry so the serial callers (tests, sync
// collect()) stay byte-identical; the acceptance worker uses this to shrink
// the python suite wall-clock from ~20 minutes to ~the slowest file.
import { spawn } from 'node:child_process';

function terminatePythonSuite(child) {
  if (!child.pid) return;
  if (process.platform !== 'win32') {
    try {
      process.kill(-child.pid, 'SIGKILL');
      return;
    } catch (error) {
      if (error?.code !== 'ESRCH') child.kill('SIGKILL');
      return;
    }
  }
  child.kill('SIGKILL');
}

// Portable skill suites fail closed unless Python itself is isolated (-I), so
// they get their own argv. Same bounded-output and process-group discipline as
// the harness lane: identical verdict semantics, one isolation flag apart.
function spawnPortableSkillSuite(path, index, env) {
  return spawnSuiteWithArgs(['-I', '-B', '-c', PYTHON_SUITE_RUNNER, path], index, env);
}

function spawnPythonSuite(path, index, env) {
  return spawnSuiteWithArgs(['-B', '-c', PYTHON_SUITE_RUNNER, path], index, env);
}

function spawnSuiteWithArgs(args, index, env) {
  return new Promise((resolve) => {
    const child = spawn('python3', args, {
      cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'],
      detached: process.platform !== 'win32',
    });
    const chunks = [];
    let retained = 0;
    let total = 0;
    let overflow = false;
    let settled = false;
    const finish = (row) => {
      if (settled) return;
      settled = true;
      resolve(row);
    };
    const drain = (chunk, retain) => {
      total += chunk.length;
      if (!overflow && total > PARALLEL_PYTHON_OUTPUT_MAX_BYTES) {
        overflow = true;
        terminatePythonSuite(child);
      }
      if (!retain || retained >= PARALLEL_PYTHON_OUTPUT_MAX_BYTES) return;
      const bytes = chunk.subarray(0, PARALLEL_PYTHON_OUTPUT_MAX_BYTES - retained);
      chunks.push(bytes);
      retained += bytes.length;
    };
    child.stdout.on('data', (chunk) => drain(chunk, true));
    child.stderr.on('data', (chunk) => drain(chunk, false));
    child.on('error', () => finish({ code: null, out: '', index, overflow: true }));
    child.on('close', (code) => finish({
      code, out: Buffer.concat(chunks).toString('utf8'), index, overflow,
    }));
  });
}

async function runPythonPool(files, workers, env, spawnSuite = spawnPythonSuite) {
  const results = new Array(files.length);
  let cursor = 0;
  async function lane() {
    while (cursor < files.length) {
      const index = cursor;
      cursor += 1;
      results[index] = await spawnSuite(files[index], index, env);
    }
  }
  await Promise.all(Array.from(
    { length: Math.min(workers, Math.max(1, files.length)) }, () => lane(),
  ));
  return results;
}

function summarizePythonPool(dir, found, results) {
  let count = 0;
  const entries = found.python.map((path, index) => {
    const row = results[index];
    const tests = Number(
      (String(row?.out ?? '').match(/(?:^|\n)FORGE_PY_TESTS=(\d+)/) ?? [])[1],
    );
    const observed = Number.isNaN(tests) ? 0 : tests;
    count += observed;
    return [
      relative(dir, path).replace(/\\/g, '/'),
      row?.code === 0 && observed > 0 && !row?.overflow,
    ];
  });
  const relativeNames = entries.map(([name]) => name);
  const missing = REQUIRED_PYTHON.filter((name) => !relativeNames.includes(name));
  if (found.python.length === 0 || missing.length > 0) {
    entries.push([`recursive Python discovery (missing: ${missing.join(', ') || 'none found'})`, false]);
  }
  for (const failure of found.errors) {
    entries.push([
      `recursive harness discovery unreadable: ${failure.path} (${failure.message})`, false,
    ]);
  }
  return { ok: entries.every(([, ok]) => ok), count, files: found.python.length, entries };
}

// The parallel lane runs the isolated and shared pools separately (different
// argv) and folds them back into one result array whose index order matches
// [...isolated, ...shared] so a single summarizer covers both.
function mergePortableSkillPools(found, isolatedResults, sharedResults) {
  return summarizePortableSkillPool(found, [...isolatedResults, ...sharedResults]);
}

function summarizePortableSkillPool(found, results) {
  let count = 0;
  const entries = [];
  const all = [...found.isolated, ...found.shared];
  all.forEach((path, index) => {
    const row = results[index];
    const tests = Number(
      (String(row?.out ?? '').match(/(?:^|\n)FORGE_PY_TESTS=(\d+)/) ?? [])[1],
    );
    const observed = Number.isNaN(tests) ? 0 : tests;
    count += observed;
    entries.push([
      relative(ROOT, path).replace(/\\/g, '/'),
      row?.code === 0 && observed > 0 && !row?.overflow,
    ]);
  });
  if (all.length === 0) {
    entries.push(['repository Python discovery (no suites outside harness/)', false]);
  }
  for (const failure of found.errors) {
    entries.push([
      `repository Python discovery unreadable: ${failure.path} (${failure.message})`,
      false,
    ]);
  }
  return { ok: entries.every(([, ok]) => ok), count, files: all.length, entries };
}

export async function runPythonHarnessParallel(dir = HARNESS_DIR, { workers = 8 } = {}) {
  if (!Number.isSafeInteger(workers) || workers < 1) {
    throw new Error('parallel Python workers must be a positive integer');
  }
  const found = discoverHarnessSuites(dir);
  const skill = discoverRepositoryPythonSuites();
  const env = { ...process.env, PYTHONDONTWRITEBYTECODE: '1' };
  delete env.NODE_TEST_CONTEXT;
  const results = await runPythonPool(found.python, workers, env);
  const skillResults = await runPythonPool(
    skill.isolated, workers, env, spawnPortableSkillSuite,
  );
  const base = summarizePythonPool(dir, found, results);
  const sharedResults = await runPythonPool(skill.shared, workers, env);
  const portable = mergePortableSkillPools(
    skill, skillResults, sharedResults,
  );
  return {
    ok: base.ok && portable.ok,
    count: base.count + portable.count,
    files: base.files + portable.files,
    entries: [...base.entries, ...portable.entries],
    portable,
  };
}

export function runHarnessSuites(dir = HARNESS_DIR, root = ROOT, exec = run) {
  const found = discoverHarnessSuites(dir);
  const node = runCountedNodeFiles(found.node, root, exec);
  const python = runPythonSuites(dir, exec, found);
  return {
    entries: [...python.entries, ['recursive harness Node suites', node.ok]],
    node,
    python,
  };
}
