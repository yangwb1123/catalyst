"""Frozen constants for ADR-0057."""

CANONICALIZATION = "forgeos.canonical-json/v1"
CONTRACT_PROFILE_ID = "bootstrap_planning_repo_read_only_v1"
RUNTIME_PROFILE = "authenticated_bootstrap_repo_read_grant_issuance_v1"
SIGNATURE_PROFILE_ID = "forgeos.ed25519-domain-sha256/v1"

PROFILE_API = "forgeos.ed25519-domain-sha256-profile/v1"
ROOT_API = "forgeos.governance-trust-root/v1"
POLICY_API = "forgeos.bootstrap-grant-policy/v1"
REQUEST_API = "forgeos.bootstrap-grant-request/v1"
RECEIPT_API = "forgeos.grant-issuance-receipt/v1"
LEDGER_API = "forgeos.grant-issuance-ledger/v1"
RESULT_API = "forgeos.bootstrap-grant-issuance-result/v1"

PROFILE_DOMAIN = b"forgeos.ed25519-domain-sha256-profile.v1\0"
ROOT_DOMAIN = b"forgeos.governance-trust-root.v1\0"
POLICY_DOMAIN = b"forgeos.bootstrap-grant-policy.v1\0"
REQUEST_DOMAIN = b"forgeos.bootstrap-grant-request.v1\0"
GRANT_ENVELOPE_DOMAIN = b"forgeos.capability-grant.envelope.v1\0"
RECEIPT_DOMAIN = b"forgeos.grant-issuance-receipt.v1\0"
LEDGER_DOMAIN = b"forgeos.grant-issuance-ledger.v1\0"
RECORD_KEY_DOMAIN = b"forgeos.grant-issuance-record-key.v1\0"

POLICY_SIGNATURE_DOMAIN = b"forgeos.bootstrap-grant-policy.signature.v1\0"
REQUEST_SIGNATURE_DOMAIN = b"forgeos.bootstrap-grant-request.signature.v1\0"
GRANT_SIGNATURE_DOMAIN = b"forgeos.capability-grant.signature.v1\0"
RECEIPT_SIGNATURE_DOMAIN = b"forgeos.grant-issuance-receipt.signature.v1\0"
LEDGER_SIGNATURE_DOMAIN = b"forgeos.grant-issuance-ledger.signature.v1\0"

MAX_PROFILE_BYTES = 16 * 1024
MAX_ROOT_BYTES = 256 * 1024
MAX_POLICY_BYTES = 512 * 1024
MAX_REQUEST_BYTES = 1024 * 1024
MAX_GRANT_BYTES = 1024 * 1024
MAX_RECEIPT_BYTES = 256 * 1024
MAX_RESULT_BYTES = 2 * 1024 * 1024
MAX_LEDGER_BYTES = 16 * 1024 * 1024
MAX_GOLDEN_BYTES = 20 * 1024 * 1024
MAX_TTL_MS = 3_600_000
MAX_POLICY_VALIDITY_MS = 86_400_000
MAX_REQUEST_VALIDITY_MS = 300_000
MAX_TIMEOUT_MS = 300_000
MAX_OUTPUT_BYTES = 1_048_576
MAX_LEDGER_ENTRIES = 256

KEY_USAGES = ("grant_issue", "policy_sign", "request_auth")
RESULTS = ("exact_replay", "stored")
FIXTURE = "docs/contracts/fixtures/bootstrap-grant-issuance-v1.json"
GOLDEN_FILE_SHA256 = "60a234a15080f7c08367ea53f7a3cbfee6722c8ac015bbb09132bdcbdb31b011"
