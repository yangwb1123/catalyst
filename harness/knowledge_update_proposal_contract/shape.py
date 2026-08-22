"""Closed primitive and nested shapes shared by KnowledgeUpdateProposal objects."""

from __future__ import annotations

import re
from typing import Any

from .canonical import ContractError, canonical_json
from .constants import (MAX_ARTIFACTS, MAX_MUTATION_REASONS, MAX_REFERENCE_BYTES,
                        MAX_SHORT_BYTES)


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
    if re.fullmatch(r"[a-z][a-z0-9._:/-]{0,159}", result) is None:
        raise ContractError(f"{label} must be a lowercase stable identifier")
    return result


def identifier(value: Any, label: str) -> str:
    result = text(value, label)
    if re.fullmatch(r"[a-z0-9][a-z0-9._:/-]{0,159}", result) is None:
        raise ContractError(f"{label} must be an ADR-0045 identifier")
    return result


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if not isinstance(value, str) or value not in allowed:
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


def reasons(value: Any, label: str, minimum: int = 0,
            maximum: int = MAX_MUTATION_REASONS) -> list[str]:
    nodes = array(value, label, minimum, maximum)
    for index, reason in enumerate(nodes):
        identifier(reason, f"{label}[{index}]")
    if any(left.encode() >= right.encode() for left, right in zip(nodes, nodes[1:])):
        raise ContractError(f"{label} must be strictly UTF-8 sorted and unique")
    return nodes


def validate_principal(value: Any, label: str = "proposer") -> dict[str, Any]:
    node = require_keys(value, label, {"authority_domain", "principal_id", "principal_type"})
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    enum(node["principal_type"], f"{label}.principal_type",
         ("agent", "human", "operator", "service"))
    return node


def validate_task_binding(value: Any, label: str = "task_binding") -> dict[str, Any]:
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
    return node


def validate_artifacts(value: Any, label: str = "bindings.artifacts") -> list[Any]:
    nodes = array(value, label, 0, MAX_ARTIFACTS)
    fields = {"artifact_kind", "artifact_ref", "artifact_sha256"}
    for index, value in enumerate(nodes):
        item = require_keys(value, f"{label}[{index}]", fields)
        text(item["artifact_kind"], f"{label}[{index}].artifact_kind")
        text(item["artifact_ref"], f"{label}[{index}].artifact_ref", MAX_REFERENCE_BYTES)
        sha256(item["artifact_sha256"], f"{label}[{index}].artifact_sha256")
    sorted_nodes(nodes, label)
    return nodes


def validate_bindings(value: Any, label: str = "bindings") -> dict[str, Any]:
    fields = {"artifacts", "context_sha256", "impact_sha256", "plan_sha256",
              "policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
    node = require_keys(value, label, fields)
    validate_artifacts(node["artifacts"], f"{label}.artifacts")
    for field in ("context_sha256", "policy_sha256", "source_tree_sha256"):
        sha256(node[field], f"{label}.{field}")
    for field in ("impact_sha256", "plan_sha256", "risk_sha256"):
        nullable_sha256(node[field], f"{label}.{field}")
    text(node["source_revision"], f"{label}.source_revision")
    return node


def validate_grant_ref(value: Any, label: str = "capability_grant_ref") -> dict[str, Any]:
    node = require_keys(value, label, {"authority_domain", "grant_id", "grant_sha256"})
    text(node["authority_domain"], f"{label}.authority_domain")
    sha256(node["grant_sha256"], f"{label}.grant_sha256")
    if node["grant_id"] != f"capability-grant-{node['grant_sha256']}":
        raise ContractError(f"{label} identity is inconsistent")
    return node


def validate_knowledge_scope(value: Any, label: str = "knowledge_scope") -> dict[str, Any]:
    fields = {"object_kind", "object_ref", "object_scope_sha256", "scope_kind"}
    node = require_keys(value, label, fields)
    if node["scope_kind"] != "governance_object" or node["object_kind"] != "knowledge":
        raise ContractError(f"{label} must be a knowledge governance_object")
    identifier(node["object_ref"], f"{label}.object_ref")
    sha256(node["object_scope_sha256"], f"{label}.object_scope_sha256")
    return node


def validate_claim_ref(value: Any, label: str, nullable: bool = False) -> dict[str, Any] | None:
    if nullable and value is None:
        return None
    node = require_keys(value, label, {"canonical_sha256", "record_id"})
    sha256(node["canonical_sha256"], f"{label}.canonical_sha256")
    identifier(node["record_id"], f"{label}.record_id")
    return node
