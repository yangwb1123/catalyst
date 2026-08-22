import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

import {
  assertEngineeringSliceUpgrades,
  LEGACY_ENGINEERING_FILES,
} from './engineering-upgrade-fixture.mjs';
import { discoverHarnessSuites } from '../acceptance-tests.mjs';
import {
  renderScaffoldState, SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import { run, seedProjectInstances } from './forge-upgrade.mjs';

const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));
const INIT_PATH = join(SCAFFOLD_DIR, 'forge-init.mjs');
const RETIRED_V30_HELPER = join('harness', 'agent_engineering', 'test_support.py');
const RETIRED_V30_HELPER_BYTES = Buffer.from(
  '"""Frozen retired v30 test helper; never discovered or executed."""\n',
);
function scaffoldLegacyProject(t) {
  const root = mkdtempSync(join(tmpdir(), 'forge-upgrade-engineering-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = join(root, 'project');
  const result = spawnSync(process.execPath, [INIT_PATH, target, '--name', 'legacy-project'], {
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  const projectPath = join(target, '.agent', 'project.yml');
  const legacy = readFileSync(projectPath, 'utf8').replace(/\nengineering_spec:[\s\S]*$/, '\n');
  writeFileSync(projectPath, legacy);
  for (const relative of LEGACY_ENGINEERING_FILES) {
    rmSync(join(target, relative), { recursive: true, force: true });
  }
  return { target, projectPath, legacy };
}

function seedRetiredV30Helper(target) {
  const statePath = join(target, SCAFFOLD_STATE_FILE);
  const state = JSON.parse(readFileSync(statePath, 'utf8'));
  state.copied.push(RETIRED_V30_HELPER);
  writeFileSync(statePath, renderScaffoldState(state.copied));
  const helper = join(target, RETIRED_V30_HELPER);
  mkdirSync(dirname(helper), { recursive: true });
  writeFileSync(helper, RETIRED_V30_HELPER_BYTES);
}

function assertRetiredV30HelperCompatibility(target, result) {
  const helper = join(target, RETIRED_V30_HELPER);
  assert.equal(result.removed.includes(RETIRED_V30_HELPER), true);
  assert.equal(result.pruned, 0);
  assert.equal(existsSync(helper), true, 'plain upgrade must retain the retired helper');
  const found = discoverHarnessSuites(join(target, 'harness'));
  assert.equal(found.python.includes(helper), false,
    'recursive acceptance must not execute the exact retained v30 helper');
}

function assertGeneratedAcceptanceAndCheck(target) {
  const env = { ...process.env, PYTHONDONTWRITEBYTECODE: '1' };
  const check = spawnSync('python3', ['-B', 'harness/check.py', '.'], {
    cwd: target, encoding: 'utf8', env,
  });
  assert.equal(check.status, 0, `${check.stdout}\n${check.stderr}`);
  if (process.env.FORGE_ACCEPT_INNER) return;
  const acceptance = spawnSync(process.execPath, ['harness/acceptance.mjs', '--json'], {
    cwd: target, encoding: 'utf8', env,
  });
  assert.equal(acceptance.status, 0, `${acceptance.stdout}\n${acceptance.stderr}`);
  const rows = JSON.parse(acceptance.stdout);
  assert.equal(rows.find(({ criterion }) => criterion === 'test_pass')?.status, 'PASS');
  assert.equal(rows.find(({ criterion }) => criterion === 'arch_violations')?.status, 'PASS');
}

test('legacy project upgrades to shadow contracts without rewriting project identity', (t) => {
  const { target, projectPath, legacy } = scaffoldLegacyProject(t);
  seedRetiredV30Helper(target);
  const result = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  assert.equal(result.applied, true);
  assert.equal(readFileSync(projectPath, 'utf8'), legacy, 'upgrade must preserve legacy project.yml');
  assertLegacyGovernanceAssets(target);
  assertLegacyContractAssets(target);
  assertLegacyHarnessAssets(target);
  assertLegacyGovernanceHarness(target);
  assertLegacySemanticAndGrantChecks(target);
  assertLegacyIssuanceChecks(target);
  assertLegacyExecutionChecks(target);
  assertEngineeringSliceUpgrades(target);
  assertRetiredV30HelperCompatibility(target, result);
  assertGeneratedAcceptanceAndCheck(target);
});

function assertLegacyGovernanceAssets(target) {
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'activation.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'backend-decision-gates.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'eval', 'backend-decision-package.schema.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'frontend-design-gates.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'engineering', 'frontend-profiles.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'eval', 'frontend-design-package.schema.yml')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'frontend-client-engineering.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'frontend-code-architecture.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'ui-geometry.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'evidence-claim-management.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'context-engineering.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'policy-authority.md')), true);
  assert.equal(existsSync(join(target, '.agent', 'skills', 'backend-engineering.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0042-frontend-design-decision-contract.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0043-frontend-code-architecture-governance.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0044-business-ui-geometry-contract.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0045-canonical-evidence-claim-contract.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0046-local-governance-record-journal.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0047-shadow-cognitive-atom-projection-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0048-artifact-provenance-evidence-adapter-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0049-command-observation-evidence-adapter-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0050-evolve-repo-locator-evidence-adapter-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0051-local-gate-command-observation-producer-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0052-local-evolve-repo-locator-observation-producer-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0053-local-go-package-dependency-graph-observation-producer-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0054-local-governance-semantic-view-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0055-shadow-context-package-v1.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0056-capability-grant-v1-contract-only.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0057-authenticated-bootstrap-repo-read-grant-issuance.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'adr', '0058-authenticated-bootstrap-repo-read-execution.md')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'governance-evidence-claim-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'governance-record-journal-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'governance-semantic-view-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'context-package-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'capability-grant-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'bootstrap-grant-issuance-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'bootstrap-repo-read-execution-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'cognitive-atom-projection-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'artifact-evidence-adapter-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'command-observation-evidence-adapter-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'evolve-repo-locator-evidence-adapter-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'local-gate-command-observation-producer-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'local-evolve-repo-locator-observation-producer-v1.schema.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'local-go-package-dependency-graph-observation-producer-v1.schema.json')), true);
}

function assertLegacyContractAssets(target) {
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'governance-evidence-claim-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'governance-semantic-view-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'context-package-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'capability-grant-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'bootstrap-grant-issuance-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'bootstrap-repo-read-execution-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'cognitive-atom-projection-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'artifact-evidence-adapter-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'command-observation-evidence-adapter-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'evolve-repo-locator-evidence-adapter-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'local-gate-command-observation-producer-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'local-evolve-repo-locator-observation-producer-v1.json')), true);
  assert.equal(existsSync(join(target, 'docs', 'contracts', 'fixtures', 'local-go-package-dependency-graph-observation-producer-v1.json')), true);
  const journalSkill = readFileSync(
    join(target, '.agent', 'skills', 'evidence-claim-management.md'), 'utf8',
  );
  assert.match(journalSkill, /forge-runtime governance journal show/);
  assert.match(journalSkill, /not_executed/);
  assert.match(journalSkill, /GateObservedWith/);
  assert.match(journalSkill, /PURE_CONTRACT_FIXTURE/);
  assert.match(journalSkill, /CAPTURED_LOCAL_EVOLVE_LOCATOR_SET/);
  assert.match(journalSkill, /OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH/);
  assert.equal(existsSync(join(target, 'forge-runtime')), false,
    'upgrade must not install the Rust journal runtime');
  assert.equal(existsSync(join(target, 'forge-core')), false,
    'upgrade must not install Catalyst-only Go producers');
  assert.equal(existsSync(join(target, 'forge-kernel')), false,
    'upgrade must not install or synthesize forge-kernel, trust roots, keys, or state');
  assert.equal(existsSync(join(target, 'forge')), false,
    'upgrade must not install or replace the host forge executable');
}

function assertLegacyHarnessAssets(target) {
  assert.equal(existsSync(join(target, '.arch', 'frontend-architecture.v1.json')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend-architecture', 'check.mjs')), true);
  assert.equal(existsSync(join(target, 'harness', 'engineering_detector_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_decision_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_decision_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_evidence_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'backend_package_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', '__init__.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'composition.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'composition_support.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'geometry.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'governance.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'evidence.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'model.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design', 'package.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'frontend_design_test_support.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_frontend_design_adversarial.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_frontend_business_ui_composition_boundaries.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_frontend_business_ui_geometry.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_frontend_geometry_coordinate_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_legacy_ai_batch_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'context_package_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'capability_grant_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'capability_grant_contract', 'contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'capability_grant_contract', 'assessment.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_grant_issuance_contract', 'check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_grant_issuance_contract', 'authority.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_grant_issuance_contract', 'ledger.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_repo_read_execution_contract', 'check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_repo_read_execution_contract', 'ledger.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'bootstrap_repo_read_execution_contract', 'results.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'engineering_routing', 'contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_contract', 'codec.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'cognitive_atom_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'cognitive_atom_contract', 'projection.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'artifact_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'artifact_evidence_adapter', 'adapter.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'command_observation_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'command_observation_evidence_adapter', 'adapter.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'evolve_repo_locator_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'evolve_repo_locator_evidence_adapter', 'adapter.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'local_command_observation_producer_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'local_command_observation_producer', 'fixture.py')), true);
}

function assertLegacyGovernanceHarness(target) {
  assert.equal(existsSync(join(target, 'harness', 'evolve_locator_observation_producer', 'check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'evolve_locator_observation_producer', 'test_governance.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'go_package_dependency_graph_observation_producer', 'check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'go_package_dependency_graph_observation_producer', 'test_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'source_adapters.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'registry_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'semantic_view.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'capability_grant.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'test_capability_grant.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'bootstrap_grant_issuance.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'test_bootstrap_grant_issuance.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'bootstrap_repo_read_execution.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'test_bootstrap_repo_read_execution.py')), true);
  const semanticSelfTest = join(
    target, 'harness', 'governance_engineering', 'test_semantic_view.py',
  );
  assert.equal(existsSync(semanticSelfTest), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'evolve_locator_adapter.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'local_command_observation_producer.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'evolve_locator_observation_producer.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'go_package_dependency_graph_observation_producer.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'governance_engineering', 'test_go_package_dependency_graph_observation_producer.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_governance_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_context_package_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_capability_grant_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_capability_grant_scope_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_bootstrap_grant_issuance_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_bootstrap_grant_issuance_ledger_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_bootstrap_repo_read_execution_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_bootstrap_repo_read_execution_ledger_contract.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_cognitive_atom_contract_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_artifact_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_command_observation_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_evolve_repo_locator_evidence_adapter_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_local_command_observation_producer_check.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_governance_engineering_integration.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_governance_evolve_locator_integration.py')), true);
  assert.equal(existsSync(join(target, 'harness', 'test_governance_local_command_observation_producer_integration.py')), true);
}

function assertLegacySemanticAndGrantChecks(target) {
  const semanticTest = spawnSync(
    'python3', ['-B', 'harness/governance_engineering/test_semantic_view.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    semanticTest.status, 0,
    `upgraded semantic-view self-test must pass\n${semanticTest.stdout}\n${semanticTest.stderr}`,
  );
  const semanticCheck = spawnSync(
    'python3', ['-B', 'harness/governance_engineering/semantic_view.py', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(semanticCheck.status, 0, `${semanticCheck.stdout}\n${semanticCheck.stderr}`);
  assert.match(semanticCheck.stdout, /semantic-view-check: PASS/);
  const capabilityGrantTest = spawnSync(
    'python3', ['-B', 'harness/test_capability_grant_contract_check.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    capabilityGrantTest.status, 0,
    `upgraded CapabilityGrant self-test must pass\n${capabilityGrantTest.stdout}\n${capabilityGrantTest.stderr}`,
  );
  const capabilityGrantScopeTest = spawnSync(
    'python3', ['-B', 'harness/test_capability_grant_scope_contract.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    capabilityGrantScopeTest.status, 0,
    `upgraded CapabilityGrant scope self-test must pass\n${capabilityGrantScopeTest.stdout}\n${capabilityGrantScopeTest.stderr}`,
  );
  const capabilityGrantCheck = spawnSync(
    'python3', ['-B', 'harness/capability_grant_contract_check.py', '--golden', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    capabilityGrantCheck.status, 0,
    `${capabilityGrantCheck.stdout}\n${capabilityGrantCheck.stderr}`,
  );
}

function assertLegacyIssuanceChecks(target) {
  const bootstrapContractTests = spawnSync(
    'python3', ['-B', '-m', 'unittest',
      'harness/test_bootstrap_grant_issuance_contract.py',
      'harness/test_bootstrap_grant_issuance_ledger_contract.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    bootstrapContractTests.status, 0,
    `upgraded bootstrap issuance contract tests must pass\n${bootstrapContractTests.stdout}\n${bootstrapContractTests.stderr}`,
  );
  const bootstrapGrantTest = spawnSync(
    'python3', ['-B', 'harness/governance_engineering/test_bootstrap_grant_issuance.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    bootstrapGrantTest.status, 0,
    `upgraded bootstrap issuance governance self-test must pass\n${bootstrapGrantTest.stdout}\n${bootstrapGrantTest.stderr}`,
  );
  const bootstrapGrantCheck = spawnSync(
    'python3', ['-S', '-B', 'harness/bootstrap_grant_issuance_contract/check.py', '--golden', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    bootstrapGrantCheck.status, 0,
    `${bootstrapGrantCheck.stdout}\n${bootstrapGrantCheck.stderr}`,
  );
  assert.match(bootstrapGrantCheck.stdout, /Ed25519 NOT authenticated/);
}

function assertLegacyExecutionChecks(target) {
  const executionContractTests = spawnSync(
    'python3', ['-B', '-m', 'unittest',
      'harness/test_bootstrap_repo_read_execution_contract.py',
      'harness/test_bootstrap_repo_read_execution_ledger_contract.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    executionContractTests.status, 0,
    `upgraded bootstrap execution tests must pass\n${executionContractTests.stdout}\n${executionContractTests.stderr}`,
  );
  const executionGovernanceTest = spawnSync(
    'python3', ['-B', 'harness/governance_engineering/test_bootstrap_repo_read_execution.py'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(
    executionGovernanceTest.status, 0,
    `upgraded bootstrap execution governance must pass\n${executionGovernanceTest.stdout}\n${executionGovernanceTest.stderr}`,
  );
  const executionCheck = spawnSync(
    'python3', ['-S', '-B', 'harness/bootstrap_repo_read_execution_contract/check.py', '--golden', '.'],
    { cwd: target, encoding: 'utf8' },
  );
  assert.equal(executionCheck.status, 0, `${executionCheck.stdout}\n${executionCheck.stderr}`);
  assert.match(executionCheck.stdout, /effect NOT authenticated/);
  const check = spawnSync('python3', ['-B', 'harness/check.py', '.'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(check.status, 0, `${check.stdout}\n${check.stderr}`);
  assert.match(check.stdout, /forge-check: PASS/);
}

test('upgrade never overwrites configured frontend architecture project instances', (t) => {
  const { target } = scaffoldLegacyProject(t);
  const configured = {
    contract: Buffer.from('{"project":"configured-targets"}\n'),
    baseline: Buffer.from('{"project":"owned-debt"}\n'),
    waivers: Buffer.from('{"project":"approved-exception"}\n'),
  };
  const paths = {
    contract: join(target, '.arch', 'frontend-architecture.v1.json'),
    baseline: join(target, '.arch', 'frontend-architecture-baseline.v1.json'),
    waivers: join(target, '.arch', 'frontend-architecture-waivers.v1.json'),
  };
  for (const [key, path] of Object.entries(paths)) writeFileSync(path, configured[key]);
  writeFileSync(join(target, 'harness', 'gate.mjs'), '// force a governed upgrade write\n');

  const result = run({ from: SOURCE_ROOT, target, apply: true, backup: true, prune: false });
  for (const [key, path] of Object.entries(paths)) {
    assert.ok(readFileSync(path).equals(configured[key]), `${key} project instance must be preserved`);
  }
  assert.equal(result.projectInitialized, 0);
  assert.equal(result.drift.changed.some((rel) => rel.startsWith('.arch/frontend-architecture')), false);
});
test('stale missing-instance plan preserves a file created concurrently before seed', (t) => {
  const { target } = scaffoldLegacyProject(t);
  const relative = join('.arch', 'frontend-architecture.v1.json');
  const destination = join(target, relative);
  const concurrentBytes = Buffer.from('{"project":"created-by-another-agent"}\n');

  // The path was absent when the upgrade planned it. Another process wins the
  // create race before the planned seed executes.
  assert.equal(existsSync(destination), false);
  const stalePlan = [relative]; writeFileSync(destination, concurrentBytes, { flag: 'wx' });

  const initialized = seedProjectInstances(stalePlan, SOURCE_ROOT, target);
  assert.equal(initialized, 0);
  assert.ok(readFileSync(destination).equals(concurrentBytes));
});
