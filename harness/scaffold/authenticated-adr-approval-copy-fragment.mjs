// ADR-0079/0080 Proposed authenticated ADR approval v1 structural candidate.
// This exact 22-file source projection excludes future Go authority/services,
// production keys/state, Skills, adapters, routes and runtime bindings.
import { join } from 'node:path';

const CONTRACT_PACKAGE = [
  '__init__.py', 'approvals.py', 'authority.py', 'canonical.py', 'constants.py',
  'contract.py', 'documents.py', 'fixture.py', 'ledger.py', 'policy.py',
  'proposal.py', 'revocation.py', 'shape.py',
].map((name) => join('harness', 'authenticated_adr_approval_contract', name));

export const AUTHENTICATED_ADR_APPROVAL_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0079-authenticated-architecture-decision-approval-v1-prerequisite.md'),
  join('docs', 'contracts',
    'authenticated-architecture-decision-approval-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'authenticated-architecture-decision-approval-v1.json'),
  join('docs', 'contracts', 'fixtures',
    'ADR-9002-authenticated-approval-target.md'),
  ...CONTRACT_PACKAGE,
  join('harness', 'authenticated_adr_approval_contract_check.py'),
  join('harness', 'test_authenticated_adr_approval_contract.py'),
  join('docs', 'adr',
    'ADR-0080-authenticated-architecture-decision-approval-v1-proposed-candidate-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'authenticated_adr_approval_candidate.py'),
  join('harness', 'governance_engineering',
    'test_authenticated_adr_approval_candidate.py'),
];

export const AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES =
  AUTHENTICATED_ADR_APPROVAL_COPIED_FILES;
