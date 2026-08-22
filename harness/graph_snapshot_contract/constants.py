"""Frozen ADR-0065 wire constants, domains, vocabularies, and bounds."""

from __future__ import annotations

import re

CANONICALIZATION = "forgeos.canonical-json/v1"
ENVELOPE_API = "forgeos.governance.local-go-graph-snapshot-projection/v1"
REQUEST_API = "forgeos.governance.local-go-graph-snapshot-projection-request/v1"
SNAPSHOT_API = "forgeos.governance.graph-snapshot/v1"
PROFILE_ID = "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1"
TEST_SOURCE_PROFILE_ID = (
    "adr-0053-selected-go-module-lexical-package-test-source-"
    "partial-graph-snapshot-v1"
)
GRAPH_API = "forgeos.go-package-dependency-graph-observation/v1"
GRAPH_PROFILE = "selected-go-module-lexical-dependency-graph-v1"
FIXTURE_API = "forgeos.governance.local-go-graph-snapshot-projection.fixture/v1"
FIXTURE_PATH = "docs/contracts/fixtures/graph-snapshot-v1.json"

RESULT = (
    "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical "
    "module/package subgraph only; coverage partial and system/freshness unknown; "
    "no selected-build, cross-surface completeness, truth, authority, completion, "
    "persistence, execution, impact, or effect attestation)"
)
CHECKED = (
    "VALID_AUTHORITY_FREE_GRAPH_SNAPSHOT_V1 "
    "(exact ADR-0053 partial projection only; system/freshness unknown; no authority)"
)
FIXTURE_SEMANTICS = (
    "PURE_CONTRACT_FIXTURE (deterministic caller bytes only; partial selected-module "
    "lexical graph and unknown system/freshness; no live capture, selected build, "
    "truth, authority, completion, persistence, execution, impact, or effect attestation)"
)

REQUEST_DOMAIN = b"forgeos.governance.local-go-graph-snapshot-projection-request.v1\0"
SOURCE_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-source-identity.v1\0"
SOURCE_DOMAIN = b"forgeos.governance.graph-snapshot-source.v1\0"
EXTRACTOR_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-extractor-identity.v1\0"
EXTRACTOR_DOMAIN = b"forgeos.governance.graph-snapshot-extractor.v1\0"
NODE_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-node-identity.v1\0"
NODE_DOMAIN = b"forgeos.governance.graph-snapshot-node.v1\0"
EDGE_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-edge-identity.v1\0"
EDGE_DOMAIN = b"forgeos.governance.graph-snapshot-edge.v1\0"
UNRESOLVED_NODE_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-node-identity.v1\0"
UNRESOLVED_NODE_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-node.v1\0"
UNRESOLVED_EDGE_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-edge-identity.v1\0"
UNRESOLVED_EDGE_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-edge.v1\0"
SOURCE_SET_DOMAIN = b"forgeos.governance.graph-snapshot-source-set.v1\0"
EXTRACTOR_SET_DOMAIN = b"forgeos.governance.graph-snapshot-extractor-set.v1\0"
NODE_SET_DOMAIN = b"forgeos.governance.graph-snapshot-node-set.v1\0"
EDGE_SET_DOMAIN = b"forgeos.governance.graph-snapshot-edge-set.v1\0"
UNRESOLVED_NODE_SET_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-node-set.v1\0"
UNRESOLVED_EDGE_SET_DOMAIN = b"forgeos.governance.graph-snapshot-unresolved-edge-set.v1\0"
CROSSWALK_SET_DOMAIN = b"forgeos.governance.graph-snapshot-adr-0062-node-crosswalk-set.v1\0"
COVERAGE_DOMAIN = b"forgeos.governance.graph-snapshot-coverage.v1\0"
SNAPSHOT_IDENTITY_DOMAIN = b"forgeos.governance.graph-snapshot-identity.v1\0"
SNAPSHOT_DOMAIN = b"forgeos.governance.graph-snapshot.v1\0"
ENVELOPE_DOMAIN = b"forgeos.governance.local-go-graph-snapshot-projection.v1\0"
ADR_0062_NODE_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan-node.v1\0"

