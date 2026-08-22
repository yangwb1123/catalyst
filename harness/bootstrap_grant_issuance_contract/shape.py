"""Closed reusable shapes for authenticated bootstrap Grant issuance."""

from __future__ import annotations

import base64
from typing import Any

from capability_grant_contract.shape import canonical_path

from .canonical import ContractError, canonical_json
from .constants import (CONTRACT_PROFILE_ID, MAX_OUTPUT_BYTES, MAX_TIMEOUT_MS,
                        SIGNATURE_PROFILE_ID)


def require_keys(value: Any, label: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ContractError(f"{label} fields must be exactly {sorted(keys)!r}")
    return value


def text(value: Any, label: str, maximum: int = 160) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= maximum:
        raise ContractError(f"{label} must be non-empty text of at most {maximum} bytes")
    return value


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if not isinstance(value, str) or value not in allowed:
        raise ContractError(f"{label} must be one of {allowed!r}")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise ContractError(f"{label} must be 64 lowercase hex characters") from error
    if len(decoded) != 32 or decoded.hex() != value:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return value


def integer(value: Any, label: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def fixed_base64url(value: Any, label: str, raw_bytes: int, encoded_chars: int) -> str:
    if not isinstance(value, str) or len(value) != encoded_chars or "=" in value:
        raise ContractError(f"{label} must be {encoded_chars}-character unpadded base64url")
    try:
        decoded = base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_",
                                   validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ContractError(f"{label} is not canonical unpadded base64url") from error
    canonical = base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=")
    if len(decoded) != raw_bytes or canonical != value:
        raise ContractError(f"{label} must encode exactly {raw_bytes} bytes")
    return value


def validate_signature(value: Any, label: str, profile_sha256: str) -> dict[str, Any]:
    keys = {"key_id", "profile_id", "profile_sha256", "signature_base64url"}
    node = require_keys(value, label, keys)
    text(node["key_id"], f"{label}.key_id")
    if node["profile_id"] != SIGNATURE_PROFILE_ID:
        raise ContractError(f"{label}.profile_id is not the frozen Ed25519 profile")
    if sha256(node["profile_sha256"], f"{label}.profile_sha256") != profile_sha256:
        raise ContractError(f"{label} does not bind the frozen signature profile")
    fixed_base64url(node["signature_base64url"], f"{label}.signature_base64url", 64, 86)
    return node


def validate_principal(value: Any, label: str) -> dict[str, Any]:
    node = require_keys(value, label, {"authority_domain", "principal_id", "principal_type"})
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    enum(node["principal_type"], f"{label}.principal_type", ("agent", "service"))
    return node


def principal_identity(value: dict[str, Any]) -> bytes:
    return canonical_json(value)


def validate_capability(value: Any, label: str) -> dict[str, Any]:
    keys = {"capability_contract_sha256", "capability_id", "capability_version"}
    node = require_keys(value, label, keys)
    sha256(node["capability_contract_sha256"], f"{label}.capability_contract_sha256")
    if node["capability_id"] != "repository-reader" or node["capability_version"] != "1":
        raise ContractError(f"{label} must be exact repository-reader/v1")
    return node


def validate_task_binding(value: Any, label: str) -> dict[str, Any]:
    keys = {"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
            "project_id", "role", "run_id", "target_id", "task_id"}
    node = require_keys(value, label, keys)
    if node["attempt_id"] is not None or node["target_id"] is not None:
        raise ContractError(f"{label} attempt_id and target_id must be null")
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "local", "test"))
    for field in keys - {"attempt_id", "environment_class", "target_id"}:
        text(node[field], f"{label}.{field}")
    return node


def validate_budget(value: Any, label: str) -> dict[str, Any]:
    keys = {"max_calls", "max_cost_usd_micros", "max_input_tokens", "max_network_bytes",
            "max_output_bytes", "max_output_tokens", "timeout_ms"}
    node = require_keys(value, label, keys)
    expected = {"max_calls": 1, "max_cost_usd_micros": 0, "max_input_tokens": 0,
                "max_network_bytes": 0, "max_output_tokens": 0}
    if any(node[field] != wanted for field, wanted in expected.items()):
        raise ContractError(f"{label} violates the bootstrap read-only hard budget")
    integer(node["max_output_bytes"], f"{label}.max_output_bytes", 0, MAX_OUTPUT_BYTES)
    integer(node["timeout_ms"], f"{label}.timeout_ms", 1, MAX_TIMEOUT_MS)
    return node


def budget_covers(policy: dict[str, Any], request: dict[str, Any]) -> bool:
    return (request["max_output_bytes"] <= policy["max_output_bytes"] and
            request["timeout_ms"] <= policy["timeout_ms"] and
            all(request[field] == policy[field] for field in policy
                if field not in {"max_output_bytes", "timeout_ms"}))


def validate_scope(value: Any, label: str) -> dict[str, Any]:
    node = require_keys(value, label, {"allow", "deny", "effect_id"})
    if node["effect_id"] != "repo.read" or node["deny"] != []:
        raise ContractError(f"{label} must be repo.read with an empty deny list")
    if not isinstance(node["allow"], list) or len(node["allow"]) != 1:
        raise ContractError(f"{label}.allow must contain exactly one clause")
    clause = require_keys(node["allow"][0], f"{label}.allow[0]", {"resources"})
    resources = clause["resources"]
    if not isinstance(resources, list) or not 1 <= len(resources) <= 16:
        raise ContractError(f"{label} must contain 1..16 exact repository paths")
    for index, resource in enumerate(resources):
        _validate_repo_path(resource, f"{label}.allow[0].resources[{index}]")
    encoded = [canonical_json(resource) for resource in resources]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} resources must be canonical-byte sorted and unique")
    return node


def _validate_repo_path(value: Any, label: str) -> None:
    node = require_keys(value, label, {"match", "path", "scope_kind"})
    if node["match"] != "exact" or node["scope_kind"] != "repo_path":
        raise ContractError(f"{label} must be an exact repo_path")
    canonical_path(node["path"], f"{label}.path", False)


def validate_request_bindings(value: Any, label: str) -> dict[str, Any]:
    node = require_keys(value, label,
                        {"context_sha256", "source_revision", "source_tree_sha256"})
    sha256(node["context_sha256"], f"{label}.context_sha256")
    text(node["source_revision"], f"{label}.source_revision", 160)
    sha256(node["source_tree_sha256"], f"{label}.source_tree_sha256")
    return node


def validate_profile_id(value: Any, label: str) -> None:
    if value != CONTRACT_PROFILE_ID:
        raise ContractError(f"{label} must be {CONTRACT_PROFILE_ID!r}")
