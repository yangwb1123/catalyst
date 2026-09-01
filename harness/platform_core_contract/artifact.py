"""Pure ArtifactRef v1 validation."""

from __future__ import annotations

import re
from typing import Any

from .codec import (ContractError, exact_object, lower_token,
                    validate_document_shape, validate_hash, validate_text)
from .constants import (ARTIFACT_FIELDS, ARTIFACT_TYPE_PROFILE,
                        CANONICALIZATION, MAX_ARTIFACT_BYTES,
                        MAX_UNIX_MILLISECONDS, REFERENCE_MISMATCH,
                        RELATION_MISMATCH)
from .identity import validate_typed_id
from .references import validate_entity_ref, validate_record_ref

_RETENTION = {"audit", "durable", "ephemeral", "legal_hold", "project"}
_SENSITIVITY = {"confidential", "internal", "public", "secret"}
_MEDIA_TYPE = re.compile(r"[a-z0-9][a-z0-9!#$&^_.+-]*/[a-z0-9][a-z0-9!#$&^_.+-]*")


def validate_artifact_ref(value: Any) -> dict[str, Any]:
    """Return one exact structurally valid ArtifactRef declaration."""
    validate_document_shape(value, ARTIFACT_TYPE_PROFILE, "ArtifactRef")
    node = exact_object(value, ARTIFACT_FIELDS, "ArtifactRef")
    _validate_artifact_values(node)
    _validate_artifact_references(node)
    _validate_artifact_relations(node)
    return node


def _validate_artifact_values(node: dict[str, Any]) -> None:
    if node["canonicalization"] != CANONICALIZATION:
        raise ContractError("ArtifactRef canonicalization is unsupported")
    kind = validate_text(node["artifact_kind"], "artifact_kind", 64)
    if not lower_token(kind):
        raise ContractError("artifact_kind must be a lowercase token")
    validate_hash(node["content_digest"], "content_digest")
    validate_typed_id(node["logical_id"], "art", "logical_id")
    _validate_artifact_details(node)


def _validate_artifact_details(node: dict[str, Any]) -> None:
    media_type = validate_text(node["media_type"], "media_type", 128)
    if _MEDIA_TYPE.fullmatch(media_type) is None:
        raise ContractError("media_type must be lowercase type/subtype without parameters")
    validate_typed_id(node["producer_attempt_id"], "atm", "producer_attempt_id")
    validate_entity_ref(node["source_snapshot_ref"], "source_snapshot_ref")
    validate_record_ref(node["provenance_ref"], "provenance_ref")
    _validate_artifact_bounds(node)


def _validate_artifact_references(node: dict[str, Any]) -> None:
    if node["source_snapshot_ref"]["entity_type"] != "project_snapshot":
        raise ContractError("source_snapshot_ref must reference project_snapshot",
                            REFERENCE_MISMATCH)


def _validate_artifact_relations(node: dict[str, Any]) -> None:
    if node["content_id"] != "sha256:" + node["content_digest"]:
        raise ContractError("content_id must equal sha256: plus content_digest",
                            RELATION_MISMATCH)


def _validate_artifact_bounds(node: dict[str, Any]) -> None:
    timestamp = node["created_at_unix_ms"]
    size = node["size_bytes"]
    if type(timestamp) is not int or not 1 <= timestamp <= MAX_UNIX_MILLISECONDS:
        raise ContractError("created_at_unix_ms is outside the supported range")
    if type(size) is not int or not 0 <= size <= MAX_ARTIFACT_BYTES:
        raise ContractError(f"size_bytes must be in 0..{MAX_ARTIFACT_BYTES}")
    if (type(node["retention_class"]) is not str or
            node["retention_class"] not in _RETENTION):
        raise ContractError("retention_class is unsupported")
    if type(node["sensitivity"]) is not str or node["sensitivity"] not in _SENSITIVITY:
        raise ContractError("sensitivity is unsupported")
