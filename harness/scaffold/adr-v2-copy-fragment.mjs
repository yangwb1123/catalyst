// Pure proposed-only ADR v2 scaffold projection. Catalyst-specific Go runtime
// and writes_adr wiring are deliberately outside this universal file list.
import { join } from 'node:path';


export const ADR_V2_PACKAGE_FILES = [
  '__init__.py', 'codec.py', 'constants.py', 'document.py', 'fixture.py',
  'shape.py',
].map((name) => join('harness', 'architecture_decision_record_v2', name));

export const ADR_V2_COPIED_FILES = [
  join('docs', 'adr', '0067-proposed-only-adr-v2-frontmatter.md'),
  join('docs', 'contracts', 'architecture-decision-record-v2.schema.json'),
  join('docs', 'contracts', 'fixtures', 'ADR-9001-proposed-boundary.md'),
  join('harness', 'architecture_decision_record_v2_check.py'),
  ...ADR_V2_PACKAGE_FILES,
  join('harness', 'test_architecture_decision_record_v2_check.py'),
  join('harness', 'governance_engineering', 'architecture_decision_record_v2.py'),
  join('harness', 'governance_engineering', 'test_architecture_decision_record_v2.py'),
];

export const ADR_V2_EXPECTED_FILES = ADR_V2_COPIED_FILES;
