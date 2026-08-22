// ADR-0075 portable distribution of the two existing partial projectors only.
// ADR-0065/0066, their Schemas and goldens remain owned by the graph fragment.
import { join } from 'node:path';

const PACKAGE_ROOT = join('skills', 'knowledge-graph-curation');
const VENDOR_ROOT = join(PACKAGE_ROOT, 'scripts', '_vendor');

const VENDOR_GROUPS = {
  governance_contract: ['__init__.py', 'codec.py', 'constants.py'],
  local_command_observation_producer: [
    '__init__.py', 'codec.py', 'constants.py', 'profiles.py',
  ],
  go_package_dependency_graph_observation_producer: [
    '__init__.py', 'codec.py', 'constants.py', 'graph_contract.py',
    'profiles.py', 'semantics.py',
  ],
  graph_snapshot_contract: [
    '__init__.py', 'codec.py', 'constants.py', 'coverage.py', 'derive.py',
    'lexical_test_source_constants.py', 'lexical_test_source_coverage.py',
    'lexical_test_source_derive.py', 'lexical_test_source_provenance.py',
    'lexical_test_source_snapshot.py', 'lexical_test_source_topology.py',
    'profiles.py', 'provenance.py', 'records.py', 'snapshot.py', 'topology.py',
    'unresolved.py',
  ],
};

const VENDOR_FILES = Object.entries(VENDOR_GROUPS).flatMap(
  ([directory, names]) => names.map((name) => join(VENDOR_ROOT, directory, name)),
);

export const KNOWLEDGE_GRAPH_CURATION_PACKAGE_FILES = [
  join(PACKAGE_ROOT, 'SKILL.md'),
  join(PACKAGE_ROOT, 'agents', 'openai.yaml'),
  join(PACKAGE_ROOT, 'references', 'contract.md'),
  join(PACKAGE_ROOT, 'references', 'evals.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures',
    'graph-snapshot-go-test-source-v1.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures', 'graph-snapshot-v1.json'),
  join(PACKAGE_ROOT, 'references',
    'graph-snapshot-go-test-source-v1.schema.json'),
  join(PACKAGE_ROOT, 'references', 'graph-snapshot-v1.schema.json'),
  join(PACKAGE_ROOT, 'references', 'package-manifest.json'),
  join(PACKAGE_ROOT, 'scripts', '_adapter.py'),
  join(VENDOR_ROOT, '__init__.py'),
  ...VENDOR_FILES,
  join(PACKAGE_ROOT, 'scripts', 'check_package.py'),
  join(PACKAGE_ROOT, 'scripts', 'project_go_test_source_snapshot.py'),
  join(PACKAGE_ROOT, 'scripts', 'project_module_package_snapshot.py'),
  join(PACKAGE_ROOT, 'tests', 'test_package_integrity.py'),
  join(PACKAGE_ROOT, 'tests', 'test_portable_projectors.py'),
];

export const KNOWLEDGE_GRAPH_CURATION_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0075-portable-knowledge-graph-curation-partial-projectors-skill.md'),
  join('harness', 'governance_engineering',
    'knowledge_graph_curation_portable.py'),
  join('harness', 'governance_engineering',
    'test_knowledge_graph_curation_portable.py'),
  ...KNOWLEDGE_GRAPH_CURATION_PACKAGE_FILES,
];

export const KNOWLEDGE_GRAPH_CURATION_EXPECTED_FILES =
  KNOWLEDGE_GRAPH_CURATION_COPIED_FILES;
