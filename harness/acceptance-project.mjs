// Project-specific acceptance probes that must not be faked by the harness's
// own self-tests. A ForgeOS source checkout carries forge-core/go.mod, so its Go
// test/build/vet checks are mandatory. A copied harness normally does not carry
// forge-core; there the same criteria are honestly INAPPLICABLE.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { isAbsolute, join, relative, sep } from 'node:path';

import {
  PASS,
  FAIL,
  NA,
  ROOT,
  APPLICABLE,
  INAPPLICABLE,
  NO_TOOL,
  categorize,
  result,
  run,
} from './acceptance-kernel.mjs';
import {
  observedTestCount, probeDiscoveredProjectTests, probeProjectOperations,
} from './adapters/project.mjs';

const IMMATURE_LIFECYCLES = new Set(['idea', 'mvp', 'growth']);
export const CRITICAL_NA_CRITERIA = [
  'lint', 'build', 'typecheck', 'coverage', 'dependency_vulnerabilities',
];

function coreDir(root) {
  const dir = join(root, 'forge-core');
  return existsSync(join(dir, 'go.mod')) ? dir : null;
}

function rootGoDir(root) {
  return existsSync(join(root, 'go.mod')) ? root : null;
}

const SOURCE_SKIP_DIRS = new Set([
  '.git', '.forge', 'node_modules', 'vendor', 'dist', 'build', 'target', 'coverage',
]);

function hasTypeScriptSources(dir) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return null;
  }
  let unreadable = false;
  for (const entry of entries) {
    if (entry.isDirectory()) {
      if (SOURCE_SKIP_DIRS.has(entry.name)) continue;
      const nested = hasTypeScriptSources(join(dir, entry.name));
      if (nested === true) return true;
      if (nested === null) unreadable = true;
    } else if (entry.isFile() && /\.(?:ts|tsx)$/.test(entry.name)) {
      return true;
    }
  }
  return unreadable ? null : false;
}

function packageBuildScript(root) {
  const manifest = join(root, 'package.json');
  if (!existsSync(manifest)) return { present: false, script: null };
  try {
    const pkg = JSON.parse(readFileSync(manifest, 'utf8'));
    return { present: true, script: pkg?.scripts?.build || null };
  } catch (err) {
    return { present: true, error: err.message, script: null };
  }
}

function commandDetail(label, r) {
  if (r.ok) return `${label}: PASS`;
  const clipped = r.out?.length > 1600
    ? `${r.out.slice(0, 600)}\n… output clipped …\n${r.out.slice(-1000)}`
    : r.out;
  const output = clipped ? ` — ${clipped}` : '';
  return `${label}: FAIL (exit ${r.code ?? 'unavailable'})${output}`;
}

function aggregateRows(criterion, rows, noneDetail) {
  const relevant = rows.filter(
    (row) => (row.category ?? categorize(row.status, row.detail)) !== INAPPLICABLE,
  );
  if (relevant.length === 0) {
    return { ...result(criterion, NA, noneDetail), category: INAPPLICABLE };
  }
  const detail = relevant.map((row) => row.detail).join('; ');
  if (relevant.some((row) => row.status === FAIL)) {
    return { ...result(criterion, FAIL, detail), category: APPLICABLE };
  }
  if (relevant.some((row) => row.status === NA)) {
    return { ...result(criterion, NA, detail), category: NO_TOOL };
  }
  return { ...result(criterion, PASS, detail), category: APPLICABLE };
}

// A helper result consumed by probeTests(): it is not emitted as a separate
// acceptance criterion because Go tests are part of the existing test_pass
// contract. Missing forge-core is a legitimate copied-project shape.
export function probeForgeCoreTests(root = ROOT, exec = run) {
  const cwd = coreDir(root);
  if (!cwd) {
    return result('forge_core_test', NA, 'forge-core/go.mod absent; core Go tests are inapplicable');
  }
  const args = ['test', '-count=1', './...'];
  const r = exec('go', args, {}, cwd);
  if (!r.ok) return result('forge_core_test', FAIL, commandDetail('go test', r));
  const evidence = exec('go', ['test', '-list', '.', './...'], {}, cwd);
  if (!evidence.ok) return result('forge_core_test', FAIL, commandDetail('go test -list', evidence));
  const count = observedTestCount({ lang: 'go' }, evidence.out);
  if (count === 0) return result('forge_core_test', FAIL, 'go test exited 0 but executed 0 tests');
  if (count === null) {
    return result('forge_core_test', NA, 'go test exited 0 but emitted no observable test count');
  }
  return result('forge_core_test', PASS, `go test PASS (${count} test(s) observed)`);
}

