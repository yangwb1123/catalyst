"""Self-sealing and strict decoding for the four operational records."""

from __future__ import annotations

import copy
import hashlib
from collections.abc import Callable

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import (ARTIFACT_RECEIPT_DOMAIN, ARTIFACT_RECEIPT_FIELDS,
                        ARTIFACT_RECEIPT_PREFIX, EVENT_DOMAIN, EVENT_FIELDS,
                        EVENT_PREFIX, EXECUTION_RECEIPT_DOMAIN,
                        EXECUTION_RECEIPT_FIELDS, EXECUTION_RECEIPT_PREFIX,
                        INVOCATION_DOMAIN, INVOCATION_FIELDS, INVOCATION_PREFIX,
                        MAX_ARTIFACT_RECEIPT_BYTES, MAX_ARTIFACT_REF_BYTES,
                        MAX_EVENT_BYTES, MAX_EXECUTION_RECEIPT_BYTES,
                        MAX_INVOCATION_BYTES)
from .shape import (artifact_ref, validate_artifact_receipt_shape,
                    validate_event_shape, validate_execution_receipt_shape,
                    validate_invocation_shape)

Validator = Callable[..., dict[str, object]]


def _blanked(value: object, fields: set[str], id_field: str,
             hash_field: str, label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ContractError(f"{label} digest input must have exact top-level fields")
    result = copy.deepcopy(value)
    result[id_field] = ""
    result[hash_field] = ""
    return result


def _digest(value: object, fields: set[str], id_field: str, hash_field: str,
            label: str, domain: bytes, maximum: int, validator: Validator) -> str:
    blank = _blanked(value, fields, id_field, hash_field, label)
    validator(blank, allow_blank=True)
    raw = canonical_json(blank)
    if len(raw) > maximum:
        raise ContractError(f"{label} blank preimage exceeds {maximum} bytes")
    return hashlib.sha256(domain + raw).hexdigest()


def _validate(value: object, id_field: str, hash_field: str, label: str,
              fields: set[str], domain: bytes, maximum: int,
              validator: Validator) -> dict[str, object]:
    record = validator(value)
    if len(canonical_json(record)) > maximum:
        raise ContractError(f"{label} exceeds {maximum} bytes")
    expected = _digest(record, fields, id_field, hash_field, label,
                       domain, maximum, validator)
    if record[hash_field] != expected:
        raise ContractError(f"{hash_field} does not match the canonical preimage")
    return record


def _seal(value: object, id_field: str, hash_field: str, prefix: str,
          label: str, fields: set[str], domain: bytes, maximum: int,
          validator: Validator) -> dict[str, object]:
    record = copy.deepcopy(value)
    if not isinstance(record, dict):
        raise ContractError(f"{label} must be an object")
    if record.get(id_field) != "" or record.get(hash_field) != "":
        raise ContractError(f"sealing {label} requires blank identity fields")
    digest = _digest(record, fields, id_field, hash_field, label,
                     domain, maximum, validator)
    record[id_field], record[hash_field] = f"{prefix}{digest}", digest
    return _validate(record, id_field, hash_field, label, fields,
                     domain, maximum, validator)


def validate_artifact_receipt(value: object) -> dict[str, object]:
    return _validate(value, "artifact_receipt_id", "artifact_receipt_sha256",
                     "ArtifactReceipt", ARTIFACT_RECEIPT_FIELDS,
                     ARTIFACT_RECEIPT_DOMAIN, MAX_ARTIFACT_RECEIPT_BYTES,
                     validate_artifact_receipt_shape)


def seal_artifact_receipt(value: object) -> dict[str, object]:
    return _seal(value, "artifact_receipt_id", "artifact_receipt_sha256",
                 ARTIFACT_RECEIPT_PREFIX, "ArtifactReceipt", ARTIFACT_RECEIPT_FIELDS,
                 ARTIFACT_RECEIPT_DOMAIN, MAX_ARTIFACT_RECEIPT_BYTES,
                 validate_artifact_receipt_shape)


def decode_artifact_receipt(raw: bytes) -> dict[str, object]:
    return validate_artifact_receipt(
        decode_canonical_json(raw, MAX_ARTIFACT_RECEIPT_BYTES))


def validate_capability_invocation(value: object) -> dict[str, object]:
    return _validate(value, "invocation_id", "invocation_sha256",
                     "CapabilityInvocation", INVOCATION_FIELDS, INVOCATION_DOMAIN,
                     MAX_INVOCATION_BYTES, validate_invocation_shape)


def seal_capability_invocation(value: object) -> dict[str, object]:
    return _seal(value, "invocation_id", "invocation_sha256", INVOCATION_PREFIX,
                 "CapabilityInvocation", INVOCATION_FIELDS, INVOCATION_DOMAIN,
                 MAX_INVOCATION_BYTES, validate_invocation_shape)


def decode_capability_invocation(raw: bytes) -> dict[str, object]:
    return validate_capability_invocation(decode_canonical_json(raw, MAX_INVOCATION_BYTES))


def validate_interaction_event(value: object) -> dict[str, object]:
    return _validate(value, "event_id", "event_sha256", "InteractionEvent",
                     EVENT_FIELDS, EVENT_DOMAIN, MAX_EVENT_BYTES, validate_event_shape)


def seal_interaction_event(value: object) -> dict[str, object]:
    return _seal(value, "event_id", "event_sha256", EVENT_PREFIX,
                 "InteractionEvent", EVENT_FIELDS, EVENT_DOMAIN,
                 MAX_EVENT_BYTES, validate_event_shape)


def decode_interaction_event(raw: bytes) -> dict[str, object]:
    return validate_interaction_event(decode_canonical_json(raw, MAX_EVENT_BYTES))


def validate_execution_receipt(value: object) -> dict[str, object]:
    return _validate(value, "execution_receipt_id", "execution_receipt_sha256",
                     "ExecutionReceipt", EXECUTION_RECEIPT_FIELDS,
                     EXECUTION_RECEIPT_DOMAIN, MAX_EXECUTION_RECEIPT_BYTES,
                     validate_execution_receipt_shape)


def seal_execution_receipt(value: object) -> dict[str, object]:
    return _seal(value, "execution_receipt_id", "execution_receipt_sha256",
                 EXECUTION_RECEIPT_PREFIX, "ExecutionReceipt",
                 EXECUTION_RECEIPT_FIELDS, EXECUTION_RECEIPT_DOMAIN,
                 MAX_EXECUTION_RECEIPT_BYTES, validate_execution_receipt_shape)


def decode_execution_receipt(raw: bytes) -> dict[str, object]:
    return validate_execution_receipt(
        decode_canonical_json(raw, MAX_EXECUTION_RECEIPT_BYTES))


def decode_artifact_ref(raw: bytes) -> dict[str, object]:
    """Decode the exact reused ArtifactRef value object as a standalone document."""
    return artifact_ref(decode_canonical_json(raw, MAX_ARTIFACT_REF_BYTES))
