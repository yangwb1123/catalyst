"""Strict bounded canonical JSON for local observation production v1."""

from __future__ import annotations

import json
import unicodedata

from governance_contract import ContractError

from .constants import (MAX_DEPTH, MAX_FIELDS, MAX_I64, MAX_LIST_ITEMS,
                        MAX_MANIFEST_BYTES, MAX_TEXT_BYTES, MAX_TEXT_SCALARS,
                        MIN_I64)


def forbidden_scalar(character: str) -> bool:
    """Match the Go producer's closed forbidden-Unicode set."""
    code = ord(character)
    return (unicodedata.category(character) == "Cc" or 0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _valid_key(key: object) -> bool:
    if not isinstance(key, str) or not key or not "a" <= key[0] <= "z":
        return False
    return all(character == "_" or "a" <= character <= "z" or
               "0" <= character <= "9" for character in key)


def _walk_limits(value: object, depth: int = 1, *, enforce_text: bool = True) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"producer JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("producer integer is outside signed int64")
        return
    if isinstance(value, str):
        if len(value) > MAX_TEXT_SCALARS:
            raise ContractError(f"producer string exceeds {MAX_TEXT_SCALARS} Unicode scalars")
        if enforce_text and any(forbidden_scalar(character) for character in value):
            raise ContractError("producer string contains forbidden Unicode control scalar")
        try:
            encoded = value.encode("utf-8")
        except UnicodeError as error:
            raise ContractError(f"producer string is not valid UTF-8: {error}") from error
        if len(encoded) > MAX_TEXT_BYTES:
            raise ContractError(f"producer string exceeds {MAX_TEXT_BYTES} UTF-8 bytes")
        return
    if isinstance(value, list):
        if len(value) > MAX_LIST_ITEMS:
            raise ContractError(f"producer array exceeds {MAX_LIST_ITEMS} items")
        for item in value:
            _walk_limits(item, depth + 1, enforce_text=enforce_text)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported producer JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"producer object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not _valid_key(key):
            raise ContractError(f"producer object key {key!r} is not ASCII snake_case")
        _walk_limits(key, depth + 1, enforce_text=enforce_text)
        _walk_limits(item, depth + 1, enforce_text=enforce_text)


def canonical_json(value: object, *, max_bytes: int = MAX_MANIFEST_BYTES) -> bytes:
    """Return Go-compatible sorted compact UTF-8 JSON within a byte budget."""
    try:
        _walk_limits(value)
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode("utf-8")
        if len(encoded) > max_bytes:
            raise ContractError(f"producer JSON exceeds {max_bytes} bytes")
        return encoded
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"producer canonical JSON failed: {error}") from error


def _reject_constant(value: str) -> None:
    raise ContractError(f"non-finite producer JSON number {value!r} is forbidden")


def _reject_float(value: str) -> None:
    raise ContractError(f"floating producer JSON number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("producer integer is outside signed int64")
    parsed = int(value)
    if not MIN_I64 <= parsed <= MAX_I64:
        raise ContractError("producer integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


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
                raise ContractError(f"producer JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_json(raw: bytes, *, max_bytes: int = MAX_MANIFEST_BYTES,
                enforce_text: bool = True) -> object:
    """Decode strict UTF-8 JSON, rejecting duplicate keys, floats and drift."""
    try:
        if len(raw) > max_bytes:
            raise ContractError(f"producer JSON exceeds {max_bytes} bytes")
        text = raw.decode("utf-8")
        _precheck_nesting(text)
        value = json.loads(text, object_pairs_hook=_pairs_object, parse_int=_parse_int,
                           parse_float=_reject_float, parse_constant=_reject_constant)
        _walk_limits(value, enforce_text=enforce_text)
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError,
            json.JSONDecodeError) as error:
        raise ContractError(f"invalid producer UTF-8 JSON: {error}") from error
