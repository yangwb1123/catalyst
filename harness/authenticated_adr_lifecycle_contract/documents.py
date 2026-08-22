"""Lifecycle transition request, receipts, entry, and delivery envelope."""

from __future__ import annotations

import hashlib
from typing import Any

from .authority import lifecycle_key
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (
    ACCEPTANCE_API, ACCEPTANCE_DOMAIN, CANONICALIZATION, ENTRY_API, ENTRY_DOMAIN,
    MAX_ACCEPTANCE_BYTES, MAX_ENTRY_BYTES, MAX_REQUEST_BYTES,
    MAX_REQUEST_VALIDITY_MS, MAX_RESULT_BYTES, MAX_SUPERSESSIONS,
    MAX_SUPERSESSION_BYTES, PROFILE_ID, RECORD_KEY_DOMAIN, REQUEST_API,
    REQUEST_DOMAIN, REQUEST_KEY_USAGE, RESULT_API, STATE_KEY_USAGE,
    SUPERSESSION_API, SUPERSESSION_DOMAIN,
)
from .prerequisite import validate_prerequisite
from .proposal import decode_proposal_document
from .shape import (IDEMPOTENCY_RE, adr_id, integer, require_keys, sha256,
                    sorted_unique, text, validate_signature)

REQUEST_FIELDS = {
    "acceptance_prerequisite", "api_version", "canonicalization",
    "expected_current_head_set_sha256", "expected_ledger_sha256",
    "expected_next_sequence", "expires_at_unix_ms", "idempotency_key", "kind",
    "profile_id", "proposal_document_base64url", "request_id", "request_sha256",
    "requested_at_unix_ms", "signature", "supersession_targets", "trust_epoch",
    "trust_root_sha256",
}
TARGET_FIELDS = {
    "acceptance_id", "acceptance_sha256", "adr_id", "proposal_binding_sha256",
}
ACCEPTANCE_FIELDS = {
    "acceptance_id", "acceptance_sha256", "accepted_at_unix_ms", "adr_id",
    "api_version", "authorization_receipt_physical_sha256",
    "authorization_receipt_sha256", "canonicalization", "kind", "ledger_sequence",
    "profile_id", "proposal_binding_sha256", "record_key_sha256", "request_sha256",
    "signature", "supersedes", "trust_epoch", "trust_root_sha256",
}
SUPERSESSION_FIELDS = {
    "api_version", "canonicalization", "kind", "ledger_sequence", "profile_id",
    "receipt_id", "receipt_sha256", "request_sha256", "signature",
    "superseded_at_unix_ms", "superseded_by_acceptance_id", "superseded_by_adr_id",
    "superseded_by_proposal_binding_sha256", "target_acceptance_id", "target_adr_id",
    "target_proposal_binding_sha256", "trust_epoch", "trust_root_sha256",
}
ENTRY_FIELDS = {
    "acceptance_receipt", "api_version", "canonicalization", "entry_sha256", "kind",
    "prior_entry_sha256", "profile_id", "request",
    "resulting_current_head_set_sha256", "sequence", "supersession_receipts",
}
RESULT_FIELDS = {
    "api_version", "canonicalization", "delivery_disposition", "entry_sha256",
    "kind", "ledger_sha256", "materialized_view_sha256", "receipt", "state_sha256",
}


def request_sha256(value: dict[str, Any]) -> str:
    return self_digest(REQUEST_DOMAIN, value, ("request_id", "request_sha256"),
                       MAX_REQUEST_BYTES,
                       "ArchitectureDecisionLifecycleTransitionRequest", signed=True)


def record_key_sha256(idempotency_key: str) -> str:
    text(idempotency_key, "idempotency_key", IDEMPOTENCY_RE)
    return hashlib.sha256(RECORD_KEY_DOMAIN + idempotency_key.encode("ascii")).hexdigest()


