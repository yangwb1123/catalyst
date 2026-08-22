// ADR-0086/0087 unverified legacy Memory/ADR read-import source distribution.
// Exactly eighteen Python/governance files are copied. Catalyst Go parity is not.
import { join } from 'node:path';

export const LEGACY_GOVERNANCE_READ_IMPORT_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0086-legacy-governance-read-only-import-v1.md'),
  join('docs', 'contracts', 'legacy-governance-read-import-v1.schema.json'),
  join('docs', 'contracts', 'fixtures',
    'legacy-governance-read-import-ADR-0001.md'),
  join('docs', 'contracts', 'fixtures',
    'legacy-governance-read-import-ADR-0002.md'),
  join('docs', 'contracts', 'fixtures',
    'legacy-governance-read-import-memory-v1.jsonl'),
  join('docs', 'contracts', 'fixtures',
    'legacy-governance-read-import-request-v1.json'),
  join('docs', 'contracts', 'fixtures',
    'legacy-governance-read-import-view-v1.json'),
  join('harness', 'legacy_governance_read_import_contract', '__init__.py'),
  join('harness', 'legacy_governance_read_import_contract', 'canonical.py'),
  join('harness', 'legacy_governance_read_import_contract', 'constants.py'),
  join('harness', 'legacy_governance_read_import_contract', 'memory.py'),
  join('harness', 'legacy_governance_read_import_contract', 'projection.py'),
  join('harness', 'legacy_governance_read_import_contract', 'source.py'),
  join('harness', 'legacy_governance_read_import_contract_check.py'),
  join('harness', 'test_legacy_governance_read_import_contract.py'),
  join('docs', 'adr',
    'ADR-0087-legacy-governance-read-import-governance-and-source-distribution.md'),
  join('harness', 'governance_engineering',
    'legacy_governance_read_import_candidate.py'),
  join('harness', 'governance_engineering',
    'test_legacy_governance_read_import_candidate.py'),
];

export const LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES =
  LEGACY_GOVERNANCE_READ_IMPORT_COPIED_FILES;