MAX_GRAPH_BYTES = 16 << 20
MAX_GRAPH_BASE64URL_BYTES = 22_369_622
MAX_REQUEST_BYTES = 24 << 20
MAX_SNAPSHOT_BYTES = 64 << 20
MAX_ENVELOPE_BYTES = 96 << 20
MAX_FIXTURE_BYTES = 192 << 20
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY_ITEMS = 65_536
MAX_EDGE_UNION = 81_920
MAX_TEST_SOURCE_EDGE_UNION = 98_304
MAX_PACKAGE_NODES = 16_384
MAX_NODES = 16_385
MAX_TEST_SOURCE_NODES = 32_769
MAX_DEPENDENCY_CANDIDATES = 65_536
MAX_CONTAINS_EDGES = 16_384
MAX_UNRESOLVED_NODES = 17_408
MAX_UNRESOLVED_EDGES = 65_536
MAX_CROSSWALKS = 16_384
MAX_LOCATORS_PER_RECORD = 16_384
MAX_AGGREGATE_LOCATORS = 131_072
MAX_PATH_SCALARS = 4_096
MAX_PATH_BYTES = 16_384
MAX_IDENTIFIER_BYTES = 160
MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808

HASH_RE = re.compile(r"^[0-9a-f]{64}$")
IDENTIFIER_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]*$")
BASE64URL_RE = re.compile(
    r"^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{2}|[A-Za-z0-9_-]{3})?$")

ENVELOPE_FIELDS = {
    "api_version", "canonicalization", "envelope_sha256", "request", "snapshot",
}
REQUEST_FIELDS = {
    "api_version", "canonicalization", "graph_observation_base64url",
    "graph_observation_sha256", "project_id", "projector_profile_id",
    "request_sha256", "run_id",
}
SNAPSHOT_FIELDS = {
    "adr_0062_node_crosswalk", "api_version", "canonicalization", "coverage",
    "coverage_sha256", "crosswalk_set_sha256", "edge_set_sha256", "edges",
    "extractor_set_sha256", "extractors", "freshness", "node_set_sha256",
    "nodes", "profile_id", "project_id", "request_sha256", "result",
    "snapshot_id", "snapshot_identity_sha256", "snapshot_sha256",
    "source_set_sha256", "sources", "system_knowledge_status",
    "system_unknown_reason_codes", "unresolved_edge_set_sha256",
    "unresolved_edges", "unresolved_node_set_sha256", "unresolved_nodes",
}
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

IDENTITY_FIELDS = {
    "source": ("graph_api_version", "graph_observation_sha256", "graph_profile_id",
               "observer_run_id", "source_revision", "source_tree_sha256", "source_type"),
    "extractor": ("extractor_type", "extractor_version", "producer_id",
                  "projector_profile_id"),
    "node": ("identity_namespace", "identity_profile_id", "node_type", "project_id",
             "qualified_name_components"),
    "edge": ("category_axes", "from_node_id", "identity_profile_id",
             "import_discriminator", "parallel_discriminator", "relation",
             "source_role", "to_node_id"),
    "unresolved_node": ("candidate_identity_namespace", "candidate_identity_profile_id",
                        "candidate_qualified_name_components", "kind", "project_id",
                        "reason_code"),
    "unresolved_edge": ("category_axes", "from_node_id", "identity_profile_id",
                        "import_discriminator", "parallel_discriminator", "project_id",
                        "reason_code", "relation", "resolution", "resolution_detail",
                        "source_role", "target_candidate"),
    "snapshot": ("coverage_sha256", "crosswalk_set_sha256", "edge_set_sha256",
                 "extractor_set_sha256", "node_set_sha256", "profile_id", "project_id",
                 "request_sha256", "source_set_sha256", "unresolved_edge_set_sha256",
                 "unresolved_node_set_sha256"),
}

RESOLUTION_REASONS = {
    "ambiguous_local": "multiple_compile_packages",
    "unresolved_local": "no_compile_package",
    "nested_module_boundary": "nested_module_boundary",
    "stdlib_candidate": "stdlib_candidate_not_resolved",
    "external_candidate": "external_candidate_not_resolved",
    "cgo_pseudo": "cgo_pseudo_not_resolved",
    "unsupported": "noncanonical_import_path",
}
GO_COVERAGE_BASE_REASONS = [
    "all_regular_go_files_lexical_union_not_selected_build",
    "compile_test_runtime_reachability_not_observed",
    "go_module_graph_not_resolved",
    "single_selected_go_module_only",
    "source_observation_not_atomic_snapshot",
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
SURFACES = [
    "adr_decision", "api_event_contract", "business_domain",
    "data_schema_migration", "deployment_environment", "go_module_package_lexical",
    "operations_runtime_signal", "other_language_module_package", "owner_policy",
    "symbol_call_runtime", "test_verification",
]
FRESHNESS_REASONS = [
    "source_observation_clock_unauthenticated",
    "source_observation_not_atomic_snapshot",
    "zero_duration_expiry_no_freshness_attestation",
]
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
    "test_and_verification_surfaces_not_observed",
]
