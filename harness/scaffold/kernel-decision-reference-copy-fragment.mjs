// ADR-0090/0091 Kernel decision reference core source distribution.
// Exactly nineteen Python/governance files are copied. Catalyst Go/Rust are not.
import { join } from 'node:path';

export const KERNEL_DECISION_REFERENCE_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0090-kernel-decision-reference-core-v1.md'),
  join('docs', 'contracts', 'kernel-decision-reference-core-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'kernel-decision-reference-closure-v1.json'),
  ...[
    '__init__.py', 'atoms.py', 'closure.py', 'codec.py', 'constants.py',
    'fixture.py', 'graph.py', 'shape.py', 'transaction.py',
  ].map((name) => join('harness', 'kernel_decision_contract', name)),
  join('harness', 'kernel_decision_contract_check.py'),
  join('harness', 'test_kernel_decision_contract.py'),
  join('harness', 'test_kernel_decision_reference_graph.py'),
  join('harness', 'test_kernel_decision_strict.py'),
  join('docs', 'adr',
    'ADR-0091-kernel-decision-reference-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'kernel_decision_reference_candidate.py'),
  join('harness', 'governance_engineering',
    'test_kernel_decision_reference_candidate.py'),
];

export const KERNEL_DECISION_REFERENCE_EXPECTED_FILES =
  KERNEL_DECISION_REFERENCE_COPIED_FILES;
