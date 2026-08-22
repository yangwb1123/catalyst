// Universal ADR-0069 planning-only projector assets; no Catalyst Go runtime.
import { join } from 'node:path';

export const OWNERSHIP_PROJECTION_PACKAGE_FILES = [
  '__init__.py', 'check.py', 'codec.py', 'constants.py', 'fixture.py',
  'physical.py', 'projection.py', 'request.py', 'shapes.py', 'sources.py',
  'yaml_flow.py', 'yaml_resources.py', 'yaml_scalars.py', 'yaml_subset.py',
].map((name) => join('harness', 'planning_capability_ownership_projection', name));

export const OWNERSHIP_PROJECTION_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0069-planning-capability-ownership-projection-v1.md'),
  join('docs', 'contracts', 'planning-capability-ownership-projection-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'planning-capability-ownership-projection-v1.json'),
  ...OWNERSHIP_PROJECTION_PACKAGE_FILES,
  join('harness', 'governance_engineering', 'planning_capability_ownership.py'),
  join('harness', 'governance_engineering', 'test_planning_capability_ownership.py'),
  join('harness', 'test_planning_capability_ownership_projection.py'),
  join('harness', 'test_planning_capability_ownership_projection_adversarial.py'),
];

export const OWNERSHIP_PROJECTION_EXPECTED_FILES = OWNERSHIP_PROJECTION_COPIED_FILES;
