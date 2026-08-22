// Universal authority-neutral Registry v1 contract assets. Catalyst's Go
// implementation is deliberately not part of this scaffold projection.
import { join } from 'node:path';

export const CAPABILITY_REGISTRY_PACKAGE_FILES = [
  '__init__.py', 'builder.py', 'check.py', 'codec.py', 'constants.py',
  'digests.py', 'filesystem.py', 'fixture.py', 'physical.py', 'records.py',
  'resolver.py', 'shapes.py', 'validation.py',
].map((name) => join('harness', 'capability_registry_contract', name));

export const CAPABILITY_REGISTRY_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0068-authority-neutral-capability-registry-v1.md'),
  join('docs', 'contracts', 'capability-registry-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'capability-registry-v1.json'),
  ...CAPABILITY_REGISTRY_PACKAGE_FILES,
  join('harness', 'test_capability_registry_contract.py'),
  join('harness', 'governance_engineering', 'capability_registry.py'),
  join('harness', 'governance_engineering', 'test_capability_registry.py'),
];

export const CAPABILITY_REGISTRY_EXPECTED_FILES = CAPABILITY_REGISTRY_COPIED_FILES;
