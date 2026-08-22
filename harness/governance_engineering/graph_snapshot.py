"""ADR-0065/0066 authority-free GraphSnapshot governance integration."""

from __future__ import annotations

import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/graph-snapshot-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/graph-snapshot-v1.json"
CHECKER_RELATIVE = "harness/graph_snapshot_contract_check.py"
SKILL_RELATIVE = ".agent/skills/knowledge-graph-curation.md"
DECISION_RELATIVE = (
    "docs/adr/0065-authority-free-graph-snapshot-v1-contract.md"
)
TEST_SOURCE_SCHEMA_RELATIVE = (
    "docs/contracts/graph-snapshot-go-test-source-v1.schema.json"
)
TEST_SOURCE_FIXTURE_RELATIVE = (
    "docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json"
)
TEST_SOURCE_DECISION_RELATIVE = (
    "docs/adr/0066-local-go-lexical-test-source-graph-snapshot.md"
)
PROFILE_ID = "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1"
TEST_SOURCE_PROFILE_ID = (
    "adr-0053-selected-go-module-lexical-package-test-source-partial-"
    "graph-snapshot-v1"
)
RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package subgraph only; coverage partial and system/freshness unknown; "
    "no selected-build, cross-surface completeness, truth, authority, completion, "
    "persistence, execution, impact, or effect attestation)"
)
TEST_SOURCE_RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package/test-source subgraph only; test nodes are source sets, not "
    "tests or outcomes; coverage partial and system/freshness unknown; no "
    "selected-build, cross-surface completeness, truth, authority, completion, "
    "persistence, execution, verification, impact, or effect attestation)"
)

GRAPH_SNAPSHOT = {
    "api_version": "forgeos.governance.local-go-graph-snapshot-projection/v1",
    "snapshot_api_version": "forgeos.governance.graph-snapshot/v1",
    "projector_profile_id": PROFILE_ID,
    "mode": "deterministic_pure_local_projector_and_evaluator",
    "input_kind": "exact_canonical_adr_0053_graph_observation_caller_bytes",
    "output_kind": "authority_free_partial_graph_snapshot",
    "projection_scope": {
        "node_types": ["module", "package"],
        "resolved_relations": ["contains", "depends_on"],
        "coverage_surface_count": 11,
        "coverage_status": "partial",
        "freshness_status": "unknown",
        "system_knowledge_status": "unknown",
        "identity_semantics": (
            "caller_declared_project_scoped_semantic_name_stable_only"
        ),
        "unresolved_mapping": "exact_bijection_from_input_gaps",
        "adr_0062_crosswalk": "deterministic_non_equivalent_identity_mapping",
    },
    "local_execution": {
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
    },
    "authority_semantics": {
        "delivery": "shipped_go_and_python_pure_projector_strict_checker",
        "source_or_project_identity_authentication": False,
        "selected_build_attestation": False,
        "cross_surface_completeness_attestation": False,
        "freshness_attestation": False,
        "truth_attestation": False,
        "authority_attestation": False,
        "completion_attestation": False,
        "permission_attestation": False,
        "persistence_attestation": False,
        "execution_attestation": False,
        "impact_attestation": False,
        "effect_attestation": False,
        "satisfies_g3_or_assessment_join": False,
        "positive_result": RESULT,
        "attestations": [],
    },
    "semantic_validation": {
        "schema_alone_sufficient": False,
        "exact_input_reconstruction_required": True,
        "identity_record_set_and_envelope_digests_recomputed": True,
        "node_edge_crosswalk_and_unresolved_bijections_recomputed": True,
        "coverage_freshness_and_unknown_reasons_recomputed": True,
        "complete_canonical_envelope_byte_comparison": True,
        "collision_dangling_endpoint_and_limit_fail_closed": True,
    },
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
}

