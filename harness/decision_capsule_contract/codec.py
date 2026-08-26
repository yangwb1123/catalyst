"""Bounded exact canonical codec for Decision Capsule replay records."""

from __future__ import annotations

import copy
import re

from kernel_operational_contract.codec import (
    ContractError,
    canonical_json as _operational_canonical_json, decode_canonical_json,
    read_bounded_file,
)
from kernel_operational_contract.constants import (
    KEY_RE, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_I64,
    MAX_STRING_BYTES, MIN_I64,
)

from .constants import MAX_CLOSURE_BYTES

_FORBIDDEN_SCALAR_RE = re.compile(
    "[\\x00-\\x1f\\x7f-\\x9f\\ud800-\\udfff\\u061c\\u200e\\u200f\\u2028\\u2029\\u202a-\\u202e\\u2066-\\u2069]")


def validate_json_tree(value: object, maximum: int) -> None:
    """Fail closed on non-JSON, cyclic, or over-bound programmatic values."""
    try:
        _canonical_size(value, maximum)
    except ContractError:
        raise
    except (MemoryError, TypeError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"JSON tree validation failed: {error}") from error


def bounded_deepcopy(value: object, maximum: int) -> object:
    validate_json_tree(value, maximum)
    try:
        return copy.deepcopy(value)
    except (MemoryError, TypeError, ValueError, RecursionError) as error:
        raise ContractError(f"bounded JSON copy failed: {error}") from error


def _bounded_add(total: int, increment: int, maximum: int) -> int:
    total += increment
    if total > maximum:
        raise ContractError(f"canonical JSON exceeds {maximum} bytes")
    return total


def exact_object_shell(value: object, fields: set[str], label: str) -> dict[str, object]:
    """Reject hostile containers and wrong outer fields before nested work."""
    if type(value) is not dict or len(value) != len(fields):
        raise ContractError(f"{label} must have exact fields")
    if any(type(key) is not str for key in value):
        raise ContractError(f"{label} must have exact string keys")
    if any(key not in fields for key in value) or any(field not in value for field in fields):
        raise ContractError(f"{label} must have exact fields")
    return value


def require_constants(value: dict[str, object], expected: dict[str, object]) -> None:
    """Check short local discriminators without invoking subclass behavior."""
    for field, required in expected.items():
        actual = value[field]
        if type(actual) is not type(required) or actual != required:
            raise ContractError(f"{field} must be {required!r}")


def require_identity_strings(value: dict[str, object], *fields: str) -> None:
    if any(type(value[field]) is not str for field in fields):
        raise ContractError("identity fields must be exact strings")


def _utf8_width(character: str) -> int:
    code = ord(character)
    if code <= 0x7f:
        return 1
    if code <= 0x7ff:
        return 2
    if code <= 0xffff:
        return 3
    return 4


def _string_size(value: str, maximum: int) -> int:
    if _FORBIDDEN_SCALAR_RE.search(value):
        raise ContractError("string contains a forbidden Unicode scalar")
    total, content_bytes = 2, 0
    for character in value:
        width = _utf8_width(character)
        content_bytes += width
        if content_bytes > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        emitted = 2 if character in {'"', "\\"} else width
        total = _bounded_add(total, emitted, maximum)
    return total


def _canonical_size(value: object, maximum: int, depth: int = 1) -> int:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if value is None:
        return 4
    if type(value) is bool:
        return 4 if value else 5
    if type(value) is int:
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("integer is outside signed int64")
        return len(str(value))
    if type(value) is str:
        return _string_size(value, maximum)
    if type(value) is list:
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"array exceeds {MAX_ARRAY_ITEMS} items")
        total = 2
        for index, item in enumerate(value):
            total = _bounded_add(total, int(index > 0), maximum)
            total = _bounded_add(
                total, _canonical_size(item, maximum, depth + 1), maximum)
        return total
    if type(value) is dict:
        if len(value) > MAX_FIELDS:
            raise ContractError(f"object exceeds {MAX_FIELDS} fields")
        total = 2
        for index, (key, item) in enumerate(value.items()):
            if type(key) is not str or KEY_RE.fullmatch(key) is None:
                raise ContractError(
                    "object key must be an exact ASCII snake_case string")
            total = _bounded_add(total, int(index > 0), maximum)
            total = _bounded_add(total, _string_size(key, maximum) + 1, maximum)
            total = _bounded_add(
                total, _canonical_size(item, maximum, depth + 1), maximum)
        return total
    raise ContractError("unsupported JSON value type")


def canonical_json(value: object) -> bytes:
    """Encode canonical JSON under the local 28-MiB outer ceiling."""
    try:
        validate_json_tree(value, MAX_CLOSURE_BYTES)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error
    return _operational_canonical_json(value)


__all__ = [
    "ContractError", "canonical_json", "decode_canonical_json", "read_bounded_file",
]
