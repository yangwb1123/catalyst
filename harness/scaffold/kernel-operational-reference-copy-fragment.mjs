// ADR-0088/0089 Kernel operational reference core source distribution.
// Exactly eighteen Python/governance files are copied. Catalyst Go/Rust are not.
import { join } from 'node:path';

export const KERNEL_OPERATIONAL_REFERENCE_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0088-kernel-operational-reference-core-v1.md'),
  join('docs', 'contracts', 'kernel-operational-reference-core-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'kernel-operational-reference-closure-v1.json'),
  ...[
    '__init__.py', 'closure.py', 'codec.py', 'constants.py',
    'fixture.py', 'graph.py', 'records.py', 'shape.py',
  ].map((name) => join('harness', 'kernel_operational_contract', name)),
  join('harness', 'kernel_operational_contract_check.py'),
  join('harness', 'test_kernel_operational_contract.py'),
  join('harness', 'test_kernel_operational_cross_contract.py'),
  join('harness', 'test_kernel_operational_reference_graph.py'),
  join('docs', 'adr',
    'ADR-0089-kernel-operational-reference-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'kernel_operational_reference_candidate.py'),
  join('harness', 'governance_engineering',
    'test_kernel_operational_reference_candidate.py'),
];

export const KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES =
  KERNEL_OPERATIONAL_REFERENCE_COPIED_FILES;
