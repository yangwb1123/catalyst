// Project-probe and lifecycle-N/A tests kept separate from test_acceptance.mjs
// so both files remain below the harness's own 500-line structural gate.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  lstatSync, mkdtempSync, mkdirSync, readdirSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  decide,
  FAIL,
  LOAD_BEARING,
  NA,
  PASS,
  probeBuild,
  probeAppTests,
  probeForgeCoreTests,
  probeProjectTests,
  probeTypecheck,
  readProjectLifecycle,
} from './acceptance.mjs';
import {
  COMMAND_OUTPUT_MAX_BYTES, INAPPLICABLE, NO_TOOL, run, withCategory,
} from './acceptance-kernel.mjs';

function acceptanceRows(extra) {
  return [
    ...LOAD_BEARING.map((criterion) => ({ criterion, status: PASS, detail: 'green' })),
    ...extra,
  ];
}

test('forge-core probes run go test/build/vet from the module and propagate failures', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-core-probes-'));
  try {
    const core = join(dir, 'forge-core');
    mkdirSync(core, { recursive: true });
    writeFileSync(join(core, 'go.mod'), 'module example.test/core\ngo 1.22\n');
    const calls = [];
    const green = (cmd, args, env, cwd) => {
      calls.push({ cmd, args, env, cwd });
      return {
        ok: true,
        code: 0,
        out: args.includes('-list') ? 'TestOne\nok example.test/core' : 'ok',
      };
    };
    assert.equal(probeForgeCoreTests(dir, green).status, PASS);
    assert.equal(probeBuild(dir, green).status, PASS);
    assert.equal(probeTypecheck(dir, green).status, PASS);
    assert.deepEqual(
      calls.map(({ cmd, args, cwd }) => [cmd, args.join(' '), cwd]),
      [
        ['go', 'test -count=1 ./...', core],
        ['go', 'test -list . ./...', core],
        ['go', 'build ./...', core],
        ['go', 'vet ./...', core],
      ],
    );

    const red = probeBuild(dir, () => ({ ok: false, code: 1, out: 'compile broke' }));
    assert.equal(red.status, FAIL, 'a real core build failure must remain FAIL');
    assert.match(red.detail, /compile broke/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('command runner keeps verbose evidence within a finite fail-closed buffer', () => {
  const aboveNodeDefault = 1024 * 1024 + 4096;
  const verbose = run(process.execPath, [
    '-e', `process.stdout.write('v'.repeat(${aboveNodeDefault}))`,
  ]);
  assert.equal(verbose.ok, true, verbose.out.slice(-200));
  assert.equal(verbose.out.length, aboveNodeDefault);

  const overflow = run(process.execPath, [
    '-e', `process.stdout.write('x'.repeat(${COMMAND_OUTPUT_MAX_BYTES + 65536}))`,
  ]);
  assert.equal(overflow.ok, false, 'output beyond the finite cap must fail closed');
  assert.equal(overflow.code, null);
  assert.match(overflow.out, /ENOBUFS/);
  assert.match(overflow.out, /^x+/, 'partial command evidence must survive ENOBUFS');
  assert.ok(
    overflow.out.length <= COMMAND_OUTPUT_MAX_BYTES + 1024 * 1024,
    'captured evidence must remain bounded near the configured limit',
  );
});

test('projects without forge-core get honest inapplicable build/typecheck N/A', () => {
  const dir = mkdtempSync(join(tmpdir(), 'no-forge-core-'));
  try {
    const build = withCategory(probeBuild(dir));
    const typecheck = withCategory(probeTypecheck(dir));
    assert.equal(build.status, NA);
    assert.equal(build.category, INAPPLICABLE);
    assert.equal(typecheck.status, NA);
    assert.equal(typecheck.category, INAPPLICABLE);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('package build scripts and TypeScript sources are applicable, never free N/A exemptions', () => {
  const dir = mkdtempSync(join(tmpdir(), 'web-project-probes-'));
  try {
    writeFileSync(join(dir, 'package.json'), JSON.stringify({ scripts: { build: 'vite build' } }));
    const buildCalls = [];
    const build = probeBuild(dir, (cmd, args, env, cwd) => {
      buildCalls.push([cmd, args, cwd]);
      return { ok: false, code: 1, out: 'bundle failed' };
    });
    assert.equal(build.status, FAIL);
    assert.deepEqual(buildCalls, [['npm', ['run', 'build'], dir]]);

    mkdirSync(join(dir, 'src'));
    writeFileSync(join(dir, 'src', 'main.ts'), 'export const n: number = 1;\n');
    const unconfigured = withCategory(probeTypecheck(dir));
    assert.equal(unconfigured.status, NA);
    assert.equal(unconfigured.category, NO_TOOL, 'TS without a configured checker blocks production');

    writeFileSync(join(dir, 'tsconfig.json'), '{}\n');
    const calls = [];
    const checked = probeTypecheck(dir, (cmd, args, env, cwd) => {
      calls.push([cmd, args, cwd]);
      return { ok: true, code: 0, out: '' };
    });
    assert.equal(checked.status, PASS);
    assert.deepEqual(calls, [['tsc', ['--noEmit'], dir]]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('production blocks critical no_tool N/A but exempts inapplicable criteria', () => {
  const results = acceptanceRows([
    { criterion: 'lint', status: NA, detail: 'golangci-lint not installed', category: NO_TOOL },
    { criterion: 'coverage', status: NA, detail: 'no source languages detected', category: INAPPLICABLE },
    { criterion: 'build', status: NA, detail: 'no build step here', category: INAPPLICABLE },
    { criterion: 'typecheck', status: NA, detail: 'no TS sources here', category: INAPPLICABLE },
    { criterion: 'dependency_vulnerabilities', status: PASS, detail: 'SCA clean' },
  ]);
  const verdict = decide(results, 'production');
  assert.equal(verdict.accepted, false);
  assert.match(verdict.line, /lint not satisfied \(N-A\/no_tool; lifecycle=production\)/);
  assert.doesNotMatch(verdict.line, /build not satisfied|typecheck not satisfied|coverage not satisfied/);
});

test('immature lifecycle exempts no_tool; production accepts all-inapplicable N/A', () => {
  const critical = ['lint', 'coverage', 'build', 'typecheck', 'dependency_vulnerabilities'];
  const noTool = acceptanceRows(
    critical.map((criterion) => ({ criterion, status: NA, detail: 'tool missing', category: NO_TOOL })),
  );
  assert.equal(decide(noTool, 'growth').accepted, true);

  const inapplicable = acceptanceRows(
    critical.map((criterion) => ({ criterion, status: NA, detail: 'not applicable', category: INAPPLICABLE })),
  );
  assert.equal(decide(inapplicable, 'production').accepted, true);
});

test('unknown lifecycle is strict and missing critical rows cannot silently pass', () => {
  const verdict = decide(acceptanceRows([]), 'unknown');
  assert.equal(verdict.accepted, false);
  assert.match(verdict.line, /lint not satisfied \(absent; lifecycle=unknown\)/);
  assert.match(verdict.line, /build not satisfied \(absent; lifecycle=unknown\)/);
  assert.match(verdict.line, /dependency_vulnerabilities not satisfied \(absent; lifecycle=unknown\)/);
});

test('project lifecycle reader accepts quoted YAML and fails safe when absent', () => {
  const dir = mkdtempSync(join(tmpdir(), 'project-lifecycle-'));
  try {
    assert.equal(readProjectLifecycle(dir), 'unknown');
    mkdirSync(join(dir, '.agent'));
    writeFileSync(join(dir, '.agent', 'project.yml'), 'lifecycle: "production" # strict\n');
    assert.equal(readProjectLifecycle(dir), 'production');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('project tests recursively run fixed argv for Go/Node/Python/Rust/Java modules', () => {
  const root = mkdtempSync(join(tmpdir(), 'recursive-project-tests-'));
  try {
    const files = {
      'go-svc/go.mod': 'module example.test/go-svc\ngo 1.22\n',
      'node-svc/package.json': JSON.stringify({ scripts: { test: 'node --test' } }),
      'py-svc/pyproject.toml': '[project]\nname = "py-svc"\nversion = "0.1.0"\n',
      'forge-runtime/Cargo.toml': '[workspace]\nmembers = []\n',
      'forge-runtime/src/lib.rs': 'pub fn answer() -> u8 { 42 }\n',
      'java-svc/settings.gradle': 'rootProject.name = "java-svc"\n',
      'java-svc/gradlew': '#!/bin/sh\n',
      'java-svc/src/main/java/App.java': 'class App {}\n',
    };
    for (const [rel, text] of Object.entries(files)) {
      const path = join(root, rel);
      mkdirSync(join(path, '..'), { recursive: true });
      writeFileSync(path, text);
    }
    const calls = [];
    const result = probeProjectTests(root, (cmd, args, env, cwd) => {
      calls.push([cmd, args, cwd]);
      return {
        ok: true,
        code: 0,
        out: [
          'TestOne',
          'running 1 test',
          '# tests 1',
          '1 passed',
          '1 tests completed',
        ].join('\n'),
      };
    });
    assert.equal(result.status, PASS);
    assert.deepEqual(calls, [
      ['cargo', ['--version'], join(root, 'forge-runtime')],
      ['cargo', ['test', '--all-targets', '--all-features'], join(root, 'forge-runtime')],
      ['./gradlew', ['--version'], join(root, 'java-svc')],
      ['./gradlew', ['test'], join(root, 'java-svc')],
      ['go', ['version'], join(root, 'go-svc')],
      ['go', ['test', '-count=1', './...'], join(root, 'go-svc')],
      ['go', ['test', '-list', '.', './...'], join(root, 'go-svc')],
      ['npm', ['--version'], join(root, 'node-svc')],
      ['npm', ['test'], join(root, 'node-svc')],
      ['python3', ['-c', 'import pytest'], join(root, 'py-svc')],
      ['python3', ['-m', 'pytest', '-q'], join(root, 'py-svc')],
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('a discovered project test failure or missing test script cannot be omitted', () => {
  const root = mkdtempSync(join(tmpdir(), 'project-test-fail-closed-'));
  try {
    mkdirSync(join(root, 'go-svc'));
    writeFileSync(join(root, 'go-svc', 'go.mod'), 'module example.test/broken\ngo 1.22\n');
    const failed = probeProjectTests(root, (cmd, args) => (
      args[0] === 'version'
        ? { ok: true, code: 0, out: 'go version' }
        : { ok: false, code: 1, out: 'broken tests' }
    ));
    assert.equal(failed.status, FAIL);
    assert.match(failed.detail, /broken tests/);

    rmSync(join(root, 'go-svc'), { recursive: true, force: true });
    mkdirSync(join(root, 'node-svc'));
    writeFileSync(join(root, 'node-svc', 'package.json'), '{}\n');
    const missing = withCategory(probeProjectTests(root, () => {
      throw new Error('missing scripts.test must not execute npm');
    }));
    assert.equal(missing.status, NA);
    assert.equal(missing.category, NO_TOOL);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('successful project commands with zero or unobservable tests cannot PASS', () => {
  const cases = [
    {
      name: 'cargo',
      files: {
        'Cargo.toml': '[package]\nname = "empty"\nversion = "0.1.0"\n',
        'src/lib.rs': 'pub fn empty() {}\n',
      },
      zero: 'running 0 tests\ntest result: ok. 0 passed; 0 failed',
    },
    {
      name: 'go',
      files: {
        'go.mod': 'module example.test/empty\ngo 1.22\n',
        'main.go': 'package empty\n',
      },
      zero: '? example.test/empty [no test files]',
    },
    {
      name: 'npm',
      files: {
        'package.json': JSON.stringify({ scripts: { test: 'node --test' } }),
      },
      zero: 'TAP version 13\n# tests 0',
    },
    {
      name: 'pytest',
      files: {
        'pyproject.toml': '[project]\nname = "empty"\nversion = "0.1.0"\n',
      },
      zero: 'no tests ran in 0.01s',
    },
    {
      name: 'maven',
      files: {
        'pom.xml': '<project></project>\n',
        'mvnw': '#!/bin/sh\n',
        'src/main/java/App.java': 'class App {}\n',
      },
      zero: 'Tests run: 0, Failures: 0, Errors: 0, Skipped: 0',
    },
    {
      name: 'gradle',
      files: {
        'settings.gradle': 'rootProject.name = "empty"\n',
        'gradlew': '#!/bin/sh\n',
        'src/main/java/App.java': 'class App {}\n',
      },
      zero: '0 tests completed, 0 failed',
    },
  ];
  for (const fixture of cases) {
    const root = mkdtempSync(join(tmpdir(), `zero-${fixture.name}-`));
    try {
      for (const [rel, text] of Object.entries(fixture.files)) {
        const path = join(root, rel);
        mkdirSync(join(path, '..'), { recursive: true });
        writeFileSync(path, text);
      }
      const row = probeProjectTests(root, (cmd, args) => (
        args[0] === '--version' || args[0] === 'version' || args[0] === '-c'
          ? { ok: true, code: 0, out: 'tool available' }
          : { ok: true, code: 0, out: fixture.zero }
      ));
      assert.equal(row.status, FAIL, `${fixture.name} zero-test run must fail`);
      assert.match(row.detail, /executed 0 tests/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }

  const root = mkdtempSync(join(tmpdir(), 'unknown-node-count-'));
  try {
    writeFileSync(join(root, 'package.json'), JSON.stringify({ scripts: { test: 'custom-runner' } }));
    const row = withCategory(probeProjectTests(root, (cmd, args) => (
      args[0] === '--version'
        ? { ok: true, code: 0, out: 'npm 1' }
        : { ok: true, code: 0, out: 'custom runner completed' }
    )));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    assert.match(row.detail, /no observable test count/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('manifestless Go, Node, and Python source are explicit unconfigured test targets', () => {
  const root = mkdtempSync(join(tmpdir(), 'unowned-native-source-'));
  try {
    writeFileSync(join(root, 'orphan.go'), 'package orphan\n');
    writeFileSync(join(root, 'orphan.js'), 'export const orphan = true;\n');
    writeFileSync(join(root, 'orphan.py'), 'orphan = True\n');
    const row = withCategory(probeProjectTests(root, () => {
      throw new Error('unconfigured source must not spawn a test command');
    }));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    for (const lang of ['go', 'node', 'python']) {
      assert.match(row.detail, new RegExp(`${lang} source detected outside`));
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('an unreadable project subtree is an explicit no_tool failure, never inapplicable', () => {
  const root = mkdtempSync(join(tmpdir(), 'unreadable-project-source-'));
  try {
    const blocked = join(root, 'blocked');
    mkdirSync(blocked);
    writeFileSync(join(blocked, 'orphan.py'), 'hidden = True\n');
    const readDir = (path, options) => {
      if (path === blocked) throw new Error('EACCES synthetic unreadable project subtree');
      return readdirSync(path, options);
    };
    const row = withCategory(probeProjectTests(root, () => {
      throw new Error('an unreadable source tree must not spawn a test command');
    }, readDir));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    assert.match(row.detail, /project discovery unreadable at blocked/);
    assert.match(row.detail, /EACCES synthetic unreadable project subtree/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('a manifest lstat failure cannot turn a listed project into inapplicable N/A', () => {
  const root = mkdtempSync(join(tmpdir(), 'unreadable-project-manifest-'));
  try {
    const blocked = join(root, 'blocked');
    const manifest = join(blocked, 'package.json');
    mkdirSync(blocked);
    writeFileSync(manifest, JSON.stringify({ scripts: { test: 'node --test' } }));
    const discoveryIO = {
      readDir: readdirSync,
      lstat(path) {
        if (path === manifest) {
          const err = new Error('EACCES synthetic manifest lstat');
          err.code = 'EACCES';
          throw err;
        }
        return lstatSync(path);
      },
    };
    const row = withCategory(probeProjectTests(root, () => {
      throw new Error('an uninspectable manifest must not spawn a test command');
    }, discoveryIO));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    assert.match(row.detail, /project discovery unreadable at blocked\/package\.json/);
    assert.match(row.detail, /EACCES synthetic manifest lstat/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('an existing non-regular manifest is an explicit discovery failure', () => {
  const root = mkdtempSync(join(tmpdir(), 'nonregular-project-manifest-'));
  try {
    mkdirSync(join(root, 'package.json'));
    const row = withCategory(probeProjectTests(root, () => {
      throw new Error('a directory-shaped manifest must not spawn a test command');
    }));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    assert.match(row.detail, /project discovery unreadable at package\.json/);
    assert.match(row.detail, /expected a regular project manifest\/tool file/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('a project path cannot spoof an inapplicable category through its label', () => {
  const root = mkdtempSync(join(tmpdir(), 'category-injection-project-'));
  try {
    const project = join(root, 'no source languages');
    mkdirSync(project);
    writeFileSync(join(project, 'package.json'), '{}\n');
    const row = withCategory(probeProjectTests(root, () => {
      throw new Error('an unconfigured package must not spawn a test command');
    }));
    assert.equal(row.status, NA);
    assert.equal(row.category, NO_TOOL);
    assert.match(row.detail, /package\.json has no scripts\.test/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('app_test_pass rejects a Go example whose command exits 0 but enumerates zero tests', () => {
  const root = mkdtempSync(join(tmpdir(), 'zero-go-example-'));
  try {
    const app = join(root, 'examples', 'empty-go');
    mkdirSync(join(app, 'test'), { recursive: true });
    writeFileSync(join(app, 'go.mod'), 'module example.test/empty-go\ngo 1.22\n');
    writeFileSync(join(app, 'test', 'empty_test.go'), 'package test\n');
    const row = probeAppTests(root, (cmd, args) => {
      if (args[0] === 'version') return { ok: true, code: 0, out: 'go version' };
      if (args.includes('-list')) return { ok: true, code: 0, out: 'ok example.test/empty-go/test' };
      return { ok: true, code: 0, out: 'ok example.test/empty-go/test' };
    });
    assert.equal(row.status, FAIL);
    assert.match(row.detail, /executed 0 tests/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
