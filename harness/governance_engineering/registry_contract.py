"""Protected GovernanceContractRegistry shape and artifact pin targets."""

import json

from governance_contract import ContractError, read_bounded_file
from local_go_package_impact_prescan_contract import (
    validate_golden_fixture as validate_impact_golden_fixture,
)

POLICY_RELATIVE = "engineering/governance-contracts.yml"
POLICY_SHA256 = "7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54"
POLICY_HEADER = {
    "status": "active_contract",
    "runtime_binding": (
        "cross_language_codec_local_journal_semantic_view_atom_projection_"
        "context_package_capability_grant_approval_record_transition_receipt_"
        "and_knowledge_update_proposal_contract_only_"
        "authenticated_bootstrap_"
        "repo_read_grant_issuance_authenticated_bootstrap_repo_read_execution_"
        "artifact_command_"
        "evolve_locator_adapters_local_gate_"
        "command_evolve_locator_and_go_package_dependency_graph_producers_shadow"
        "_local_go_package_impact_prescan_graph_snapshot_partial_projector_and_"
        "proposed_adr_v2_capability_registry_exact_resolver_and_planning_"
        "capability_ownership_projection_and_project_source_snapshot_evaluators_"
        "with_linux_local_project_source_snapshot_producer_and_portable_"
        "context_engineering_skill_and_portable_evidence_claim_validation_skill_"
        "and_portable_policy_authority_declaration_assessment_skill_and_"
        "portable_adr_governance_proposed_document_validation_skill_and_"
        "portable_knowledge_graph_curation_partial_projection_skill_and_"
        "portable_change_impact_cost_risk_lexical_prescan_projection_skill_and_"
        "proposed_work_intent_v1_candidate_contract_source_distribution_only_"
        "and_proposed_authenticated_adr_approval_v1_structural_prerequisite_"
        "candidate_source_distribution_only_with_catalyst_repository_only_go_"
        "authority_evidence_and_proposed_authenticated_adr_lifecycle_v1_"
        "structural_candidate_source_distribution_only_with_catalyst_repository_"
        "only_authenticated_adr_lifecycle_v1_go_authority_evidence_and_source_"
        "only_governance_distribution_and_proposed_legacy_governance_read_"
        "import_v1_unverified_read_only_candidate_with_source_only_python_"
        "distribution_and_catalyst_repository_only_go_parity_and_proposed_"
        "kernel_operational_reference_core_v1_structural_candidate_with_source_"
        "only_python_distribution_and_catalyst_repository_only_go_and_rust_parity_"
        "and_proposed_kernel_decision_reference_core_v1_structural_candidate_"
        "with_source_only_python_distribution_and_catalyst_repository_only_go_"
        "and_rust_parity_and_proposed_decision_capsule_structural_replay_core_"
        "v1_structural_candidate_with_source_only_python_distribution_and_"
        "catalyst_repository_only_go_and_rust_parity"
    ),
    "version": 39,
    "completion_authority": "forge_accept",
}
POLICY_FIELDS = {
    "api_version", "kind", "status", "runtime_binding", "owner", "version",
    "completion_authority", "scope", "canonicalization", "identity",
    "claim_states", "shadow_admissibility", "evidence_semantics", "journal",
    "semantic_view", "context_package", "capability_grant", "approval_record",
    "transition_receipt", "knowledge_update_proposal",
    "bootstrap_grant_issuance", "bootstrap_repo_read_execution",
    "cognitive_atom_projection", "artifact_evidence_adapter",
    "command_observation_evidence_adapter", "legacy",
    "evolve_repo_locator_evidence_adapter",
    "local_gate_command_observation_producer",
    "local_evolve_repo_locator_observation_producer",
    "local_go_package_dependency_graph_observation_producer",
    "local_go_package_impact_prescan",
    "graph_snapshot", "graph_snapshot_test_source",
    "architecture_decision_record_v2",
    "capability_registry",
    "planning_capability_ownership",
    "project_source_snapshot", "evidence_claim_portable_validation",
    "policy_authority_portable_declaration_assessment",
    "adr_governance_portable_proposed_document_validation",
    "knowledge_graph_curation_portable_projection",
    "change_impact_cost_risk_portable_projection",
    "work_intent_v1_candidate_contract",
    "authenticated_adr_approval_v1_candidate_contract",
    "authenticated_adr_approval_v1_go_authority_evidence",
    "authenticated_adr_lifecycle_v1_candidate_contract",
    "authenticated_adr_lifecycle_v1_go_authority_evidence",
    "legacy_governance_read_import_v1_candidate_contract",
    "kernel_operational_reference_core_v1_candidate_contract",
    "kernel_decision_reference_core_v1_candidate_contract",
    "decision_capsule_structural_replay_core_v1_candidate_contract",
    "canonical_refs", "contract_pins", "reference_implementations",
    "non_capabilities",
}
PIN_TARGETS = {
    "schema_sha256": "docs/contracts/governance-evidence-claim-v1.schema.json",
    "journal_schema_sha256": "docs/contracts/governance-record-journal-v1.schema.json",
    "semantic_view_schema_sha256":
        "docs/contracts/governance-semantic-view-v1.schema.json",
    "golden_fixture_sha256":
        "docs/contracts/fixtures/governance-evidence-claim-v1.json",
    "semantic_view_golden_fixture_sha256":
        "docs/contracts/fixtures/governance-semantic-view-v1.json",
    "context_package_schema_sha256": "docs/contracts/context-package-v1.schema.json",
    "context_package_golden_fixture_sha256":
        "docs/contracts/fixtures/context-package-v1.json",
    "context_package_package_manifest_sha256":
        "skills/context-engineering/references/package-manifest.json",
    "evidence_claim_package_manifest_sha256":
        "skills/evidence-claim-management/references/package-manifest.json",
    "policy_authority_package_manifest_sha256":
        "skills/policy-authority/references/package-manifest.json",
    "adr_governance_package_manifest_sha256":
        "skills/adr-governance/references/package-manifest.json",
    "knowledge_graph_curation_package_manifest_sha256":
        "skills/knowledge-graph-curation/references/package-manifest.json",
    "change_impact_cost_risk_package_manifest_sha256":
        "skills/change-impact-cost-risk/references/package-manifest.json",
    "capability_grant_schema_sha256": "docs/contracts/capability-grant-v1.schema.json",
    "capability_grant_golden_fixture_sha256":
        "docs/contracts/fixtures/capability-grant-v1.json",
    "approval_record_schema_sha256":
        "docs/contracts/approval-record-v1.schema.json",
    "approval_record_golden_fixture_sha256":
        "docs/contracts/fixtures/approval-record-v1.json",
    "transition_receipt_schema_sha256":
        "docs/contracts/transition-receipt-v1.schema.json",
    "transition_receipt_golden_fixture_sha256":
        "docs/contracts/fixtures/transition-receipt-v1.json",
    "knowledge_update_proposal_schema_sha256":
        "docs/contracts/knowledge-update-proposal-v1.schema.json",
    "knowledge_update_proposal_golden_fixture_sha256":
        "docs/contracts/fixtures/knowledge-update-proposal-v1.json",
    "bootstrap_grant_issuance_schema_sha256":
        "docs/contracts/bootstrap-grant-issuance-v1.schema.json",
    "bootstrap_grant_issuance_golden_fixture_sha256":
        "docs/contracts/fixtures/bootstrap-grant-issuance-v1.json",
    "bootstrap_repo_read_execution_schema_sha256":
        "docs/contracts/bootstrap-repo-read-execution-v1.schema.json",
    "bootstrap_repo_read_execution_golden_fixture_sha256":
        "docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json",
    "cognitive_atom_schema_sha256":
        "docs/contracts/cognitive-atom-projection-v1.schema.json",
    "cognitive_atom_golden_fixture_sha256":
        "docs/contracts/fixtures/cognitive-atom-projection-v1.json",
    "artifact_evidence_adapter_schema_sha256":
        "docs/contracts/artifact-evidence-adapter-v1.schema.json",
    "artifact_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/artifact-evidence-adapter-v1.json",
    "command_observation_evidence_adapter_schema_sha256":
        "docs/contracts/command-observation-evidence-adapter-v1.schema.json",
    "command_observation_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/command-observation-evidence-adapter-v1.json",
    "evolve_repo_locator_evidence_adapter_schema_sha256":
        "docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json",
    "evolve_repo_locator_evidence_adapter_golden_fixture_sha256":
        "docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json",
    "local_gate_command_observation_producer_schema_sha256":
        "docs/contracts/local-gate-command-observation-producer-v1.schema.json",
    "local_gate_command_observation_producer_golden_fixture_sha256":
        "docs/contracts/fixtures/local-gate-command-observation-producer-v1.json",
    "local_evolve_repo_locator_observation_producer_schema_sha256":
        "docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json",
    "local_evolve_repo_locator_observation_producer_golden_fixture_sha256":
        "docs/contracts/fixtures/local-evolve-repo-locator-observation-producer-v1.json",
    "local_go_package_dependency_graph_observation_producer_schema_sha256":
        "docs/contracts/local-go-package-dependency-graph-observation-producer-v1.schema.json",
    "local_go_package_dependency_graph_observation_producer_golden_fixture_sha256":
        "docs/contracts/fixtures/local-go-package-dependency-graph-observation-producer-v1.json",
    "local_go_package_impact_prescan_schema_sha256":
        "docs/contracts/local-go-package-impact-prescan-v1.schema.json",
    "local_go_package_impact_prescan_golden_fixture_sha256":
        "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json",
    "graph_snapshot_schema_sha256":
        "docs/contracts/graph-snapshot-v1.schema.json",
    "graph_snapshot_golden_fixture_sha256":
        "docs/contracts/fixtures/graph-snapshot-v1.json",
    "graph_snapshot_test_source_schema_sha256":
        "docs/contracts/graph-snapshot-go-test-source-v1.schema.json",
    "graph_snapshot_test_source_golden_fixture_sha256":
        "docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json",
    "architecture_decision_record_v2_schema_sha256":
        "docs/contracts/architecture-decision-record-v2.schema.json",
    "architecture_decision_record_v2_golden_fixture_sha256":
        "docs/contracts/fixtures/ADR-9001-proposed-boundary.md",
    "capability_registry_schema_sha256":
        "docs/contracts/capability-registry-v1.schema.json",
    "capability_registry_golden_fixture_sha256":
        "docs/contracts/fixtures/capability-registry-v1.json",
    "planning_capability_ownership_schema_sha256":
        "docs/contracts/planning-capability-ownership-projection-v1.schema.json",
    "planning_capability_ownership_golden_fixture_sha256":
        "docs/contracts/fixtures/planning-capability-ownership-projection-v1.json",
    "planning_capability_ownership_catalog_source_sha256":
        "docs/design/ai-engineering-os/capability-catalog.v1.yml",
    "planning_capability_ownership_mapping_source_sha256":
        "docs/design/ai-engineering-os/capability-skill-map.v1.yml",
    "project_source_snapshot_schema_sha256":
        "docs/contracts/project-source-snapshot-v1.schema.json",
    "project_source_snapshot_golden_fixture_sha256":
        "docs/contracts/fixtures/project-source-snapshot-v1.json",
    "project_source_snapshot_package_manifest_sha256":
        "skills/project-snapshot/references/package-manifest.json",
    "work_intent_v1_schema_sha256":
        "docs/contracts/work-intent-v1.schema.json",
    "work_intent_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/work-intent-v1.json",
    "authenticated_adr_approval_v1_schema_sha256":
        "docs/contracts/authenticated-architecture-decision-approval-v1.schema.json",
    "authenticated_adr_approval_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json",
    "authenticated_adr_approval_v1_proposal_fixture_sha256":
        "docs/contracts/fixtures/ADR-9002-authenticated-approval-target.md",
    "authenticated_adr_lifecycle_v1_schema_sha256":
        "docs/contracts/authenticated-architecture-decision-lifecycle-v1.schema.json",
    "authenticated_adr_lifecycle_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json",
    "authenticated_adr_lifecycle_v1_proposal_head_a_fixture_sha256":
        "docs/contracts/fixtures/ADR-9003-lifecycle-head-a.md",
    "authenticated_adr_lifecycle_v1_proposal_head_b_fixture_sha256":
        "docs/contracts/fixtures/ADR-9004-lifecycle-head-b.md",
    "authenticated_adr_lifecycle_v1_proposal_join_fixture_sha256":
        "docs/contracts/fixtures/ADR-9005-lifecycle-join.md",
    "authenticated_adr_lifecycle_v1_go_authority_decision_sha256":
        "docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-"
        "authority-service-v1.md",
    "legacy_governance_read_import_v1_schema_sha256":
        "docs/contracts/legacy-governance-read-import-v1.schema.json",
    "legacy_governance_read_import_v1_memory_fixture_sha256":
        "docs/contracts/fixtures/legacy-governance-read-import-memory-v1.jsonl",
    "legacy_governance_read_import_v1_adr_0001_fixture_sha256":
        "docs/contracts/fixtures/legacy-governance-read-import-ADR-0001.md",
    "legacy_governance_read_import_v1_adr_0002_fixture_sha256":
        "docs/contracts/fixtures/legacy-governance-read-import-ADR-0002.md",
    "legacy_governance_read_import_v1_request_fixture_sha256":
        "docs/contracts/fixtures/legacy-governance-read-import-request-v1.json",
    "legacy_governance_read_import_v1_view_fixture_sha256":
        "docs/contracts/fixtures/legacy-governance-read-import-view-v1.json",
    "legacy_governance_read_import_v1_decision_sha256":
        "docs/adr/ADR-0086-legacy-governance-read-only-import-v1.md",
    "kernel_operational_reference_core_v1_schema_sha256":
        "docs/contracts/kernel-operational-reference-core-v1.schema.json",
    "kernel_operational_reference_core_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/kernel-operational-reference-closure-v1.json",
    "kernel_operational_reference_core_v1_decision_sha256":
        "docs/adr/ADR-0088-kernel-operational-reference-core-v1.md",
    "kernel_decision_reference_core_v1_schema_sha256":
        "docs/contracts/kernel-decision-reference-core-v1.schema.json",
    "kernel_decision_reference_core_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json",
    "kernel_decision_reference_core_v1_decision_sha256":
        "docs/adr/ADR-0090-kernel-decision-reference-core-v1.md",
    "decision_capsule_structural_replay_core_v1_schema_sha256":
        "docs/contracts/decision-capsule-structural-replay-core-v1.schema.json",
    "decision_capsule_structural_replay_core_v1_golden_fixture_sha256":
        "docs/contracts/fixtures/decision-capsule-structural-replay-v1.json",
    "decision_capsule_structural_replay_core_v1_decision_sha256":
        "docs/adr/ADR-0092-decision-capsule-structural-replay-core-v1.md",
}

