"""Frozen ADR-0066 lexical test-source profile constants and bounds."""

from __future__ import annotations

from .constants import (
    CANONICALIZATION, COVERAGE_DOMAIN, CROSSWALK_SET_DOMAIN, EDGE_SET_DOMAIN,
    EXTRACTOR_SET_DOMAIN, FRESHNESS_REASONS, GRAPH_API, GRAPH_PROFILE,
    IDENTITY_FIELDS,
    MAX_CROSSWALKS, MAX_DEPTH, MAX_ENVELOPE_BYTES, MAX_FIELDS,
    MAX_FIXTURE_BYTES, MAX_GRAPH_BASE64URL_BYTES, MAX_GRAPH_BYTES,
    MAX_IDENTIFIER_BYTES, MAX_LOCATORS_PER_RECORD, MAX_PATH_BYTES,
    MAX_PATH_SCALARS, MAX_REQUEST_BYTES, MAX_SNAPSHOT_BYTES,
    MAX_TEST_SOURCE_EDGE_UNION, MAX_TEST_SOURCE_NODES, MAX_UNRESOLVED_EDGES,
    MAX_UNRESOLVED_NODES, NODE_SET_DOMAIN, SNAPSHOT_API, SNAPSHOT_DOMAIN,
    SNAPSHOT_IDENTITY_DOMAIN, SOURCE_SET_DOMAIN, SURFACES,
    TEST_SOURCE_PROFILE_ID, UNRESOLVED_EDGE_SET_DOMAIN,
    UNRESOLVED_NODE_SET_DOMAIN,
)

ENVELOPE_API = (
    "forgeos.governance.local-go-test-source-graph-snapshot-projection/v1"
)
REQUEST_API = (
    "forgeos.governance.local-go-test-source-graph-snapshot-projection-request/v1"
)
FIXTURE_API = (
    "forgeos.governance.local-go-test-source-graph-snapshot-projection.fixture/v1"
)
FIXTURE_PATH = "docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json"

REQUEST_DOMAIN = (
    b"forgeos.governance.local-go-test-source-graph-snapshot-projection-request.v1\0"
)
ENVELOPE_DOMAIN = (
    b"forgeos.governance.local-go-test-source-graph-snapshot-projection.v1\0"
)

RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package/test-source subgraph only; test nodes are source sets, not tests "
    "or outcomes; coverage partial and system/freshness unknown; no selected-build, "
    "cross-surface completeness, truth, authority, completion, persistence, "
    "execution, verification, impact, or effect attestation)"
)
CHECKED = (
    "VALID_AUTHORITY_FREE_GO_TEST_SOURCE_GRAPH_SNAPSHOT_V1 "
    "(exact ADR-0053 lexical test-source projection only; no test outcome or authority)"
)
FIXTURE_SEMANTICS = (
    "PURE_CONTRACT_FIXTURE (deterministic caller bytes only; package-scoped lexical "
    "test source sets, partial coverage, and unknown system/freshness; no test case, "
    "execution, outcome, verification, truth, authority, persistence, or effect "
    "attestation)"
)

MAX_PACKAGE_NODES = 16_384
MAX_TEST_NODES = 16_384
MAX_NODES = MAX_TEST_SOURCE_NODES
MAX_PACKAGE_CONTAINS_EDGES = 16_384
MAX_TEST_CONTAINS_EDGES = 16_384
MAX_EDGE_UNION = MAX_TEST_SOURCE_EDGE_UNION
MAX_DEPENDENCY_CANDIDATES = 65_536
MAX_AGGREGATE_LOCATORS = 132_097

GO_BASE_REASONS = [
    "all_regular_go_files_lexical_union_not_selected_build",
    "compile_runtime_reachability_not_observed",
    "go_module_graph_not_resolved",
    "single_selected_go_module_only",
    "source_observation_not_atomic_snapshot",
]
TEST_BASE_REASONS = [
    "go_test_files_lexical_source_set_only",
    "selected_test_build_not_observed",
    "source_observation_not_atomic_snapshot",
    "test_case_identity_not_observed",
    "test_execution_not_observed",
    "test_outcome_and_coverage_not_observed",
]
RESOLUTION_COVERAGE_REASONS = {
    "ambiguous_local": "ambiguous_local_dependency_present",
    "cgo_pseudo": "cgo_pseudo_dependency_present",
    "external_candidate": "external_candidate_dependency_present",
    "nested_module_boundary": "nested_module_boundary_dependency_present",
    "stdlib_candidate": "stdlib_candidate_dependency_present",
    "unresolved_local": "unresolved_local_dependency_present",
    "unsupported": "unsupported_import_dependency_present",
}
SYSTEM_UNKNOWN_REASONS = [
    "adr_owner_policy_surfaces_not_observed",
    "api_event_contract_surfaces_not_observed",
    "business_domain_surfaces_not_observed",
    "call_and_runtime_reachability_not_observed",
    "data_and_migration_surfaces_not_observed",
    "deployment_and_operations_surfaces_not_observed",
    "freshness_not_attested",
    "other_language_module_package_surfaces_not_observed",
    "selected_build_semantics_not_observed",
    "test_execution_and_verification_outcomes_not_observed",
]

FIXTURE_FIELDS = {"api_version", "expected", "fixture_semantics", "input"}
FIXTURE_INPUT_FIELDS = {
    "canonical_graph_observation_json", "graph_observation_sha256",
    "project_id", "run_id",
}
FIXTURE_EXPECTED_FIELDS = {
    "canonical_envelope_json", "coverage_sha256", "crosswalk_set_sha256",
    "edge_set_sha256", "envelope_sha256", "extractor_set_sha256",
    "node_set_sha256", "request_sha256", "snapshot_id",
    "snapshot_identity_sha256", "snapshot_sha256", "source_set_sha256",
    "unresolved_edge_set_sha256", "unresolved_node_set_sha256",
}

__all__ = [name for name in globals() if name.isupper()]
