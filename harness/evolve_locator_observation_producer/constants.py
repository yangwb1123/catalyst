"""Frozen constants for local Evolve locator observation production v1."""

import re

FIXTURE_PATH = (
    "docs/contracts/fixtures/"
    "local-evolve-repo-locator-observation-producer-v1.json"
)
SCHEMA_PATH = (
    "docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json"
)
FIXTURE_API = (
    "forgeos.governance.local-evolve-repo-locator-observation-production."
    "fixture/v1"
)
PRODUCTION_API = (
    "forgeos.governance.local-evolve-repo-locator-observation-production/v1"
)
PARAMETERS_API = "forgeos.evolve-capture.parameters/v1"
REPORT_API = "forgeos.evolve-capture.report/v1"
SOURCE_API = "forgeos.command-capture.source-tree/v1"
OBSERVATION_API = "forgeos.evolve-repo-locator/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
SCAN_CONTRACT = "evolve_scan_v1"
MARKER_PREFIX = "EVOLVE_SCAN_V1: "
REPORT_PROFILE = "evolve-scan-canonical-marker-v1"
SOURCE_PROFILE = "git-worktree-source-tree-v1"
PRODUCER_ID = "forgeos.local-evolve-repo-locator-observer"
PRODUCER_VERSION = "v1"
FIXTURE_SEMANTICS = (
    "PURE_CONTRACT_FIXTURE (deterministic bytes only; no live repository "
    "capture, scan judgment, completion, truth, authority, identity, "
    "persistence, or effect attestation)"
)
RESULT = (
    "CAPTURED_LOCAL_EVOLVE_LOCATOR_SET (local report/source capture only; "
    "locator set may be empty; no scan judgment, completion, truth, "
    "authority, claim, atom, persistence, or effect attestation)"
)
CHECKED = (
    "VALID_LOCAL_EVOLVE_LOCATOR_OBSERVATION_PRODUCER_FIXTURE "
    "(contract bytes only; no live repository capture or authority)"
)
PARAMETERS_DOMAIN = (
    b"forgeos.governance.local-evolve-repo-locator-parameters.v1\0"
)
SOURCE_DOMAIN = b"forgeos.governance.local-command-source-tree-profile.v1\0"
PRODUCTION_DOMAIN = (
    b"forgeos.governance.local-evolve-repo-locator-observation-production.v1\0"
)

MAX_FIXTURE_BYTES = 32 << 20
MAX_PRODUCTION_BYTES = 16 << 20
MAX_SOURCE_BYTES = 8 << 30
MAX_FILE_BYTES = 1 << 30
MAX_EVIDENCE_FILE_BYTES = 1 << 20
MAX_REPORT_PAYLOAD_BYTES = 64 << 10
MAX_REPORT_BYTES = len(MARKER_PREFIX.encode("utf-8")) + MAX_REPORT_PAYLOAD_BYTES
MAX_OBSERVATIONS = 240
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_LIST_ITEMS = 65_536
MAX_STRING_BYTES = 16 << 20
MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808
HASH_RE = re.compile(r"^[a-f0-9]{64}$")
ID_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]{0,159}$")
EVOLVE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")

DEPTHS = {"advisory", "opportunistic", "standard", "thorough"}
DIMENSIONS = (
    "code", "dependencies", "security", "performance",
    "architecture_drift", "test_coverage",
)
DIMENSION_RANK = {value: index for index, value in enumerate(DIMENSIONS)}

PRODUCTION_FIELDS = {
    "api_version", "canonicalization", "observations", "parameters_manifest",
    "report_manifest", "source_manifest",
}
PARAMETERS_FIELDS = {
    "api_version", "canonicalization", "contract", "expected_depth",
    "report_profile_id", "source_profile_id",
}
REPORT_FIELDS = {
    "api_version", "bytes", "canonical_report", "canonicalization",
    "profile_id", "sha256",
}
REPORT_VALUE_FIELDS = {"version", "depth", "dimensions", "opportunities"}
DIMENSION_FIELDS = {"name", "status", "evidence"}
OPPORTUNITY_FIELDS = {"id", "dimension", "title", "evidence", "obvious"}
EVIDENCE_FIELDS = {"path", "detail"}
WRAPPER_FIELDS = {
    "api_version", "expected", "fixture_semantics", "preimages", "production",
}
PREIMAGES_FIELDS = {"source_regular_files"}
FILE_PREIMAGE_FIELDS = {"path", "utf8"}
EXPECTED_FIELDS = {
    "canonical_observation_jsons", "canonical_parameters_manifest_json",
    "canonical_production_json", "canonical_report_manifest_json",
    "canonical_source_manifest_json", "parameters_sha256", "production_sha256",
    "report_sha256", "result", "source_tree_sha256",
}
