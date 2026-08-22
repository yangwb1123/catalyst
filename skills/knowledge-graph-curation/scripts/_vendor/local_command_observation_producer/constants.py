"""Frozen local command observation production fixture constants."""

import re

FIXTURE_PATH = "docs/contracts/fixtures/local-gate-command-observation-producer-v1.json"
FIXTURE_API = "forgeos.governance.local-gate-command-observation-production.fixture/v1"
PRODUCTION_API = "forgeos.governance.local-gate-command-observation-production/v1"
ENVIRONMENT_API = "forgeos.command-capture.environment/v1"
TOOL_API = "forgeos.command-capture.tool/v1"
SOURCE_API = "forgeos.command-capture.source-tree/v1"
CANONICALIZATION = "forgeos.canonical-json/v1"
PRODUCER_ID = "forgeos.local-gate-command-observer"
PRODUCER_VERSION = "v1"
ENVIRONMENT_PROFILE = "scrubbed-parent-environment-v1"
TOOL_PROFILE = "resolved-top-level-executable-v1"
SOURCE_PROFILE = "git-worktree-source-tree-v1"
FIXTURE_SEMANTICS = (
    "PURE_CONTRACT_FIXTURE (deterministic bytes only; no live process execution, "
    "pass, criterion, completion, truth, authority, identity, persistence, or "
    "external-effect attestation)"
)
RESULT = (
    "OBSERVED_LOCAL_PROCESS (local process capture only; no pass, criterion, "
    "completion, truth, authority, identity, persistence, or external-effect attestation)"
)
CHECKED = (
    "VALID_LOCAL_COMMAND_OBSERVATION_PRODUCER_FIXTURE (contract bytes only; "
    "no live process execution or authority)"
)
ENVIRONMENT_DOMAIN = b"forgeos.governance.local-command-environment-profile.v1\0"
TOOL_DOMAIN = b"forgeos.governance.local-command-tool-profile.v1\0"
SOURCE_DOMAIN = b"forgeos.governance.local-command-source-tree-profile.v1\0"
PRODUCTION_DOMAIN = b"forgeos.governance.local-command-observation-production.v1\0"
MAX_FIXTURE_BYTES = 32 << 20
MAX_MANIFEST_BYTES = 16 << 20
MAX_SOURCE_BYTES = 8 << 30
MAX_FILE_BYTES = 1 << 30
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_LIST_ITEMS = 65_536
MAX_TEXT_BYTES = 16_384
MAX_TEXT_SCALARS = 4_096
MAX_I64 = 9_223_372_036_854_775_807
MIN_I64 = -9_223_372_036_854_775_808
HASH_RE = re.compile(r"^[a-f0-9]{64}$")
ENV_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
REVISION_RE = re.compile(r"^git-(?:sha1:[a-f0-9]{40}|sha256:[a-f0-9]{64})$")
COMMANDS = {
    ("node", "harness/gate.mjs"),
    ("python3", "harness/check.py", "."),
    ("node", "harness/acceptance.mjs"),
    ("node", "harness/acceptance.mjs", "--json"),
}
SECRET_FRAGMENTS = {
    "API_KEY", "AUTH", "BEARER", "COOKIE", "CREDENTIAL", "CREDENTIALS",
    "OAUTH", "PASSWD", "PASSWORD", "PRIVATE_KEY", "SECRET", "SESSION", "TOKEN",
}
SECRET_PREFIXES = {
    "ANTHROPIC_", "AWS_", "AZURE_", "CLOUDFLARE_", "DIGITALOCEAN_",
    "DOCKER_AUTH", "GCP_", "GCLOUD_", "GITHUB_", "GITLAB_", "GOOGLE_",
    "KUBE", "OCI_", "OPENAI_", "SSH_", "GPG_", "VAULT_",
}

PRODUCTION_FIELDS = {
    "api_version", "canonicalization", "environment_manifest", "observation",
    "source_manifest", "tool_manifest",
}
ENVIRONMENT_FIELDS = {"api_version", "canonicalization", "profile_id", "variables"}
VARIABLE_FIELDS = {"name", "value"}
TOOL_FIELDS = {
    "api_version", "bytes", "canonicalization", "final_path", "mode", "profile_id",
    "requested_path", "resolved_path", "sha256", "symlink_hops",
}
HOP_FIELDS = {"path", "target"}
SOURCE_FIELDS = {"api_version", "canonicalization", "entries", "profile_id", "source_revision"}
ENTRY_FIELDS = {
    "bytes", "content_sha256", "executable", "index_mode", "kind", "path",
    "symlink_target", "tracking",
}
WRAPPER_FIELDS = {"api_version", "expected", "fixture_semantics", "preimages", "production"}
PREIMAGES_FIELDS = {"source_regular_files", "tool"}
FILE_PREIMAGE_FIELDS = {"path", "utf8"}
TOOL_PREIMAGE_FIELDS = {"final_path", "utf8"}
EXPECTED_FIELDS = {
    "canonical_environment_manifest_json", "canonical_observation_json",
    "canonical_production_json", "canonical_source_manifest_json",
    "canonical_tool_manifest_json", "environment_sha256", "production_sha256",
    "result", "source_tree_sha256", "tool_snapshot_sha256",
}
