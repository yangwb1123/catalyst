"""Frozen identifiers and bounds for the ADR-0082 structural prerequisite."""

from pathlib import Path

CANONICALIZATION = "forgeos.canonical-json/v1"
PROFILE_ID = "authenticated_architecture_decision_lifecycle_v1"
SIGNATURE_PROFILE_ID = "forgeos.ed25519-domain-sha256/v1"

BUNDLE_API = "forgeos.authenticated-architecture-decision-lifecycle-bundle/v1"
TRUST_ROOT_API = "forgeos.architecture-decision-lifecycle-trust-root/v1"
REQUEST_API = "forgeos.architecture-decision-lifecycle-transition-request/v1"
ACCEPTANCE_API = "forgeos.architecture-decision-lifecycle-acceptance-receipt/v1"
SUPERSESSION_API = "forgeos.architecture-decision-lifecycle-supersession-receipt/v1"
ENTRY_API = "forgeos.architecture-decision-lifecycle-ledger-entry/v1"
LEDGER_API = "forgeos.architecture-decision-lifecycle-ledger/v1"
VIEW_API = "forgeos.architecture-decision-lifecycle-materialized-view/v1"
STATE_API = "forgeos.architecture-decision-lifecycle-state/v1"
RESULT_API = "forgeos.architecture-decision-lifecycle-transition-result/v1"

TRUST_ROOT_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.trust-root.v1\0"
PREREQUISITE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.acceptance-prerequisite.v1\0"
REQUEST_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.request.v1\0"
ACCEPTANCE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.v1\0"
SUPERSESSION_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.v1\0"
ENTRY_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.entry.v1\0"
LEDGER_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.ledger.v1\0"
HEAD_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.head-set.v1\0"
VIEW_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.materialized-view.v1\0"
STATE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.state.v1\0"
RECORD_KEY_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.idempotency-record-key.v1\0"

REQUEST_SIGNATURE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.request.signature.v1\0"
ACCEPTANCE_SIGNATURE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.signature.v1\0"
SUPERSESSION_SIGNATURE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.signature.v1\0"
STATE_SIGNATURE_DOMAIN = b"forgeos.authenticated-architecture-decision-lifecycle.state.signature.v1\0"

REQUEST_KEY_USAGE = "architecture_decision_lifecycle_request_auth"
STATE_KEY_USAGE = "architecture_decision_lifecycle_state_sign"
KEY_USAGES = (REQUEST_KEY_USAGE, STATE_KEY_USAGE)

MAX_PROFILE_BYTES = 16 * 1024
MAX_ROOT_BYTES = 64 * 1024
MAX_PROPOSAL_BYTES = 256 * 1024
MAX_REQUEST_BYTES = 1024 * 1024
MAX_ACCEPTANCE_BYTES = 256 * 1024
MAX_SUPERSESSION_BYTES = 256 * 1024
MAX_ENTRY_BYTES = 4 * 1024 * 1024
MAX_LEDGER_BYTES = 64 * 1024 * 1024
MAX_VIEW_BYTES = 8 * 1024 * 1024
MAX_STATE_BYTES = 80 * 1024 * 1024
MAX_RESULT_BYTES = 512 * 1024
MAX_GOLDEN_BYTES = 96 * 1024 * 1024
MAX_REQUEST_VALIDITY_MS = 300_000
MAX_ENTRIES = 256
MAX_DECISIONS = 256
MAX_SUPERSESSIONS = 64
MAX_APPROVAL_LEDGER_ENTRIES = 64
MAX_APPROVAL_REVOCATION_SNAPSHOTS = 256

SCHEMA_PATH = Path(
    "docs/contracts/authenticated-architecture-decision-lifecycle-v1.schema.json"
)
FIXTURE_PATH = Path(
    "docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json"
)
APPROVAL_SCHEMA_PATH = Path(
    "docs/contracts/authenticated-architecture-decision-approval-v1.schema.json"
)
ADR_V2_SCHEMA_PATH = Path("docs/contracts/architecture-decision-record-v2.schema.json")
PROPOSAL_FIXTURE_PATHS = (
    Path("docs/contracts/fixtures/ADR-9003-lifecycle-head-a.md"),
    Path("docs/contracts/fixtures/ADR-9004-lifecycle-head-b.md"),
    Path("docs/contracts/fixtures/ADR-9005-lifecycle-join.md"),
)

# Filled only after all owned bytes are stable.
SCHEMA_SHA256 = "17f0f3f79680fd5d7825f574cc20f279f1fc9061ab33a73ef2e86e075d59bcf1"
GOLDEN_SHA256 = "47f8ceb9c4362f37fe5c48e17342a9ec3bedbb9ccfb87b585cabd3aa7c71dccb"
APPROVAL_SCHEMA_SHA256 = "9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198"
ADR_V2_SCHEMA_SHA256 = "ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b"
PROPOSAL_PHYSICAL_SHA256 = (
    "6c9cd0e4b95c968bb280d51b72d74f08a79620077d72611b5634a77b181a0a0b",
    "a76e566c7e18801dbc70c42b8b04ce9190cbc3d18892c80cd40c2ff4ec448bf0",
    "c96d2ef2db3311c16572ed5d753c2435193ab58a1abb7c1fc8a9c45d4d9c5dee",
)
PROPOSAL_BODY_SHA256 = (
    "57f677e4ee15042d34a7a612fdb80d9dc89e7229b0a30ff08fe9d7f0696ec011",
    "327691dbc26ab3d46b4a5df95ce7f52051948241fa3a1b3a6a5c57a81b0d6ee2",
    "bf84ae13e8d90f69f5092e7b8c7348152b122d7428a0132b0ce982a0f5a2398a",
)
PROPOSAL_SELF_SHA256 = (
    "6f5ffda702b67563f3977d89a399c458ad3502b91c1c7ec2dd8c2b50a17a3604",
    "b48ed2d6adc5aa601c0177f628a25881053a12b708efb45e806d83e50bc7b13d",
    "1a8c1afd4ba511afd41477248be4998f939bbee741b4579159beeb2b2f383281",
)

SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_AUTHENTICATED_ADR_LIFECYCLE_V1_CANDIDATE "
    "(declared exact bytes/digests/relations only; no Ed25519, external-root, "
    "trusted-time, revocation-currentness, authorization, repository-mutation, "
    "Accepted-source, atomicity, persistence, CAS, durability, rollback-resistance, "
    "architecture-compliance, permission, or effect attestation)"
)
