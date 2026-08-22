import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { PROJECT_SNAPSHOT_EXPECTED_FILES } from './project-snapshot-copy-fragment.mjs';

export const PROJECT_SNAPSHOT_LEGACY_FILES = PROJECT_SNAPSHOT_EXPECTED_FILES;
const SCHEMA = join('docs', 'contracts', 'project-source-snapshot-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures', 'project-source-snapshot-v1.json');
const SCHEMA_SHA256 = '7e281850579329356090dd19b9f66a8544f7af04c37ac01bca87bad1824a23b1';
const GOLDEN_SHA256 = '4b23a9c5896a7b279fb4f7a17a4939791f94489c5311354c8b008fdfe665de89';

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

export function assertProjectSnapshotScaffold(target) {
  for (const relative of PROJECT_SNAPSHOT_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Project Snapshot scaffold asset missing: ${relative}`);
  }
  assert.equal(sha256(join(target, SCHEMA)), SCHEMA_SHA256,
    'Project Snapshot Schema must match the frozen physical pin');
  assert.equal(sha256(join(target, GOLDEN)), GOLDEN_SHA256,
    'Project Snapshot golden must match the frozen physical pin');

  const checker = spawnSync('python3', ['-I', '-B',
    'harness/project_source_snapshot_contract/check.py', '--golden', '.'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(checker.status, 0, `${checker.stdout}\n${checker.stderr}`);
  assert.match(checker.stdout, /authority neutral/);

  const packageCheck = spawnSync('python3', ['-I', '-B',
    'skills/project-snapshot/scripts/check_package.py'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(packageCheck.status, 0,
    `${packageCheck.stdout}\n${packageCheck.stderr}`);
  assert.match(packageCheck.stdout, /portable package VALID/);
  const portableTests = spawnSync('python3', ['-I', '-B',
    'skills/project-snapshot/tests/test_portable_scripts.py'], {
    cwd: target, encoding: 'utf8',
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
  });
  assert.equal(portableTests.status, 0,
    `${portableTests.stdout}\n${portableTests.stderr}`);
  const packageTests = spawnSync('python3', ['-I', '-B',
    'skills/project-snapshot/tests/test_package_integrity.py'], {
    cwd: target, encoding: 'utf8',
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
  });
  assert.equal(packageTests.status, 0,
    `${packageTests.stdout}\n${packageTests.stderr}`);
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal scaffold must not copy the Catalyst Project Snapshot runtime');
  assert.equal(existsSync(join(target, 'forge')), false,
    'universal scaffold must not install a compatible forge runtime');
}
