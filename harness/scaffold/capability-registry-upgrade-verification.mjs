import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { CAPABILITY_REGISTRY_EXPECTED_FILES } from './capability-registry-copy-fragment.mjs';

export const CAPABILITY_REGISTRY_LEGACY_FILES = CAPABILITY_REGISTRY_EXPECTED_FILES;
const FIXTURE = join('docs', 'contracts', 'fixtures', 'capability-registry-v1.json');
const FIXTURE_SHA256 = '0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5';

function runPython(target, args, input) {
  return spawnSync('python3', ['-B', 'harness/capability_registry_contract/check.py', ...args],
    { cwd: target, encoding: 'utf8', input });
}

function assertFilesAndFixture(target) {
  for (const relative of CAPABILITY_REGISTRY_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `Capability Registry scaffold asset missing: ${relative}`);
  }
  const fixture = readFileSync(join(target, FIXTURE));
  assert.equal(createHash('sha256').update(fixture).digest('hex'), FIXTURE_SHA256,
    'Capability Registry golden bytes must match the frozen physical pin');
}

function assertLogicalCli(target) {
  const fixture = JSON.parse(readFileSync(join(target, FIXTURE), 'utf8'));
  const registry = JSON.stringify(fixture.registry);
  const request = JSON.stringify(fixture.requests.resolved_exact);
  const validate = runPython(target, ['validate', '--registry', '-'], registry);
  assert.equal(validate.status, 0, `${validate.stdout}\n${validate.stderr}`);
  assert.equal(validate.stdout, `${registry}\n`);
  const requestPath = join(target, '.agent', 'capability-registry-request.tmp.json');
  try {
    writeFileSync(requestPath, request, { flag: 'wx' });
    const resolve = runPython(
      target, ['resolve', '--registry', '-', '--request', requestPath], registry);
    assert.equal(resolve.status, 0, `${resolve.stdout}\n${resolve.stderr}`);
    assert.equal(resolve.stdout, `${JSON.stringify(fixture.assessments.resolved_exact)}\n`);
  } finally {
    if (existsSync(requestPath)) unlinkSync(requestPath);
  }
}

export function assertCapabilityRegistryScaffold(target) {
  assertFilesAndFixture(target);
  assertLogicalCli(target);
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal scaffold must not claim Catalyst Go implementation availability');
}
