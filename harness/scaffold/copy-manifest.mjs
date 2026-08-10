// ForgeOS forge-init copy manifests (data-driven — the 70% universal
// governance) — split out of forge-init.mjs to keep it under the harness's own
// 500-line file cap (the same "extract pure data/logic, keep the I/O boundary
// thin" pattern used elsewhere in this repo, e.g. mode_gating_check.py /
// prompt_artifacts.go). Pure data, no imports, no disk I/O.
import { join } from 'node:path';

// Whole .agent/ governance-asset directories copied VERBATIM (recursively). These
// are universal: the declarative role cards / skills / workflows / acceptance
// schema / routing policy / mode table that check.py validates and acceptance.mjs
// consumes. Project IDENTITY (PROJECT/ROADMAP/CURRENT_SPRINT/project.yml) is NOT
// here — it is generated per project in forge-init.mjs.
// Exported (with COPIED_FILES) so test_forge-init.mjs's manifest-integrity guard
// walks harness/ against the REAL manifest, catching drift the moment harness/
// grows out of sync with these lists.
export const GOVERNANCE_DIRS = [
  join('.agent', 'agents'),
  join('.agent', 'skills'),
  join('.agent', 'workflows'),
  join('.agent', 'eval'),
  join('.agent', 'routing'),
  join('.agent', 'policies'),
  join('.agent', 'engineering'),
  join('.ai', 'prompts'),
];

// Persisted by forge-init so a later forge-upgrade can distinguish a genuinely
// retired copied file from a user's unrelated file. This state is project
// metadata, not part of the copied 70%, and therefore is never source-overwritten.
export const SCAFFOLD_STATE_FILE = join('.agent', 'scaffold-state.json');

// Project-owned detector instances are seeded once but are NOT part of the
// upgrade-synchronized 70%. Their targets, debt baseline and approved waivers
// evolve with the governed project. forge-upgrade may add a missing instance to
// a legacy project, but it must never overwrite an existing one.
export const PROJECT_INSTANCE_FILES = [
  join('.arch', 'frontend-architecture.v1.json'),
  join('.arch', 'frontend-architecture-baseline.v1.json'),
  join('.arch', 'frontend-architecture-waivers.v1.json'),
];

