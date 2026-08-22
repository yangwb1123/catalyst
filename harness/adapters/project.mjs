// Manifest-aware project-command planning and execution for acceptance probes.
//
// Plans are data, not shell strings at the execution boundary: every command is
// split into a fixed executable plus argv and is passed to spawnSync with no
// shell. Java selects only the repository-local `mvnw` / `gradlew` wrappers;
// it never interpolates a project path or manifest value into a command.
import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import {
  dirname, extname, isAbsolute, join, relative, sep,
} from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseRules } from '../arch/scan.mjs';
import {
  hasLanguageSource, langForExt, walkAdapterSources,
} from './detection.mjs';
import { executeCommandPlan } from './project-execution.mjs';

const ADAPTERS_DIR = dirname(fileURLToPath(import.meta.url));

export const PROJECT_ADAPTER_LANGS = ['rust', 'java'];
// Go lint must run from each go.mod root; invoking it from a repository root
// without a module is an infrastructure error, not a verdict on Go sources.
export const PROJECT_LINT_LANGS = ['go', ...PROJECT_ADAPTER_LANGS];
export const PROJECT_OPERATIONS = ['test', 'lint', 'typecheck', 'build'];
export { executeCommandPlan, observedTestCount } from './project-execution.mjs';

const SHELL_TOKENS = new Set(['&&', '||', '|', ';', '>', '>>', '<', '<<']);
const PROJECT_SKIP_DIRS = new Set([
  '.git', '.forge', '.gradle', '.idea', '.venv', 'node_modules', 'vendor',
  'dist', 'build', 'coverage', 'target', 'out', 'testdata', '__pycache__',
]);
const TEST_PROJECT_SKIP_DIRS = new Set([
  ...PROJECT_SKIP_DIRS, '.agent', '.ai', '.arch', '.github', 'docs', 'harness', 'skills',
]);

const JAVA_PROFILES = [
  {
    name: 'maven',
    manifests: ['pom.xml'],
    wrapper: 'mvnw',
    command: './mvnw',
  },
  {
    name: 'gradle',
    manifests: [
      'build.gradle', 'build.gradle.kts', 'settings.gradle', 'settings.gradle.kts',
    ],
    wrapper: 'gradlew',
    command: './gradlew',
  },
];

function regularFile(path, context = null) {
  try {
    const st = (context?.lstat ?? lstatSync)(path);
    if (st.isFile()) return true;
    const err = new Error('expected a regular project manifest/tool file');
    if (!context) throw err;
    recordDiscoveryError(context, path, err);
    return false;
  } catch (err) {
    // Only a genuinely absent leaf is inapplicable. Permission failures and an
    // existing symlink/directory/FIFO-shaped manifest are discovery failures;
    // swallowing either would let an unreadable project become a false green.
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return false;
    if (!context) throw err;
    recordDiscoveryError(context, path, err);
    return false;
  }
}

function hasAny(root, names, context = null) {
  // Inspect every alternative, even after finding one valid manifest. A valid
  // pyproject.toml/build.gradle must not hide an unsafe sibling setup.py or
  // build.gradle.kts from the discovery-error ledger.
  let found = false;
  for (const name of names) {
    if (regularFile(join(root, name), context)) found = true;
  }
  return found;
}

function hasProjectManifest(root, lang, context = null) {
  if (lang === 'go') return regularFile(join(root, 'go.mod'), context);
  if (lang === 'rust') return regularFile(join(root, 'Cargo.toml'), context);
  if (lang === 'java') {
    return JAVA_PROFILES.some((profile) => hasAny(root, profile.manifests, context));
  }
  return false;
}

function createDiscoveryContext(root, discoveryIO = readdirSync) {
  const io = typeof discoveryIO === 'function'
    ? { readDir: discoveryIO }
    : (discoveryIO ?? {});
  return {
    root,
    readDir: io.readDir ?? readdirSync,
    lstat: io.lstat ?? lstatSync,
    errors: new Map(),
  };
}

function recordDiscoveryError(context, path, err) {
  if (!context) return;
  const rel = relative(context.root, path).replace(/\\/g, '/') || '.';
  const message = err?.message ?? String(err);
  context.errors.set(rel, message);
}

