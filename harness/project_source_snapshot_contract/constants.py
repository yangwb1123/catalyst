"""Frozen values and resource limits for Project Source Snapshot v1."""

CANONICAL = "forgeos.canonical-json/v1"
API_REQUEST = "forgeos.governance.local-project-source-snapshot-request/v1"
API_ENVELOPE = "forgeos.governance.local-project-source-snapshot-production/v1"
API_SNAPSHOT = "forgeos.project-source-snapshot/v1"
API_MANIFEST = "forgeos.project-source-manifest/v1"
API_COVERAGE = "forgeos.project-source-coverage/v1"
PROFILE_ID = "local-git-worktree-bounded-sensitive-path-exclusion-v1"
PATH_POLICY_ID = "forgeos.project-source-sensitive-path-policy/v1"
EXTRACTOR_ID = "local-git-worktree-project-source-snapshot"
EXTRACTOR_VERSION = "1"
ENVELOPE_KIND = "LocalProjectSourceSnapshotProduction"
SNAPSHOT_KIND = "ProjectSourceSnapshot"
CONSISTENCY = "bounded_interval_two_endpoint_exact_match"
POSITIVE_RESULT = (
    "CAPTURED_BOUNDED_LOCAL_PROJECT_SOURCE_OBSERVATION (the collector worktree-leaf "
    "reader did not open paths matched by its fixed path policy; Git and "
    "repository-metadata access are outside that guarantee; this is not an atomic, "
    "current, complete, secret-free, authenticated, authorized, persistent, or "
    "effect-attesting project or graph snapshot)"
)

GIT_IDENTITY = "unauthenticated_local_path_binary"
GIT_CONFIG = "best_effort_hardened_environment_and_flags_repository_config_unauthenticated"
GIT_NETWORK = "not_provided"

MAX_ENVELOPE_BYTES = 33_554_432
MAX_MANIFEST_BYTES = 16_777_216
MAX_FILE_BYTES = 67_108_864
MAX_GIT_BYTES = 67_108_864
MAX_TOTAL_BYTES = 1_073_741_824
MAX_UNIVERSE = 16_384
MAX_EXCLUDED = 4_096
MAX_IGNORED = 262_144
MAX_PATH_BYTES = 16_384
MAX_PATH_SCALARS = 4_096
MAX_PATH_COMPONENTS = 256
MAX_SHORT_TEXT_BYTES = 1_024
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY_ITEMS = 262_144
MIN_I64 = -(1 << 63)
MAX_I64 = (1 << 63) - 1

FIXTURE_PATH = "docs/contracts/fixtures/project-source-snapshot-v1.json"
FIXTURE_SHA256 = "4b23a9c5896a7b279fb4f7a17a4939791f94489c5311354c8b008fdfe665de89"

DOMAINS = {
    "path": "forgeos.project-source-snapshot.path.v1",
    "request": "forgeos.project-source-snapshot.request.v1",
    "entry": "forgeos.project-source-snapshot.entry-record.v1",
    "exclusion": "forgeos.project-source-snapshot.exclusion-record.v1",
    "entry_set": "forgeos.project-source-snapshot.entry-set.v1",
    "exclusion_set": "forgeos.project-source-snapshot.exclusion-set.v1",
    "manifest": "forgeos.project-source-snapshot.source-manifest.v1",
    "coverage": "forgeos.project-source-snapshot.coverage.v1",
    "snapshot_identity": "forgeos.project-source-snapshot.snapshot-identity.v1",
    "snapshot": "forgeos.project-source-snapshot.snapshot-record.v1",
    "envelope": "forgeos.project-source-snapshot.envelope.v1",
}

CONTROL_COMPONENTS = frozenset({".git", ".forge"})
SENSITIVE_COMPONENTS = frozenset({".ssh", ".aws", ".azure", ".gnupg", "secrets"})
SENSITIVE_BASENAMES = frozenset({
    ".env", ".netrc", ".npmrc", ".pypirc", ".dockercfg", "kubeconfig",
    "credentials", "credentials.json", "service-account.json", "id_rsa",
    "id_dsa", "id_ecdsa", "id_ed25519",
})
SENSITIVE_PREFIXES = (".env.",)
SENSITIVE_SUFFIXES = (".pem", ".key", ".p12", ".pfx", ".jks", ".keystore")

COVERAGE_SPECS = (
    ("atomicity", "unknown", 0,
     ("bounded_interval_observation", "writer_quiescence_not_provided")),
    ("configuration_semantics", "not_performed", 0,
     ("configuration_classifier_not_run",)),
    ("content_secret_scan", "not_performed", 0,
     ("allowed_content_secret_absence_not_attested", "path_policy_only")),
    ("currentness", "unknown", 0,
     ("clock_not_authenticated", "current_head_not_attested")),
    ("deployment_semantics", "not_performed", 0,
     ("deployment_classifier_not_run",)),
    ("freshness", "unknown", 0, ("freshness_not_assessed",)),
    ("git_control_metadata", "not_observed", 0,
     ("git_binary_and_local_config_unauthenticated", "git_metadata_not_projected")),
    ("graph_topology", "not_performed", 0, ("graph_extractor_not_run",)),
    ("ignored_paths", "partial", "ignored_path_count",
     ("content_and_locators_not_observed", "count_only")),
    ("nested_repositories_and_submodules", "not_observed", 0,
     ("gitlinks_rejected", "nested_repository_semantics_not_inspected")),
    ("nonignored_untracked", "partial", "untracked_count",
     ("bounded_interval_not_atomic", "git_exclude_standard_applied",
      "ignored_paths_not_enumerated_as_source")),
    ("tracked_worktree", "partial", "tracked_count",
     ("bounded_interval_not_atomic", "git_stage_zero_only",
      "head_is_revision_hint_only", "nonordinary_index_flags_rejected",
      "worktree_bytes_not_index_blob")),
)
