// ADR-0076 portable distribution of the existing ADR-0062 lexical prescan only.
// ADR-0053/0062, the Schema, golden and routed repo adapter remain universal.
import { join } from 'node:path';

const PACKAGE_ROOT = join('skills', 'change-impact-cost-risk');
const VENDOR_ROOT = join(PACKAGE_ROOT, 'scripts', '_vendor');

const VENDOR_GROUPS = {
  go_package_dependency_graph_observation_producer: [
    '__init__.py', 'codec.py', 'constants.py', 'graph_contract.py',
    'profiles.py', 'semantics.py',
  ],
  governance_contract: ['__init__.py', 'codec.py', 'constants.py'],
  local_command_observation_producer: [
    '__init__.py', 'codec.py', 'constants.py', 'profiles.py',
  ],
  local_go_package_impact_prescan_contract: [
    '__init__.py', 'codec.py', 'constants.py', 'derive.py', 'graph.py',
    'profiles.py',
  ],
};

const VENDOR_FILES = Object.entries(VENDOR_GROUPS).flatMap(
  ([directory, names]) => names.map((name) => join(VENDOR_ROOT, directory, name)),
);

export const CHANGE_IMPACT_COST_RISK_PACKAGE_FILES = [
  join(PACKAGE_ROOT, 'SKILL.md'),
  join(PACKAGE_ROOT, 'agents', 'openai.yaml'),
  join(PACKAGE_ROOT, 'references', 'contract.md'),
  join(PACKAGE_ROOT, 'references', 'evals.json'),
  join(PACKAGE_ROOT, 'references', 'fixtures',
    'local-go-package-impact-prescan-v1.json'),
  join(PACKAGE_ROOT, 'references',
    'local-go-package-impact-prescan-v1.schema.json'),
  join(PACKAGE_ROOT, 'references', 'package-manifest.json'),
  join(PACKAGE_ROOT, 'scripts', '_adapter.py'),
  join(VENDOR_ROOT, '__init__.py'),
  ...VENDOR_FILES,
  join(PACKAGE_ROOT, 'scripts', 'check_package.py'),
  join(PACKAGE_ROOT, 'scripts', 'project_local_go_package_impact_prescan.py'),
  join(PACKAGE_ROOT, 'tests', 'test_package_integrity.py'),
  join(PACKAGE_ROOT, 'tests', 'test_portable_projector.py'),
];

export const CHANGE_IMPACT_COST_RISK_COPIED_FILES = [
  join('docs', 'adr',
    'ADR-0076-portable-change-impact-cost-risk-lexical-prescan-skill.md'),
  join('harness', 'governance_engineering',
    'change_impact_cost_risk_portable.py'),
  join('harness', 'governance_engineering',
    'test_change_impact_cost_risk_portable.py'),
  ...CHANGE_IMPACT_COST_RISK_PACKAGE_FILES,
];

export const CHANGE_IMPACT_COST_RISK_EXPECTED_FILES =
  CHANGE_IMPACT_COST_RISK_COPIED_FILES;
