// ADR-0077/0078 Proposed WorkIntent v1 source distribution. This exact
// fourteen-file projection excludes Catalyst Go/Rust, Skills and runtime state.
import { join } from 'node:path';

const CONTRACT_PACKAGE = [
  '__init__.py', 'codec.py', 'constants.py', 'fixture.py', 'record.py', 'shape.py',
].map((name) => join('harness', 'work_intent_contract', name));

export const WORK_INTENT_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0077-authority-neutral-work-intent-v1-contract.md'),
  join('docs', 'adr',
    'ADR-0078-work-intent-v1-proposed-candidate-governance-and-source-distribution.md'),
  join('docs', 'contracts', 'work-intent-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'work-intent-v1.json'),
  join('harness', 'work_intent_contract_check.py'),
  ...CONTRACT_PACKAGE,
  join('harness', 'test_work_intent_contract_check.py'),
  join('harness', 'governance_engineering', 'work_intent_candidate.py'),
  join('harness', 'governance_engineering', 'test_work_intent_candidate.py'),
];

export const WORK_INTENT_EXPECTED_FILES = WORK_INTENT_COPIED_FILES;
