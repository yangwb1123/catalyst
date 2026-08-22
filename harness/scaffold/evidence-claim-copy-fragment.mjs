// ADR-0072 portable Evidence Claim package; source-only, no Catalyst runtime.
import { join } from 'node:path';

const PACKAGE_ROOT = join('skills', 'evidence-claim-management');
const VENDOR_ROOT = join(
  PACKAGE_ROOT, 'scripts', '_vendor', 'governance_contract',
);

const VENDOR_FILES = [
  '__init__.py', 'codec.py', 'constants.py', 'fixture.py', 'record_set.py',
  'semantics.py', 'shape.py',
].map((name) => join(VENDOR_ROOT, name));

export const EVIDENCE_CLAIM_PACKAGE_FILES = [
  join(PACKAGE_ROOT, 'SKILL.md'),
  join(PACKAGE_ROOT, 'agents', 'openai.yaml'),
  join(PACKAGE_ROOT, 'references', 'contract.md'),
  join(PACKAGE_ROOT, 'references', 'evals.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures',
    'governance-evidence-claim-v1.json'),
  join(PACKAGE_ROOT, 'references', 'package-manifest.json'),
  join(PACKAGE_ROOT, 'scripts', '_vendor', '__init__.py'),
  ...VENDOR_FILES,
  join(PACKAGE_ROOT, 'scripts', 'check_package.py'),
  join(PACKAGE_ROOT, 'scripts', 'validate.py'),
  join(PACKAGE_ROOT, 'tests', 'test_package_integrity.py'),
  join(PACKAGE_ROOT, 'tests', 'test_portable_scripts.py'),
];

export const EVIDENCE_CLAIM_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0072-portable-evidence-claim-validation-skill.md'),
  ...EVIDENCE_CLAIM_PACKAGE_FILES,
];

export const EVIDENCE_CLAIM_EXPECTED_FILES = EVIDENCE_CLAIM_COPIED_FILES;
