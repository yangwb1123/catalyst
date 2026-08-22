// Pure ADR-0065/0066 scaffold data shared by the production copy manifest and
// fresh/legacy verification. It deliberately excludes Catalyst-only Go runtime.
import { join } from 'node:path';


export const GRAPH_SNAPSHOT_PACKAGE_FILES = [
  '__init__.py', 'codec.py', 'constants.py', 'coverage.py', 'derive.py',
  'dispatch.py', 'fixture.py', 'profiles.py', 'provenance.py', 'records.py',
  'snapshot.py', 'lexical_test_source_constants.py',
  'lexical_test_source_coverage.py', 'lexical_test_source_derive.py',
  'lexical_test_source_fixture.py', 'lexical_test_source_provenance.py',
  'lexical_test_source_snapshot.py', 'lexical_test_source_topology.py',
  'lexical_test_source_validation.py', 'topology.py',
  'unresolved.py', 'validation.py',
].map((name) => join('harness', 'graph_snapshot_contract', name));

export const GRAPH_SNAPSHOT_COPIED_FILES = [
  join('docs', 'adr', '0065-authority-free-graph-snapshot-v1-contract.md'),
  join('docs', 'adr', '0066-local-go-lexical-test-source-graph-snapshot.md'),
  join('docs', 'contracts', 'graph-snapshot-v1.schema.json'),
  join('docs', 'contracts', 'graph-snapshot-go-test-source-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'graph-snapshot-v1.json'),
  join('docs', 'contracts', 'fixtures', 'graph-snapshot-go-test-source-v1.json'),
  join('harness', 'graph_snapshot_contract_check.py'),
  ...GRAPH_SNAPSHOT_PACKAGE_FILES,
  join('harness', 'test_graph_snapshot_contract_check.py'),
  join('harness', 'test_graph_snapshot_test_source_contract.py'),
  join('harness', 'test_go_dependency_graph_contract_shared.py'),
  join('harness', 'go_package_dependency_graph_observation_producer', 'graph_contract.py'),
  join('harness', 'governance_engineering', 'graph_snapshot.py'),
  join('harness', 'governance_engineering', 'test_graph_snapshot.py'),
];

export const GRAPH_SNAPSHOT_EXPECTED_FILES = [
  join('.agent', 'skills', 'knowledge-graph-curation.md'),
  ...GRAPH_SNAPSHOT_COPIED_FILES,
];
