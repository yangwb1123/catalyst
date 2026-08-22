"""Authorization request, structurally related receipt, and delivery result wires."""

from __future__ import annotations

import hashlib
import re
from typing import Any

from .approvals import (declared_outcome, declared_reason_codes,
                        validate_approval_records)
from .authority import key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (
    CANONICALIZATION,
    MAX_RECEIPT_BYTES,
    MAX_REQUEST_BYTES,
    MAX_RESULT_BYTES,
    PROFILE_ID,
    RECEIPT_API,
    RECEIPT_DOMAIN,
    RECORD_KEY_DOMAIN,
    REQUEST_API,
    REQUEST_DOMAIN,
    RESULT_API,
)
from .shape import (array, enum, integer, require_keys, sha256, stable_id,
                    sorted_unique_strings, validate_principal, validate_signature)

REQUEST_FIELDS = {
    "api_version", "approval_records", "canonicalization", "expected_ledger_sha256",
    "expected_next_sequence", "expires_at_unix_ms", "idempotency_key", "kind",
    "policy_sha256", "profile_id", "proposal_binding", "request_id", "request_sha256",
    "requested_at_unix_ms", "requester", "revocation_sequence", "revocation_sha256",
    "signature", "trust_epoch", "trust_root_sha256",
}
RECEIPT_FIELDS = {
    "api_version", "authorization_decision", "authorization_expires_at_unix_ms",
    "canonicalization", "evaluated_at_unix_ms", "kind", "ledger_sequence",
    "policy_sha256", "prior_receipt_sha256", "profile_id", "proposal_binding_sha256",
    "qualifying_approval_ids", "reason_codes", "receipt_id", "receipt_sha256",
    "record_key_sha256", "request_sha256", "revocation_sequence", "revocation_sha256",
    "signature", "trust_epoch", "trust_root_sha256",
}
RESULT_FIELDS = {"api_version", "canonicalization", "delivery_disposition", "kind", "receipt"}


def request_sha256(value: dict[str, Any]) -> str:
    return self_digest(REQUEST_DOMAIN, value, ("request_id", "request_sha256"),
                       MAX_REQUEST_BYTES,
                       "ArchitectureDecisionApprovalAuthorizationRequest", signed=True)


