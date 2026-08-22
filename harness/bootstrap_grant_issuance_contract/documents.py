"""Signed Policy, Request, and durable Receipt structural validation."""

from __future__ import annotations

import hashlib
import re
from typing import Any

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, MAX_POLICY_BYTES, MAX_POLICY_VALIDITY_MS,
                        MAX_RECEIPT_BYTES, MAX_REQUEST_BYTES, MAX_REQUEST_VALIDITY_MS,
                        MAX_TTL_MS, POLICY_API, POLICY_DOMAIN, RECEIPT_API, RECEIPT_DOMAIN,
                        RECORD_KEY_DOMAIN, REQUEST_API, REQUEST_DOMAIN)
from .shape import (budget_covers, enum, integer, require_keys, sha256, text,
                    validate_budget, validate_capability, validate_principal,
                    validate_profile_id, validate_request_bindings, validate_scope,
                    validate_signature, validate_task_binding)

POLICY_FIELDS = {
    "api_version", "budget", "canonicalization", "capability", "disposition",
    "effect_id", "kind", "max_ttl_ms", "policy_id", "policy_sha256", "profile_id",
    "scope", "signature", "subject", "task_binding", "trust_epoch",
    "trust_root_sha256", "validity",
}
REQUEST_FIELDS = {
    "api_version", "bindings", "budget", "canonicalization", "capability", "effect_id",
    "expires_at_unix_ms", "idempotency_key", "kind", "policy_sha256", "profile_id",
    "request_sha256", "requested_at_unix_ms", "requested_ttl_ms", "scope", "signature",
    "subject", "task_binding", "trust_epoch", "trust_root_sha256",
}
RECEIPT_FIELDS = {
    "api_version", "canonicalization", "decision", "denial_reason",
    "grant_envelope_sha256", "grant_id", "grant_sha256", "kind", "ledger_sequence",
    "policy_sha256", "prior_receipt_sha256", "profile_id", "receipt_sha256",
    "record_key_sha256", "request_sha256", "signature", "stored_at_unix_ms",
    "trust_epoch", "trust_root_sha256",
}


def policy_sha256(value: dict[str, Any]) -> str:
    return self_digest(POLICY_DOMAIN, value, "policy_sha256", MAX_POLICY_BYTES,
                       "BootstrapGrantPolicy", signed=True)


def validate_policy(value: Any, profile_hash: str) -> dict[str, Any]:
    node = require_keys(value, "BootstrapGrantPolicy", POLICY_FIELDS)
    bounded_canonical_json(node, MAX_POLICY_BYTES, "BootstrapGrantPolicy")
    _require_envelope(node, POLICY_API, "BootstrapGrantPolicy")
    validate_profile_id(node["profile_id"], "policy.profile_id")
    text(node["policy_id"], "policy.policy_id")
    enum(node["disposition"], "policy.disposition", ("allow", "deny"))
    if node["effect_id"] != "repo.read":
        raise ContractError("policy.effect_id must be repo.read")
    validate_capability(node["capability"], "policy.capability")
    validate_principal(node["subject"], "policy.subject")
    validate_task_binding(node["task_binding"], "policy.task_binding")
    validate_scope(node["scope"], "policy.scope")
    validate_budget(node["budget"], "policy.budget")
    integer(node["max_ttl_ms"], "policy.max_ttl_ms", 1, MAX_TTL_MS)
    _validate_policy_validity(node["validity"])
    _validate_common_authority(node, "policy")
    validate_signature(node["signature"], "policy.signature", profile_hash)
    _require_self_digest(node, "policy_sha256", policy_sha256, "policy")
    return node


def _validate_policy_validity(value: Any) -> None:
    keys = {"expires_at_unix_ms", "not_before_unix_ms"}
    node = require_keys(value, "policy.validity", keys)
    start = integer(node["not_before_unix_ms"], "policy.validity.not_before_unix_ms",
                    0, 2**63 - 1)
    end = integer(node["expires_at_unix_ms"], "policy.validity.expires_at_unix_ms",
                  0, 2**63 - 1)
    if not start < end or end - start > MAX_POLICY_VALIDITY_MS:
        raise ContractError("policy validity must be ordered within 24 hours")


