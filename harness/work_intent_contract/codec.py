"""Bounded exact canonical JSON codec for WorkIntent v1."""

from __future__ import annotations
import re

import json
import os
from pathlib import Path

from .constants import (KEY_RE, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_I64,
                        MAX_RECORD_BYTES, MAX_STRING_BYTES, MIN_I64)


class ContractError(ValueError):
    """Raised when bytes or values violate the candidate contract."""


def read_bounded_file(path: Path, label: str,
                      maximum: int = MAX_RECORD_BYTES) -> bytes:
    """Read a regular input once with a strict N+1 bound."""
    try:
        with path.open("rb") as stream:
            if os.fstat(stream.fileno()).st_size > maximum:
                raise ContractError(f"{label} exceeds {maximum} bytes")
            raw = stream.read(maximum + 1)
    except MemoryError as error:
        raise ContractError(f"{label} read exhausted memory") from error
    if len(raw) > maximum:
        raise ContractError(f"{label} exceeds {maximum} bytes")
    return raw


def _reject_constant(value: str) -> None:
    raise ContractError(f"non-finite JSON number {value!r} is forbidden")


def _reject_float(value: str) -> None:
    raise ContractError(f"floating JSON number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("integer is outside signed int64")
    parsed = int(value)
    if not MIN_I64 <= parsed <= MAX_I64:
        raise ContractError("integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


_FORBIDDEN_SCALAR_RE = re.compile('[\\x00-\\x1f\\x7f-\\x9f\\ud800-\\udfff\\u061c\\u200e\\u200f\\u2028\\u2029\\u202a-\\u202e\\u2066-\\u2069]')


def _forbidden_scalar(character: str) -> bool:
    return _FORBIDDEN_SCALAR_RE.fullmatch(character) is not None


def _walk_limits(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("integer is outside signed int64")
        return
    if isinstance(value, str):
        if _FORBIDDEN_SCALAR_RE.search(value) is not None:
            raise ContractError("string contains a forbidden Unicode scalar")
        if len(value.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"array exceeds {MAX_ARRAY_ITEMS} items")
        for item in value:
            _walk_limits(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not isinstance(key, str) or KEY_RE.fullmatch(key) is None:
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        _walk_limits(key)
        _walk_limits(item, depth + 1)


def canonical_json(value: object) -> bytes:
    """Return compact UTF-8 canonical JSON with recursively sorted keys."""
    try:
        _walk_limits(value)
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error


def _precheck_nesting(text: str) -> None:
    depth, in_string, escaped = 0, False, False
    for character in text:
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
        elif character == '"':
            in_string = True
        elif character in "[{":
            depth += 1
            if depth > MAX_DEPTH:
                raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_json(raw: bytes) -> object:
    """Decode bounded UTF-8 JSON while rejecting duplicates, floats, and excess depth."""
    if not isinstance(raw, bytes):
        raise ContractError("input must be exact bytes")
    if len(raw) > MAX_RECORD_BYTES:
        raise ContractError(f"input exceeds {MAX_RECORD_BYTES} bytes")
    try:
        text = raw.decode("utf-8")
        _precheck_nesting(text)
        value = json.loads(text, object_pairs_hook=_pairs_object, parse_int=_parse_int,
                           parse_float=_reject_float, parse_constant=_reject_constant)
        _walk_limits(value)
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"invalid UTF-8 JSON: {error}") from error


def decode_canonical_json(raw: bytes) -> object:
    """Decode one exact canonical JSON value with no trailing LF or whitespace."""
    value = decode_json(raw)
    if canonical_json(value) != raw:
        raise ContractError("input is not exact compact canonical JSON")
    return value
