"""Bounded exact-canonical JSON codec for ContextPackage v1."""

from __future__ import annotations

import hashlib
import json

from .constants import (MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_I64, MAX_OBJECT_FIELDS,
                        MAX_PACKAGE_BYTES, MAX_REQUEST_BYTES, MAX_STRING_BYTES)


class ContractError(ValueError):
    """Raised for any fail-closed contract violation."""


def _reject_constant(value: str) -> None:
    raise ContractError(f"non-finite number {value!r} is forbidden")


def _reject_float(value: str) -> None:
    raise ContractError(f"floating number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    if len(value.removeprefix("-")) > 19:
        raise ContractError("integer is outside signed int64")
    parsed = int(value)
    if not -MAX_I64 - 1 <= parsed <= MAX_I64:
        raise ContractError("integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def forbidden_scalar(character: str, *, content: bool = False) -> bool:
    code = ord(character)
    allowed_control = content and code in {9, 10}
    return ((code < 32 and not allowed_control) or code == 127 or
            0xD800 <= code <= 0xDFFF or code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _walk(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not -MAX_I64 - 1 <= value <= MAX_I64:
            raise ContractError("integer is outside signed int64")
        return
    if isinstance(value, str):
        if len(value.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"array exceeds {MAX_ARRAY_ITEMS} items")
        for item in value:
            _walk(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")
    if len(value) > MAX_OBJECT_FIELDS:
        raise ContractError(f"object exceeds {MAX_OBJECT_FIELDS} fields")
    for key, item in value.items():
        if not isinstance(key, str):
            raise ContractError("object key must be a string")
        _walk(key, depth + 1)
        _walk(item, depth + 1)


def canonical_json(value: object) -> bytes:
    """Return ForgeOS compact canonical JSON bytes."""
    _walk(value)
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(",", ":")).encode("utf-8")
    except (MemoryError, UnicodeError, ValueError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error


def _decode(raw: bytes, maximum: int, label: str) -> object:
    if len(raw) > maximum:
        raise ContractError(f"{label} exceeds {maximum} bytes")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_float,
                           parse_constant=_reject_constant)
        _walk(value)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"invalid UTF-8 JSON: {error}") from error
    if canonical_json(value) != raw:
        raise ContractError(f"{label} is not exact compact canonical JSON")
    return value


def decode_request(raw: bytes) -> dict[str, object]:
    value = _decode(raw, MAX_REQUEST_BYTES, "build request")
    if not isinstance(value, dict):
        raise ContractError("build request must be an object")
    from .shape import validate_request
    validate_request(value)
    return value


def decode_package(raw: bytes) -> dict[str, object]:
    value = _decode(raw, MAX_PACKAGE_BYTES, "context package")
    if not isinstance(value, dict):
        raise ContractError("context package must be an object")
    from .shape import validate_package_shape
    validate_package_shape(value)
    return value


def digest(domain: bytes, payload: bytes) -> str:
    return hashlib.sha256(domain + payload).hexdigest()


def plain_sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()
