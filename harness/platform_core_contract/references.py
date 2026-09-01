"""Actor, entity, record, and product scope references."""

from __future__ import annotations

import re
from typing import Any

from . import identity
from .codec import (ContractError, exact_object, validate_hash,
                    validate_schema_name, validate_text)
from .constants import REFERENCE_MISMATCH, SCOPE_FIELDS

_ENTITY_FIELDS = {"entity_id", "entity_type"}
_ACTOR_FIELDS = {"actor_id", "actor_type"}
_RECORD_FIELDS = {"record_id", "record_sha256", "record_type"}
_ACTOR_TYPES = {"agent", "harness", "human", "service", "system"}


def validate_entity_ref(value: Any, label: str) -> dict[str, Any]:
    """Return one exact typed EntityRef."""
    node = exact_object(value, _ENTITY_FIELDS, label)
    identity.validate_entity_id(node["entity_type"], node["entity_id"], label)
    return node


def validate_actor_ref(value: Any) -> dict[str, Any]:
    """Return one exact caller-declared ActorRef."""
    node = exact_object(value, _ACTOR_FIELDS, "actor_ref")
    identity.validate_typed_id(node["actor_id"], "acr", "actor_ref.actor_id")
    if type(node["actor_type"]) is not str or node["actor_type"] not in _ACTOR_TYPES:
        raise ContractError("actor_ref.actor_type is unsupported")
    return node


def validate_record_ref(value: Any, label: str) -> dict[str, Any]:
    """Return one exact unresolved RecordRef."""
    node = exact_object(value, _RECORD_FIELDS, label)
    record_id = validate_text(node["record_id"], f"{label}.record_id", 160)
    if re.fullmatch(r"[a-z][a-z0-9._:/-]*", record_id) is None:
        raise ContractError(f"{label}.record_id has invalid opaque identifier text")
    validate_hash(node["record_sha256"], f"{label}.record_sha256")
    validate_schema_name(node["record_type"], f"{label}.record_type")
    return node


def validate_scope(value: Any) -> dict[str, Any]:
    """Return one exact contiguous product ScopeRef."""
    node = exact_object(value, SCOPE_FIELDS, "scope_ref")
    validate_scope_values(node)
    validate_scope_references(node)
    return node


def validate_scope_values(node: dict[str, Any]) -> None:
    """Validate only ScopeRef identifiers and field vocabulary."""
    identity.validate_typed_id(node["space_id"], "spc", "scope_ref.space_id")
    prefixes = {
        "action_id": "act", "attempt_id": "atm", "change_id": "chg",
        "objective_id": "obj", "project_id": "prj",
        "project_snapshot_id": "psn", "session_id": "ses", "turn_id": "trn",
        "work_graph_id": "wgr", "work_item_id": "wki",
    }
    for field, prefix in prefixes.items():
        if node[field] is not None:
            identity.validate_typed_id(node[field], prefix, f"scope_ref.{field}")


def validate_scope_references(node: dict[str, Any]) -> None:
    """Validate parent/child bindings after all field values are valid."""
    _validate_ancestry(node)


def scope_contains(scope: dict[str, Any], reference: dict[str, Any]) -> bool:
    """Whether one scoped product identity equals the supplied EntityRef."""
    field_by_type = {
        "action": "action_id", "attempt": "attempt_id", "change": "change_id",
        "objective": "objective_id", "project": "project_id",
        "project_snapshot": "project_snapshot_id", "session": "session_id",
        "turn": "turn_id", "work_graph": "work_graph_id", "work_item": "work_item_id",
    }
    if reference["entity_type"] == "space":
        return reference["entity_id"] == scope["space_id"]
    field = field_by_type.get(reference["entity_type"])
    return field is not None and reference["entity_id"] == scope[field]


def _validate_ancestry(scope: dict[str, Any]) -> None:
    links = (
        ("project_snapshot_id", "project_id"), ("change_id", "objective_id"),
        ("work_graph_id", "change_id"), ("work_item_id", "work_graph_id"),
        ("attempt_id", "work_item_id"), ("session_id", "attempt_id"),
        ("turn_id", "session_id"), ("action_id", "turn_id"),
    )
    for child, parent in links:
        if scope[child] is not None and scope[parent] is None:
            raise ContractError(f"scope_ref.{child} requires {parent}",
                                REFERENCE_MISMATCH)
