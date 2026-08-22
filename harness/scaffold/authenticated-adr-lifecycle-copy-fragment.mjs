// ADR-0081/0082/0083 lifecycle source-only candidate projection.
// The exact 24 files contain the frozen Python structural core, its evidence
// documents, and candidate governance. No Go authority, runtime, state, Skill,
// adapter, route, key, receipt, or persistence implementation is distributed.
import { join } from 'node:path';

const CONTRACT_PACKAGE = [
  '__init__.py', 'authority.py', 'canonical.py', 'constants.py', 'contract.py',
  'documents.py', 'fixture.py', 'ledger.py', 'prerequisite.py', 'proposal.py',
  'shape.py', 'state.py',
].map((name) => join('harness', 'authenticated_adr_lifecycle_contract', name));

export const AUTHENTICATED_ADR_LIFECYCLE_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0082-authenticated-architecture-decision-lifecycle-v1-prerequisite.md'),
  join('docs', 'contracts',
    'authenticated-architecture-decision-lifecycle-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'authenticated-architecture-decision-lifecycle-v1.json'),
  ...['ADR-9003-lifecycle-head-a.md', 'ADR-9004-lifecycle-head-b.md',
    'ADR-9005-lifecycle-join.md']
    .map((name) => join('docs', 'contracts', 'fixtures', name)),
  ...CONTRACT_PACKAGE,
  join('harness', 'authenticated_adr_lifecycle_contract_check.py'),
  join('harness', 'test_authenticated_adr_lifecycle_contract.py'),
  join('docs', 'adr',
    'ADR-0081-authenticated-architecture-decision-approval-authorization-service-v1.md'),
  join('docs', 'adr',
    'ADR-0083-authenticated-architecture-decision-lifecycle-v1-proposed-candidate-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'authenticated_adr_lifecycle_candidate.py'),
  join('harness', 'governance_engineering',
    'test_authenticated_adr_lifecycle_candidate.py'),
];

export const AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES =
  AUTHENTICATED_ADR_LIFECYCLE_COPIED_FILES;