def request_sha256(value: dict[str, Any]) -> str:
    return self_digest(REQUEST_DOMAIN, value, "request_sha256", MAX_REQUEST_BYTES,
                       "BootstrapGrantRequest", signed=True)


def validate_request(value: Any, profile_hash: str) -> dict[str, Any]:
    node = require_keys(value, "BootstrapGrantRequest", REQUEST_FIELDS)
    bounded_canonical_json(node, MAX_REQUEST_BYTES, "BootstrapGrantRequest")
    _require_envelope(node, REQUEST_API, "BootstrapGrantRequest")
    validate_profile_id(node["profile_id"], "request.profile_id")
    if node["effect_id"] != "repo.read":
        raise ContractError("request.effect_id must be repo.read")
    validate_request_bindings(node["bindings"], "request.bindings")
    validate_capability(node["capability"], "request.capability")
    validate_principal(node["subject"], "request.subject")
    validate_task_binding(node["task_binding"], "request.task_binding")
    validate_scope(node["scope"], "request.scope")
    validate_budget(node["budget"], "request.budget")
    _validate_request_time_and_key(node)
    _validate_common_authority(node, "request")
    validate_signature(node["signature"], "request.signature", profile_hash)
    sha256(node["policy_sha256"], "request.policy_sha256")
    _require_self_digest(node, "request_sha256", request_sha256, "request")
    return node


def _validate_request_time_and_key(node: dict[str, Any]) -> None:
    key = node["idempotency_key"]
    if not isinstance(key, str) or re.fullmatch(r"[A-Za-z0-9._:@+\-]{16,128}", key) is None:
        raise ContractError("request.idempotency_key must be 16..128 closed visible ASCII bytes")
    start = integer(node["requested_at_unix_ms"], "request.requested_at_unix_ms",
                    0, 2**63 - 1)
    end = integer(node["expires_at_unix_ms"], "request.expires_at_unix_ms", 0, 2**63 - 1)
    if not start < end or end - start > MAX_REQUEST_VALIDITY_MS:
        raise ContractError("request validity must be ordered within five minutes")
    integer(node["requested_ttl_ms"], "request.requested_ttl_ms", 1, MAX_TTL_MS)


def validate_policy_request(policy: dict[str, Any], request: dict[str, Any]) -> None:
    equal_fields = ("capability", "effect_id", "profile_id", "scope", "subject",
                    "task_binding", "trust_epoch", "trust_root_sha256")
    if any(request[field] != policy[field] for field in equal_fields):
        raise ContractError("Policy and Request exact fields differ")
    if request["policy_sha256"] != policy["policy_sha256"]:
        raise ContractError("Request does not bind the exact Policy digest")
    if not budget_covers(policy["budget"], request["budget"]):
        raise ContractError("Request budget exceeds Policy budget")
    if request["requested_ttl_ms"] > policy["max_ttl_ms"]:
        raise ContractError("Request TTL exceeds Policy maximum")
    validity = policy["validity"]
    if not (validity["not_before_unix_ms"] <= request["requested_at_unix_ms"] <
            request["expires_at_unix_ms"] <= validity["expires_at_unix_ms"]):
        raise ContractError("Request validity is outside Policy validity")


def record_key_sha256(idempotency_key: str) -> str:
    if (not isinstance(idempotency_key, str) or
            re.fullmatch(r"[A-Za-z0-9._:@+\-]{16,128}", idempotency_key) is None):
        raise ContractError("idempotency key must be 16..128 closed visible ASCII bytes")
    return hashlib.sha256(RECORD_KEY_DOMAIN + idempotency_key.encode("ascii")).hexdigest()


