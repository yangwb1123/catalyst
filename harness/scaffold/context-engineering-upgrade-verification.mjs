import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  CONTEXT_ENGINEERING_EXPECTED_FILES,
} from './context-engineering-copy-fragment.mjs';

export const CONTEXT_ENGINEERING_LEGACY_FILES = CONTEXT_ENGINEERING_EXPECTED_FILES;
const SCHEMA = join('docs', 'contracts', 'context-package-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures', 'context-package-v1.json');
const MANIFEST = join(
  'skills', 'context-engineering', 'references', 'package-manifest.json',
);
const SCHEMA_SHA256 = '2e2a934393026c96ebe7e2098462303192fd345aae10eebcf79544a69d7621e3';
const GOLDEN_SHA256 = '1a1c9866f7472055736866be9007040cc8e3d938bb04244bd04fd3bec2aa4b55';
const MANIFEST_SHA256 = '7590df136eb828ba3ffe4892efffa2ab4a77fb87dff8a1bffccdde2d015852c5';

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target, encoding: 'utf8',
    env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1' },
  });
}

export function assertContextEngineeringScaffold(target) {
  for (const relative of CONTEXT_ENGINEERING_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Context Engineering scaffold asset missing: ${relative}`);
  }
  assert.equal(sha256(join(target, SCHEMA)), SCHEMA_SHA256,
    'ContextPackage Schema must match the frozen physical pin');
  assert.equal(sha256(join(target, GOLDEN)), GOLDEN_SHA256,
    'ContextPackage golden must match the frozen physical pin');
  assert.equal(sha256(join(target, MANIFEST)), MANIFEST_SHA256,
    'Context Engineering manifest must match the closed package pin');

  const packageCheck = runPython(target, [
    '-I', '-B', 'skills/context-engineering/scripts/check_package.py',
  ]);
  assert.equal(packageCheck.status, 0,
    `${packageCheck.stdout}\n${packageCheck.stderr}`);
  assert.match(packageCheck.stdout, /portable package VALID/);
  const portableTests = runPython(target, [
    '-I', '-B', 'skills/context-engineering/tests/test_portable_scripts.py',
  ]);
  assert.equal(portableTests.status, 0,
    `${portableTests.stdout}\n${portableTests.stderr}`);
  const goldenCheck = runPython(target, [
    '-B', 'harness/context_package_contract_check.py', '--golden', '.',
  ]);
  assert.equal(goldenCheck.status, 0,
    `${goldenCheck.stdout}\n${goldenCheck.stderr}`);
  assert.match(goldenCheck.stdout, /ContextPackage v1 golden: OK/);

  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal scaffold must not copy the Catalyst ContextPackage Go runtime');
  assert.equal(existsSync(join(target, 'forge-runtime')), false,
    'universal scaffold must not copy the Catalyst ContextPackage Rust runtime');
  assert.equal(existsSync(join(target, '.codex', 'skills', 'context-engineering')), false,
    'universal scaffold must not install a host Context Engineering Skill');
}
