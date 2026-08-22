"""Self-digest sealing and exact WorkIntent v1 instance validation."""

from __future__ import annotations

import copy
import hashlib

from .codec import ContractError, canonical_json, decode_canonical_json
from .constants import DIGEST_DOMAIN, MAX_RECORD_BYTES, TOP_FIELDS
from .shape import validate_shape


def _blanked(record: dict[str, object]) -> dict[str, object]:
    if not isinstance(record, dict):
        raise ContractError("WorkIntent must be an object")
    if set(record) != TOP_FIELDS:
        raise ContractError("digest input must contain the exact WorkIntent top-level fields")
    blank = copy.deepcopy(record)
    blank["work_intent_id"] = ""
    blank["work_intent_sha256"] = ""
    return blank


def work_intent_digest(record: dict[str, object]) -> str:
    """Compute the v1 digest from the exact blank-identity canonical preimage."""
    blank = _blanked(record)
    validate_shape(blank, allow_blank_identity=True)
    preimage = canonical_json(blank)
    if len(preimage) > MAX_RECORD_BYTES:
        raise ContractError(f"blank identity preimage exceeds {MAX_RECORD_BYTES} bytes")
    return hashlib.sha256(DIGEST_DOMAIN + preimage).hexdigest()


def validate_work_intent(value: object) -> dict[str, object]:
    """Validate shape, final bound, and the two self-identity fields."""
    record = validate_shape(value)
    sealed = canonical_json(record)
    if len(sealed) > MAX_RECORD_BYTES:
        raise ContractError(f"sealed WorkIntent exceeds {MAX_RECORD_BYTES} bytes")
    digest = work_intent_digest(record)
    if record["work_intent_sha256"] != digest:
        raise ContractError("work_intent_sha256 does not match the canonical preimage")
    return record


def seal_work_intent(value: object) -> dict[str, object]:
    """Seal an exact blank-identity declaration without mutating its caller."""
    record = copy.deepcopy(value)
    if not isinstance(record, dict):
        raise ContractError("WorkIntent must be an object")
    if record.get("work_intent_id") != "" or record.get("work_intent_sha256") != "":
        raise ContractError("sealing requires empty work_intent_id and work_intent_sha256")
    digest = work_intent_digest(record)
    record["work_intent_sha256"] = digest
    record["work_intent_id"] = f"work-intent-{digest}"
    return validate_work_intent(record)


def decode_work_intent(raw: bytes) -> dict[str, object]:
    """Decode and validate one exact canonical WorkIntent instance (no LF)."""
    value = decode_canonical_json(raw)
    return validate_work_intent(value)


def canonical_work_intent(record: object) -> bytes:
    """Validate and encode one sealed WorkIntent."""
    return canonical_json(validate_work_intent(record))
