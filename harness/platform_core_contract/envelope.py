"""Shared staged Command/Event envelope validation."""

from __future__ import annotations

from typing import Any

from .artifact import (_validate_artifact_references,
                       _validate_artifact_relations,
                       _validate_artifact_values)
from .codec import (ContractError, canonical_json, lower_token, validate_i64,
                    validate_schema_name)
from .constants import (CANONICALIZATION, ENVELOPE_VERSION,
                        DOCUMENT_INVALID, MAX_EXTENSION_FIELDS, MAX_EXTENSIONS_BYTES,
                        MAX_PAYLOAD_BYTES, REFERENCE_MISMATCH,
                        RELATION_MISMATCH)
from .identity import validate_typed_id
from .references import (validate_actor_ref, validate_scope_references,
                         validate_scope_values)


def validate_common_values(node: dict[str, Any]) -> None:
    """Validate shared identifiers and independent field values."""
    if (node["canonicalization"] != CANONICALIZATION or
            validate_i64(node["envelope_version"], "envelope_version", 1)
            != ENVELOPE_VERSION):
        raise ContractError("envelope canonicalization or envelope_version is unsupported")
    validate_i64(node["schema_version"], "schema_version", 1)
    validate_typed_id(node["message_id"], "msg", "message_id")
    validate_typed_id(node["correlation_id"], "cor", "correlation_id")
    if node["causation_id"] is not None:
        validate_typed_id(node["causation_id"], "msg", "causation_id")
    validate_schema_name(node["schema_name"], "schema_name")
    validate_actor_ref(node["actor_ref"])
    validate_scope_values(node["scope_ref"])
    _validate_body_values(node)


def _validate_body_values(node: dict[str, Any]) -> None:
    payload, artifact = node["payload"], node["payload_artifact_ref"]
    if payload is not None:
        canonical_json(payload, MAX_PAYLOAD_BYTES)
    if artifact is not None:
        _validate_artifact_values(artifact)
    _validate_extensions(node["extensions"])


def validate_common_references(node: dict[str, Any]) -> None:
    """Validate scope ancestry and nested Artifact bindings."""
    validate_scope_references(node["scope_ref"])
    artifact = node["payload_artifact_ref"]
    if artifact is None:
        return
    _validate_artifact_references(artifact)
    scope = node["scope_ref"]
    if artifact["source_snapshot_ref"]["entity_id"] != scope["project_snapshot_id"]:
        raise ContractError("payload_artifact_ref source snapshot must match scope",
                            REFERENCE_MISMATCH)
    if artifact["producer_attempt_id"] != scope["attempt_id"]:
        raise ContractError("payload_artifact_ref producer attempt must match scope",
                            REFERENCE_MISMATCH)


def validate_common_relations(node: dict[str, Any]) -> None:
    """Validate shared cross-field relations after all references."""
    if node["causation_id"] == node["message_id"]:
        raise ContractError("causation_id must not equal message_id",
                            RELATION_MISMATCH)
    payload, artifact = node["payload"], node["payload_artifact_ref"]
    if (payload is None) == (artifact is None):
        raise ContractError("exactly one of payload or payload_artifact_ref is required",
                            RELATION_MISMATCH)
    if artifact is not None:
        _validate_artifact_relations(artifact)


def validate_idempotency_key(value: Any) -> None:
    """Validate one bounded visible-ASCII idempotency key."""
    if type(value) is not str or not 16 <= len(value) <= 128:
        raise ContractError("idempotency_key must have 16..128 visible ASCII bytes")
    if any(not "!" <= character <= "~" for character in value):
        raise ContractError("idempotency_key must contain visible ASCII only")


def _validate_extensions(value: dict[str, Any]) -> None:
    if len(value) > MAX_EXTENSION_FIELDS:
        raise ContractError(f"extensions exceeds {MAX_EXTENSION_FIELDS} fields")
    for key in value:
        if type(key) is not str:
            raise ContractError("extension keys must be text", DOCUMENT_INVALID)
        parts = key.split(".")
        if (len(parts) != 2 or not 2 <= len(parts[0]) <= 32 or
                not 1 <= len(parts[1]) <= 64 or
                any(not lower_token(part) for part in parts)):
            raise ContractError(f"extension key {key!r} is not a bounded namespace")
    canonical_json(value, MAX_EXTENSIONS_BYTES)