function sourceWalkOptions(context) {
  if (!context) return {};
  return {
    readDir: context.readDir,
    onError: (path, err) => recordDiscoveryError(context, path, err),
  };
}

function walkProjectRoots(root, lang, acc = [], context = null) {
  let entries;
  try {
    entries = (context?.readDir ?? readdirSync)(root, { withFileTypes: true });
  } catch (err) {
    recordDiscoveryError(context, root, err);
    return acc;
  }
  for (const entry of entries) {
    if (!entry.isDirectory() || PROJECT_SKIP_DIRS.has(entry.name)) continue;
    const child = join(root, entry.name);
    if (hasProjectManifest(child, lang, context)) acc.push(child);
    // A manifest is not proof that it owns every nested manifest. Keep walking
    // so a root workspace/aggregator cannot hide an independent child project.
    walkProjectRoots(child, lang, acc, context);
  }
  return acc;
}

function pathIsWithin(path, root) {
  const rel = relative(root, path);
  return rel === ''
    || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith(`..${sep}`));
}

function hasUnownedSource(root, lang, projectRoots, context = null) {
  return walkAdapterSources(root, [], sourceWalkOptions(context))
    .filter((path) => langForExt(extname(path)) === lang)
    .some((path) => !projectRoots.some((projectRoot) => pathIsWithin(path, projectRoot)));
}

function inspectTestProjectTree(root, context) {
  const dirs = [];
  const files = [];
  function walk(dir) {
    dirs.push(dir);
    let entries;
    try {
      entries = context.readDir(dir, { withFileTypes: true });
    } catch (err) {
      recordDiscoveryError(context, dir, err);
      return;
    }
    for (const entry of entries) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (!TEST_PROJECT_SKIP_DIRS.has(entry.name)) walk(path);
      } else if (entry.isFile()) {
        files.push(path);
      }
    }
  }
  walk(root);
  return { dirs, files };
}

export function commandArgv(command, expected) {
  if (typeof command !== 'string') return null;
  const parts = command.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0 || parts[0] !== expected) return null;
  if (parts.some(
    (token) => SHELL_TOKENS.has(token) || /[;&|<>]/.test(token) || token.includes('\0'),
  )) {
    return null;
  }
  return { cmd: parts[0], args: parts.slice(1) };
}

function unavailablePlan(root, lang, operation, applicable, reason) {
  return {
    root, lang, operation, applicable, configured: false, reason,
  };
}

function profileCommands(lang, profile) {
  const text = readFileSync(join(ADAPTERS_DIR, `${lang}.yml`), 'utf8');
  const parsed = parseRules(text);
  const map = profile ? parsed?.toolchains?.[profile]?.commands : parsed?.commands;
  return {
    test: map?.test?.run,
    lint: map?.lint?.run,
    typecheck: map?.typecheck?.run,
    build: map?.build?.run,
  };
}

function commandPlan(root, lang, operation, profile, expected) {
  const command = profileCommands(lang, profile)?.[operation];
  const argv = commandArgv(command, expected);
  if (!argv) {
    return unavailablePlan(
      root, lang, operation, true,
      `${lang} adapter ${operation} command is missing or not argv-safe`,
    );
  }
  return {
    root,
    lang,
    operation,
    profile,
    applicable: true,
    configured: true,
    tool: expected,
    probeArgs: ['--version'],
    ...argv,
  };
}

function rustPlan(root, operation, context = null) {
  const manifest = regularFile(join(root, 'Cargo.toml'), context);
  const source = hasLanguageSource(root, 'rust', sourceWalkOptions(context));
  if (!manifest && !source) {
    return unavailablePlan(
      root, 'rust', operation, false,
      'no source languages detected: no Rust project',
    );
  }
  if (!manifest) {
    return unavailablePlan(
      root, 'rust', operation, true,
      'Rust source detected but Cargo.toml is missing',
    );
  }
  return commandPlan(root, 'rust', operation, null, 'cargo');
}

function goPlan(root, operation, context = null) {
  const manifest = regularFile(join(root, 'go.mod'), context);
  const source = hasLanguageSource(root, 'go', sourceWalkOptions(context));
  if (!manifest && !source) {
    return unavailablePlan(
      root, 'go', operation, false,
      'no source languages detected: no Go project',
    );
  }
  if (!manifest) {
    return unavailablePlan(
      root, 'go', operation, true,
      'Go source detected but go.mod is missing',
    );
  }
  const expected = operation === 'lint' ? 'golangci-lint' : 'go';
  return commandPlan(root, 'go', operation, null, expected);
}

