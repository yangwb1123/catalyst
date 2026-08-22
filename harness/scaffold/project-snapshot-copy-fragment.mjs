// ADR-0070 universal Project Snapshot assets; no Catalyst Go runtime.
import { join } from 'node:path';

const CONTRACT_PACKAGE = [
  '__init__.py', 'check.py', 'codec.py', 'constants.py', 'derive.py',
  'fixture.py', 'shapes.py', 'validation.py',
].map((name) => join('harness', 'project_source_snapshot_contract', name));

const PORTABLE_VENDOR = [
  '__init__.py', 'codec.py', 'constants.py', 'derive.py', 'shapes.py',
  'validation.py',
].map((name) => join(
  'skills', 'project-snapshot', 'scripts', '_vendor',
  'project_source_snapshot_contract', name,
));

export const PROJECT_SNAPSHOT_COPIED_FILES = [
  join('.agent', 'skills', 'project-snapshot.md'),
  join('docs', 'adr', 'ADR-0070-local-project-source-snapshot-v1.md'),
  join('docs', 'contracts', 'project-source-snapshot-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'project-source-snapshot-v1.json'),
  ...CONTRACT_PACKAGE,
  join('harness', 'test_project_source_snapshot_contract.py'),
  join('harness', 'test_project_source_snapshot_contract_adversarial.py'),
  join('harness', 'governance_engineering', 'project_source_snapshot.py'),
  join('harness', 'governance_engineering', 'test_project_source_snapshot.py'),
  join('skills', 'project-snapshot', 'SKILL.md'),
  join('skills', 'project-snapshot', 'agents', 'openai.yaml'),
  join('skills', 'project-snapshot', 'references', 'contract.md'),
  join('skills', 'project-snapshot', 'references', 'evals.json'),
  join('skills', 'project-snapshot', 'references', 'package-manifest.json'),
  join('skills', 'project-snapshot', 'scripts', 'capture.py'),
  join('skills', 'project-snapshot', 'scripts', 'check_package.py'),
  join('skills', 'project-snapshot', 'tests', 'test_portable_scripts.py'),
  join('skills', 'project-snapshot', 'tests', 'test_package_integrity.py'),
  join('skills', 'project-snapshot', 'scripts', '_vendor', '__init__.py'),
  ...PORTABLE_VENDOR,
];

export const PROJECT_SNAPSHOT_EXPECTED_FILES = PROJECT_SNAPSHOT_COPIED_FILES;
