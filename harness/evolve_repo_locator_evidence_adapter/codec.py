"""Strict bounded canonical JSON codec for Evolve locator adaptation."""

from __future__ import annotations
import re

import json

from governance_contract import ContractError

from .constants import (MAX_DEPTH, MAX_FIELDS, MAX_I64, MAX_ITEMS,
                        MAX_REQUEST_BYTES, MAX_STRING_BYTES, MIN_I64)


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


_FORBIDDEN_SCALAR_RE = re.compile('[\\x00-\\x1f\\x7f\\ud800-\\udfff\\u061c\\u200e\\u200f\\u2028\\u2029\\u202a-\\u202e\\u2066-\\u2069]')


def _forbidden_scalar(character: str) -> bool:
    return _FORBIDDEN_SCALAR_RE.fullmatch(character) is not None


def _valid_key(key: object) -> bool:
    if not isinstance(key, str) or not key or not "a" <= key[0] <= "z":
        return False
    return all(character == "_" or "a" <= character <= "z" or
               "0" <= character <= "9" for character in key)


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
            raise ContractError("string contains forbidden Unicode control scalar")
        if len(value.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        return
    if isinstance(value, list):
        if len(value) > MAX_ITEMS:
            raise ContractError(f"array exceeds {MAX_ITEMS} items")
        for item in value:
            _walk_limits(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not _valid_key(key):
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        _walk_limits(key, depth + 1)
        _walk_limits(item, depth + 1)


def canonical_json(value: object) -> bytes:
    """Return exact compact UTF-8 canonical bytes for an adapter value."""
    try:
        _walk_limits(value)
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_REQUEST_BYTES:
            raise ContractError(f"adapter JSON exceeds {MAX_REQUEST_BYTES} bytes")
        return encoded
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("adapter canonical JSON processing exhausted memory") from error


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


def decode_json(raw: bytes, *, max_bytes: int = MAX_REQUEST_BYTES) -> object:
    """Decode bounded strict UTF-8 JSON without accepting floats or duplicates."""
    try:
        if len(raw) > max_bytes:
            raise ContractError(f"adapter JSON exceeds {max_bytes} bytes")
        text = raw.decode("utf-8")
        _precheck_nesting(text)
        value = json.loads(text, object_pairs_hook=_pairs_object, parse_int=_parse_int,
                           parse_float=_reject_float, parse_constant=_reject_constant)
        _walk_limits(value)
        return value
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("adapter JSON decoding exhausted memory") from error
    except (UnicodeError, ValueError, RecursionError, json.JSONDecodeError) as error:
        raise ContractError(f"invalid UTF-8 JSON: {error}") from error


def decode_request(raw: bytes) -> dict[str, object]:
    """Decode one exact compact canonical adapter request."""
    value = decode_json(raw)
    if not isinstance(value, dict):
        raise ContractError("adapter request root must be an object")
    if canonical_json(value) != raw:
        raise ContractError("adapter request is not exact compact canonical JSON")
    return value


def canonical_locator(locator: object) -> bytes:
    """Canonicalize one exact Evolve locator object."""
    return canonical_json(locator)


def canonical_observation(observation: object) -> bytes:
    """Canonicalize one exact Evolve locator observation object."""
    return canonical_json(observation)
