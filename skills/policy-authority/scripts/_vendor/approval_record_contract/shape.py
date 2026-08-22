"""Closed primitive and nested shapes for ApprovalRecord v1."""

from __future__ import annotations

import base64
import re
from typing import Any

from .canonical import ContractError, canonical_json
from .constants import MAX_PROOF_BYTES, MAX_SHORT_BYTES


def require_keys(value: Any, label: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    if set(value) != keys:
        raise ContractError(f"{label} fields must be exactly {sorted(keys)!r}")
    return value


def text(value: Any, label: str, maximum: int = MAX_SHORT_BYTES) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= maximum:
        raise ContractError(f"{label} must be non-empty text of at most {maximum} bytes")
    return value


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if value not in allowed:
        raise ContractError(f"{label} must be one of {allowed!r}")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return value


def integer(value: Any, label: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def array(value: Any, label: str, minimum: int, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} item count must be {minimum}..{maximum}")
    return value


def sorted_unique_nodes(values: list[Any], label: str) -> None:
    encoded = [canonical_json(value) for value in values]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")


def sorted_unique_strings(values: Any, label: str, allowed: tuple[str, ...],
                          minimum: int = 1) -> list[str]:
    result = array(values, label, minimum, len(allowed))
    if any(not isinstance(value, str) or value not in allowed for value in result):
        raise ContractError(f"{label} contains an unsupported value")
    if any(left.encode() >= right.encode() for left, right in zip(result, result[1:])):
        raise ContractError(f"{label} must be strictly UTF-8 sorted and unique")
    return result


def validate_identifier(value: Any, label: str) -> str:
    result = text(value, label)
    if re.fullmatch(r"[a-z][a-z0-9._-]{0,159}", result) is None:
        raise ContractError(f"{label} must be a lowercase stable identifier")
    return result


def validate_base64url(value: Any, label: str) -> str:
    encoded = text(value, label, MAX_PROOF_BYTES)
    if len(encoded) < 16 or re.fullmatch(r"[A-Za-z0-9_-]+", encoded) is None:
        raise ContractError(f"{label} must be canonical unpadded base64url text")
    try:
        decoded = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
    except ValueError as error:
        raise ContractError(f"{label} is not base64url") from error
    if base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=") != encoded:
        raise ContractError(f"{label} is not canonical unpadded base64url")
    return encoded


def validate_principal(value: Any, label: str,
                       allowed: tuple[str, ...] = ("agent", "human", "operator", "service")) -> None:
    node = require_keys(value, label, {"authority_domain", "principal_id", "principal_type"})
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    enum(node["principal_type"], f"{label}.principal_type", allowed)


def principal_key(value: dict[str, Any]) -> tuple[str, str, str]:
    return value["authority_domain"], value["principal_id"], value["principal_type"]


def validate_principal_array(value: Any, label: str, maximum: int = 32) -> list[dict[str, Any]]:
    nodes = array(value, label, 0, maximum)
    for index, node in enumerate(nodes):
        validate_principal(node, f"{label}[{index}]")
    sorted_unique_nodes(nodes, label)
    return nodes


def validate_artifacts(value: Any, label: str = "bindings.artifacts") -> list[dict[str, Any]]:
    nodes = array(value, label, 1, 32)
    keys = {"artifact_kind", "artifact_ref", "artifact_sha256"}
    for index, item in enumerate(nodes):
        node = require_keys(item, f"{label}[{index}]", keys)
        text(node["artifact_kind"], f"{label}[{index}].artifact_kind")
        text(node["artifact_ref"], f"{label}[{index}].artifact_ref", 4096)
        sha256(node["artifact_sha256"], f"{label}[{index}].artifact_sha256")
    sorted_unique_nodes(nodes, label)
    return nodes


def validate_ref_array(value: Any, label: str, kind: str) -> list[dict[str, Any]]:
    nodes = array(value, label, 0, 32)
    fields = ({"condition_id", "condition_ref", "condition_sha256"} if kind == "condition"
              else {"authority_domain", "risk_acceptance_id", "risk_acceptance_sha256"})
    for index, item in enumerate(nodes):
        node = require_keys(item, f"{label}[{index}]", fields)
        if kind == "condition":
            text(node["condition_id"], f"{label}[{index}].condition_id")
            text(node["condition_ref"], f"{label}[{index}].condition_ref", 4096)
            sha256(node["condition_sha256"], f"{label}[{index}].condition_sha256")
        else:
            text(node["authority_domain"], f"{label}[{index}].authority_domain")
            text(node["risk_acceptance_id"], f"{label}[{index}].risk_acceptance_id")
            sha256(node["risk_acceptance_sha256"],
                   f"{label}[{index}].risk_acceptance_sha256")
    sorted_unique_nodes(nodes, label)
    return nodes

