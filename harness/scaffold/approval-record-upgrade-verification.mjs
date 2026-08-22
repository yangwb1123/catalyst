// ADR-0059 legacy-upgrade assertions kept separate from the upgrade orchestrator test.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

function runPython(target, args, label) {
  const result = spawnSync('python3', args, { cwd: target, encoding: 'utf8' });
  assert.equal(
    result.status, 0,
    `upgraded ${label} must pass\n${result.stdout}\n${result.stderr}`,
  );
  return result;
}

export function assertApprovalRecordUpgrade(target) {
  const files = [
    ['docs', 'adr', '0059-approval-record-v1-contract-only.md'],
    ['docs', 'contracts', 'approval-record-v1.schema.json'],
    ['docs', 'contracts', 'fixtures', 'approval-record-v1.json'],
    ['harness', 'approval_record_contract_check.py'],
    ['harness', 'approval_record_contract', 'contract.py'],
    ['harness', 'governance_engineering', 'approval_record.py'],
    ['harness', 'governance_engineering', 'test_approval_record.py'],
    ['harness', 'test_approval_record_contract_check.py'],
    ['harness', 'test_approval_record_capability_grant_contract.py'],
  ];
  for (const parts of files) assert.equal(existsSync(join(target, ...parts)), true);
  runPython(target, ['-B', 'harness/test_approval_record_contract_check.py'],
    'ApprovalRecord tests');
  runPython(target, ['-B', 'harness/test_approval_record_capability_grant_contract.py'],
    'ApprovalRecord Grant compatibility tests');
  runPython(target, ['-B', 'harness/governance_engineering/test_approval_record.py'],
    'ApprovalRecord governance test');
  const check = runPython(target,
    ['-B', 'harness/approval_record_contract_check.py', '--golden', '.'],
    'ApprovalRecord golden checker');
  assert.match(check.stdout, /declarations only; no authority/);
  assert.equal(existsSync(join(target, 'forge-core')), false);
  assert.equal(existsSync(join(target, 'forge-runtime')), false);
}