GRAPH_SNAPSHOT_TEST_SOURCE = {
    "api_version": (
        "forgeos.governance.local-go-test-source-graph-snapshot-projection/v1"
    ),
    "snapshot_api_version": "forgeos.governance.graph-snapshot/v1",
    "projector_profile_id": TEST_SOURCE_PROFILE_ID,
    "mode": "deterministic_pure_local_projector_and_evaluator",
    "input_kind": "exact_canonical_adr_0053_graph_observation_caller_bytes",
    "output_kind": "authority_free_partial_graph_snapshot",
    "projection_scope": {
        "node_types": ["module", "package", "test"],
        "resolved_relations": ["contains", "depends_on"],
        "partial_surfaces": [
            "go_module_package_lexical", "test_verification",
        ],
        "coverage_surface_count": 11,
        "coverage_status": "partial",
        "freshness_status": "unknown",
        "system_knowledge_status": "unknown",
        "identity_semantics": (
            "caller_declared_project_scoped_semantic_name_stable_only"
        ),
        "test_identity_semantics": (
            "lexical_test_source_set_not_test_case_or_execution"
        ),
        "cross_profile_stable_ids": True,
        "profile_bound_full_records": True,
        "unresolved_mapping": "exact_bijection_from_input_gaps",
        "adr_0062_crosswalk": (
            "deterministic_package_only_non_equivalent_identity_mapping"
        ),
    },
    "local_execution": {
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
    },
    "authority_semantics": {
        "delivery": "shipped_go_and_python_pure_projector_strict_checker",
        "source_or_project_identity_authentication": False,
        "selected_build_attestation": False,
        "cross_surface_completeness_attestation": False,
        "freshness_attestation": False,
        "test_case_identity_attestation": False,
        "test_execution_or_outcome_attestation": False,
        "verified_subject_attestation": False,
        "truth_attestation": False,
        "authority_attestation": False,
        "completion_attestation": False,
        "permission_attestation": False,
        "persistence_attestation": False,
        "execution_attestation": False,
        "verification_attestation": False,
        "impact_attestation": False,
        "effect_attestation": False,
        "satisfies_g3_or_assessment_join": False,
        "positive_result": TEST_SOURCE_RESULT,
        "attestations": [],
    },
    "semantic_validation": {
        "schema_alone_sufficient": False,
        "exact_input_reconstruction_required": True,
        "dedicated_transport_profile_negotiation_required": True,
        "legacy_adr_0065_profile_and_golden_unchanged": True,
        "test_node_and_module_contains_edge_bijections_recomputed": True,
        "disjoint_go_and_test_coverage_partition_recomputed": True,
        "identity_record_set_and_envelope_digests_recomputed": True,
        "node_edge_crosswalk_and_unresolved_bijections_recomputed": True,
        "coverage_freshness_and_unknown_reasons_recomputed": True,
        "complete_canonical_envelope_byte_comparison": True,
        "collision_dangling_endpoint_and_limit_fail_closed": True,
    },
    "positive_result": TEST_SOURCE_RESULT,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "graph_snapshot_schema": SCHEMA_RELATIVE,
    "graph_snapshot_golden_fixture": FIXTURE_RELATIVE,
    "graph_snapshot_checker": CHECKER_RELATIVE,
    "graph_snapshot_skill": SKILL_RELATIVE,
    "graph_snapshot_decision": DECISION_RELATIVE,
    "graph_snapshot_test_source_schema": TEST_SOURCE_SCHEMA_RELATIVE,
    "graph_snapshot_test_source_golden_fixture": TEST_SOURCE_FIXTURE_RELATIVE,
    "graph_snapshot_test_source_checker": CHECKER_RELATIVE,
    "graph_snapshot_test_source_skill": SKILL_RELATIVE,
    "graph_snapshot_test_source_decision": TEST_SOURCE_DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "graph_snapshot_go": {
        "ref": "forge-core/internal/graphsnapshot",
        "projection": "catalyst_repository_only_pure_bytes_projector",
    },
    "graph_snapshot_python": {
        "ref": "harness/graph_snapshot_contract",
        "projection": "universal_scaffold_pure_projector_and_strict_checker",
    },
    "graph_snapshot_test_source_go": {
        "ref": "forge-core/internal/graphsnapshot",
        "projection": "catalyst_repository_only_pure_bytes_projector",
    },
    "graph_snapshot_test_source_python": {
        "ref": "harness/graph_snapshot_contract",
        "projection": "universal_scaffold_pure_projector_and_strict_checker",
    },
}

NON_CAPABILITY = (
    "GraphSnapshot v1 projects only an exact caller-supplied ADR-0053 selected-Go-"
    "module lexical observation into a partial module/package graph; all other "
    "surfaces and freshness remain unknown, and it performs no live capture, "
    "selected-build or cross-surface analysis, authenticated provenance, truth, "
    "authority, permission, completion, persistence, execution, impact, Cost/Risk/"
    "AssessmentReceipt derivation, G3 or Assessment Join satisfaction, or effect"
)
TEST_SOURCE_NON_CAPABILITY = (
    "GraphSnapshot Go lexical test-source profile v1 projects only exact caller-"
    "supplied ADR-0053 bytes into a partial module/package/test-source-set graph; "
    "test nodes are lexical source sets rather than test cases, executions, "
    "outcomes, coverage or verified-subject mappings, both Go and test surfaces "
    "remain partial, system/freshness remain unknown, and it performs no live "
    "capture, selected-build or cross-surface completeness analysis, authenticated "
    "provenance, truth, authority, permission, completion, persistence, execution, "
    "verification, impact, Cost/Risk/AssessmentReceipt derivation, G3 or Assessment "
    "Join satisfaction, or effect"
)

