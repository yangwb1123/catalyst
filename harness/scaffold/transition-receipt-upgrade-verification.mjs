// ADR-0060 legacy-upgrade assertions kept separate from the upgrade orchestrator test.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

function runPython(target, args, label) {
  const result = spawnSync('python3', args, { cwd: target, encoding: 'utf8' });
  assert.equal(result.status, 0,
    `upgraded ${label} must pass\n${result.stdout}\n${result.stderr}`);
  return result;
}

export function assertTransitionReceiptUpgrade(target) {
  const files = [
    ['docs', 'adr', '0060-transition-receipt-v1-contract-only.md'],
    ['docs', 'contracts', 'transition-receipt-v1.schema.json'],
    ['docs', 'contracts', 'fixtures', 'transition-receipt-v1.json'],
    ['harness', 'transition_receipt_contract_check.py'],
    ['harness', 'transition_receipt_contract', 'contract.py'],
    ['harness', 'governance_engineering', 'transition_receipt.py'],
    ['harness', 'governance_engineering', 'test_transition_receipt.py'],
    ['harness', 'test_transition_receipt_contract_check.py'],
    ['harness', 'test_transition_receipt_cross_contract.py'],
  ];
  for (const parts of files) assert.equal(existsSync(join(target, ...parts)), true);
  runPython(target, ['-B', 'harness/test_transition_receipt_contract_check.py'],
    'TransitionReceipt tests');
  runPython(target, ['-B', 'harness/test_transition_receipt_cross_contract.py'],
    'TransitionReceipt cross-contract tests');
  runPython(target, ['-B', 'harness/governance_engineering/test_transition_receipt.py'],
    'TransitionReceipt governance test');
  const check = runPython(target,
    ['-B', 'harness/transition_receipt_contract_check.py', '--golden', '.'],
    'TransitionReceipt golden checker');
  assert.match(check.stdout, /declarations only; no transition authority/);
  assert.equal(existsSync(join(target, 'forge-core')), false);
  assert.equal(existsSync(join(target, 'forge-runtime')), false);
  assert.equal(existsSync(join(target, 'forge-kernel')), false);
  assert.equal(existsSync(join(target, '.forge', 'governance')), false);
}
