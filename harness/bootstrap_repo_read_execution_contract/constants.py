"""Frozen constants for the ADR-0058 structural execution contract."""

CANONICALIZATION = "forgeos.canonical-json/v1"
PROFILE_ID = "authenticated_bootstrap_repo_read_execution_v1"
SIGNATURE_PROFILE_ID = "forgeos.ed25519-domain-sha256/v1"

ROOT_API = "forgeos.bootstrap-repo-read-execution-trust-root/v1"
MANIFEST_API = "forgeos.repo-read-expected-manifest/v1"
POLICY_API = "forgeos.bootstrap-repo-read-execution-policy/v1"
INVOCATION_API = "forgeos.bootstrap-repo-read-invocation/v1"
RECEIPT_API = "forgeos.bootstrap-repo-read-usage-receipt/v1"
RESULT_API = "forgeos.bootstrap-repo-read-execution-result/v1"
METADATA_API = "forgeos.bootstrap-repo-read-result-metadata/v1"
DELIVERY_API = "forgeos.bootstrap-repo-read-execution-delivery/v1"
LEDGER_API = "forgeos.bootstrap-repo-read-usage-ledger/v1"

ROOT_DOMAIN = b"forgeos.bootstrap-repo-read-execution-trust-root.v1\0"
MANIFEST_DOMAIN = b"forgeos.repo-read-expected-manifest.v1\0"
POLICY_DOMAIN = b"forgeos.bootstrap-repo-read-execution-policy.v1\0"
INVOCATION_DOMAIN = b"forgeos.bootstrap-repo-read-invocation.v1\0"
RECEIPT_DOMAIN = b"forgeos.bootstrap-repo-read-usage-receipt.v1\0"
RESULT_DOMAIN = b"forgeos.bootstrap-repo-read-execution-result.v1\0"
METADATA_DOMAIN = b"forgeos.bootstrap-repo-read-result-metadata.v1\0"
LEDGER_DOMAIN = b"forgeos.bootstrap-repo-read-usage-ledger.v1\0"
RECORD_KEY_DOMAIN = b"forgeos.bootstrap-repo-read-idempotency-record-key.v1\0"

POLICY_SIGNATURE_DOMAIN = b"forgeos.bootstrap-repo-read-execution-policy.signature.v1\0"
INVOCATION_SIGNATURE_DOMAIN = b"forgeos.bootstrap-repo-read-invocation.signature.v1\0"
RECEIPT_SIGNATURE_DOMAIN = b"forgeos.bootstrap-repo-read-usage-receipt.signature.v1\0"
LEDGER_SIGNATURE_DOMAIN = b"forgeos.bootstrap-repo-read-usage-ledger.signature.v1\0"

MAX_ROOT_BYTES = 256 * 1024
MAX_MANIFEST_BYTES = 256 * 1024
MAX_POLICY_BYTES = 512 * 1024
MAX_INVOCATION_BYTES = 512 * 1024
MAX_RECEIPT_BYTES = 256 * 1024
MAX_RESULT_BYTES = 2 * 1024 * 1024
MAX_METADATA_BYTES = 256 * 1024
MAX_DELIVERY_BYTES = 3 * 1024 * 1024
MAX_LEDGER_BYTES = 16 * 1024 * 1024
MAX_GOLDEN_BYTES = 40 * 1024 * 1024
MAX_OUTPUT_BYTES = 1_048_576
MAX_TIMEOUT_MS = 300_000
MAX_FRESHNESS_MS = 300_000
MAX_ENTRIES = 256
MAX_PATHS = 16
RESERVATION_ENTRY_OVERHEAD_BYTES = 4096
ORPHAN_ENTRY_OVERHEAD_BYTES = 1024

KEY_USAGES = (
    "execution_policy_sign",
    "execution_receipt_sign",
    "execution_request_auth",
)
STATES = (
    "completed",
    "effect_intent",
    "failed_consumed",
    "quarantined",
    "reserved_no_repo_io",
)
FAILED_REASONS = (
    "content_mismatch",
    "cooperative_timeout_exceeded",
    "repository_identity_changed",
    "repository_read_failed",
)
QUARANTINE_REASONS = (
    "effect_outcome_uncertain",
    "orphaned_effect_intent",
    "orphaned_reserved_no_repo_io",
)

FIXTURE = "docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json"
GOLDEN_FILE_SHA256 = "309b3da66c64669239ce40bd086cdcbb518d59dc7fd5e1bad60d6acf9107480d"