// Individual files copied verbatim: the red-lines, the architecture rules, and
// the FULL harness — every TOOL plus its SELF-TEST, so check + accept both RUN in
// the fresh project and self-govern (the harness runs its own tests under
// acceptance's test_pass). Listed explicitly (not a blind harness/ copy) to omit
// __pycache__ and the human-only READMEs. The adapters/<lang>.yml command maps
// ARE copied (the lint criterion reads them); only adapters/README.md is omitted.
export const COPIED_FILES = [
  join('.agent', 'AGENTS.md'),
  join('.arch', 'rules.yaml'),
  // The runtime release executor requires this real (non-symlink) directory
  // before it grants exact Edit permission for an immutable phase emit set.
  join('docs', 'release', 'README.md'),
  // Canonical planning-only capability vocabulary and its one-owner Skill map.
  // Agent Engineering references these existing catalogs instead of inventing
  // a parallel capability namespace; copying keeps fresh/updated projects valid.
  join('docs', 'design', 'ai-engineering-os', 'capability-catalog.v1.yml'),
  join('docs', 'design', 'ai-engineering-os', 'capability-skill-map.v1.yml'),
  join('docs', 'design', 'ai-engineering-os', 'backend-decision-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'frontend-design-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'frontend-code-architecture-standard.md'),
  join('docs', 'design', 'ai-engineering-os', 'governance-contracts.md'),
  // The copied AFDS standard links this accepted decision. Keep the governing
  // rationale in the same projection so generated projects have no dangling
  // local documentation links.
  join('docs', 'adr', '0042-frontend-design-decision-contract.md'),
  join('docs', 'adr', '0043-frontend-code-architecture-governance.md'),
  join('docs', 'adr', '0044-business-ui-geometry-contract.md'),
  join('docs', 'adr', '0045-canonical-evidence-claim-contract.md'),
  join('docs', 'adr', '0046-local-governance-record-journal.md'),
  join('docs', 'adr', '0047-shadow-cognitive-atom-projection-v1.md'),
  join('docs', 'adr', '0048-artifact-provenance-evidence-adapter-v1.md'),
  join('docs', 'adr', '0049-command-observation-evidence-adapter-v1.md'),
  join('docs', 'adr', '0050-evolve-repo-locator-evidence-adapter-v1.md'),
  join('docs', 'adr', '0051-local-gate-command-observation-producer-v1.md'),
  join('docs', 'adr', '0052-local-evolve-repo-locator-observation-producer-v1.md'),
  join('docs', 'contracts', 'governance-evidence-claim-v1.schema.json'),
  join('docs', 'contracts', 'governance-record-journal-v1.schema.json'),
  join('docs', 'contracts', 'cognitive-atom-projection-v1.schema.json'),
  join('docs', 'contracts', 'artifact-evidence-adapter-v1.schema.json'),
  join('docs', 'contracts', 'command-observation-evidence-adapter-v1.schema.json'),
  join('docs', 'contracts', 'evolve-repo-locator-evidence-adapter-v1.schema.json'),
  join('docs', 'contracts', 'local-gate-command-observation-producer-v1.schema.json'),
  join('docs', 'contracts', 'local-evolve-repo-locator-observation-producer-v1.schema.json'),
  join('docs', 'contracts', 'fixtures', 'governance-evidence-claim-v1.json'),
  join('docs', 'contracts', 'fixtures', 'cognitive-atom-projection-v1.json'),
  join('docs', 'contracts', 'fixtures', 'artifact-evidence-adapter-v1.json'),
  join('docs', 'contracts', 'fixtures', 'command-observation-evidence-adapter-v1.json'),
  join('docs', 'contracts', 'fixtures', 'evolve-repo-locator-evidence-adapter-v1.json'),
  join('docs', 'contracts', 'fixtures', 'local-gate-command-observation-producer-v1.json'),
  join('docs', 'contracts', 'fixtures', 'local-evolve-repo-locator-observation-producer-v1.json'),
  // harness tools
  join('harness', 'gate.mjs'),
  join('harness', 'policies.yml'),
  join('harness', 'check.py'),
  join('harness', 'agent_engineering_check.py'), // imported by check.py; validates scoped Agent Engineering contracts
  join('harness', 'governance_contract_check.py'), // strict EvidenceRecord/KnowledgeClaim shadow codec and validator
  join('harness', 'governance_contract', '__init__.py'),
  join('harness', 'governance_contract', 'codec.py'),
  join('harness', 'governance_contract', 'constants.py'),
  join('harness', 'governance_contract', 'fixture.py'),
  join('harness', 'governance_contract', 'record_set.py'),
  join('harness', 'governance_contract', 'semantics.py'),
  join('harness', 'governance_contract', 'shape.py'),
  join('harness', 'cognitive_atom_contract_check.py'), // strict CognitiveAtom shadow reprojection and byte comparator
  join('harness', 'cognitive_atom_contract', '__init__.py'),
  join('harness', 'cognitive_atom_contract', 'constants.py'),
  join('harness', 'cognitive_atom_contract', 'fixture.py'),
  join('harness', 'cognitive_atom_contract', 'projection.py'),
  join('harness', 'artifact_evidence_adapter_check.py'), // pure Artifact v1 to EvidenceRecord shadow adapter
  join('harness', 'artifact_evidence_adapter', '__init__.py'),
  join('harness', 'artifact_evidence_adapter', 'adapter.py'),
  join('harness', 'artifact_evidence_adapter', 'codec.py'),
  join('harness', 'artifact_evidence_adapter', 'constants.py'),
  join('harness', 'artifact_evidence_adapter', 'fixture.py'),
  join('harness', 'command_observation_evidence_adapter_check.py'), // pure command observation to gate/test EvidenceRecord shadow adapter
  join('harness', 'command_observation_evidence_adapter', '__init__.py'),
  join('harness', 'command_observation_evidence_adapter', 'adapter.py'),
  join('harness', 'command_observation_evidence_adapter', 'codec.py'),
  join('harness', 'command_observation_evidence_adapter', 'constants.py'),
  join('harness', 'command_observation_evidence_adapter', 'fixture.py'),
  join('harness', 'evolve_repo_locator_evidence_adapter_check.py'), // pure Evolve locator to EvidenceRecord shadow adapter
  join('harness', 'evolve_repo_locator_evidence_adapter', '__init__.py'),
  join('harness', 'evolve_repo_locator_evidence_adapter', 'adapter.py'),
  join('harness', 'evolve_repo_locator_evidence_adapter', 'codec.py'),
  join('harness', 'evolve_repo_locator_evidence_adapter', 'constants.py'),
  join('harness', 'evolve_repo_locator_evidence_adapter', 'fixture.py'),
  join('harness', 'local_command_observation_producer_check.py'), // ADR-0051 pure contract-fixture validator
  join('harness', 'local_command_observation_producer', '__init__.py'),
  join('harness', 'local_command_observation_producer', 'codec.py'),
  join('harness', 'local_command_observation_producer', 'constants.py'),
  join('harness', 'local_command_observation_producer', 'fixture.py'),
  join('harness', 'local_command_observation_producer', 'profiles.py'),
  join('harness', 'local_command_observation_producer', 'semantics.py'),
  join('harness', 'evolve_locator_observation_producer', '__init__.py'),
  join('harness', 'evolve_locator_observation_producer', 'check.py'),
  join('harness', 'evolve_locator_observation_producer', 'codec.py'),
  join('harness', 'evolve_locator_observation_producer', 'constants.py'),
  join('harness', 'evolve_locator_observation_producer', 'fixture.py'),
  join('harness', 'evolve_locator_observation_producer', 'profiles.py'),
  join('harness', 'evolve_locator_observation_producer', 'semantics.py'),
  join('harness', 'evolve_locator_observation_producer', 'test_adversarial.py'),
  join('harness', 'evolve_locator_observation_producer', 'test_contract.py'),
  join('harness', 'evolve_locator_observation_producer', 'test_governance.py'),
  join('harness', 'backend_decision_contract.py'), // canonical backend trigger/dimension/floor vocabulary and byte pins
  join('harness', 'backend_decision_check.py'), // BackendDecisionPackage contract + instance validator (shadow)
  join('harness', 'backend_evidence_check.py'), // typed/subject-bound bounded evidence resolution
  join('harness', 'backend_package_check.py'), // source-resolving BackendDecisionPackage instance semantics
  join('harness', 'frontend_design', '__init__.py'), // package marker required by package-style imports and copied self-tests
  join('harness', 'frontend_design', 'contract.py'), // canonical AFDS trigger/dimension/profile vocabulary and byte pins
  join('harness', 'frontend_design', 'composition.py'), // business-role/task/data composition and presentation-reference validation
  join('harness', 'frontend_design', 'composition_support.py'), // shared bounded JSON, reference, and scalar helpers
  join('harness', 'frontend_design', 'geometry.py'), // capture-context-bound declarative geometry-report validation
  join('harness', 'frontend_design', 'governance.py'), // cross-record profile, override, risk, and review governance
  join('harness', 'frontend_design_check.py'), // FrontendDesignPackage contract + instance CLI (shadow)
  join('harness', 'frontend_design', 'evidence.py'), // artifact/proof separation and bounded PNG/digest checks
  join('harness', 'frontend_design', 'model.py'), // classification, flow, state/action and capture-context semantics
  join('harness', 'frontend_design', 'package.py'), // source-resolving FrontendDesignPackage instance semantics
  join('harness', 'frontend_design_test_support.py'), // shared source-bound AFDS fixture builder
  join('harness', 'frontend-architecture', 'check.mjs'), // standalone shadow frontend architecture detector
  join('harness', 'frontend-architecture', 'contract.mjs'), // strict project contract/baseline/waiver validation
  join('harness', 'frontend-architecture', 'graph.mjs'), // ownership, direction, public API, SCC and review metrics
  join('harness', 'frontend-architecture', 'typescript-adapter.mjs'), // compiler-backed TS/TSX import model
  join('harness', 'completion_evidence_check.py'), // instance-level completion honesty validator
  join('harness', 'engineering_detector_check.py'), // activation/capability/detector wiring validator
  join('harness', 'engineering_check_support.py'), // strict YAML and repository-reference primitives
  join('harness', 'engineering_routing_check.py'), // context-route and assurance-profile validation
  join('harness', 'governance_engineering_check.py'), // Evidence/Claim registry, pin, Skill, and detector integration
  join('harness', 'governance_engineering', '__init__.py'),
  join('harness', 'governance_engineering', 'source_adapters.py'), // versioned source-adapter registry/schema checks split from the root gate
  join('harness', 'governance_engineering', 'evolve_locator_adapter.py'), // ADR-0050 registry/schema/detector/Skill freeze
  join('harness', 'governance_engineering', 'local_command_observation_producer.py'), // ADR-0051 producer registry/schema/Skill freeze
  join('harness', 'governance_engineering', 'evolve_locator_observation_producer.py'), // ADR-0052 shipped producer freeze
  join('harness', 'mode_gating_check.py'), // imported by check.py; without it check.py fails to import
  join('harness', 'release_boundary_check.py'), // imported by check.py; pins docs-only deploy/rollback trust boundary
  join('harness', 'workflow_control_check.py'), // imported by check.py; fails closed on dangling/unsupported workflow control
  join('harness', 'acceptance.mjs'),
  // acceptance.mjs is split into a dependency-free kernel (shared run/result/
  // splitCmd + PASS/FAIL/NA/ROOT/HARNESS_DIR) and the adapter-backed quality
  // probes (lint + coverage); acceptance.mjs imports BOTH, so a fresh project
  // missing either fails to import the gate (ERR_MODULE_NOT_FOUND) — copy-anywhere
  // iron rule.
  join('harness', 'acceptance-kernel.mjs'),
  join('harness', 'acceptance-quality.mjs'),
  join('harness', 'acceptance-project.mjs'),
  join('harness', 'acceptance-tests.mjs'),
  // Adapter runtime: adapters.mjs owns common declarations, detection.mjs scans
  // all supported source extensions, and project.mjs safely selects/executes
  // Rust Cargo or Java wrapper argv for project-level acceptance.
  join('harness', 'adapters.mjs'),
  join('harness', 'adapters', 'detection.mjs'),
  join('harness', 'adapters', 'project-execution.mjs'),
  join('harness', 'adapters', 'project.mjs'),
  join('harness', 'yaml2json.py'),
  join('harness', 'scorecard.mjs'),
  join('harness', 'scorecard-update.mjs'),
  join('harness', 'secret-scan.mjs'),
  join('harness', 'spec_check.py'),
  join('harness', 'sca.mjs'), // imported by acceptance.mjs's dependency_vulnerabilities criterion
  join('harness', 'sca_fetch.mjs'), // operator-run OSV refresh tool for sca.mjs's DB; imports sca.mjs
  // select-tests.mjs is the incremental (advisory) test selector — a fast edit-time
  // signal that NEVER replaces the full forge accept; it imports acceptance-kernel.mjs
  // (already copied). A scaffolded project inherits the same dev-loop accelerator.
  join('harness', 'select-tests.mjs'),
  join('harness', 'arch', 'arch-check.mjs'),
  join('harness', 'arch', 'scan.mjs'),
  join('harness', 'arch', 'scan-functions.mjs'),
  // per-language adapter command maps (read at runtime by adapters.mjs / the
  // lint criterion); the adapters/README.md prose is intentionally omitted.
  join('harness', 'adapters', 'go.yml'),
  join('harness', 'adapters', 'java.yml'),
  join('harness', 'adapters', 'python.yml'),
  join('harness', 'adapters', 'rust.yml'),
  join('harness', 'adapters', 'typescript.yml'),
  // harness self-tests (acceptance's test_pass runs these — the harness self-governs).
  // test_enforce.mjs (pins the warn|block enforce resolution in the copied adapters.mjs)
  // was once dropped here — the drift test_forge-init.mjs's manifest guard now forbids.
  join('harness', 'test_check.py'),
  join('harness', 'test_check_bounded_input.py'),
  join('harness', 'test_agent_engineering_check.py'),
  join('harness', 'test_governance_contract_check.py'),
  join('harness', 'test_cognitive_atom_contract_check.py'),
  join('harness', 'test_artifact_evidence_adapter_check.py'),
  join('harness', 'test_command_observation_evidence_adapter_check.py'),
  join('harness', 'test_evolve_repo_locator_evidence_adapter_check.py'),
  join('harness', 'test_local_command_observation_producer_check.py'),
  join('harness', 'test_governance_engineering_integration.py'),
  join('harness', 'test_governance_evolve_locator_integration.py'),
  join('harness', 'test_governance_local_command_observation_producer_integration.py'),
  join('harness', 'test_backend_decision_check.py'),
  join('harness', 'test_frontend_design_adversarial.py'),
  join('harness', 'test_frontend_business_ui_composition_boundaries.py'),
  join('harness', 'test_frontend_business_ui_geometry.py'),
  join('harness', 'test_frontend_geometry_coordinate_contract.py'),
  join('harness', 'test_frontend_design_check.py'),
  join('harness', 'frontend-architecture', 'test_frontend-architecture.mjs'),
  join('harness', 'test_legacy_ai_batch_contract.py'),
  join('harness', 'test_mode_gating_check.py'),
  join('harness', 'test_release_boundary_check.py'),
  join('harness', 'test_workflow_control_check.py'),
  join('harness', 'test_yaml2json.py'),
  join('harness', 'test_acceptance.mjs'),
  join('harness', 'test_acceptance_project.mjs'),
  join('harness', 'test_acceptance_discovery.mjs'),
  join('harness', 'test_adapters.mjs'),
  join('harness', 'test_polyglot_adapters.mjs'),
  join('harness', 'test_gate.mjs'),
  join('harness', 'test_enforce.mjs'),
  join('harness', 'test_scorecard.mjs'),
  join('harness', 'test_scorecard-telemetry.mjs'),
  join('harness', 'test_scorecard-update.mjs'),
  join('harness', 'test_secret-scan.mjs'),
  join('harness', 'test_sca.mjs'),
  join('harness', 'test_sca_fetch.mjs'),
  join('harness', 'test_select-tests.mjs'),
  join('harness', 'arch', 'test_arch-check.mjs'),
  join('harness', 'arch', 'test_arch-go-exports.mjs'),
];