// Project tests are load-bearing for recursively discovered Go, Node, Python,
// Rust, and Java manifests/workspaces. A missing tool/config remains N/A/no_tool
// here and is promoted to a test_pass failure by acceptance.mjs.
export function probeProjectTests(root = ROOT, exec = run, discoveryIO = readdirSync) {
  const examples = join(root, 'examples');
  const outsideExamples = (plan) => {
    const rel = relative(examples, plan.root);
    return isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`);
  };
  const rows = probeDiscoveredProjectTests(root, exec, discoveryIO, outsideExamples);
  return aggregateRows(
    'project_test',
    rows,
    'no source languages detected: no project test targets',
  );
}

// build is real whenever this repository carries forge-core. A missing Go
// executable is a failure, not N/A: the source tree declared a required target.
function probeNativeBuild(root, exec) {
  const cwd = coreDir(root);
  if (cwd) {
    const r = exec('go', ['build', './...'], {}, cwd);
    return result('build', r.ok ? PASS : FAIL, commandDetail('forge-core go build ./...', r));
  }

  const goRoot = rootGoDir(root);
  if (goRoot) {
    const r = exec('go', ['build', './...'], {}, goRoot);
    if (r.code === null) return result('build', NA, 'Go build target detected but go is not installed');
    return result('build', r.ok ? PASS : FAIL, commandDetail('go build ./...', r));
  }

  const pkg = packageBuildScript(root);
  if (pkg.error) return result('build', FAIL, `package.json is invalid: ${pkg.error}`);
  if (pkg.script) {
    const r = exec('npm', ['run', 'build'], {}, root);
    if (r.code === null) return result('build', NA, 'package build script detected but npm is not installed');
    return result('build', r.ok ? PASS : FAIL, commandDetail('npm run build', r));
  }
  if (existsSync(join(root, 'pyproject.toml'))) {
    return result('build', NA, 'Python project build target detected but no build tool is configured');
  }
  return result('build', NA,
    `no build step: ${pkg.present ? 'package.json has no scripts.build' : 'no build target detected'}`);
}

export function probeBuild(root = ROOT, exec = run) {
  const native = probeNativeBuild(root, exec);
  const rows = [native, ...probeProjectOperations(root, 'build', exec)];
  return aggregateRows('build', rows, native.detail);
}

// Go compilation performs type checking; go vet adds the repository's static
// type/format analysis surface. It is therefore the concrete typecheck proof for
// forge-core, while projects with neither forge-core nor TS sources remain N/A.
function probeNativeTypecheck(root, exec) {
  const cwd = coreDir(root) || rootGoDir(root);
  if (cwd) {
    const r = exec('go', ['vet', './...'], {}, cwd);
    if (r.code === null && cwd === root) {
      return result('typecheck', NA, 'Go sources detected but go vet is not installed');
    }
    const label = cwd === root ? 'go vet ./...' : 'forge-core go vet ./...';
    return result('typecheck', r.ok ? PASS : FAIL, commandDetail(label, r));
  }

  const hasTypeScript = hasTypeScriptSources(root);
  if (hasTypeScript === null) {
    return result('typecheck', NA, 'could not inspect project sources for a typecheck target');
  }
  if (!hasTypeScript) {
    return result('typecheck', NA, 'no TS sources / type-checker and no forge-core Go module');
  }
  if (!existsSync(join(root, 'tsconfig.json'))) {
    return result('typecheck', NA, 'TypeScript sources detected but no tsconfig.json/type-checker is configured');
  }
  const r = exec('tsc', ['--noEmit'], {}, root);
  if (r.code === null) return result('typecheck', NA, 'TypeScript sources detected but tsc is not installed');
  return result('typecheck', r.ok ? PASS : FAIL, commandDetail('tsc --noEmit', r));
}

export function probeTypecheck(root = ROOT, exec = run) {
  const native = probeNativeTypecheck(root, exec);
  const rows = [native, ...probeProjectOperations(root, 'typecheck', exec)];
  return aggregateRows('typecheck', rows, native.detail);
}

// Read the central lifecycle knob without adding a YAML dependency. Missing or
// malformed input is "unknown", which criticalNAReasons treats strictly.
export function readProjectLifecycle(root = ROOT) {
  try {
    const text = readFileSync(join(root, '.agent', 'project.yml'), 'utf8');
    const match = text.match(/^\s*lifecycle\s*:\s*([^\s#]+)/m);
    return match?.[1]?.trim().replace(/^(['"])(.*)\1$/, '$2').toLowerCase() || 'unknown';
  } catch {
    return 'unknown';
  }
}

// Mirrors forge-core's N/A exemption policy for the acceptance criteria whose
// absence can otherwise hide production readiness gaps. INAPPLICABLE is exempt
// at every lifecycle. NO_TOOL is exempt only before production. Unknown
// lifecycle/category is fail-safe and blocks.
export function criticalNAReasons(results, lifecycle = 'mvp') {
  const byName = new Map(results.map((r) => [r.criterion, r]));
  const immature = IMMATURE_LIFECYCLES.has(String(lifecycle).toLowerCase());
  const strictLifecycle = immature ? null : (lifecycle || 'unknown');
  const reasons = [];

  for (const name of CRITICAL_NA_CRITERIA) {
    const row = byName.get(name);
    if (!row) {
      if (strictLifecycle) reasons.push(`${name} not satisfied (absent; lifecycle=${strictLifecycle})`);
      continue;
    }
    if (row.status !== NA) continue;
    const category = row.category ?? categorize(row.status, row.detail);
    if (category === INAPPLICABLE) continue;
    if (category === NO_TOOL && immature) continue;
    reasons.push(`${name} not satisfied (N-A/${category || 'unknown'}; lifecycle=${lifecycle || 'unknown'})`);
  }
  return reasons;
}