SKILL_MARKERS = [
    "ADR-0065", "ADR-0066", "forgeos.governance.graph-snapshot/v1", PROFILE_ID,
    TEST_SOURCE_PROFILE_ID,
    "caller-declared", "PARTIAL", "UNKNOWN", "unresolved", "freshness",
    "schema-only", "G3", "Assessment Join", "forge accept",
]

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root", "graph_snapshot"],
    "positive": "test_registry_classifies_partial_projector_without_authority",
    "negative": "test_scope_authority_and_non_capability_drift_fail_closed",
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("graph_snapshot") != GRAPH_SNAPSHOT:
        issues.append(f"{path}: GraphSnapshot projector/evaluator contract drifted")
    if data.get("graph_snapshot_test_source") != GRAPH_SNAPSHOT_TEST_SOURCE:
        issues.append(
            f"{path}: GraphSnapshot test-source projector contract drifted"
        )
    scope = _mapping(data.get("scope"))
    expected_projectors = ["graph_snapshot", "graph_snapshot_test_source"]
    if scope.get("shipped_projectors") != expected_projectors:
        issues.append(f"{path}: shipped GraphSnapshot projector scope drifted")
    if scope.get("shipped_evaluators") != [
            "local_go_package_impact_prescan", "graph_snapshot",
            "graph_snapshot_test_source", "architecture_decision_record_v2",
            "capability_registry", "planning_capability_ownership",
            "project_source_snapshot"]:
        issues.append(f"{path}: shipped pure evaluator scope drifted")
    forbidden = (scope.get("shipped_producers") or []) + (
        scope.get("shipped_runtime_profiles") or [])
    if any(projector in forbidden for projector in expected_projectors):
        issues.append(f"{path}: GraphSnapshot cannot be a producer or authority runtime")
    if "GraphSnapshot" not in (scope.get("shipped_projections") or []):
        issues.append(f"{path}: GraphSnapshot shipped projection kind is missing")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: GraphSnapshot non-capability boundary drifted")
    if TEST_SOURCE_NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(
            f"{path}: GraphSnapshot test-source non-capability boundary drifted"
        )
    return issues


def _load_schema(repo_root, relative=SCHEMA_RELATIVE):
    path = repo_root / relative
    try:
        raw = read_bounded_file(path, label=relative, max_bytes=1_048_576)
        return path, json.loads(raw.decode("utf-8")), None
    except (OSError, ContractError, UnicodeDecodeError,
            json.JSONDecodeError) as error:
        return path, None, error


def schema_issues(repo_root):
    path, schema, error = _load_schema(repo_root)
    if error:
        return [f"{path}: cannot validate GraphSnapshot Schema: {error}"]
    issues = []
    profile = _mapping(schema.get("x-forgeos-profile"))
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    semantics = _mapping(schema.get("x-forgeos-semantic-validation"))
    taxonomy = _mapping(schema.get("x-forgeos-taxonomy"))
    limits = _mapping(schema.get("x-forgeos-limits"))
    if profile.get("projector_profile_id") != PROFILE_ID:
        issues.append(f"{path}: projector profile drifted")
    if profile.get("coverage_status") != "partial" or any(
            profile.get(field) != "unknown" for field in (
                "freshness_status", "system_knowledge_status")):
        issues.append(f"{path}: partial/unknown status contract drifted")
    false_authority = (
        "authenticates_project_observer_projector_git_or_clock",
        "selected_build_or_system_graph_completeness_attestation",
        "truth_authority_permission_completion_persistence_execution_effect_attestation",
        "change_impact_cost_risk_or_assessment_output",
        "satisfies_g3_or_assessment_join",
        "rust_implementation_delivered",
    )
    if any(authority.get(field) is not False for field in false_authority):
        issues.append(f"{path}: authority/non-capability metadata drifted")
    if semantics.get("schema_alone_sufficient") is not False or len(
            semantics.get("required_checks") or []) != 14:
        issues.append(f"{path}: exact semantic reconstruction contract drifted")
    if len(_mapping(taxonomy.get("relation_semantics"))) != 20:
        issues.append(f"{path}: relation taxonomy drifted")
    if len(_mapping(taxonomy.get("node_families")).get("any") or []) != 31:
        issues.append(f"{path}: node taxonomy drifted")
    expected_limits = {
        "max_graph_observation_decoded_bytes": 16_777_216,
        "max_resolved_edge_union": 81_920,
        "max_total_resolved_and_unresolved_dependency_candidates": 65_536,
        "coverage_surface_count": 11,
    }
    if any(limits.get(field) != value for field, value in expected_limits.items()):
        issues.append(f"{path}: GraphSnapshot resource limits drifted")
    return issues