def validate_request(value: Any, profile_hash: str, lifecycle_root: dict[str, Any],
                     approval_root: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    label = "ArchitectureDecisionLifecycleTransitionRequest"
    node = require_keys(value, label, REQUEST_FIELDS)
    bounded_canonical_json(node, MAX_REQUEST_BYTES, label)
    expected = (REQUEST_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    prerequisite = validate_prerequisite(node["acceptance_prerequisite"],
                                         profile_hash, approval_root)
    _, metadata = decode_proposal_document(
        node["proposal_document_base64url"], prerequisite["proposal_binding"],
        "request.proposal_document_base64url")
    _validate_request_scalars(node, prerequisite, metadata)
    _validate_request_authority(node, profile_hash, lifecycle_root)
    _validate_request_identity(node)
    return node, metadata


def _validate_request_scalars(node: dict[str, Any], prerequisite: dict[str, Any],
                              metadata: dict[str, Any]) -> None:
    requested = integer(node["requested_at_unix_ms"], "request.requested_at_unix_ms")
    expires = integer(node["expires_at_unix_ms"], "request.expires_at_unix_ms")
    authorization = prerequisite["authorization_receipt"]
    if (requested != prerequisite["observed_at_unix_ms"] or
            not requested < expires <= requested + MAX_REQUEST_VALIDITY_MS or
            expires > authorization["authorization_expires_at_unix_ms"]):
        raise ContractError("request time must exactly consume the observed prerequisite window")
    sequence = integer(node["expected_next_sequence"],
                       "request.expected_next_sequence", 1)
    if sequence == 1 and node["expected_ledger_sha256"] is not None:
        raise ContractError("genesis request requires null expected ledger digest")
    if sequence > 1:
        sha256(node["expected_ledger_sha256"], "request.expected_ledger_sha256")
    sha256(node["expected_current_head_set_sha256"],
           "request.expected_current_head_set_sha256")
    targets = _validate_targets(node["supersession_targets"])
    supersedes = [target["adr_id"] for target in targets]
    binding = prerequisite["proposal_binding"]
    if metadata["supersedes"] != supersedes or binding["adr_id"] in supersedes:
        raise ContractError("request targets differ from immutable proposal supersedes")
    text(node["idempotency_key"], "request.idempotency_key", IDEMPOTENCY_RE)


def _validate_targets(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or len(value) > MAX_SUPERSESSIONS:
        raise ContractError("request supersession_targets exceeds its closed bound")
    targets = []
    for index, item in enumerate(value):
        label = f"request.supersession_targets[{index}]"
        node = require_keys(item, label, TARGET_FIELDS)
        adr_id(node["adr_id"], f"{label}.adr_id")
        text(node["acceptance_id"], f"{label}.acceptance_id")
        sha256(node["acceptance_sha256"], f"{label}.acceptance_sha256")
        sha256(node["proposal_binding_sha256"], f"{label}.proposal_binding_sha256")
        targets.append(node)
    identifiers = [node["adr_id"] for node in targets]
    expected = sorted(set(identifiers), key=lambda item: item.encode("utf-8"))
    if identifiers != expected:
        raise ContractError("request supersession_targets must be sorted and unique")
    return targets


def _validate_request_authority(node: dict[str, Any], profile_hash: str,
                                root: dict[str, Any]) -> None:
    if (sha256(node["trust_root_sha256"], "request.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("request does not bind the lifecycle trust root")
    integer(node["trust_epoch"], "request.trust_epoch", 1)
    key = lifecycle_key(root, REQUEST_KEY_USAGE)
    validate_signature(node["signature"], "request.signature", profile_hash,
                       key["key_id"])


def _validate_request_identity(node: dict[str, Any]) -> None:
    expected = request_sha256(node)
    if sha256(node["request_sha256"], "request.request_sha256") != expected:
        raise ContractError("lifecycle request self digest does not match")
    if node["request_id"] != f"architecture-decision-lifecycle-request-{expected}":
        raise ContractError("lifecycle request ID does not match its digest")


def acceptance_sha256(value: dict[str, Any]) -> str:
    return self_digest(ACCEPTANCE_DOMAIN, value,
                       ("acceptance_id", "acceptance_sha256"),
                       MAX_ACCEPTANCE_BYTES,
                       "ArchitectureDecisionLifecycleAcceptanceReceipt", signed=True)


def validate_acceptance(value: Any, profile_hash: str,
                        root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionLifecycleAcceptanceReceipt"
    node = require_keys(value, label, ACCEPTANCE_FIELDS)
    bounded_canonical_json(node, MAX_ACCEPTANCE_BYTES, label)
    expected = (ACCEPTANCE_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    _validate_acceptance_scalars(node)
    _validate_state_signature(node, profile_hash, root, "acceptance")
    expected_hash = acceptance_sha256(node)
    if node["acceptance_sha256"] != expected_hash:
        raise ContractError("acceptance receipt self digest does not match")
    if node["acceptance_id"] != f"architecture-decision-acceptance-{expected_hash}":
        raise ContractError("acceptance ID does not match its receipt digest")
    return node


def _validate_acceptance_scalars(node: dict[str, Any]) -> None:
    adr_id(node["adr_id"], "acceptance.adr_id")
    integer(node["accepted_at_unix_ms"], "acceptance.accepted_at_unix_ms")
    integer(node["ledger_sequence"], "acceptance.ledger_sequence", 1)
    integer(node["trust_epoch"], "acceptance.trust_epoch", 1)
    for field in ("acceptance_sha256", "authorization_receipt_physical_sha256",
                  "authorization_receipt_sha256", "proposal_binding_sha256",
                  "record_key_sha256", "request_sha256", "trust_root_sha256"):
        sha256(node[field], f"acceptance.{field}")
    supersedes = sorted_unique(node["supersedes"], "acceptance.supersedes",
                               MAX_SUPERSESSIONS)
    for index, item in enumerate(supersedes):
        adr_id(item, f"acceptance.supersedes[{index}]")


def supersession_sha256(value: dict[str, Any]) -> str:
    return self_digest(SUPERSESSION_DOMAIN, value, ("receipt_id", "receipt_sha256"),
                       MAX_SUPERSESSION_BYTES,
                       "ArchitectureDecisionLifecycleSupersessionReceipt", signed=True)


def validate_supersession(value: Any, profile_hash: str,
                          root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionLifecycleSupersessionReceipt"
    node = require_keys(value, label, SUPERSESSION_FIELDS)
    bounded_canonical_json(node, MAX_SUPERSESSION_BYTES, label)
    expected = (SUPERSESSION_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    _validate_supersession_scalars(node)
    _validate_state_signature(node, profile_hash, root, "supersession")
    expected_hash = supersession_sha256(node)
    if node["receipt_sha256"] != expected_hash:
        raise ContractError("supersession receipt self digest does not match")
    if node["receipt_id"] != f"architecture-decision-supersession-{expected_hash}":
        raise ContractError("supersession receipt ID does not match its digest")
    return node


def _validate_supersession_scalars(node: dict[str, Any]) -> None:
    for field in ("superseded_by_adr_id", "target_adr_id"):
        adr_id(node[field], f"supersession.{field}")
    integer(node["ledger_sequence"], "supersession.ledger_sequence", 1)
    integer(node["superseded_at_unix_ms"], "supersession.superseded_at_unix_ms")
    integer(node["trust_epoch"], "supersession.trust_epoch", 1)
    for field in ("receipt_sha256", "request_sha256",
                  "superseded_by_proposal_binding_sha256",
                  "target_proposal_binding_sha256", "trust_root_sha256"):
        sha256(node[field], f"supersession.{field}")
    for field in ("superseded_by_acceptance_id", "target_acceptance_id"):
        text(node[field], f"supersession.{field}")


def _validate_state_signature(node: dict[str, Any], profile_hash: str,
                              root: dict[str, Any], label: str) -> None:
    if (node["trust_root_sha256"] != root["root_sha256"] or
            node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError(f"{label} receipt does not bind lifecycle trust root")
    key = lifecycle_key(root, STATE_KEY_USAGE)
    validate_signature(node["signature"], f"{label}.signature", profile_hash,
                       key["key_id"])


def entry_sha256(value: dict[str, Any]) -> str:
    return self_digest(ENTRY_DOMAIN, value, ("entry_sha256",), MAX_ENTRY_BYTES,
                       "ArchitectureDecisionLifecycleLedgerEntry")


def validate_entry_shape(value: Any, profile_hash: str,
                         lifecycle_root: dict[str, Any],
                         approval_root: dict[str, Any],
                         ) -> tuple[dict[str, Any], dict[str, Any]]:
    label = "ArchitectureDecisionLifecycleLedgerEntry"
    node = require_keys(value, label, ENTRY_FIELDS)
    bounded_canonical_json(node, MAX_ENTRY_BYTES, label)
    expected = (ENTRY_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    sequence = integer(node["sequence"], "entry.sequence", 1)
    if node["prior_entry_sha256"] is not None:
        sha256(node["prior_entry_sha256"], "entry.prior_entry_sha256")
    sha256(node["resulting_current_head_set_sha256"],
           "entry.resulting_current_head_set_sha256")
    request, metadata = validate_request(node["request"], profile_hash,
                                         lifecycle_root, approval_root)
    validate_acceptance(node["acceptance_receipt"], profile_hash, lifecycle_root)
    receipts = node["supersession_receipts"]
    if not isinstance(receipts, list) or len(receipts) > MAX_SUPERSESSIONS:
        raise ContractError("entry supersession_receipts exceeds its closed bound")
    for receipt in receipts:
        validate_supersession(receipt, profile_hash, lifecycle_root)
    target_ids = [item["adr_id"] for item in request["supersession_targets"]]
    if [item["target_adr_id"] for item in receipts] != target_ids:
        raise ContractError("supersession receipts must exactly follow sorted targets")
    if sequence != request["expected_next_sequence"]:
        raise ContractError("entry sequence differs from request CAS sequence")
    if sha256(node["entry_sha256"], "entry.entry_sha256") != entry_sha256(node):
        raise ContractError("lifecycle entry self digest does not match")
    return node, metadata


def validate_result(value: Any) -> dict[str, Any]:
    label = "ArchitectureDecisionLifecycleTransitionResult"
    node = require_keys(value, label, RESULT_FIELDS)
    bounded_canonical_json(node, MAX_RESULT_BYTES, label)
    expected = (RESULT_API, CANONICALIZATION, label)
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    if node["delivery_disposition"] not in {"exact_replay", "stored"}:
        raise ContractError("lifecycle result delivery disposition is unsupported")
    for field in ("entry_sha256", "ledger_sha256", "materialized_view_sha256",
                  "state_sha256"):
        sha256(node[field], f"result.{field}")
    if not isinstance(node["receipt"], dict):
        raise ContractError("result.receipt must carry an exact acceptance receipt")
    return node
