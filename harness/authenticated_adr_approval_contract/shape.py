"""Closed scalar, collection, principal, key, and signature shapes."""

from __future__ import annotations

import base64
import re
from typing import Any

from .canonical import ContractError, canonical_json
from .constants import SIGNATURE_PROFILE_ID


def require_keys(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ContractError(f"{label} fields must be exactly {sorted(fields)!r}")
    return value


def text(value: Any, label: str, maximum: int = 160) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= maximum:
        raise ContractError(f"{label} must be non-empty text of at most {maximum} bytes")
    return value


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if not isinstance(value, str) or value not in allowed:
        raise ContractError(f"{label} must be one of {allowed!r}")
    return value


def integer(value: Any, label: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise ContractError(f"{label} must be 64 lowercase hex characters") from error
    if len(decoded) != 32 or decoded.hex() != value:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return value


def stable_ref(value: Any, label: str) -> str:
    result = text(value, label)
    if re.fullmatch(r"[a-z0-9][a-z0-9._:/-]{0,159}", result) is None:
        raise ContractError(f"{label} must use the ADR reference grammar")
    return result


def stable_id(value: Any, label: str) -> str:
    result = text(value, label)
    if re.fullmatch(r"[a-z][a-z0-9._-]{0,159}", result) is None:
        raise ContractError(f"{label} must be a lowercase stable identifier")
    return result


def fixed_base64url(value: Any, label: str, raw_bytes: int) -> str:
    encoded_chars = (raw_bytes * 8 + 5) // 6
    if not isinstance(value, str) or len(value) != encoded_chars or "=" in value:
        raise ContractError(f"{label} must encode exactly {raw_bytes} bytes")
    decoded = _decode_base64url(value, label)
    if len(decoded) != raw_bytes:
        raise ContractError(f"{label} must encode exactly {raw_bytes} bytes")
    return value


def decode_base64url(value: Any, label: str, maximum_raw: int) -> bytes:
    if not isinstance(value, str) or not value or "=" in value:
        raise ContractError(f"{label} must be non-empty unpadded base64url")
    decoded = _decode_base64url(value, label)
    if len(decoded) > maximum_raw:
        raise ContractError(f"{label} decoded byte length exceeds {maximum_raw}")
    return decoded


def _decode_base64url(value: str, label: str) -> bytes:
    try:
        decoded = base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_",
                                   validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ContractError(f"{label} is not canonical unpadded base64url") from error
    if base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=") != value:
        raise ContractError(f"{label} is not canonical unpadded base64url")
    return decoded


def validate_principal(value: Any, label: str,
                       allowed: tuple[str, ...] = ("agent", "human", "operator", "service"),
                       ) -> dict[str, Any]:
    fields = {"authority_domain", "principal_id", "principal_type"}
    node = require_keys(value, label, fields)
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    enum(node["principal_type"], f"{label}.principal_type", allowed)
    return node


def principal_identity(value: dict[str, Any]) -> bytes:
    return canonical_json(value)


def array(value: Any, label: str, minimum: int, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} must contain {minimum}..{maximum} items")
    return value


def sorted_unique_nodes(value: Any, label: str, minimum: int, maximum: int) -> list[Any]:
    array(value, label, minimum, maximum)
    encoded = [canonical_json(item) for item in value]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} must be canonical-byte sorted and unique")
    return value


def sorted_unique_strings(value: Any, label: str, minimum: int, maximum: int) -> list[str]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} must contain {minimum}..{maximum} items")
    for index, item in enumerate(value):
        text(item, f"{label}[{index}]")
    if any(left.encode() >= right.encode() for left, right in zip(value, value[1:])):
        raise ContractError(f"{label} must be UTF-8 sorted and unique")
    return value


def validate_signature(value: Any, label: str, profile_hash: str) -> dict[str, Any]:
    fields = {"key_id", "profile_id", "profile_sha256", "signature_base64url"}
    node = require_keys(value, label, fields)
    text(node["key_id"], f"{label}.key_id")
    if node["profile_id"] != SIGNATURE_PROFILE_ID:
        raise ContractError(f"{label}.profile_id is unsupported")
    if sha256(node["profile_sha256"], f"{label}.profile_sha256") != profile_hash:
        raise ContractError(f"{label} does not bind the signature profile")
    fixed_base64url(node["signature_base64url"], f"{label}.signature_base64url", 64)
    return node
