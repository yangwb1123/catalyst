"""Strict bounded compact canonical JSON for the Registry wire."""

from __future__ import annotations

import json
import os
import unicodedata
from pathlib import Path

from .constants import (
    MAX_DEPTH, MAX_FIELDS, MAX_I64, MAX_ITEMS, MAX_STRING_BYTES, MIN_I64,
)


class ContractError(ValueError):
    """Caller bytes or in-memory values violate the frozen contract."""


def _forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (unicodedata.category(character) == "Cc" or code == 0x7F or
            0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _valid_key(value: object) -> bool:
    return (isinstance(value, str) and bool(value) and "a" <= value[0] <= "z" and
            all(char == "_" or "a" <= char <= "z" or "0" <= char <= "9"
                for char in value))


def _walk(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("integer is outside signed int64")
        return
    if isinstance(value, str):
        if len(value.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        if any(_forbidden_scalar(character) for character in value):
            raise ContractError("string contains forbidden Unicode scalar")
        return
    if isinstance(value, list):
        if len(value) > MAX_ITEMS:
            raise ContractError(f"array exceeds {MAX_ITEMS} items")
        for item in value:
            _walk(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not _valid_key(key):
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        _walk(key)
        _walk(item, depth + 1)


def canonical_json(value: object, *, max_bytes: int | None = None) -> bytes:
    try:
        _walk(value)
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error
    if max_bytes is not None and len(raw) > max_bytes:
        raise ContractError(f"canonical JSON exceeds {max_bytes} bytes")
    return raw


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _reject_number(value: str) -> None:
    raise ContractError(f"floating or non-finite number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("integer is outside signed int64")
    parsed = int(value)
    if not MIN_I64 <= parsed <= MAX_I64:
        raise ContractError("integer is outside signed int64")
    return parsed


def _precheck_depth(text: str) -> None:
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


def decode_canonical(raw: bytes, *, max_bytes: int, label: str) -> object:
    try:
        if not isinstance(raw, bytes) or not 1 <= len(raw) <= max_bytes:
            raise ContractError(f"{label} byte length must be 1..{max_bytes}")
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs, parse_int=_parse_int,
                           parse_float=_reject_number, parse_constant=_reject_number)
        _walk(value)
        if canonical_json(value, max_bytes=max_bytes) != raw:
            raise ContractError(f"{label} is not exact compact canonical JSON")
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError,
            json.JSONDecodeError) as error:
        raise ContractError(f"invalid {label} UTF-8 JSON: {error}") from error


def read_bounded(path: Path, maximum: int, label: str) -> bytes:
    try:
        with path.open("rb") as stream:
            if os.fstat(stream.fileno()).st_size > maximum:
                raise ContractError(f"{label} exceeds {maximum} bytes")
            raw = stream.read(maximum + 1)
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot read {label}: {error}") from error
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return raw
