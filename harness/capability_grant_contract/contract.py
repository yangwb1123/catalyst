"""Strict decoders and golden-envelope validation."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .assessment import validate_assessment, validate_assessment_shape, validate_request
from .canonical import ContractError, decode_canonical, read_bounded_file
from .constants import (MAX_ASSESSMENT_BYTES, MAX_GOLDEN_BYTES, MAX_REQUEST_BYTES,
                        MAX_GRANT_BYTES, MAX_VOCABULARY_BYTES, VOCABULARY_SHA256)
from .grant import validate_grant
from .vocabulary import validate_vocabulary

FIXTURE = Path("docs/contracts/fixtures/capability-grant-v1.json")
GOLDEN_FIELDS = {"assessment_request", "effect_vocabulary", "expected_assessment", "grant"}


def decode_request(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_REQUEST_BYTES, "declared assessment request")
    return validate_request(value)


def decode_assessment(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_ASSESSMENT_BYTES, "declared assessment")
    return validate_assessment_shape(value)


def decode_grant(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_GRANT_BYTES, "CapabilityGrant")
    return validate_grant(value)


def decode_vocabulary(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_VOCABULARY_BYTES, "effect vocabulary")
    return validate_vocabulary(value)


def validate_golden(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != GOLDEN_FIELDS:
        raise ContractError("CapabilityGrant golden envelope has unexpected fields")
    vocabulary = validate_vocabulary(value["effect_vocabulary"])
    grant = validate_grant(value["grant"])
    request = validate_request(value["assessment_request"])
    if grant != request["grant"]:
        raise ContractError("golden root Grant differs from assessment request Grant")
    if grant["effect_vocabulary_sha256"] != vocabulary["vocabulary_sha256"]:
        raise ContractError("golden Grant is not bound to its frozen vocabulary")
    if vocabulary["vocabulary_sha256"] != VOCABULARY_SHA256:
        raise ContractError("golden vocabulary identity drifted")
    validate_assessment(request, value["expected_assessment"])
    return value


def load_golden(repo_root: Path) -> dict[str, Any]:
    path = repo_root / FIXTURE
    raw = read_bounded_file(path, MAX_GOLDEN_BYTES, "CapabilityGrant golden fixture")
    # Repository text files carry one terminal LF; embedded instance values are
    # still re-encoded and instance-mode inputs remain byte-exact.
    fixture_raw = raw[:-1] if raw.endswith(b"\n") else raw
    return validate_golden(decode_canonical(fixture_raw, MAX_GOLDEN_BYTES,
                                            "CapabilityGrant golden fixture"))
