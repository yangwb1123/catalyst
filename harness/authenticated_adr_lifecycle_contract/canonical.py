"""Strict canonical JSON helpers shared with the frozen approval prerequisite."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from authenticated_adr_approval_contract.canonical import (
    ContractError,
    bounded_canonical_json,
    decode_canonical,
    read_bounded_file,
    self_digest,
    signature_message,
)


def sha256_bytes(value: bytes) -> str:
    if not isinstance(value, bytes):
        raise ContractError("SHA-256 input must be bytes")
    return hashlib.sha256(value).hexdigest()


def physical_pin(path: Path, expected: str, maximum: int, label: str) -> bytes:
    raw = read_bounded_file(path, maximum, label)
    if sha256_bytes(raw) != expected:
        raise ContractError(f"{label} physical SHA-256 does not match the frozen pin")
    return raw


def digest_value(domain: bytes, value: Any, maximum: int, label: str) -> str:
    encoded = bounded_canonical_json(value, maximum, label)
    return hashlib.sha256(domain + encoded).hexdigest()


__all__ = [
    "ContractError", "bounded_canonical_json", "decode_canonical", "digest_value",
    "physical_pin", "read_bounded_file", "self_digest", "sha256_bytes",
    "signature_message",
]
