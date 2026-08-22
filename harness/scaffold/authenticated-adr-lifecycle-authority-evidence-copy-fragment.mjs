// ADR-0084/0085 lifecycle authority evidence source-only projection.
// Exactly four governance files are distributed. The Catalyst Go contract,
// authority, keys, state, runtime and effects remain outside the scaffold.
import { join } from 'node:path';

export const AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0084-authenticated-architecture-decision-lifecycle-authority-service-v1.md'),
  join('docs', 'adr',
    'ADR-0085-authenticated-architecture-decision-lifecycle-authority-evidence-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'authenticated_adr_lifecycle_authority_evidence.py'),
  join('harness', 'governance_engineering',
    'test_authenticated_adr_lifecycle_authority_evidence.py'),
];

export const AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES =
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_COPIED_FILES;
