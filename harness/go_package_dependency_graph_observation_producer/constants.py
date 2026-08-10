"""Frozen ADR-0053 local Go dependency observation constants."""

import re

FIXTURE_PATH = (
    "docs/contracts/fixtures/"
    "local-go-package-dependency-graph-observation-producer-v1.json"
)
SCHEMA_PATH = (
    "docs/contracts/"
    "local-go-package-dependency-graph-observation-producer-v1.schema.json"
)
FIXTURE_API = (
    "forgeos.governance.local-go-package-dependency-graph-observation-"
    "production.fixture/v1"
)
PRODUCTION_API = (
    "forgeos.governance.local-go-package-dependency-graph-observation-"
    "production/v1"
)
GRAPH_API = "forgeos.go-package-dependency-graph-observation/v1"
PARAMETERS_API = "forgeos.go-package-dependency-capture.parameters/v1"
SOURCE_API = "forgeos.command-capture.source-tree/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
GRAPH_PROFILE = "selected-go-module-lexical-dependency-graph-v1"
FILE_SELECTION_PROFILE = "selected-module-all-regular-go-files-union-v1"
IMPORT_RESOLUTION_PROFILE = "selected-module-lexical-import-resolution-v1"
MODULE_PROFILE = "selected-go-mod-module-directive-v1"
PARSER_PROFILE = "go-parser-imports-only-no-partial-facts-v1"
SOURCE_PROFILE = "git-worktree-source-tree-v1"
PRODUCER_ID = "forgeos.local-go-package-dependency-graph-observer"
PRODUCER_VERSION = "v1"

RESULT = (
    "OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH (all-regular-Go-file lexical "
    "import-header/source observation only; no selected build, dependency "
    "availability, compile success, architecture judgment, impact closure, "
    "completeness, truth, authority, claim, atom, persistence, or effect "
    "attestation)"
)
FIXTURE_SEMANTICS = (
    "PURE_CONTRACT_FIXTURE (deterministic bytes only; no live repository "
    "capture or Go parsing, "
    "selected build, dependency availability, compile success, architecture "
    "judgment, impact closure, completeness, truth, authority, claim, atom, "
    "persistence, or effect attestation)"
)
CHECKED = (
    "VALID_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER_FIXTURE "
    "(contract bytes only; no live Go parsing or authority)"
)

PARAMETERS_DOMAIN = (
    b"forgeos.governance.local-go-package-dependency-graph-parameters.v1\0"
)
GRAPH_DOMAIN = (
    b"forgeos.governance.local-go-package-dependency-graph-observation.v1\0"
)
SOURCE_DOMAIN = b"forgeos.governance.local-command-source-tree-profile.v1\0"
PRODUCTION_DOMAIN = (
    b"forgeos.governance.local-go-package-dependency-graph-observation-"
    b"production.v1\0"
)

MAX_FIXTURE_BYTES = 32 << 20
MAX_PRODUCTION_BYTES = 16 << 20
MAX_GO_MOD_BYTES = 1 << 20
MAX_GO_FILE_BYTES = 4 << 20
MAX_AGGREGATE_PARSER_BYTES = 64 << 20
MAX_GO_FILES = 16_384
MAX_NESTED_MODULES = 1_024
MAX_IMPORTS_PER_FILE = 1_024
MAX_IMPORT_OCCURRENCES = 65_536
MAX_PACKAGES = 16_384
MAX_EDGES = 65_536
MAX_DIAGNOSTICS = 16_384
MAX_SOURCE_ENTRIES = 65_536
MAX_SOURCE_BYTES = 8 << 30
MAX_SOURCE_FILE_BYTES = 1 << 30
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_LIST_ITEMS = 65_536
MAX_STRING_BYTES = 16_384
MAX_TEXT_SCALARS = 4_096
MAX_RUN_ID_BYTES = 160
MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808

HASH_RE = re.compile(r"^[a-f0-9]{64}$")
RUN_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]{0,159}$")
GO_KEYWORDS = {
    "break", "case", "chan", "const", "continue", "default", "defer",
    "else", "fallthrough", "for", "func", "go", "goto", "if", "import",
    "interface", "map", "package", "range", "return", "select", "struct",
    "switch", "type", "var",
}

ROLES = ("compile", "test")
ROLE_RANK = {value: index for index, value in enumerate(ROLES)}
RESOLUTIONS = (
    "local", "stdlib_candidate", "external_candidate", "unresolved_local",
    "ambiguous_local", "nested_module_boundary", "cgo_pseudo", "unsupported",
)
RESOLUTION_DETAILS = {
    "local": {None},
    "stdlib_candidate": {None},
    "external_candidate": {None},
    "unresolved_local": {"no_compile_package"},
    "ambiguous_local": {"multiple_compile_packages"},
    "nested_module_boundary": {"nested_module_boundary"},
    "cgo_pseudo": {None},
    "unsupported": {"noncanonical_import_path"},
}
DIAGNOSTIC_CODES = {
    "go_file_exceeds_parser_limit", "go_file_import_limit_exceeded",
    "go_file_invalid_utf8", "go_file_parse_error", "go_file_unsupported_text",
}

PRODUCTION_FIELDS = {
    "api_version", "canonicalization", "graph_observation",
    "parameters_manifest", "source_manifest",
}
PARAMETERS_FIELDS = {
    "api_version", "canonicalization", "file_selection_profile_id",
    "import_resolution_profile_id", "module_directory", "module_profile_id",
    "parser_profile_id", "source_profile_id",
}
GRAPH_FIELDS = {
    "api_version", "canonicalization", "coverage", "dependencies",
    "diagnostics", "files", "module", "observed_at_unix_ms", "packages",
    "producer", "profile_id", "source",
}
COVERAGE_FIELDS = {
    "go_entries_excluded_by_nested_module", "go_entries_excluded_nonregular",
    "go_entries_in_selected_subtree", "regular_go_files_parsed",
    "regular_go_files_selected", "regular_go_files_with_diagnostics",
}
MODULE_FIELDS = {
    "directory", "go_mod_bytes", "go_mod_content_sha256", "go_mod_path",
    "module_path", "nested_modules",
}
NESTED_MODULE_FIELDS = {"directory", "go_mod_path", "kind"}
FILE_FIELDS = {
    "bytes", "content_sha256", "imports", "package_name", "path", "role",
}
PACKAGE_FIELDS = {
    "compile_files", "directory", "import_path", "name", "test_files",
}
EDGE_FIELDS = {
    "from_directory", "from_package_name", "import_path", "relation",
    "resolution", "resolution_detail", "role", "source_paths",
    "target_directory", "target_package_name",
}
DIAGNOSTIC_FIELDS = {"code", "path"}
PRODUCER_FIELDS = {
    "parameters_sha256", "producer_id", "producer_type", "producer_version",
    "run_id",
}
SOURCE_BINDING_FIELDS = {"source_revision", "source_tree_sha256"}
WRAPPER_FIELDS = {
    "api_version", "expected", "fixture_semantics", "preimages", "production",
}
PREIMAGES_FIELDS = {"source_regular_files"}
PREIMAGE_FILE_FIELDS = {"path", "utf8"}
EXPECTED_FIELDS = {
    "canonical_graph_observation_json", "canonical_parameters_manifest_json",
    "canonical_production_json", "canonical_source_manifest_json",
    "graph_sha256", "parameters_sha256", "production_sha256", "result",
    "source_tree_sha256",
}
