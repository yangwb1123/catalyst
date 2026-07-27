// Rust/Java acceptance-adapter tests. Fixtures are created in temporary trees
// so this same test remains copy-anywhere when forge-init copies the harness.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  ADAPTER_LANGS, detectLanguages, loadAdapter,
} from './adapters.mjs';
import { probeCoverage, probeLint } from './acceptance-quality.mjs';
import {
  probeBuild, probeProjectTests, probeTypecheck,
} from './acceptance-project.mjs';
import {
  INAPPLICABLE, NO_TOOL, withCategory,
} from './acceptance-kernel.mjs';
import {
  commandArgv,
  executeCommandPlan,
  projectCommandPlan,
  projectCommandPlans,
  selectJavaProfile,
} from './adapters/project.mjs';

const PASS = 'PASS';
const FAIL = 'FAIL';
const NA = 'N-A';

function withTemp(prefix, fn) {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function write(root, rel, text = '') {
  const path = join(root, rel);
  mkdirSync(join(path, '..'), { recursive: true });
  writeFileSync(path, text);
}

function rustFixture(root) {
  write(root, 'Cargo.toml', '[package]\nname = "fixture"\nversion = "0.1.0"\n');
  write(root, 'src/lib.rs', 'pub fn answer() -> u8 { 42 }\n');
}

function mavenFixture(root) {
  write(root, 'pom.xml', '<project/>\n');
  write(root, 'mvnw', '#!/bin/sh\n');
  write(root, 'src/main/java/App.java', 'class App {}\n');
}

function gradleFixture(root) {
  write(root, 'build.gradle.kts', "plugins { java }\n");
  write(root, 'gradlew', '#!/bin/sh\n');
  write(root, 'src/main/java/App.java', 'class App {}\n');
}

function successfulExec(calls = []) {
  return (cmd, args, env, cwd) => {
    calls.push({ cmd, args, env, cwd });
    return {
      ok: true,
      code: 0,
      out: [
        'TestOne',
        'running 1 test',
        'test result: ok. 1 passed; 0 failed',
        '# tests 1',
        '1 passed',
        '1 tests completed',
      ].join('\n'),
    };
  };
}

test('Rust/Java declarations expose exact test/lint/typecheck/build argv maps', () => {
  assert.deepEqual(ADAPTER_LANGS, ['go', 'java', 'python', 'rust', 'typescript']);
  assert.deepEqual(loadAdapter('rust'), {
    lint: 'cargo clippy --all-targets --all-features -- -D warnings',
    test: 'cargo test --all-targets --all-features',
    typecheck: 'cargo check --all-targets --all-features',
    build: 'cargo build --all-targets --all-features',
    coverage: undefined,
  });
  assert.equal(loadAdapter('java', 'maven').test, './mvnw -q test');
  assert.equal(loadAdapter('java', 'maven').typecheck, './mvnw -q -DskipTests compile');
  assert.equal(loadAdapter('java', 'gradle').lint, './gradlew check -x test');
  assert.equal(loadAdapter('java', 'gradle').build, './gradlew assemble');
});

test('adapter detection sees .rs/.java and skips generated/testdata trees', () => {
  withTemp('polyglot-detect-', (root) => {
    write(root, 'crate/src/lib.rs', 'pub fn f() {}\n');
    write(root, 'service/src/main/java/App.java', 'class App {}\n');
    write(root, 'target/generated.rs', 'fn ignored() {}\n');
    write(root, 'testdata/ignored.java', 'class Ignored {}\n');
    assert.deepEqual(detectLanguages(root), ['java', 'rust']);
  });
});

test('common source detection never silently swallows an unreadable tree', () => {
  withTemp('polyglot-detect-unreadable-', (root) => {
    const readDir = () => {
      throw new Error('EACCES synthetic common source walker');
    };
    assert.throws(
      () => detectLanguages(root, { readDir }),
      /EACCES synthetic common source walker/,
    );
  });
});

test('Rust plans prefer cargo test/clippy/check/build with fixed argv', () => {
  withTemp('rust-plan-', (root) => {
    rustFixture(root);
    const operations = {
      test: ['test', '--all-targets', '--all-features'],
      lint: ['clippy', '--all-targets', '--all-features', '--', '-D', 'warnings'],
      typecheck: ['check', '--all-targets', '--all-features'],
      build: ['build', '--all-targets', '--all-features'],
    };
    for (const [operation, args] of Object.entries(operations)) {
      const plan = projectCommandPlan(root, 'rust', operation);
      assert.equal(plan.cmd, 'cargo');
      assert.deepEqual(plan.args, args);
      assert.equal(plan.root, root);
    }
  });
});

test('nested Rust workspace is discovered and executed from its own manifest root', () => {
  withTemp('nested-rust-plan-', (root) => {
    const runtime = join(root, 'forge-runtime');
    rustFixture(runtime);
    const plans = projectCommandPlans(root, 'rust', 'test');
    assert.equal(plans.length, 1);
    assert.equal(plans[0].root, runtime);

    const calls = [];
    const exec = successfulExec(calls);
    assert.equal(probeProjectTests(root, exec).status, PASS);
    assert.equal(probeLint(root, exec).status, PASS);
    assert.equal(probeTypecheck(root, exec).status, PASS);
    assert.equal(probeBuild(root, exec).status, PASS);
    assert.deepEqual(
      calls
        .filter(({ args }) => args[0] !== '--version')
        .map(({ cmd, args, cwd }) => [cmd, args, cwd]),
      [
        ['cargo', ['test', '--all-targets', '--all-features'], runtime],
        ['cargo', ['clippy', '--all-targets', '--all-features', '--', '-D', 'warnings'], runtime],
        ['cargo', ['check', '--all-targets', '--all-features'], runtime],
        ['cargo', ['build', '--all-targets', '--all-features'], runtime],
      ],
    );
  });
});

test('Rust/Java root and source walkers surface deterministic readdir failures', () => {
  withTemp('polyglot-unreadable-', (root) => {
    rustFixture(root);
    write(root, 'pom.xml', '<project/>\n');
    write(root, 'mvnw', '#!/bin/sh\n');
    write(root, 'src/main/java/App.java', 'class App {}\n');
    const readDir = (path, options) => {
      if (path === root) throw new Error('EACCES synthetic polyglot root');
      return readdirSync(path, options);
    };
    for (const lang of ['rust', 'java']) {
      const plans = projectCommandPlans(root, lang, 'test', readDir);
      assert.ok(plans.some((plan) => plan.configured), `${lang} manifest plan remains visible`);
      const blocked = plans.find((plan) => plan.lang === 'project');
      assert.ok(blocked, `${lang} discovery error must be an explicit plan`);
      assert.equal(blocked.configured, false);
      assert.match(blocked.reason, /project discovery unreadable at \./);
      assert.match(blocked.reason, /EACCES synthetic polyglot root/);
    }
  });
});

test('root aggregators cannot hide an independent nested failing manifest', () => {
  withTemp('root-and-nested-projects-', (root) => {
    const nestedRust = join(root, 'independent-rust');
    rustFixture(root);
    rustFixture(nestedRust);
    gradleFixture(root);
    gradleFixture(join(root, 'independent-java'));

    assert.deepEqual(
      projectCommandPlans(root, 'rust', 'test').map((plan) => plan.root),
      [root, nestedRust],
    );
    assert.deepEqual(
      projectCommandPlans(root, 'java', 'test').map((plan) => plan.root),
      [root, join(root, 'independent-java')],
    );

    const row = probeProjectTests(root, (cmd, args, env, cwd) => {
      if (args[0] === '--version') return { ok: true, code: 0, out: 'tool 1' };
      if (cwd === nestedRust) {
        return { ok: false, code: 1, out: 'independent nested tests failed' };
      }
      return successfulExec()(cmd, args, env, cwd);
    });
    assert.equal(row.status, FAIL);
    assert.match(row.detail, /independent nested tests failed/);
  });
});

test('nested project cannot hide Rust source outside every Cargo manifest', () => {
  withTemp('orphan-rust-source-', (root) => {
    rustFixture(join(root, 'forge-runtime'));
    write(root, 'loose/orphan.rs', 'pub fn orphan() {}\\n');
    const plans = projectCommandPlans(root, 'rust', 'build');
    assert.equal(plans.length, 2);
    assert.equal(plans[0].configured, true);
    assert.equal(plans[1].configured, false);
    assert.match(plans[1].reason, /outside every configured project manifest/);
  });
});

test('Java selects a complete Maven/Gradle wrapper and never a partial profile', () => {
  withTemp('java-select-', (root) => {
    write(root, 'pom.xml', '<project/>\n');
    gradleFixture(root);
    assert.equal(selectJavaProfile(root).name, 'gradle',
      'a complete Gradle wrapper wins over Maven without mvnw');
    write(root, 'mvnw', '#!/bin/sh\n');
    assert.equal(selectJavaProfile(root).name, 'maven',
      'when both are complete the stable Maven-first preference applies');
    assert.deepEqual(projectCommandPlan(root, 'java', 'build').args,
      ['-q', '-DskipTests', 'package']);
  });
});

test('argv parser rejects shell control tokens and unexpected executables', () => {
  assert.deepEqual(commandArgv('cargo test --all-targets', 'cargo'), {
    cmd: 'cargo', args: ['test', '--all-targets'],
  });
  assert.equal(commandArgv('cargo test && touch owned', 'cargo'), null);
  assert.equal(commandArgv('./gradlew test; touch owned', './gradlew'), null);
  assert.equal(commandArgv('sh -c cargo-test', 'cargo'), null);
});

test('execution uses fixed argv/cwd even when the project path contains shell text', () => {
  withTemp('argv-safe-', (base) => {
    const root = join(base, 'java;touch-owned');
    mkdirSync(root);
    gradleFixture(root);
    const calls = [];
    const row = executeCommandPlan(
      projectCommandPlan(root, 'java', 'build'),
      successfulExec(calls),
    );
    assert.equal(row.status, PASS);
    assert.deepEqual(calls.map(({ cmd, args, cwd }) => [cmd, args, cwd]), [
      ['./gradlew', ['--version'], root],
      ['./gradlew', ['assemble'], root],
    ]);
  });
});

test('missing cargo/wrapper is honest N/A no_tool, never PASS or an attempted shell', () => {
  withTemp('missing-tools-', (root) => {
    rustFixture(root);
    const calls = [];
    const rust = executeCommandPlan(
      projectCommandPlan(root, 'rust', 'build'),
      (cmd, args, env, cwd) => {
        calls.push({ cmd, args, env, cwd });
        return { ok: false, code: null, out: 'spawn cargo ENOENT' };
      },
    );
    assert.equal(rust.status, NA);
    assert.equal(withCategory(rust).category, NO_TOOL);
    assert.equal(calls.length, 1, 'missing cargo stops before cargo build');

    const javaRoot = join(root, 'java');
    mkdirSync(javaRoot);
    write(javaRoot, 'pom.xml', '<project/>\n');
    write(javaRoot, 'src/main/java/App.java', 'class App {}\n');
    const plan = projectCommandPlan(javaRoot, 'java', 'test');
    assert.equal(plan.configured, false);
    const java = executeCommandPlan(plan, () => {
      throw new Error('missing wrapper must not be executed');
    });
    assert.equal(java.status, NA);
    assert.equal(withCategory(java).category, NO_TOOL);
  });
});

test('no project is inapplicable N/A and source without a manifest is no_tool', () => {
  withTemp('no-project-', (root) => {
    const absent = projectCommandPlan(root, 'rust', 'build');
    assert.equal(absent.applicable, false);
    const row = executeCommandPlan(absent, () => {
      throw new Error('an absent project must not run a command');
    });
    assert.equal(row.status, NA);
    assert.equal(withCategory(row).category, INAPPLICABLE);

    write(root, 'src/lib.rs', 'pub fn f() {}\n');
    const unconfigured = executeCommandPlan(
      projectCommandPlan(root, 'rust', 'typecheck'),
      () => { throw new Error('Cargo.toml is required before execution'); },
    );
    assert.equal(unconfigured.status, NA);
    assert.equal(withCategory(unconfigured).category, NO_TOOL);
  });
});

test('real command failures remain FAIL; absent clippy is N/A/no_tool', () => {
  withTemp('rust-results-', (root) => {
    rustFixture(root);
    const failed = executeCommandPlan(
      projectCommandPlan(root, 'rust', 'build'),
      (cmd, args) => args[0] === '--version'
        ? { ok: true, code: 0, out: 'cargo 1' }
        : { ok: false, code: 101, out: 'error[E0308]: mismatched types' },
    );
    assert.equal(failed.status, FAIL);
    assert.match(failed.detail, /mismatched types/);

    const clippy = executeCommandPlan(
      projectCommandPlan(root, 'rust', 'lint'),
      (cmd, args) => args[0] === '--version'
        ? { ok: true, code: 0, out: 'cargo 1' }
        : { ok: false, code: 101, out: 'no such command: clippy' },
    );
    assert.equal(clippy.status, NA);
    assert.equal(withCategory(clippy).category, NO_TOOL);
  });
});

test('acceptance project probes run Rust test/check/build and preserve no-project N/A', () => {
  withTemp('rust-probes-', (root) => {
    rustFixture(root);
    const calls = [];
    const exec = successfulExec(calls);
    assert.equal(probeProjectTests(root, exec).status, PASS);
    assert.equal(probeTypecheck(root, exec).status, PASS);
    assert.equal(probeBuild(root, exec).status, PASS);
    const operations = calls
      .filter(({ args }) => args[0] !== '--version')
      .map(({ args }) => args[0]);
    assert.deepEqual(operations, ['test', 'check', 'build']);
  });
  withTemp('empty-probes-', (root) => {
    assert.equal(withCategory(probeProjectTests(root)).category, INAPPLICABLE);
    assert.equal(withCategory(probeTypecheck(root)).category, INAPPLICABLE);
    assert.equal(withCategory(probeBuild(root)).category, INAPPLICABLE);
  });
});

test('lint is fail-closed: one missing ecosystem tool cannot hide behind a PASS', () => {
  withTemp('mixed-lint-', (root) => {
    rustFixture(root);
    write(root, 'web/app.ts', 'export const n: number = 1;\n');
    const calls = [];
    const row = probeLint(root, (cmd, args, env, cwd) => {
      calls.push({ cmd, args, env, cwd });
      if (cmd === 'cargo') return { ok: false, code: null, out: 'ENOENT' };
      return { ok: true, code: 0, out: '' };
    });
    assert.equal(row.status, NA);
    assert.equal(withCategory(row).category, NO_TOOL);
    assert.ok(calls.some(({ cmd }) => cmd === 'eslint'));
    assert.ok(calls.some(({ cmd }) => cmd === 'cargo'));
  });
});

test('coverage is fail-closed and a missing Rust command is a production no_tool gap', () => {
  withTemp('mixed-coverage-', (root) => {
    rustFixture(root);
    write(root, 'go.mod', 'module example.test/mixed\ngo 1.22\n');
    write(root, 'main.go', 'package mixed\n');
    const calls = [];
    const row = probeCoverage(root, (cmd, args, env, cwd) => {
      calls.push({ cmd, args, env, cwd });
      if (cmd === 'go' && args[0] === 'version') {
        return { ok: true, code: 0, out: 'go version go1.22' };
      }
      if (cmd === 'go') {
        return { ok: true, code: 0, out: 'coverage: 90.0% of statements' };
      }
      throw new Error(`unexpected coverage executable ${cmd}`);
    });
    assert.equal(row.status, NA, 'Go PASS must not hide Rust N-A');
    assert.equal(withCategory(row).category, NO_TOOL);
    assert.deepEqual(calls.map(({ cmd, args, cwd }) => [cmd, args, cwd]), [
      ['go', ['version'], root],
      ['go', ['test', '-coverprofile=coverage.out', './...'], root],
    ]);
  });
});
