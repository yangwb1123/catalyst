"""Strict ADR-0058 canonical JSON with a raw-base64 field exception."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from bootstrap_grant_issuance_contract.canonical import ContractError, signature_message

from .constants import MAX_OUTPUT_BYTES

MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY = 256
MAX_STRING_BYTES = 16 * 1024
MAX_RAW_BASE64URL_CHARS = (MAX_OUTPUT_BYTES * 4 + 2) // 3


def _pairs(values: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in values:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _integer(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as error:
        raise ContractError("JSON number is not an integer") from error
    if str(value) != raw or not -(2**63) <= value <= 2**63 - 1:
        raise ContractError("JSON integer is not canonical signed int64")
    return value


def _reject_number(raw: str) -> None:
    raise ContractError(f"non-integer JSON number {raw!r} is forbidden")


def _forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (code <= 0x1F or code == 0x7F or 0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _validate_node(value: Any, depth: int = 1, field: str | None = None) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if isinstance(value, dict):
        _validate_object(value, depth)
    elif isinstance(value, list):
        if len(value) > MAX_ARRAY:
            raise ContractError(f"array item count exceeds {MAX_ARRAY}")
        for child in value:
            _validate_node(child, depth + 1)
    elif isinstance(value, str):
        _validate_string(value, field)
    elif isinstance(value, int) and not isinstance(value, bool):
        if not -(2**63) <= value <= 2**63 - 1:
            raise ContractError("integer is outside signed int64")
    elif value is not None and not isinstance(value, bool):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")


def _validate_object(value: dict[Any, Any], depth: int) -> None:
    if len(value) > MAX_FIELDS:
        raise ContractError(f"object field count exceeds {MAX_FIELDS}")
    for key, child in value.items():
        if not isinstance(key, str) or re.fullmatch(r"[a-z][a-z0-9_]*", key) is None:
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        if len(key.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"object key byte length exceeds {MAX_STRING_BYTES}")
        _validate_node(child, depth + 1, key)


def _validate_string(value: str, field: str | None) -> None:
    if any(_forbidden_scalar(character) for character in value):
        raise ContractError("string contains forbidden control, bidi, or surrogate scalar")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ContractError("string is not Unicode scalar text") from error
    maximum = MAX_RAW_BASE64URL_CHARS if field == "content_base64url" else MAX_STRING_BYTES
    if len(encoded) > maximum:
        raise ContractError(f"string byte length exceeds {maximum}")


def canonical_json(value: Any) -> bytes:
    _validate_node(value)
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                      sort_keys=True).encode("utf-8")


def bounded_canonical_json(value: Any, maximum: int, label: str) -> bytes:
    encoded = canonical_json(value)
    if not 1 <= len(encoded) <= maximum:
        raise ContractError(f"{label} canonical byte length must be 1..{maximum}")
    return encoded


def decode_canonical(raw: bytes, maximum: int, label: str) -> Any:
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    try:
        text = raw.decode("utf-8")
        value = json.loads(text, object_pairs_hook=_pairs, parse_int=_integer,
                           parse_float=_reject_number, parse_constant=_reject_number)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ContractError(f"{label} is not strict UTF-8 JSON: {error}") from error
    _validate_node(value)
    if canonical_json(value) != raw:
        raise ContractError(f"{label} is not exact compact canonical JSON")
    return value


def read_bounded_file(path: Path, maximum: int, label: str) -> bytes:
    try:
        with path.open("rb") as stream:
            raw = stream.read(maximum + 1)
    except OSError as error:
        raise ContractError(f"cannot read {label}: {error}") from error
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return raw


def self_digest(domain: bytes, value: Any, digest_field: str, maximum: int,
                label: str, *, signed: bool = False,
                derived_id_field: str | None = None) -> str:
    """Hash one closed artifact using its frozen empty-field preimage."""
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    bounded_canonical_json(value, maximum, label)
    payload = copy.deepcopy(value)
    payload[digest_field] = ""
    if derived_id_field is not None:
        payload[derived_id_field] = ""
    if signed:
        try:
            payload["signature"]["signature_base64url"] = ""
        except (KeyError, TypeError) as error:
            raise ContractError(f"{label} has no closed signature object") from error
    encoded = bounded_canonical_json(payload, maximum, label)
    return hashlib.sha256(domain + encoded).hexdigest()


def plain_sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


__all__ = [
    "ContractError", "bounded_canonical_json", "canonical_json", "decode_canonical",
    "plain_sha256", "read_bounded_file", "self_digest", "signature_message",
]