IMPACT_PRESCAN_RESULT = (
    "LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse "
    "dependency closure; system impact unknown; no selected-build, truth, "
    "authority, completion, persistence, execution, or effect attestation)"
)
IMPACT_PRESCAN_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "exact_compact_utf8_input": True,
    "graph_observation_digest_domain": (
        "forgeos.governance.local-go-package-dependency-graph-observation.v1\0"
    ),
    "request_digest_domain": (
        "forgeos.governance.local-go-package-impact-prescan-request.v1\0"
    ),
    "node_digest_domain": (
        "forgeos.governance.local-go-package-impact-prescan-node.v1\0"
    ),
    "edge_digest_domain": (
        "forgeos.governance.local-go-package-impact-prescan-edge.v1\0"
    ),
    "report_digest_domain": (
        "forgeos.governance.local-go-package-impact-prescan-report.v1\0"
    ),
    "envelope_digest_domain": (
        "forgeos.governance.local-go-package-impact-prescan.v1\0"
    ),
    "self_digest_rules": [
        "request_sha256 hashes the complete canonical request with request_sha256 replaced by the empty string",
        "node_sha256 hashes the canonical directory/import_path/module_path/package_name identity projection",
        "edge_sha256 hashes the canonical from_node_id/import_path/relation/role/source_paths/to_node_id identity projection",
        "report_sha256 hashes the complete canonical report with report_sha256 replaced by the empty string",
        "envelope_sha256 hashes the complete canonical envelope with envelope_sha256 replaced by the empty string",
    ],
}
IMPACT_PRESCAN_LIMITS = {
    "max_graph_observation_decoded_bytes": 16777216,
    "max_graph_observation_base64url_bytes": 22369622,
    "max_request_bytes": 25165824,
    "max_report_bytes": 16777216,
    "max_envelope_bytes": 50331648,
    "max_changed_paths": 256,
    "max_resolved_seeds": 256,
    "max_unresolved_seeds": 256,
    "max_reachable_nodes": 16384,
    "max_reachable_edges": 65536,
    "max_source_paths_per_edge": 16384,
    "max_witness_hops_per_node": 1024,
    "max_aggregate_witness_hops": 65536,
    "max_path_scalars": 4096,
    "max_path_utf8_bytes": 16384,
    "max_run_id_bytes": 160,
    "max_json_depth": 16,
    "integer_domain": "signed_int64",
    "runtime_string_length_unit": "utf8_bytes",
    "schema_max_length_unit": "unicode_code_points",
    "schema_length_keywords_are_non_authoritative_approximations": True,
}
IMPACT_PRESCAN_LOCAL_EXECUTION = {
    "execution_mode": "deterministic_pure_local_bytes_only",
    "filesystem_access": False,
    "repository_access": False,
    "implicit_adr_0053_capture": False,
    "clock_access": False,
    "environment_access": False,
    "credential_access": False,
    "process_access": False,
    "provider_access": False,
    "network_access": False,
    "database_access": False,
    "persistence": "none",
}
IMPACT_PRESCAN_AUTHORITY = {
    "delivery": "shipped_pure_local_runtime_and_strict_checker",
    "system_impact_status": "unknown",
    "selected_build_attestation": False,
    "dependency_availability_attestation": False,
    "compile_or_runtime_reachability_attestation": False,
    "truth_attestation": False,
    "authority_attestation": False,
    "completion_attestation": False,
    "permission_attestation": False,
    "persistence_attestation": False,
    "execution_attestation": False,
    "effect_attestation": False,
    "positive_result": IMPACT_PRESCAN_RESULT,
    "attestations": [],
}
IMPACT_PRESCAN_SEMANTIC_VALIDATION = {
    "schema_alone_sufficient": False,
    "required_checks": [
        "unpadded_canonical_base64url_decodes_within_limit_and_round_trips_exactly",
        "decoded_observation_is_exact_canonical_adr_0053_graph_observation",
        "graph_observation_domain_digest_and_producer_run_id_match_request",
        "changed_paths_are_strict_utf8_byte_sorted_unique_canonical_repo_paths",
        "changed_paths_partition_exactly_across_resolved_and_unresolved_seeds",
        "node_and_edge_identities_recompute_from_exact_observation_fields",
        "reachable_nodes_and_edges_are_the_exact_reverse_local_edge_fixed_point",
        "every_reachable_node_has_one_deterministic_shortest_witness",
        "closure_status_and_reason_set_recompute_exactly",
        "system_impact_is_fixed_unknown_with_the_complete_reason_set",
        "request_report_and_envelope_self_digests_recompute_exactly",
        "all_count_byte_depth_and_aggregate_witness_limits_fail_closed",
    ],
}
LOCAL_GO_PACKAGE_IMPACT_PRESCAN = {
    "api_version": "forgeos.governance.local-go-package-impact-prescan/v1",
    "mode": "deterministic_pure_local_bytes_only",
    "input_kind": "exact_adr_0053_graph_observation_and_changed_paths",
    "output_kind": (
        "forgeos.governance.local-go-package-impact-prescan-report/v1"
    ),
    "closure_semantics": (
        "exact_local_edge_reverse_fixed_point_with_deterministic_shortest_witness"
    ),
    "system_impact_status": "unknown",
    "canonicalization": IMPACT_PRESCAN_CANONICALIZATION,
    "limits": IMPACT_PRESCAN_LIMITS,
    "local_execution": IMPACT_PRESCAN_LOCAL_EXECUTION,
    "authority_semantics": IMPACT_PRESCAN_AUTHORITY,
    "semantic_validation": IMPACT_PRESCAN_SEMANTIC_VALIDATION,
    "positive_result": IMPACT_PRESCAN_RESULT,
    "attests": [],
    "persistence": "none",
}
IMPACT_PRESCAN_CANONICAL_REFS = {
    "local_go_package_impact_prescan_schema":
        "docs/contracts/local-go-package-impact-prescan-v1.schema.json",
    "local_go_package_impact_prescan_golden_fixture":
        "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json",
    "local_go_package_impact_prescan_checker":
        "harness/local_go_package_impact_prescan_contract_check.py",
    "local_go_package_impact_prescan_decision":
        "docs/adr/0062-local-go-package-impact-prescan-v1.md",
}
IMPACT_PRESCAN_REFERENCE_IMPLEMENTATIONS = {
    "local_go_package_impact_prescan_go": {
        "ref": "forge-core/internal/goimpactprescan",
        "projection": "catalyst_repository_only_pure_bytes_evaluator",
    },
    "local_go_package_impact_prescan_python": {
        "ref": "harness/local_go_package_impact_prescan_contract",
        "projection": "universal_scaffold_pure_evaluator_and_strict_checker",
    },
}
IMPACT_PRESCAN_NON_CAPABILITY = (
    "Local Go Package ImpactPreScan v1 computes only the exact ADR-0053 local "
    "lexical reverse dependency closure from caller-supplied bytes; system "
    "impact remains unknown, and it performs no live capture, selected-build "
    "analysis, dependency availability check, GraphSnapshot or final "
    "ChangeImpactReport/Cost/Risk/AssessmentReceipt derivation, authority, "
    "completion, persistence, execution, or effect"
)