// Harness sources DELIBERATELY not copied (test_forge-init.mjs's manifest guard
// whitelists these): forge-init.mjs is the SCAFFOLDER itself (a generated project
// does not carry the tool that created it) and test_forge-init.mjs exercises that
// absent tool. Any OTHER harness source must be in COPIED_FILES / GOVERNANCE_DIRS.
// All scaffold/upgrade-time tooling lives together in harness/scaffold/ (its own
// sub-package — kept out of the thin harness/ gate package). A generated project
// does not scaffold or upgrade sub-projects, so NONE of harness/scaffold/ is copied.
export const HARNESS_NOT_COPIED = [
  join('harness', 'scaffold', 'forge-init.mjs'),
  join('harness', 'scaffold', 'test_forge-init.mjs'),
  // scaffold-fs.mjs holds the copy/enumerate primitives forge-init and forge-
  // upgrade share; like forge-init itself it is a SCAFFOLD/UPGRADE-time tool, not
  // project-runtime governance (a generated project does not scaffold sub-projects),
  // so it is intentionally not copied.
  join('harness', 'scaffold', 'scaffold-fs.mjs'),
  // forge-upgrade resyncs a project's copied governance FROM a ForgeOS source repo;
  // it is an OPERATOR tool run against a project from OUTSIDE, never carried inside
  // one (a project does not upgrade itself from itself). Its functional and
  // filesystem-security self-tests are likewise upgrade-time tools. Listed here
  // so test_forge-init's manifest guard FORCES a conscious decision whenever
  // these change — the safety net, not an oversight.
  join('harness', 'scaffold', 'forge-upgrade.mjs'),
  join('harness', 'scaffold', 'upgrade-state.mjs'),
  join('harness', 'scaffold', 'test_forge-upgrade.mjs'),
  join('harness', 'scaffold', 'test_forge-upgrade-engineering.mjs'),
  join('harness', 'scaffold', 'test_scaffold_security.mjs'),
  // copy-manifest.mjs itself is this data module — scaffold/upgrade-time only,
  // same rationale as scaffold-fs.mjs (never carried into a generated project).
  join('harness', 'scaffold', 'copy-manifest.mjs'),
  join('harness', 'scaffold', 'forge-init-test-assets.mjs'),
];
