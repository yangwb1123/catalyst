"""Frozen identifiers and resource bounds for the ADR-0079 candidate core."""

from pathlib import Path

CANONICALIZATION = "forgeos.canonical-json/v1"
PROFILE_ID = "authenticated_architecture_decision_approval_v1"
SIGNATURE_PROFILE_ID = "forgeos.ed25519-domain-sha256/v1"

SIGNATURE_PROFILE_API = "forgeos.ed25519-domain-sha256-profile/v1"
TRUST_ROOT_API = "forgeos.architecture-decision-approval-trust-root/v1"
PROPOSAL_BINDING_API = "forgeos.architecture-decision-proposal-binding/v1"
POLICY_API = "forgeos.architecture-decision-approval-policy/v1"
REVOCATION_API = "forgeos.architecture-decision-approval-revocation-snapshot/v1"
REQUEST_API = "forgeos.architecture-decision-approval-authorization-request/v1"
RECEIPT_API = "forgeos.architecture-decision-approval-authorization-receipt/v1"
RESULT_API = "forgeos.architecture-decision-approval-authorization-result/v1"
LEDGER_API = "forgeos.architecture-decision-approval-authorization-ledger/v1"

SIGNATURE_PROFILE_DOMAIN = b"forgeos.ed25519-domain-sha256-profile.v1\0"
TRUST_ROOT_DOMAIN = b"forgeos.authenticated-architecture-decision-approval.trust-root.v1\0"
PROPOSAL_BINDING_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.proposal-binding.v1\0"
)
POLICY_DOMAIN = b"forgeos.authenticated-architecture-decision-approval.policy.v1\0"
REVOCATION_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.revocation-snapshot.v1\0"
)
REQUEST_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-request.v1\0"
)
RECEIPT_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-receipt.v1\0"
)
LEDGER_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-ledger.v1\0"
)
RECORD_KEY_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.idempotency-record-key.v1\0"
)

POLICY_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.policy.signature.v1\0"
)
REVOCATION_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.revocation-snapshot.signature.v1\0"
)
REQUEST_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-request.signature.v1\0"
)
RECEIPT_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-receipt.signature.v1\0"
)
LEDGER_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.authorization-ledger.signature.v1\0"
)
APPROVAL_RECORD_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.approval-record.signature.v1\0"
)
APPROVAL_RECORD_SOD_SIGNATURE_DOMAIN = (
    b"forgeos.authenticated-architecture-decision-approval.approval-record-sod.signature.v1\0"
)

KEY_USAGES = (
    "approval_authorization_state_sign",
    "approval_policy_sign",
    "approval_request_auth",
    "approval_revocation_sign",
    "architecture_approval_sign",
)
REQUIRED_DISTINCTIONS = (
    "approver_not_implementer",
    "approver_not_owner",
    "approver_not_requester",
    "approver_not_subject",
    "approvers_pairwise_distinct",
)
APPROVAL_RECORD_DISTINCTIONS = (
    "approver_not_implementer",
    "approver_not_requester",
    "approver_not_subject",
)
GATE_ID = "architecture-decision-acceptance-prerequisite-v1"

MAX_PROFILE_BYTES = 16 * 1024
MAX_ROOT_BYTES = 256 * 1024
MAX_PROPOSAL_BINDING_BYTES = 64 * 1024
MAX_POLICY_BYTES = 1024 * 1024
MAX_REVOCATION_BYTES = 512 * 1024
MAX_REQUEST_BYTES = 4 * 1024 * 1024
MAX_RECEIPT_BYTES = 256 * 1024
MAX_RESULT_BYTES = 512 * 1024
MAX_LEDGER_BYTES = 64 * 1024 * 1024
MAX_GOLDEN_BYTES = 64 * 1024 * 1024
MAX_PROPOSAL_BYTES = 256 * 1024
MAX_POLICY_VALIDITY_MS = 86_400_000
MAX_REQUEST_VALIDITY_MS = 300_000
MAX_APPROVALS = 16
MAX_LEDGER_ENTRIES = 64
MAX_REVOCATION_SNAPSHOTS = 256

FIXTURE_PATH = Path(
    "docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json"
)
APPROVAL_RECORD_SCHEMA_PATH = Path("docs/contracts/approval-record-v1.schema.json")
ADR_V2_SCHEMA_PATH = Path("docs/contracts/architecture-decision-record-v2.schema.json")
SCHEMA_PATH = Path(
    "docs/contracts/authenticated-architecture-decision-approval-v1.schema.json"
)
PROPOSAL_FIXTURE_PATH = Path(
    "docs/contracts/fixtures/ADR-9002-authenticated-approval-target.md"
)
GOLDEN_SHA256 = "936b989856ff733e2de848ba9907c10f9f626aa188648fc60372775e44dbc7b5"
SCHEMA_SHA256 = "9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198"
APPROVAL_RECORD_SCHEMA_SHA256 = (
    "bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64"
)
ADR_V2_SCHEMA_SHA256 = "ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b"
PROPOSAL_PHYSICAL_SHA256 = "6beabf33656998b942036b63c90db99c6a5f9b138cf2e5bd4a5372ec8e1ad1f2"
PROPOSAL_BODY_SHA256 = "9a798ab129919d51d8b2a3c842d281b19c1d1667a180ef8cbd4f690465730a63"
PROPOSAL_SELF_SHA256 = "1d2579dafcf152c302e22cfa4d6932e248f1171eef812297f601b60ce7ed208f"

SUCCESS_MARKER = (
    "STRUCTURALLY_VALID_AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE "
    "(declared structure/digests/relations only; no authentication, authorization, "
    "acceptance, persistence, effect, root-pin, time-currentness, "
    "revocation-currentness, CAS, or durability attestation)"
)