def _test_source_authority_issues(authority, path):
    issues = []
    false_authority = (
        "authenticates_project_observer_projector_git_source_or_clock",
        "parses_test_declarations_or_test_cases",
        "runs_compiles_or_executes_tests",
        "attests_test_pass_fail_coverage_flakiness_or_verified_subject",
        "emits_verified_by_or_observed_by",
        "selected_build_or_system_graph_completeness_attestation",
        "truth_authority_permission_completion_persistence_execution_verification_or_effect_attestation",
        "change_impact_cost_risk_or_assessment_output",
        "satisfies_g3_or_assessment_join", "rust_implementation_delivered",
    )
    if any(authority.get(field) is not False for field in false_authority):
        issues.append(f"{path}: test-source authority boundary drifted")
    if authority.get(
            "fixture_checker_go_python_runtime_cli_registry_skill_scaffold_delivered"
            ) is not True or authority.get("roadmap_closure_delivered") is not True:
        issues.append(f"{path}: test-source delivery metadata drifted")
    return issues


def test_source_schema_issues(repo_root):
    path, schema, error = _load_schema(repo_root, TEST_SOURCE_SCHEMA_RELATIVE)
    if error:
        return [f"{path}: cannot validate test-source GraphSnapshot Schema: {error}"]
    issues = []
    profile = _mapping(schema.get("x-forgeos-profile"))
    topology = _mapping(schema.get("x-forgeos-topology"))
    coverage = _mapping(schema.get("x-forgeos-coverage"))
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    semantics = _mapping(schema.get("x-forgeos-semantic-validation"))
    limits = _mapping(schema.get("x-forgeos-limits"))
    if profile.get("projector_profile_id") != TEST_SOURCE_PROFILE_ID:
        issues.append(f"{path}: test-source projector profile drifted")
    unchanged = profile.get(
        "legacy_adr_0065_profile_api_schema_fixture_and_golden_unchanged")
    if unchanged is not True:
        issues.append(f"{path}: ADR-0065 compatibility boundary drifted")
    if topology.get("node_types") != ["module", "package", "test"] or any(
            topology.get(field) is not False for field in (
                "verified_by_generated", "observed_by_generated",
                "test_case_execution_outcome_or_coverage_inferred")):
        issues.append(f"{path}: lexical test-source topology drifted")
    if coverage.get("partial_surfaces") != [
            "go_module_package_lexical", "test_verification"]:
        issues.append(f"{path}: disjoint partial coverage surface contract drifted")
    issues.extend(_test_source_authority_issues(authority, path))
    if semantics.get("schema_alone_sufficient") is not False or len(
            semantics.get("required_checks") or []) != 15:
        issues.append(f"{path}: test-source exact reconstruction contract drifted")
    expected_limits = {
        "max_graph_observation_decoded_bytes": 16_777_216,
        "max_nodes": 32_769,
        "max_resolved_edge_union": 98_304,
        "max_aggregate_source_locators": 132_097,
        "coverage_surface_count": 11,
    }
    if any(limits.get(field) != value for field, value in expected_limits.items()):
        issues.append(f"{path}: test-source resource limits drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.graph_snapshot_contract")
    if not isinstance(detector, dict):
        return ["GraphSnapshot shadow detector is missing"]
    issues = []
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("GraphSnapshot detector requires exact envelope arguments")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("GraphSnapshot detector must remain shadow and non-load-bearing")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"GraphSnapshot detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate GraphSnapshot Skill: {error}"]
    return [f"{path}: missing GraphSnapshot marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def integration_issues(data, path, repo_root, agent_root):
    from graph_snapshot_contract import (
        validate_golden_fixture, validate_test_source_golden_fixture,
    )
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(test_source_schema_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    issues.extend(validate_test_source_golden_fixture(repo_root))
    return issues