def receipt_sha256(value: dict[str, Any]) -> str:
    return self_digest(RECEIPT_DOMAIN, value, "receipt_sha256", MAX_RECEIPT_BYTES,
                       "GrantIssuanceReceipt", signed=True)


def validate_receipt(value: Any, profile_hash: str) -> dict[str, Any]:
    node = require_keys(value, "GrantIssuanceReceipt", RECEIPT_FIELDS)
    bounded_canonical_json(node, MAX_RECEIPT_BYTES, "GrantIssuanceReceipt")
    _require_envelope(node, RECEIPT_API, "GrantIssuanceReceipt")
    validate_profile_id(node["profile_id"], "receipt.profile_id")
    enum(node["decision"], "receipt.decision", ("denied", "issued"))
    integer(node["ledger_sequence"], "receipt.ledger_sequence", 1, 2**63 - 1)
    integer(node["stored_at_unix_ms"], "receipt.stored_at_unix_ms", 0, 2**63 - 1)
    for field in ("policy_sha256", "receipt_sha256", "record_key_sha256",
                  "request_sha256", "trust_root_sha256"):
        sha256(node[field], f"receipt.{field}")
    for field in ("grant_envelope_sha256", "grant_sha256", "prior_receipt_sha256"):
        if node[field] is not None:
            sha256(node[field], f"receipt.{field}")
    validate_signature(node["signature"], "receipt.signature", profile_hash)
    _validate_receipt_grant_fields(node)
    _validate_common_authority(node, "receipt")
    _require_self_digest(node, "receipt_sha256", receipt_sha256, "receipt")
    return node


def _validate_receipt_grant_fields(node: dict[str, Any]) -> None:
    fields = (node["grant_id"], node["grant_sha256"], node["grant_envelope_sha256"])
    if node["decision"] == "issued":
        if node["denial_reason"] is not None or any(value is None for value in fields):
            raise ContractError("issued receipt requires all Grant identities and no denial")
        expected_id = f"capability-grant-{node['grant_sha256']}"
        if node["grant_id"] != expected_id:
            raise ContractError("receipt grant_id does not match grant_sha256")
    elif node["denial_reason"] != "policy_denied" or any(value is not None for value in fields):
        raise ContractError("denied receipt must have policy_denied and null Grant identities")


def validate_receipt_relations(policy: dict[str, Any], request: dict[str, Any],
                               receipt: dict[str, Any]) -> None:
    expected_decision = "issued" if policy["disposition"] == "allow" else "denied"
    if receipt["decision"] != expected_decision:
        raise ContractError("receipt decision differs from authenticated Policy disposition")
    relations = (("policy_sha256", policy["policy_sha256"]),
                 ("request_sha256", request["request_sha256"]),
                 ("record_key_sha256", record_key_sha256(request["idempotency_key"])),
                 ("trust_root_sha256", request["trust_root_sha256"]),
                 ("trust_epoch", request["trust_epoch"]),
                 ("profile_id", request["profile_id"]))
    if any(receipt[field] != expected for field, expected in relations):
        raise ContractError("receipt does not bind Policy, Request, record key, or root")
    stored = receipt["stored_at_unix_ms"]
    if not request["requested_at_unix_ms"] <= stored < request["expires_at_unix_ms"]:
        raise ContractError("receipt stored time is outside Request validity")


def _validate_common_authority(node: dict[str, Any], label: str) -> None:
    sha256(node["trust_root_sha256"], f"{label}.trust_root_sha256")
    integer(node["trust_epoch"], f"{label}.trust_epoch", 1, 2**63 - 1)


def _require_envelope(node: dict[str, Any], api: str, kind: str) -> None:
    if (node["api_version"], node["canonicalization"], node["kind"]) != (
            api, CANONICALIZATION, kind):
        raise ContractError(f"{kind} envelope drifted from v1")


def _require_self_digest(node: dict[str, Any], field: str, function: Any, label: str) -> None:
    sha256(node[field], f"{label}.{field}")
    if node[field] != function(node):
        raise ContractError(f"{label} self digest does not match")
