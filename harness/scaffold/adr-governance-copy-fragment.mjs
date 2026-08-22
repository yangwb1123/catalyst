// ADR-0074 portable Proposed ADR v2 validator; source-only, no lifecycle runtime.
import { join } from 'node:path';

const PACKAGE_ROOT = join('skills', 'adr-governance');
const ADR_VENDOR_ROOT = join(
  PACKAGE_ROOT, 'scripts', '_vendor', 'architecture_decision_record_v2',
);
const GOVERNANCE_VENDOR_ROOT = join(
  PACKAGE_ROOT, 'scripts', '_vendor', 'governance_contract',
);

const ADR_VENDOR_FILES = [
  '__init__.py', 'codec.py', 'constants.py', 'document.py', 'fixture.py',
  'shape.py',
].map((name) => join(ADR_VENDOR_ROOT, name));

const GOVERNANCE_VENDOR_FILES = [
  '__init__.py', 'codec.py', 'constants.py', 'fixture.py', 'record_set.py',
  'semantics.py', 'shape.py',
].map((name) => join(GOVERNANCE_VENDOR_ROOT, name));

export const ADR_GOVERNANCE_PACKAGE_FILES = [
  join(PACKAGE_ROOT, 'SKILL.md'),
  join(PACKAGE_ROOT, 'agents', 'openai.yaml'),
  join(PACKAGE_ROOT, 'references',
    'architecture-decision-record-v2.schema.json'),
  join(PACKAGE_ROOT, 'references', 'contract.md'),
  join(PACKAGE_ROOT, 'references', 'evals.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures',
    'ADR-9001-proposed-boundary.md'),
  join(PACKAGE_ROOT, 'references', 'package-manifest.json'),
  join(PACKAGE_ROOT, 'scripts', '_vendor', '__init__.py'),
  ...ADR_VENDOR_FILES,
  ...GOVERNANCE_VENDOR_FILES,
  join(PACKAGE_ROOT, 'scripts', 'check_package.py'),
  join(PACKAGE_ROOT, 'scripts', 'validate_declared_proposed_adr.py'),
  join(PACKAGE_ROOT, 'tests', 'test_package_integrity.py'),
  join(PACKAGE_ROOT, 'tests', 'test_portable_adapter.py'),
];

export const ADR_GOVERNANCE_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0074-portable-adr-governance-proposed-document-validation-skill.md'),
  join('harness', 'governance_engineering', 'adr_governance_portable.py'),
  join('harness', 'governance_engineering',
    'test_adr_governance_portable.py'),
  ...ADR_GOVERNANCE_PACKAGE_FILES,
];

export const ADR_GOVERNANCE_EXPECTED_FILES = ADR_GOVERNANCE_COPIED_FILES;
