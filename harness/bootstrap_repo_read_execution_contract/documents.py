"""Signed execution Policy and Invocation structural contracts."""

from __future__ import annotations

from typing import Any

from bootstrap_grant_issuance_contract.grant import grant_envelope_sha256
from bootstrap_grant_issuance_contract.shape import budget_covers

from .authority import key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, INVOCATION_API, INVOCATION_DOMAIN,
                        MAX_FRESHNESS_MS, MAX_INVOCATION_BYTES, MAX_POLICY_BYTES,
                        POLICY_API, POLICY_DOMAIN, PROFILE_ID)
from .manifest import manifest_content_bytes, manifest_paths
from .shape import (action_sha256, integer, paths_from_action, record_key_sha256,
                    require_keys, sha256, text, validate_action, validate_bindings,
                    validate_budget, validate_budget_covers_action, validate_capability,
                    validate_idempotency, validate_principal, validate_signature,
                    validate_task_binding)

POLICY_FIELDS = {
    "activation", "api_version", "bindings", "budget", "canonicalization",
    "capability", "disposition", "effect_id", "execution_policy_id",
    "execution_policy_sha256", "execution_trust_epoch", "execution_trust_root_sha256",
    "grant_envelope_sha256", "grant_id", "grant_issuance_ledger_sequence",
    "grant_issuance_receipt_sha256", "grant_policy_sha256", "grant_request_sha256",
    "grant_sha256", "idempotency_key", "issuance_trust_epoch",
    "issuance_trust_root_sha256", "kind", "manifest_sha256", "profile_id",
    "requested_action", "requested_action_sha256", "signature", "subject",
    "task_binding", "validity",
}
INVOCATION_FIELDS = {
    "api_version", "bindings", "canonicalization", "capability",
    "execution_policy_sha256", "execution_trust_epoch", "execution_trust_root_sha256",
    "expires_at_unix_ms", "grant_envelope_sha256", "grant_id",
    "grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256",
    "grant_policy_sha256", "grant_request_sha256", "grant_sha256", "idempotency_key",
    "invocation_id", "invocation_sha256", "issuance_trust_epoch",
    "issuance_trust_root_sha256", "kind", "manifest_sha256", "profile_id",
    "requested_action", "requested_action_sha256", "requested_at_unix_ms", "signature",
    "subject", "task_binding",
}


def policy_sha256(value: dict[str, Any]) -> str:
    return self_digest(POLICY_DOMAIN, value, "execution_policy_sha256", MAX_POLICY_BYTES,
                       "BootstrapRepoReadExecutionPolicy", signed=True)


def invocation_sha256(value: dict[str, Any]) -> str:
    return self_digest(INVOCATION_DOMAIN, value, "invocation_sha256",
                       MAX_INVOCATION_BYTES, "BootstrapRepoReadInvocation", signed=True,
                       derived_id_field="invocation_id")


