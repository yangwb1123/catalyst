// ADR-0073 portable Policy Authority package; source-only, no authority runtime.
import { join } from 'node:path';

const PACKAGE_ROOT = join('skills', 'policy-authority');
const APPROVAL_VENDOR_ROOT = join(
  PACKAGE_ROOT, 'scripts', '_vendor', 'approval_record_contract',
);
const GRANT_VENDOR_ROOT = join(
  PACKAGE_ROOT, 'scripts', '_vendor', 'capability_grant_contract',
);

const APPROVAL_VENDOR_FILES = [
  '__init__.py', 'assessment.py', 'canonical.py', 'constants.py', 'contract.py',
  'fixture.py', 'record.py', 'shape.py',
].map((name) => join(APPROVAL_VENDOR_ROOT, name));

const GRANT_VENDOR_FILES = [
  '__init__.py', 'assessment.py', 'canonical.py', 'constants.py', 'contract.py',
  'grant.py', 'scope.py', 'shape.py', 'vocabulary.py',
].map((name) => join(GRANT_VENDOR_ROOT, name));

export const POLICY_AUTHORITY_PACKAGE_FILES = [
  join(PACKAGE_ROOT, 'SKILL.md'),
  join(PACKAGE_ROOT, 'agents', 'openai.yaml'),
  join(PACKAGE_ROOT, 'references', 'contract.md'),
  join(PACKAGE_ROOT, 'references', 'evals.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures', 'approval-record-v1.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures', 'capability-grant-v1.json'),
  join(PACKAGE_ROOT, 'references', 'package-manifest.json'),
  join(PACKAGE_ROOT, 'scripts', '_vendor', '__init__.py'),
  ...APPROVAL_VENDOR_FILES,
  ...GRANT_VENDOR_FILES,
  join(PACKAGE_ROOT, 'scripts', 'assess_declared_approval_record.py'),
  join(PACKAGE_ROOT, 'scripts', 'assess_declared_capability_grant.py'),
  join(PACKAGE_ROOT, 'scripts', 'check_package.py'),
  join(PACKAGE_ROOT, 'tests', 'test_package_integrity.py'),
  join(PACKAGE_ROOT, 'tests', 'test_portable_adapters.py'),
];

export const POLICY_AUTHORITY_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0073-portable-policy-authority-declaration-assessment-skill.md'),
  join('harness', 'governance_engineering', 'policy_authority_portable.py'),
  join('harness', 'governance_engineering',
    'test_policy_authority_portable.py'),
  ...POLICY_AUTHORITY_PACKAGE_FILES,
];

export const POLICY_AUTHORITY_EXPECTED_FILES = POLICY_AUTHORITY_COPIED_FILES;
