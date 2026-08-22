import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { OWNERSHIP_PROJECTION_EXPECTED_FILES } from './ownership-projection-copy-fragment.mjs';

export const OWNERSHIP_PROJECTION_LEGACY_FILES = OWNERSHIP_PROJECTION_EXPECTED_FILES;
const FIXTURE = join('docs', 'contracts', 'fixtures',
  'planning-capability-ownership-projection-v1.json');
const FIXTURE_SHA256 = '3d0a877bef0939cff5752fc5d602e0d3a90e19639308801008f9d2d9ff139f36';
const SOURCE_PINS = new Map([
  [join('docs', 'design', 'ai-engineering-os', 'capability-catalog.v1.yml'),
    [33000, 'bc6efe535539c5f129af51486d8e81b9844b5ee6448fae2bce649fc159658d74']],
  [join('docs', 'design', 'ai-engineering-os', 'capability-skill-map.v1.yml'),
    [5924, 'bfb2277fe66cd9f0c609b5be10ad77ad0969603edd19e5a6ccbe38b8e3409462']],
]);

export function assertOwnershipProjectionScaffold(target) {
  for (const relative of OWNERSHIP_PROJECTION_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `ownership projection scaffold asset missing: ${relative}`);
  }
  const fixture = readFileSync(join(target, FIXTURE));
  assert.equal(createHash('sha256').update(fixture).digest('hex'), FIXTURE_SHA256,
    'ownership projection golden bytes must match frozen physical pin');
  for (const [relative, [bytes, sha256]] of SOURCE_PINS) {
    const source = readFileSync(join(target, relative));
    assert.equal(source.length, bytes, `${relative} exact byte pin drifted`);
    assert.equal(createHash('sha256').update(source).digest('hex'), sha256,
      `${relative} exact source digest drifted`);
  }
  const check = spawnSync('python3', ['-B',
    'harness/planning_capability_ownership_projection/check.py', '--golden', '.'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(check.status, 0, `${check.stdout}\n${check.stderr}`);
  assert.match(check.stdout, /planning only/);
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal scaffold must not claim Catalyst Go implementation availability');
  assert.equal(existsSync(join(target, '.agent', 'skills', 'delivery-planning.md')), false,
    'projection scaffold must not generate a declared owner Skill adapter');
  const golden = JSON.parse(fixture);
  const binding = golden.projection.bindings.find(
    ({ owner_skill: owner }) => owner === 'adr-governance');
  assert.deepEqual([binding.physical_resolution, binding.skill_availability],
    ['not_performed', 'not_evaluated'],
    'an existing same-name Markdown file must remain logically unresolved');
}