def validate_policy(value: Any, profile_hash: str, execution_root: dict[str, Any],
                    manifest: dict[str, Any], issued: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadExecutionPolicy", POLICY_FIELDS)
    bounded_canonical_json(node, MAX_POLICY_BYTES, "BootstrapRepoReadExecutionPolicy")
    _validate_policy_shape(node, profile_hash)
    _validate_policy_authority(node, execution_root)
    _validate_policy_issued_relations(node, issued)
    _validate_policy_action(node, manifest, issued["grant"])
    _validate_policy_time(node, issued["grant"])
    if node["execution_policy_sha256"] != policy_sha256(node):
        raise ContractError("ExecutionPolicy self digest does not match")
    return node


def _validate_policy_shape(node: dict[str, Any], profile_hash: str) -> None:
    expected = (POLICY_API, CANONICALIZATION, "BootstrapRepoReadExecutionPolicy", PROFILE_ID)
    if (node["api_version"], node["canonicalization"], node["kind"],
            node["profile_id"]) != expected:
        raise ContractError("ExecutionPolicy envelope drifted from v1")
    pairs = {"allow": "activate_once", "deny": "do_not_activate"}
    if node["disposition"] not in pairs or node["activation"] != pairs[node["disposition"]]:
        raise ContractError("ExecutionPolicy activation/disposition pair is invalid")
    if node["effect_id"] != "repo.read":
        raise ContractError("ExecutionPolicy effect must be repo.read")
    text(node["execution_policy_id"], "execution_policy_id")
    validate_idempotency(node["idempotency_key"], "policy.idempotency_key")
    validate_bindings(node["bindings"], "policy.bindings")
    validate_capability(node["capability"], "policy.capability")
    validate_principal(node["subject"], "policy.subject")
    validate_task_binding(node["task_binding"], "policy.task_binding")
    validate_budget(node["budget"], "policy.budget")
    validate_action(node["requested_action"], "policy.requested_action")
    for field in _policy_hash_fields():
        sha256(node[field], f"policy.{field}")
    integer(node["grant_issuance_ledger_sequence"], "grant_issuance_ledger_sequence",
            1, 256)
    validate_signature(node["signature"], "policy.signature", profile_hash)


def _policy_hash_fields() -> tuple[str, ...]:
    return ("execution_policy_sha256", "execution_trust_root_sha256",
            "grant_envelope_sha256", "grant_issuance_receipt_sha256",
            "grant_policy_sha256", "grant_request_sha256", "grant_sha256",
            "issuance_trust_root_sha256", "manifest_sha256", "requested_action_sha256")


def _validate_policy_authority(node: dict[str, Any], root: dict[str, Any]) -> None:
    for field in ("execution_trust_epoch", "issuance_trust_epoch"):
        integer(node[field], f"policy.{field}", 1, 2**63 - 1)
    if (node["execution_trust_root_sha256"] != root["root_sha256"] or
            node["execution_trust_epoch"] != root["trust_epoch"] or
            node["issuance_trust_root_sha256"] != root["issuance_trust_root_sha256"] or
            node["issuance_trust_epoch"] != root["issuance_trust_epoch"]):
        raise ContractError("ExecutionPolicy root binding is invalid")
    expected = key_for_usage(root, "execution_policy_sign")["key_id"]
    if node["signature"]["key_id"] != expected:
        raise ContractError("ExecutionPolicy must use execution_policy_sign key")


def _validate_policy_issued_relations(node: dict[str, Any], issued: dict[str, Any]) -> None:
    grant, receipt = issued["grant"], issued["receipt"]
    exact = {
        "bindings": {key: grant["bindings"][key]
                     for key in ("context_sha256", "source_revision", "source_tree_sha256")},
        "capability": grant["capability"], "subject": grant["subject"],
        "task_binding": grant["task_binding"], "grant_id": grant["grant_id"],
        "grant_sha256": grant["grant_sha256"],
        "grant_envelope_sha256": grant_envelope_sha256(grant),
        "grant_issuance_receipt_sha256": receipt["receipt_sha256"],
        "grant_issuance_ledger_sequence": receipt["ledger_sequence"],
        "grant_policy_sha256": grant["bindings"]["policy_sha256"],
        "grant_request_sha256": grant["bindings"]["grant_request_sha256"],
    }
    for field, expected in exact.items():
        if node[field] != expected:
            raise ContractError(f"ExecutionPolicy does not bind issued Grant field {field}")
    if not budget_covers(grant["budget"], node["budget"]):
        raise ContractError("ExecutionPolicy budget exceeds issued Grant")


def _validate_policy_action(node: dict[str, Any], manifest: dict[str, Any],
                            grant: dict[str, Any]) -> None:
    action = node["requested_action"]
    grant_resources = grant["scope"]["allow"][0]["resources"]
    if action["resources"] != grant_resources:
        raise ContractError("requested_action must equal the complete Grant exact path set")
    if paths_from_action(action) != manifest_paths(manifest):
        raise ContractError("requested_action paths differ from expected manifest")
    if node["manifest_sha256"] != manifest["manifest_sha256"]:
        raise ContractError("ExecutionPolicy does not bind expected manifest")
    if action["usage"]["output_bytes"] != manifest_content_bytes(manifest):
        raise ContractError("requested output bytes must equal manifest raw byte total")
    if node["requested_action_sha256"] != action_sha256(action):
        raise ContractError("ExecutionPolicy requested_action digest does not match")
    validate_budget_covers_action(node["budget"], action, "policy.budget")


def _validate_policy_time(node: dict[str, Any], grant: dict[str, Any]) -> None:
    validity = require_keys(node["validity"], "policy.validity",
                            {"expires_at_unix_ms", "not_before_unix_ms"})
    start = integer(validity["not_before_unix_ms"], "policy.not_before", 0, 2**63 - 1)
    end = integer(validity["expires_at_unix_ms"], "policy.expires", 0, 2**63 - 1)
    grant_validity = grant["validity"]
    if end <= start or end - start > MAX_FRESHNESS_MS:
        raise ContractError("ExecutionPolicy validity must be ordered within five minutes")
    if start < grant_validity["not_before_unix_ms"] or end > grant_validity["expires_at_unix_ms"]:
        raise ContractError("ExecutionPolicy validity is outside issued Grant")


def validate_invocation(value: Any, profile_hash: str, execution_root: dict[str, Any],
                        policy: dict[str, Any], manifest: dict[str, Any],
                        issued: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadInvocation", INVOCATION_FIELDS)
    bounded_canonical_json(node, MAX_INVOCATION_BYTES, "BootstrapRepoReadInvocation")
    _validate_invocation_shape(node, profile_hash)
    _validate_invocation_relations(node, execution_root, policy, manifest, issued)
    _validate_invocation_time(node, policy, issued["grant"])
    digest = invocation_sha256(node)
    if node["invocation_sha256"] != digest or node["invocation_id"] != "bootstrap-repo-read-invocation-" + digest:
        raise ContractError("Invocation digest-derived identity does not match")
    return node


def _validate_invocation_shape(node: dict[str, Any], profile_hash: str) -> None:
    expected = (INVOCATION_API, CANONICALIZATION, "BootstrapRepoReadInvocation", PROFILE_ID)
    if (node["api_version"], node["canonicalization"], node["kind"],
            node["profile_id"]) != expected:
        raise ContractError("Invocation envelope drifted from v1")
    validate_idempotency(node["idempotency_key"], "invocation.idempotency_key")
    validate_bindings(node["bindings"], "invocation.bindings")
    validate_capability(node["capability"], "invocation.capability")
    validate_principal(node["subject"], "invocation.subject")
    validate_task_binding(node["task_binding"], "invocation.task_binding")
    validate_action(node["requested_action"], "invocation.requested_action")
    for field in _invocation_hash_fields():
        sha256(node[field], f"invocation.{field}")
    integer(node["grant_issuance_ledger_sequence"], "grant_issuance_ledger_sequence", 1, 256)
    validate_signature(node["signature"], "invocation.signature", profile_hash)


def _invocation_hash_fields() -> tuple[str, ...]:
    return ("execution_policy_sha256", "execution_trust_root_sha256",
            "grant_envelope_sha256", "grant_issuance_receipt_sha256",
            "grant_policy_sha256", "grant_request_sha256", "grant_sha256",
            "invocation_sha256", "issuance_trust_root_sha256", "manifest_sha256",
            "requested_action_sha256")


def _validate_invocation_relations(node: dict[str, Any], root: dict[str, Any],
                                   policy: dict[str, Any], manifest: dict[str, Any],
                                   issued: dict[str, Any]) -> None:
    fields = ("bindings", "capability", "execution_trust_epoch",
              "execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
              "grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256",
              "grant_policy_sha256", "grant_request_sha256", "grant_sha256",
              "idempotency_key", "issuance_trust_epoch", "issuance_trust_root_sha256",
              "manifest_sha256", "requested_action", "requested_action_sha256", "subject",
              "task_binding")
    if any(node[field] != policy[field] for field in fields):
        raise ContractError("Invocation differs from exact signed ExecutionPolicy")
    if node["execution_policy_sha256"] != policy["execution_policy_sha256"]:
        raise ContractError("Invocation does not bind exact ExecutionPolicy")
    if policy["disposition"] != "allow":
        raise ContractError("denied ExecutionPolicy cannot be invoked")
    if node["manifest_sha256"] != manifest["manifest_sha256"]:
        raise ContractError("Invocation does not bind expected manifest")
    if node["grant_id"] != issued["grant"]["grant_id"]:
        raise ContractError("Invocation does not bind issued Grant")
    expected_key = key_for_usage(root, "execution_request_auth")
    if node["signature"]["key_id"] != expected_key["key_id"]:
        raise ContractError("Invocation must use execution_request_auth key")
    if node["subject"] != expected_key["principal"]:
        raise ContractError("execution request principal must equal Grant subject")


def _validate_invocation_time(node: dict[str, Any], policy: dict[str, Any],
                              grant: dict[str, Any]) -> None:
    start = integer(node["requested_at_unix_ms"], "invocation.requested_at", 0, 2**63 - 1)
    end = integer(node["expires_at_unix_ms"], "invocation.expires_at", 0, 2**63 - 1)
    policy_time, grant_time = policy["validity"], grant["validity"]
    if end <= start or end - start > MAX_FRESHNESS_MS:
        raise ContractError("Invocation freshness must be ordered within five minutes")
    if (start < policy_time["not_before_unix_ms"] or end > policy_time["expires_at_unix_ms"] or
            start < grant_time["not_before_unix_ms"] or end > grant_time["expires_at_unix_ms"]):
        raise ContractError("Invocation freshness is outside Policy or Grant")


__all__ = [
    "invocation_sha256", "policy_sha256", "record_key_sha256", "validate_invocation",
    "validate_policy",
]