export function selectJavaProfile(root, context = null) {
  const states = JAVA_PROFILES.map((profile) => ({
    ...profile,
    manifest: hasAny(root, profile.manifests, context),
    wrapperPresent: regularFile(join(root, profile.wrapper), context),
  }));
  return states.find((state) => state.manifest && state.wrapperPresent)
    ?? states.find((state) => state.manifest)
    ?? null;
}

function javaPlan(root, operation, context = null) {
  const source = hasLanguageSource(root, 'java', sourceWalkOptions(context));
  const selected = selectJavaProfile(root, context);
  if (!source && !selected) {
    return unavailablePlan(
      root, 'java', operation, false,
      'no source languages detected: no Java project',
    );
  }
  if (!selected) {
    return unavailablePlan(
      root, 'java', operation, true,
      'Java source detected but no Maven/Gradle build descriptor is configured',
    );
  }
  if (!selected.wrapperPresent) {
    return unavailablePlan(
      root, 'java', operation, true,
      `Java ${selected.name} project detected but ${selected.wrapper} is missing`,
    );
  }
  return commandPlan(root, 'java', operation, selected.name, selected.command);
}

function commandPlanForContext(root, lang, operation, context) {
  if (!PROJECT_OPERATIONS.includes(operation)) {
    throw new Error(`unsupported project adapter operation '${operation}'`);
  }
  if (lang === 'go') return goPlan(root, operation, context);
  if (lang === 'rust') return rustPlan(root, operation, context);
  if (lang === 'java') return javaPlan(root, operation, context);
  throw new Error(`unsupported project adapter language '${lang}'`);
}

export function projectCommandPlan(root, lang, operation) {
  const context = createDiscoveryContext(root);
  const plan = commandPlanForContext(root, lang, operation, context);
  return context.errors.size > 0 ? discoveryErrorPlans(context, operation)[0] : plan;
}

// Run every unique manifest root, including both the repository root and nested
// manifests. We deliberately do not infer Cargo workspace / Java aggregator
// ownership from the mere presence of a root descriptor: an independent nested
// project may not be a member. Safe duplicate execution is preferable to a
// false green. Source outside every discovered project remains an explicit
// unconfigured plan so a valid child project cannot hide orphan source.
function commandPlansForContext(root, lang, operation, context) {
  const projectRoots = [
    ...(hasProjectManifest(root, lang, context) ? [root] : []),
    ...walkProjectRoots(root, lang, [], context),
  ];
  if (projectRoots.length === 0) {
    return [commandPlanForContext(root, lang, operation, context)];
  }
  const uniqueRoots = [...new Set(projectRoots)].sort();
  const plans = uniqueRoots.map(
    (projectRoot) => commandPlanForContext(projectRoot, lang, operation, context),
  );
  if (hasUnownedSource(root, lang, uniqueRoots, context)) {
    plans.push(unavailablePlan(
      root,
      lang,
      operation,
      true,
      `${lang} source detected outside every configured project manifest`,
    ));
  }
  return plans;
}

function discoveryErrorPlans(context, operation) {
  return [...context.errors.entries()].sort(([a], [b]) => a.localeCompare(b))
    .map(([rel, message]) => ({
      ...unavailablePlan(
        context.root,
        'project',
        operation,
        true,
        `project discovery unreadable at ${rel} (${message})`,
      ),
      label: `project:unreadable:${rel}`,
    }));
}

export function projectCommandPlans(root, lang, operation, discoveryIO = readdirSync) {
  const context = createDiscoveryContext(root, discoveryIO);
  return [
    ...commandPlansForContext(root, lang, operation, context),
    ...discoveryErrorPlans(context, operation),
  ];
}

function fixedTestPlan(root, lang, cmd, args, probeArgs, label) {
  return {
    root,
    lang,
    operation: 'test',
    applicable: true,
    configured: true,
    tool: cmd,
    cmd,
    args,
    probeArgs,
    label,
  };
}

