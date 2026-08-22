"""Closed primitive validators for lifecycle documents."""

from __future__ import annotations

import base64
import re
from typing import Any

from .canonical import ContractError

HASH_RE = re.compile(r"[0-9a-f]{64}")
ADR_RE = re.compile(r"ADR-((?!0000)[0-9]{4})")
ID_RE = re.compile(r"[a-z][a-z0-9._-]{0,159}")
KEY_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:-]{0,159}")
IDEMPOTENCY_RE = re.compile(r"[A-Za-z0-9._:@+\-]{16,128}")


def require_keys(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(
            f"{label} fields differ; unknown={sorted(unknown)}, missing={sorted(missing)}"
        )
    return value


def integer(value: Any, label: str, minimum: int = 0,
            maximum: int = 2**63 - 1) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def text(value: Any, label: str, pattern: re.Pattern[str] | None = None) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= 160:
        raise ContractError(f"{label} must be 1..160 UTF-8 bytes")
    if pattern is not None and pattern.fullmatch(value) is None:
        raise ContractError(f"{label} does not match its closed grammar")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be lowercase SHA-256 hex")
    return value


def adr_id(value: Any, label: str) -> str:
    if not isinstance(value, str) or ADR_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be ADR-NNNN other than ADR-0000")
    return value


def sorted_unique(values: Any, label: str, maximum: int,
                  *, nonempty: bool = False) -> list[str]:
    if not isinstance(values, list) or len(values) > maximum:
        raise ContractError(f"{label} must contain at most {maximum} strings")
    if nonempty and not values:
        raise ContractError(f"{label} must not be empty")
    if not all(isinstance(item, str) for item in values):
        raise ContractError(f"{label} must contain strings")
    expected = sorted(set(values), key=lambda item: item.encode("utf-8"))
    if values != expected:
        raise ContractError(f"{label} must be raw-UTF-8 sorted and unique")
    return values


def decode_base64url(value: Any, label: str, maximum: int) -> bytes:
    if not isinstance(value, str) or not value or "=" in value:
        raise ContractError(f"{label} must be nonempty unpadded base64url")
    if re.fullmatch(r"[A-Za-z0-9_-]+", value) is None:
        raise ContractError(f"{label} contains a non-base64url character")
    try:
        decoded = base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))
    except (ValueError, TypeError) as error:
        raise ContractError(f"{label} is malformed base64url") from error
    encoded = base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=")
    if encoded != value or not 1 <= len(decoded) <= maximum:
        raise ContractError(f"{label} is noncanonical or exceeds {maximum} bytes")
    return decoded


def validate_signature(value: Any, label: str, profile_hash: str,
                       expected_key: str) -> dict[str, Any]:
    fields = {"key_id", "profile_id", "profile_sha256", "signature_base64url"}
    node = require_keys(value, label, fields)
    text(node["key_id"], f"{label}.key_id", KEY_RE)
    if node["key_id"] != expected_key:
        raise ContractError(f"{label} uses the wrong trust-root key")
    if node["profile_id"] != "forgeos.ed25519-domain-sha256/v1":
        raise ContractError(f"{label}.profile_id drifted")
    if sha256(node["profile_sha256"], f"{label}.profile_sha256") != profile_hash:
        raise ContractError(f"{label} does not bind the signature profile")
    raw = decode_base64url(node["signature_base64url"],
                           f"{label}.signature_base64url", 64)
    if len(raw) != 64:
        raise ContractError(f"{label}.signature_base64url must encode 64 bytes")
    return node