def impact_prescan_registry_issues(data, path):
    issues = []
    if data.get("local_go_package_impact_prescan") != LOCAL_GO_PACKAGE_IMPACT_PRESCAN:
        issues.append(f"{path}: local Go Package ImpactPreScan contract drifted")
    scope = data.get("scope") if isinstance(data.get("scope"), dict) else {}
    if scope.get("shipped_evaluators") != [
            "local_go_package_impact_prescan", "graph_snapshot",
            "graph_snapshot_test_source", "architecture_decision_record_v2",
            "capability_registry", "planning_capability_ownership",
            "project_source_snapshot"]:
        issues.append(f"{path}: shipped pure evaluator scope drifted")
    if "local_go_package_impact_prescan" in (scope.get("shipped_producers") or []):
        issues.append(f"{path}: ImpactPreScan cannot be classified as a producer")
    refs = data.get("canonical_refs") if isinstance(data.get("canonical_refs"), dict) else {}
    for field, expected in IMPACT_PRESCAN_CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    implementations = data.get("reference_implementations")
    implementations = implementations if isinstance(implementations, dict) else {}
    for field, expected in IMPACT_PRESCAN_REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    non_capabilities = data.get("non_capabilities")
    if (not isinstance(non_capabilities, list) or
            IMPACT_PRESCAN_NON_CAPABILITY not in non_capabilities):
        issues.append(f"{path}: ImpactPreScan non-capability boundary drifted")
    return issues