def validate_request(value: Any, profile_hash: str, root: dict[str, Any],
                     policy: dict[str, Any], snapshot: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalAuthorizationRequest"
    node = require_keys(value, label, REQUEST_FIELDS)
    bounded_canonical_json(node, MAX_REQUEST_BYTES, label)
    expected = (REQUEST_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    _validate_request_time_and_cas(node, policy, snapshot)
    _validate_request_authority(node, profile_hash, root, policy)
    _validate_request_relations(node, policy, snapshot)
    validate_approval_records(node["approval_records"], policy, root, snapshot,
                              node["requested_at_unix_ms"])
    _validate_request_identity(node)
    return node


def _validate_request_time_and_cas(node: dict[str, Any], policy: dict[str, Any],
                                   snapshot: dict[str, Any]) -> None:
    start = integer(node["requested_at_unix_ms"], "request.requested_at_unix_ms",
                    0, 2**63 - 1)
    expires = integer(node["expires_at_unix_ms"], "request.expires_at_unix_ms",
                      0, 2**63 - 1)
    if not start < expires or expires - start > policy["max_request_validity_ms"]:
        raise ContractError("request validity exceeds the signed policy maximum")
    validity = policy["validity"]
    if not validity["not_before_unix_ms"] <= start < expires <= validity["expires_at_unix_ms"]:
        raise ContractError("request validity lies outside policy validity")
    if not snapshot["effective_at_unix_ms"] <= start < snapshot["expires_at_unix_ms"]:
        raise ContractError("request declared time lies outside revocation snapshot validity")
    sequence = integer(node["expected_next_sequence"], "request.expected_next_sequence",
                       1, 2**63 - 1)
    if sequence == 1 and node["expected_ledger_sha256"] is not None:
        raise ContractError("genesis request requires null expected ledger digest")
    if sequence > 1:
        sha256(node["expected_ledger_sha256"], "request.expected_ledger_sha256")


def _validate_request_authority(node: dict[str, Any], profile_hash: str,
                                root: dict[str, Any], policy: dict[str, Any]) -> None:
    validate_principal(node["requester"], "request.requester")
    request_key = key_for_usage(root, "approval_request_auth")
    if node["requester"] != policy["roles"]["requester"] or node["requester"] != request_key["principal"]:
        raise ContractError("requester differs from signed policy and request-auth key")
    if (sha256(node["trust_root_sha256"], "request.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("request does not bind the supplied trust root")
    integer(node["trust_epoch"], "request.trust_epoch", 1, 2**63 - 1)
    signature = validate_signature(node["signature"], "request.signature", profile_hash)
    if signature["key_id"] != request_key["key_id"]:
        raise ContractError("request signature uses the wrong root key usage")


def _validate_request_relations(node: dict[str, Any], policy: dict[str, Any],
                                snapshot: dict[str, Any]) -> None:
    if (sha256(node["policy_sha256"], "request.policy_sha256") !=
            policy["policy_sha256"] or node["proposal_binding"] !=
            policy["proposal_binding"]):
        raise ContractError("request does not bind exact policy and proposal")
    if (node["revocation_sequence"] != snapshot["revocation_sequence"] or
            sha256(node["revocation_sha256"], "request.revocation_sha256") !=
            snapshot["revocation_sha256"]):
        raise ContractError("request does not bind exact revocation snapshot")
    used_keys = {node["signature"]["key_id"], policy["signature"]["key_id"]}
    if used_keys & set(snapshot["revoked_key_ids"]):
        raise ContractError("request or policy signing key is revoked")
    key = node["idempotency_key"]
    if not isinstance(key, str) or re.fullmatch(r"[A-Za-z0-9._:@+\-]{16,128}", key) is None:
        raise ContractError("request idempotency key must be 16..128 closed visible ASCII bytes")


def _validate_request_identity(node: dict[str, Any]) -> None:
    sha256(node["request_sha256"], "request.request_sha256")
    expected = request_sha256(node)
    if node["request_sha256"] != expected:
        raise ContractError("request self digest does not match")
    if node["request_id"] != f"architecture-decision-approval-request-{expected}":
        raise ContractError("request ID does not match its digest")


def record_key_sha256(idempotency_key: str) -> str:
    if (not isinstance(idempotency_key, str) or
            re.fullmatch(r"[A-Za-z0-9._:@+\-]{16,128}", idempotency_key) is None):
        raise ContractError("idempotency key must be 16..128 closed visible ASCII bytes")
    return hashlib.sha256(RECORD_KEY_DOMAIN + idempotency_key.encode("ascii")).hexdigest()


def receipt_sha256(value: dict[str, Any]) -> str:
    return self_digest(RECEIPT_DOMAIN, value, ("receipt_id", "receipt_sha256"),
                       MAX_RECEIPT_BYTES,
                       "ArchitectureDecisionApprovalAuthorizationReceipt", signed=True)


def validate_receipt(value: Any, profile_hash: str,
                     root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalAuthorizationReceipt"
    node = require_keys(value, label, RECEIPT_FIELDS)
    bounded_canonical_json(node, MAX_RECEIPT_BYTES, label)
    expected = (RECEIPT_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    enum(node["authorization_decision"], "receipt.authorization_decision",
         ("acceptance_transition_authorized", "acceptance_transition_not_authorized"))
    _validate_receipt_scalars(node)
    _validate_receipt_authority(node, profile_hash, root)
    _validate_receipt_identity(node)
    return node


def _validate_receipt_scalars(node: dict[str, Any]) -> None:
    integer(node["evaluated_at_unix_ms"], "receipt.evaluated_at_unix_ms", 0, 2**63 - 1)
    integer(node["authorization_expires_at_unix_ms"],
            "receipt.authorization_expires_at_unix_ms", 0, 2**63 - 1)
    integer(node["ledger_sequence"], "receipt.ledger_sequence", 1, 2**63 - 1)
    integer(node["revocation_sequence"], "receipt.revocation_sequence", 1, 2**63 - 1)
    for field in ("policy_sha256", "proposal_binding_sha256", "record_key_sha256",
                  "request_sha256", "revocation_sha256", "trust_root_sha256"):
        sha256(node[field], f"receipt.{field}")
    if node["prior_receipt_sha256"] is not None:
        sha256(node["prior_receipt_sha256"], "receipt.prior_receipt_sha256")
    approvals = sorted_unique_strings(node["qualifying_approval_ids"],
                                      "receipt.qualifying_approval_ids", 0, 16)
    if any(re.fullmatch(r"approval-record-[0-9a-f]{64}", item) is None
           for item in approvals):
        raise ContractError("receipt qualifying approval ID is malformed")
    reasons = sorted_unique_strings(node["reason_codes"], "receipt.reason_codes", 0, 1)
    for index, reason in enumerate(reasons):
        stable_id(reason, f"receipt.reason_codes[{index}]")


def _validate_receipt_authority(node: dict[str, Any], profile_hash: str,
                                root: dict[str, Any]) -> None:
    if (node["trust_root_sha256"] != root["root_sha256"] or
            node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("receipt does not bind the supplied trust root")
    integer(node["trust_epoch"], "receipt.trust_epoch", 1, 2**63 - 1)
    signature = validate_signature(node["signature"], "receipt.signature", profile_hash)
    expected = key_for_usage(root, "approval_authorization_state_sign")["key_id"]
    if signature["key_id"] != expected:
        raise ContractError("receipt signature uses the wrong root key usage")


def _validate_receipt_identity(node: dict[str, Any]) -> None:
    sha256(node["receipt_sha256"], "receipt.receipt_sha256")
    expected = receipt_sha256(node)
    if node["receipt_sha256"] != expected:
        raise ContractError("receipt self digest does not match")
    if node["receipt_id"] != f"architecture-decision-approval-receipt-{expected}":
        raise ContractError("receipt ID does not match its digest")


def validate_receipt_relations(policy: dict[str, Any], request: dict[str, Any],
                               snapshot: dict[str, Any], receipt: dict[str, Any],
                               root: dict[str, Any]) -> None:
    evaluated = receipt["evaluated_at_unix_ms"]
    if not request["requested_at_unix_ms"] <= evaluated < request["expires_at_unix_ms"]:
        raise ContractError("receipt evaluation time lies outside request validity")
    if not snapshot["effective_at_unix_ms"] <= evaluated < snapshot["expires_at_unix_ms"]:
        raise ContractError("receipt evaluation time lies outside revocation validity")
    records = validate_approval_records(request["approval_records"], policy, root,
                                        snapshot, evaluated)
    if receipt["signature"]["key_id"] in snapshot["revoked_key_ids"]:
        raise ContractError("receipt signing key is revoked")
    _validate_receipt_bindings(policy, request, snapshot, receipt)
    _validate_receipt_outcome(policy, records, receipt)
    _validate_receipt_expiry(policy, request, snapshot, records, receipt)


def _validate_receipt_bindings(policy: dict[str, Any], request: dict[str, Any],
                               snapshot: dict[str, Any], receipt: dict[str, Any]) -> None:
    relations = {
        "ledger_sequence": request["expected_next_sequence"],
        "policy_sha256": policy["policy_sha256"],
        "proposal_binding_sha256": policy["proposal_binding"]["proposal_binding_sha256"],
        "record_key_sha256": record_key_sha256(request["idempotency_key"]),
        "request_sha256": request["request_sha256"],
        "revocation_sequence": snapshot["revocation_sequence"],
        "revocation_sha256": snapshot["revocation_sha256"],
        "trust_epoch": request["trust_epoch"],
        "trust_root_sha256": request["trust_root_sha256"],
    }
    if any(receipt[field] != expected for field, expected in relations.items()):
        raise ContractError("receipt does not bind request, policy, proposal, revocation, or root")
    if receipt["ledger_sequence"] == 1 and receipt["prior_receipt_sha256"] is not None:
        raise ContractError("first receipt must have null prior digest")
    if receipt["ledger_sequence"] > 1 and receipt["prior_receipt_sha256"] is None:
        raise ContractError("non-first receipt must bind a prior receipt digest")


def _validate_receipt_outcome(policy: dict[str, Any], records: list[dict[str, Any]],
                              receipt: dict[str, Any]) -> None:
    decision, approvals = declared_outcome(policy, records)
    if (receipt["authorization_decision"] != decision or
            receipt["qualifying_approval_ids"] != approvals or
            receipt["reason_codes"] != declared_reason_codes(policy, records)):
        raise ContractError("receipt differs from declared policy/ApprovalRecord relations")


def _validate_receipt_expiry(policy: dict[str, Any], request: dict[str, Any],
                             snapshot: dict[str, Any], records: list[dict[str, Any]],
                             receipt: dict[str, Any]) -> None:
    expiries = [policy["validity"]["expires_at_unix_ms"], request["expires_at_unix_ms"],
                snapshot["expires_at_unix_ms"]]
    expiries.extend(record["validity"]["expires_at_unix_ms"] for record in records)
    expected = min(expiries)
    if receipt["authorization_expires_at_unix_ms"] != expected:
        raise ContractError("receipt expiry is not the minimum declared validity bound")
    if not receipt["evaluated_at_unix_ms"] < expected:
        raise ContractError("receipt authorization window is already closed")


def validate_result(value: Any, profile_hash: str, root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalAuthorizationResult"
    node = require_keys(value, label, RESULT_FIELDS)
    bounded_canonical_json(node, MAX_RESULT_BYTES, label)
    expected = (RESULT_API, CANONICALIZATION, label)
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    enum(node["delivery_disposition"], "result.delivery_disposition",
         ("exact_replay", "stored"))
    validate_receipt(node["receipt"], profile_hash, root)
    return node
