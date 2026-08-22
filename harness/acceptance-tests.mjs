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
  'test_backend_decision_check.py',
  'test_frontend_design_adversarial.py',
  'test_frontend_business_ui_composition_boundaries.py',
  'test_frontend_business_ui_geometry.py',
  'test_frontend_geometry_coordinate_contract.py',
  'test_frontend_design_check.py',
  'test_legacy_ai_batch_contract.py',
  'test_yaml2json.py',
];

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
      }
    }
  }
  walk(dir);
  return { node: node.sort(), python: python.sort(), errors };
}

function tapCount(out) {
  const value = Number((String(out ?? '').match(/(?:^|\n)# tests (\d+)/) ?? [])[1]);
  return Number.isNaN(value) ? null : value;
}

export function runCountedNodeFiles(files, root = ROOT, exec = run) {
  if (files.length === 0) return { ok: false, count: 0, files: 0 };
  const args = [
    '--test', '--test-reporter=tap',
    ...files.map((path) => relative(root, path).replace(/\\/g, '/')),
  ];
  const r = exec('node', args, {
    FORGE_ACCEPT_INNER: '1', PYTHONDONTWRITEBYTECODE: '1',
  }, root);
  const count = tapCount(r.out);
  return { ok: r.ok && count !== null && count > 0, count, files: files.length };
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
