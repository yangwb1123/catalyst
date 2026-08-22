"""Frozen ADR-0062 wire constants and resource bounds."""

from __future__ import annotations

import re

CANONICALIZATION = "forgeos.canonical-json/v1"
ENVELOPE_API = "forgeos.governance.local-go-package-impact-prescan/v1"
REQUEST_API = "forgeos.governance.local-go-package-impact-prescan-request/v1"
REPORT_API = "forgeos.governance.local-go-package-impact-prescan-report/v1"
FIXTURE_API = "forgeos.governance.local-go-package-impact-prescan.fixture/v1"
FIXTURE_PATH = "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json"

REQUEST_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan-request.v1\0"
NODE_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan-node.v1\0"
EDGE_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan-edge.v1\0"
REPORT_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan-report.v1\0"
ENVELOPE_DOMAIN = b"forgeos.governance.local-go-package-impact-prescan.v1\0"

RESULT = (
    "LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse "
    "dependency closure; system impact unknown; no selected-build, truth, "
    "authority, completion, persistence, execution, or effect attestation)"
)
CHECKED = (
    "VALID_LOCAL_GO_PACKAGE_IMPACT_PRESCAN_V1 "
    "(contract bytes only; system impact unknown; no authority)"
)

MAX_GRAPH_BYTES = 16 << 20
MAX_GRAPH_BASE64URL_BYTES = 22_369_622
MAX_REQUEST_BYTES = 24 << 20
MAX_REPORT_BYTES = 16 << 20
MAX_ENVELOPE_BYTES = 48 << 20
MAX_FIXTURE_BYTES = 64 << 20
MAX_CHANGED_PATHS = 256
MAX_NODES = 16_384
MAX_EDGES = 65_536
MAX_SOURCE_PATHS = 16_384
MAX_WITNESS_HOPS = 1_024
MAX_AGGREGATE_WITNESS_HOPS = 65_536
MAX_PATH_SCALARS = 4_096
MAX_PATH_BYTES = 16_384
MAX_RUN_ID_BYTES = 160
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY_ITEMS = 65_536
MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808

HASH_RE = re.compile(r"^[0-9a-f]{64}$")
RUN_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]{0,159}$")
BASE64URL_RE = re.compile(r"^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{2}|[A-Za-z0-9_-]{3})?$")
REVISION_RE = re.compile(r"^(?:git-sha1:[a-f0-9]{40}|git-sha256:[a-f0-9]{64})$")

ENVELOPE_FIELDS = {
    "api_version", "canonicalization", "envelope_sha256", "report", "request",
}
FIXTURE_FIELDS = {"api_version", "expected", "input"}
FIXTURE_EXPECTED_FIELDS = {
    "canonical_envelope_json", "envelope_sha256", "report_sha256",
    "request_sha256",
}
FIXTURE_INPUT_FIELDS = {
    "canonical_graph_observation_json", "changed_paths",
    "graph_observation_sha256", "run_id",
}
REQUEST_FIELDS = {
    "api_version", "canonicalization", "changed_paths",
    "graph_observation_base64url", "graph_observation_sha256",
    "request_sha256", "run_id",
}
REPORT_FIELDS = {
    "api_version", "canonicalization", "closure_reason_codes",
    "graph_observation_sha256", "package_lexical_closure_status",
    "reachable_edges", "reachable_nodes", "report_sha256", "request_sha256",
    "resolved_seeds", "result", "run_id", "system_impact_status",
    "system_unknown_reason_codes", "unresolved_seeds",
}
NODE_IDENTITY_FIELDS = {"directory", "import_path", "module_path", "package_name"}
EDGE_IDENTITY_FIELDS = {
    "from_node_id", "import_path", "relation", "role", "source_paths", "to_node_id",
}

CLOSURE_GAPS = {
    "ambiguous_local": "ambiguous_local_dependency_present",
    "nested_module_boundary": "nested_module_boundary_dependency_present",
    "unresolved_local": "unresolved_local_dependency_present",
    "unsupported": "unsupported_import_dependency_present",
}
SYSTEM_UNKNOWN_REASONS = [
    "api_event_contract_surfaces_not_observed",
    "call_and_runtime_reachability_not_observed",
    "data_and_migration_surfaces_not_observed",
    "deployment_and_operations_surfaces_not_observed",
    "owner_adr_policy_surfaces_not_observed",
    "selected_build_semantics_not_observed",
]

GRAPH_COVERAGE_FIELDS = {
    "go_entries_excluded_by_nested_module", "go_entries_excluded_nonregular",
    "go_entries_in_selected_subtree", "regular_go_files_parsed",
    "regular_go_files_selected", "regular_go_files_with_diagnostics",
}
GRAPH_MODULE_FIELDS = {
    "directory", "go_mod_bytes", "go_mod_content_sha256", "go_mod_path",
    "module_path", "nested_modules",
}
GRAPH_NESTED_FIELDS = {"directory", "go_mod_path", "kind"}
GRAPH_SOURCE_FIELDS = {"source_revision", "source_tree_sha256"}
