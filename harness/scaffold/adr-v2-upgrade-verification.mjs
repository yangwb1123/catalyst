import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { ADR_V2_EXPECTED_FILES } from './adr-v2-copy-fragment.mjs';


export const ADR_V2_LEGACY_FILES = ADR_V2_EXPECTED_FILES;
const GOLDEN = join(
  'docs', 'contracts', 'fixtures', 'ADR-9001-proposed-boundary.md',
);
const GOLDEN_SHA256 =
  'b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194';

function assertFilesAndGoldenBytes(target) {
  for (const relative of ADR_V2_EXPECTED_FILES) {
    assert.equal(existsSync(join(target, relative)), true,
      `proposed ADR v2 scaffold asset missing: ${relative}`);
  }
  const bytes = readFileSync(join(target, GOLDEN));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), GOLDEN_SHA256,
    'proposed ADR v2 golden bytes must match the authoritative pin');
}

function assertGoldenAndAdversarialTests(target) {
  const golden = spawnSync(
    'python3', ['-B', 'harness/architecture_decision_record_v2_check.py',
      '--golden', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(golden.status, 0,
    `proposed ADR v2 golden must validate\n${golden.stdout}\n${golden.stderr}`);
  assert.match(golden.stdout, /VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2/);
  const tests = spawnSync(
    'python3', ['-B', 'harness/test_architecture_decision_record_v2_check.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(tests.status, 0,
    `proposed ADR v2 tests must pass\n${tests.stdout}\n${tests.stderr}`);
}

function assertProposedOnlyBoundary(target) {
  const raw = readFileSync(join(target, GOLDEN), 'utf8');
  const metadata = JSON.parse(raw.split('\n', 3)[1]);
  assert.equal(metadata.status, 'proposed');
  assert.equal(metadata.acceptance_id, null);
  assert.equal(metadata.accepted_at_unix_ms, null);
  assert.deepEqual(metadata.superseded_by, []);
  assert.equal('approval_record' in metadata, false);
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'universal ADR v2 scaffold must not install Catalyst-specific Go runtime');
}

export function assertADRV2Scaffold(target) {
  assertFilesAndGoldenBytes(target);
  assertGoldenAndAdversarialTests(target);
  assertProposedOnlyBoundary(target);
}
