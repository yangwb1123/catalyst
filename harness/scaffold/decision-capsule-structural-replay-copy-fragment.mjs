// ADR-0092/0093 Decision Capsule structural replay source distribution.
// Exactly nineteen source/governance files are copied. Catalyst Go/Rust are not.
import { join } from 'node:path';

export const DECISION_CAPSULE_STRUCTURAL_REPLAY_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0092-decision-capsule-structural-replay-core-v1.md'),
  join('docs', 'contracts',
    'decision-capsule-structural-replay-core-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'decision-capsule-structural-replay-v1.json'),
  ...[
    '__init__.py', 'branch.py', 'capsule.py', 'closure.py', 'codec.py',
    'constants.py', 'fixture.py', 'manifest.py', 'shape.py',
  ].map((name) => join('harness', 'decision_capsule_contract', name)),
  join('harness', 'decision_capsule_contract_check.py'),
  join('harness', 'test_decision_capsule_contract.py'),
  join('harness', 'test_decision_capsule_replay_graph.py'),
  join('harness', 'test_decision_capsule_strict.py'),
  join('docs', 'adr',
    'ADR-0093-decision-capsule-structural-replay-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'decision_capsule_structural_replay_candidate.py'),
  join('harness', 'governance_engineering',
    'test_decision_capsule_structural_replay_candidate.py'),
];

export const DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES =
  DECISION_CAPSULE_STRUCTURAL_REPLAY_COPIED_FILES;
