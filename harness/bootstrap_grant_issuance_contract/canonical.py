"""Canonical codec and digest helpers for the bootstrap issuance contract."""

from __future__ import annotations

import copy
import hashlib
from typing import Any

from capability_grant_contract.canonical import (
    ContractError,
    bounded_canonical_json,
    canonical_json,
    decode_canonical,
    read_bounded_file,
)


def self_digest(domain: bytes, value: dict[str, Any], digest_field: str,
                maximum: int, label: str, signed: bool = False) -> str:
    """Hash a bounded artifact using its frozen empty-field preimage rule."""
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    bounded_canonical_json(value, maximum, label)
    payload = copy.deepcopy(value)
    payload[digest_field] = ""
    if signed:
        try:
            payload["signature"]["signature_base64url"] = ""
        except (KeyError, TypeError) as error:
            raise ContractError(f"{label} has no closed signature object") from error
    encoded = bounded_canonical_json(payload, maximum, label)
    return hashlib.sha256(domain + encoded).hexdigest()


def envelope_digest(domain: bytes, value: Any, maximum: int, label: str) -> str:
    """Hash complete canonical envelope bytes without a self-field transform."""
    encoded = bounded_canonical_json(value, maximum, label)
    return hashlib.sha256(domain + encoded).hexdigest()


def signature_message(domain: bytes, digest_hex: str) -> bytes:
    """Return the fixed Ed25519 message; this helper does not verify a signature."""
    if not isinstance(digest_hex, str):
        raise ContractError("signature digest must be lowercase SHA-256 hex")
    try:
        raw = bytes.fromhex(digest_hex)
    except ValueError as error:
        raise ContractError("signature digest must be lowercase SHA-256 hex") from error
    if len(raw) != 32 or digest_hex != raw.hex():
        raise ContractError("signature digest must be lowercase SHA-256 hex")
    return domain + raw


__all__ = [
    "ContractError", "bounded_canonical_json", "canonical_json", "decode_canonical",
    "envelope_digest", "read_bounded_file", "self_digest", "signature_message",
]
