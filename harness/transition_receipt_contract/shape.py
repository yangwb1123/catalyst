"""Closed lexical and nested shapes shared by TransitionReceipt objects."""

from __future__ import annotations

import re
from typing import Any

from .canonical import ContractError, canonical_json
from .constants import MAX_REFERENCE_BYTES, MAX_SHORT_BYTES


def require_keys(value: Any, label: str, keys: set[str]) -> dict[str, Any]:
    if (not isinstance(value, dict) or len(value) != len(keys) or
            any(key not in value for key in keys)):
        raise ContractError(f"{label} fields must be exactly {sorted(keys)!r}")
    return value


def text(value: Any, label: str, maximum: int = MAX_SHORT_BYTES) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= maximum:
        raise ContractError(f"{label} must be non-empty text of at most {maximum} bytes")
    return value


def stable(value: Any, label: str) -> str:
    result = text(value, label)
    if re.fullmatch(r"[a-z][a-z0-9._-]{0,159}", result) is None:
        raise ContractError(f"{label} must be a lowercase stable identifier")
    return result


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if value not in allowed:
        raise ContractError(f"{label} must be one of {allowed!r}")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return value


def nullable_sha256(value: Any, label: str) -> str | None:
    return None if value is None else sha256(value, label)


def integer(value: Any, label: str, minimum: int = 0) -> int:
    if (isinstance(value, bool) or not isinstance(value, int) or
            not minimum <= value <= 2**63 - 1):
        raise ContractError(f"{label} must be an integer in {minimum}..{2**63 - 1}")
    return value


def array(value: Any, label: str, minimum: int, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} item count must be {minimum}..{maximum}")
    return value


def sorted_nodes(values: list[Any], label: str) -> None:
    encoded = [canonical_json(value) for value in values]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")


def reasons(value: Any, label: str, maximum: int) -> list[str]:
    nodes = array(value, label, 0, maximum)
    for index, reason in enumerate(nodes):
        stable(reason, f"{label}[{index}]")
    if any(left.encode() >= right.encode() for left, right in zip(nodes, nodes[1:])):
        raise ContractError(f"{label} must be strictly UTF-8 sorted and unique")
    return nodes


def validate_principal(value: Any, label: str, controller: bool = False) -> None:
    node = require_keys(value, label, {"authority_domain", "principal_id", "principal_type"})
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    allowed = ("human", "operator", "service") if controller else (
        "agent", "human", "operator", "service")
    enum(node["principal_type"], f"{label}.principal_type", allowed)


def validate_task_binding(value: Any, label: str = "task_binding") -> None:
    fields = {"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
              "project_id", "role", "run_id", "target_id", "task_id"}
    node = require_keys(value, label, fields)
    for field in ("change_id", "environment_id", "node_id", "project_id", "role", "run_id",
                  "task_id"):
        text(node[field], f"{label}.{field}")
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "local", "production", "staging", "test"))
    for field in ("attempt_id", "target_id"):
        if node[field] is not None:
            text(node[field], f"{label}.{field}")


def validate_artifacts(value: Any, label: str) -> None:
    nodes = array(value, label, 0, 32)
    for index, value in enumerate(nodes):
        item = require_keys(value, f"{label}[{index}]",
                            {"artifact_kind", "artifact_ref", "artifact_sha256"})
        text(item["artifact_kind"], f"{label}[{index}].artifact_kind")
        text(item["artifact_ref"], f"{label}[{index}].artifact_ref", MAX_REFERENCE_BYTES)
        sha256(item["artifact_sha256"], f"{label}[{index}].artifact_sha256")
    sorted_nodes(nodes, label)


def validate_authority_refs(value: Any, label: str, prefix: str) -> None:
    nodes = array(value, label, 0, 32)
    identifier, digest = f"{prefix}_id", f"{prefix}_sha256"
    for index, value in enumerate(nodes):
        item = require_keys(value, f"{label}[{index}]",
                            {"authority_domain", identifier, digest})
        text(item["authority_domain"], f"{label}[{index}].authority_domain")
        text(item[identifier], f"{label}[{index}].{identifier}")
        sha256(item[digest], f"{label}[{index}].{digest}")
    sorted_nodes(nodes, label)


def validate_evidence_refs(value: Any, label: str) -> int:
    nodes = array(value, label, 0, 32)
    for index, value in enumerate(nodes):
        item = require_keys(value, f"{label}[{index}]", {"canonical_sha256", "record_id"})
        sha256(item["canonical_sha256"], f"{label}[{index}].canonical_sha256")
        text(item["record_id"], f"{label}[{index}].record_id")
    sorted_nodes(nodes, label)
    return len(nodes)