function nodeTestPlan(root, label) {
  let pkg;
  try {
    pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  } catch (err) {
    return {
      ...unavailablePlan(root, 'node', 'test', true, `invalid package.json (${err.message})`),
      label,
    };
  }
  if (!pkg?.scripts?.test) {
    return {
      ...unavailablePlan(root, 'node', 'test', true, 'package.json has no scripts.test'),
      label,
    };
  }
  return fixedTestPlan(root, 'node', 'npm', ['test'], ['--version'], label);
}

function pythonTestPlan(root, label) {
  return {
    ...fixedTestPlan(
      root, 'python', 'python3', ['-m', 'pytest', '-q'], ['-c', 'import pytest'], label,
    ),
    tool: 'python3+pytest',
  };
}

// Test discovery covers every project ecosystem rather than only the two
// project-quality adapters. Rust/Java reuse their manifest-aware recursive
// planners; Go modules, Node packages, and Python package roots use fixed argv
// plans and are never passed through a shell.
export function discoverProjectTestPlans(root, discoveryIO = readdirSync) {
  const context = createDiscoveryContext(root, discoveryIO);
  const plans = [];
  for (const lang of PROJECT_ADAPTER_LANGS) {
    plans.push(
      ...commandPlansForContext(root, lang, 'test', context)
        .filter((plan) => plan.applicable),
    );
  }
  const tree = inspectTestProjectTree(root, context);
  const roots = new Map([
    ['go', []],
    ['node', []],
    ['python', []],
  ]);
  for (const dir of tree.dirs.sort()) {
    const rel = relative(root, dir) || '.';
    if (regularFile(join(dir, 'go.mod'), context)) {
      roots.get('go').push(dir);
      const plan = fixedTestPlan(
        dir, 'go', 'go', ['test', '-count=1', './...'], ['version'], `go:${rel}`,
      );
      plan.evidenceArgs = ['test', '-list', '.', './...'];
      plans.push(plan);
    }
    if (regularFile(join(dir, 'package.json'), context)) {
      roots.get('node').push(dir);
      plans.push(nodeTestPlan(dir, `node:${rel}`));
    }
    if (hasAny(dir, ['pyproject.toml', 'setup.cfg', 'setup.py'], context)) {
      roots.get('python').push(dir);
      plans.push(pythonTestPlan(dir, `python:${rel}`));
    }
  }
  plans.push(...unownedNativeSourcePlans(root, roots, tree.files));
  plans.push(...discoveryErrorPlans(context, 'test'));
  return plans;
}

function unownedNativeSourcePlans(root, roots, files) {
  const extensions = new Map([
    ['.go', 'go'],
    ['.mjs', 'node'], ['.cjs', 'node'], ['.js', 'node'],
    ['.jsx', 'node'], ['.ts', 'node'], ['.tsx', 'node'],
    ['.py', 'python'],
  ]);
  const orphaned = new Set();
  for (const file of files) {
    const lang = extensions.get(extname(file));
    if (!lang) continue;
    if (!roots.get(lang).some((projectRoot) => pathIsWithin(file, projectRoot))) {
      orphaned.add(lang);
    }
  }
  return [...orphaned].sort().map((lang) => ({
    ...unavailablePlan(
      root,
      lang,
      'test',
      true,
      `${lang} source detected outside every configured project manifest`,
    ),
    label: `${lang}:unowned-source`,
  }));
}

export function probeProjectOperations(root, operation, exec, discoveryIO = readdirSync) {
  const context = createDiscoveryContext(root, discoveryIO);
  const rows = [];
  const languages = operation === 'lint' ? PROJECT_LINT_LANGS : PROJECT_ADAPTER_LANGS;
  for (const lang of languages) {
    for (const plan of commandPlansForContext(root, lang, operation, context)) {
      if (plan.applicable) rows.push(executeCommandPlan(plan, exec));
    }
  }
  for (const plan of discoveryErrorPlans(context, operation)) {
    rows.push(executeCommandPlan(plan, exec));
  }
  return rows;
}

export function probeDiscoveredProjectTests(root, exec, discoveryIO = readdirSync) {
  return discoverProjectTestPlans(root, discoveryIO)
    .map((plan) => executeCommandPlan(plan, exec));
}
