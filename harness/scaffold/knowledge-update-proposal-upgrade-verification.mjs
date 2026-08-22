// ADR-0061 legacy-upgrade assertions kept outside the near-cap test orchestrator.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

export const KNOWLEDGE_UPDATE_PROPOSAL_LEGACY_FILES = [
  join('docs', 'adr', '0061-knowledge-update-proposal-v1-contract-only.md'),
  join('docs', 'contracts', 'knowledge-update-proposal-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'knowledge-update-proposal-v1.json'),
  join('harness', 'knowledge_update_proposal_contract_check.py'),
  join('harness', 'test_knowledge_update_proposal_contract_check.py'),
  join('harness', 'test_knowledge_update_proposal_cross_contract.py'),
  join('harness', 'knowledge_update_proposal_contract'),
];

function runPython(target, args, label) {
  const result = spawnSync('python3', args, { cwd: target, encoding: 'utf8' });
  assert.equal(result.status, 0,
    `upgraded ${label} must pass\n${result.stdout}\n${result.stderr}`);
  return result;
}

export function assertKnowledgeUpdateProposalUpgrade(target) {
  const files = [
    ['docs', 'adr', '0061-knowledge-update-proposal-v1-contract-only.md'],
    ['docs', 'contracts', 'knowledge-update-proposal-v1.schema.json'],
    ['docs', 'contracts', 'fixtures', 'knowledge-update-proposal-v1.json'],
    ['harness', 'knowledge_update_proposal_contract_check.py'],
    ['harness', 'knowledge_update_proposal_contract', 'contract.py'],
    ['harness', 'governance_engineering', 'knowledge_update_proposal.py'],
    ['harness', 'governance_engineering', 'test_knowledge_update_proposal.py'],
    ['harness', 'test_knowledge_update_proposal_contract_check.py'],
    ['harness', 'test_knowledge_update_proposal_cross_contract.py'],
  ];
  for (const parts of files) assert.equal(existsSync(join(target, ...parts)), true);
  runPython(target, ['-B', 'harness/test_knowledge_update_proposal_contract_check.py'],
    'KnowledgeUpdateProposal tests');
  runPython(target, ['-B', 'harness/test_knowledge_update_proposal_cross_contract.py'],
    'KnowledgeUpdateProposal cross-contract tests');
  runPython(target, ['-B', 'harness/governance_engineering/test_knowledge_update_proposal.py'],
    'KnowledgeUpdateProposal governance test');
  const check = runPython(target,
    ['-B', 'harness/knowledge_update_proposal_contract_check.py', '--golden', '.'],
    'KnowledgeUpdateProposal golden checker');
  assert.match(check.stdout, /declarations only; no adoption or apply authority/);
  assert.equal(existsSync(join(target, 'forge-core')), false);
  assert.equal(existsSync(join(target, 'forge-runtime')), false);
  assert.equal(existsSync(join(target, 'forge-kernel')), false);
  assert.equal(existsSync(join(target, '.forge', 'governance')), false);
  assert.equal(existsSync(join(target, '.forge', 'knowledge')), false);
}