def impact_prescan_schema_issues(repo_root):
    relative = IMPACT_PRESCAN_CANONICAL_REFS["local_go_package_impact_prescan_schema"]
    path = repo_root / relative
    try:
        schema = json.loads(read_bounded_file(path, label=relative, max_bytes=1048576))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate ImpactPreScan Schema: {error}"]
    expected = {
        "x-forgeos-canonicalization": IMPACT_PRESCAN_CANONICALIZATION,
        "x-forgeos-limits": IMPACT_PRESCAN_LIMITS,
        "x-forgeos-local-execution": IMPACT_PRESCAN_LOCAL_EXECUTION,
        "x-forgeos-authority-semantics": IMPACT_PRESCAN_AUTHORITY,
        "x-forgeos-semantic-validation": IMPACT_PRESCAN_SEMANTIC_VALIDATION,
    }
    return [f"{path}: {field} drifted" for field, value in expected.items()
            if schema.get(field) != value]


def impact_prescan_fixture_issues(repo_root):
    return validate_impact_golden_fixture(repo_root)


def impact_prescan_integration_issues(data, path, repo_root):
    issues = impact_prescan_registry_issues(data, path)
    issues.extend(impact_prescan_schema_issues(repo_root))
    issues.extend(impact_prescan_fixture_issues(repo_root))
    return issues
