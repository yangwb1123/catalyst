"""Closed reusable shapes for ADR-0058."""

from __future__ import annotations

import base64
import hashlib
import re
from typing import Any

from bootstrap_grant_issuance_contract.shape import (fixed_base64url, integer,
                                                      principal_identity, require_keys,
                                                      sha256, text, validate_budget,
                                                      validate_capability,
                                                      validate_principal,
                                                      validate_signature,
                                                      validate_task_binding)
from capability_grant_contract.canonical import digest
from capability_grant_contract.constants import ACTION_DOMAIN
from capability_grant_contract.scope import validate_requested_action
from capability_grant_contract.shape import canonical_path

from .canonical import ContractError, canonical_json
from .constants import (CANONICALIZATION, MAX_OUTPUT_BYTES, MAX_TIMEOUT_MS,
                        PROFILE_ID)

IDEMPOTENCY = re.compile(r"^[A-Za-z0-9._:@+\-]{16,128}$")


def validate_profile(value: Any, label: str) -> None:
    if value != PROFILE_ID:
        raise ContractError(f"{label} must be {PROFILE_ID!r}")


def validate_envelope(node: dict[str, Any], api: str, kind: str, label: str) -> None:
    expected = (api, CANONICALIZATION, kind, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")


def validate_bindings(value: Any, label: str) -> dict[str, Any]:
    fields = {"context_sha256", "source_revision", "source_tree_sha256"}
    node = require_keys(value, label, fields)
    sha256(node["context_sha256"], f"{label}.context_sha256")
    text(node["source_revision"], f"{label}.source_revision", 160)
    sha256(node["source_tree_sha256"], f"{label}.source_tree_sha256")
    return node


def validate_idempotency(value: Any, label: str) -> str:
    if not isinstance(value, str) or IDEMPOTENCY.fullmatch(value) is None:
        raise ContractError(f"{label} must be 16..128 bytes of closed visible ASCII")
    return value


def record_key_sha256(value: Any) -> str:
    key = validate_idempotency(value, "idempotency_key")
    from .constants import RECORD_KEY_DOMAIN
    return hashlib.sha256(RECORD_KEY_DOMAIN + key.encode("ascii")).hexdigest()


def validate_action(value: Any, label: str) -> dict[str, Any]:
    node = validate_requested_action(value)
    if node["effect_id"] != "repo.read" or not 1 <= len(node["resources"]) <= 16:
        raise ContractError(f"{label} must contain 1..16 repo.read resources")
    usage = node["usage"]
    fixed = {"call_count": 1, "cost_usd_micros": 0, "input_tokens": 0,
             "network_bytes": 0, "output_tokens": 0}
    if any(usage[field] != expected for field, expected in fixed.items()):
        raise ContractError(f"{label} violates the bootstrap read-only usage profile")
    integer(usage["output_bytes"], f"{label}.usage.output_bytes", 0, MAX_OUTPUT_BYTES)
    integer(usage["timeout_ms"], f"{label}.usage.timeout_ms", 1, MAX_TIMEOUT_MS)
    for index, resource in enumerate(node["resources"]):
        if resource["match"] != "exact" or resource["scope_kind"] != "repo_path":
            raise ContractError(f"{label}.resources[{index}] must be an exact repo_path")
    return node


def action_sha256(value: Any) -> str:
    return digest(ACTION_DOMAIN, validate_action(value, "requested_action"))


def validate_observed_usage(value: Any, label: str) -> dict[str, Any]:
    fields = {"call_count", "cost_usd_micros", "elapsed_ms", "input_tokens",
              "network_bytes", "output_bytes", "output_tokens"}
    node = require_keys(value, label, fields)
    fixed = {"call_count": 1, "cost_usd_micros": 0, "input_tokens": 0,
             "network_bytes": 0, "output_tokens": 0}
    if any(node[field] != expected for field, expected in fixed.items()):
        raise ContractError(f"{label} violates the observed repo-read usage profile")
    integer(node["elapsed_ms"], f"{label}.elapsed_ms", 0, MAX_TIMEOUT_MS)
    integer(node["output_bytes"], f"{label}.output_bytes", 0, MAX_OUTPUT_BYTES)
    return node


def decode_raw_base64url(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or "=" in value:
        raise ContractError(f"{label} must be canonical unpadded base64url")
    try:
        raw = base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_",
                               validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ContractError(f"{label} must be canonical unpadded base64url") from error
    if base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=") != value:
        raise ContractError(f"{label} must be canonical unpadded base64url")
    return raw


def validate_path(value: Any, label: str) -> str:
    result = canonical_path(value, label, False)
    if len(result.split("/")) > 256:
        raise ContractError(f"{label} exceeds 256 path components")
    if result.split("/", 1)[0].casefold() in {".git", ".forge"}:
        raise ContractError(f"{label} cannot enter repository control directories")
    return result


def paths_from_action(action: dict[str, Any]) -> list[str]:
    return [resource["path"] for resource in action["resources"]]


def validate_budget_covers_action(budget: dict[str, Any], action: dict[str, Any],
                                  label: str) -> None:
    validate_budget(budget, label)
    usage = action["usage"]
    pairs = (("max_calls", "call_count"), ("max_cost_usd_micros", "cost_usd_micros"),
             ("max_input_tokens", "input_tokens"), ("max_network_bytes", "network_bytes"),
             ("max_output_bytes", "output_bytes"), ("max_output_tokens", "output_tokens"),
             ("timeout_ms", "timeout_ms"))
    if any(usage[used] > budget[maximum] for maximum, used in pairs):
        raise ContractError(f"{label} does not cover requested_action usage")


__all__ = [
    "ContractError", "action_sha256", "canonical_json", "decode_raw_base64url",
    "fixed_base64url", "integer", "paths_from_action", "principal_identity",
    "record_key_sha256", "require_keys", "sha256", "text", "validate_action",
    "validate_bindings", "validate_budget", "validate_budget_covers_action",
    "validate_capability", "validate_envelope", "validate_idempotency",
    "validate_observed_usage", "validate_path", "validate_principal",
    "validate_profile", "validate_signature", "validate_task_binding",
]
