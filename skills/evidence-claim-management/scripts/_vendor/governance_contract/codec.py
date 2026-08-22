"""Bounded strict JSON codec and domain-separated record digest."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

from .constants import (DOMAINS, KEY_RE, MAX_DEPTH, MAX_FIELDS, MAX_I64, MAX_ITEMS,
                        MAX_RECORD_BYTES, MAX_SET_BYTES, MAX_STRING_BYTES, MIN_I64)


class ContractError(ValueError):
    """Raised when bytes cannot represent a canonical bounded record set."""


def read_bounded_file(path: Path, *, label: str,
                      max_bytes: int = MAX_SET_BYTES) -> bytes:
    """Read at most max_bytes, rejecting known size early and growth after stat."""
    try:
        with path.open("rb") as stream:
            if os.fstat(stream.fileno()).st_size > max_bytes:
                raise ContractError(f"{label} exceeds {max_bytes} bytes")
            raw = stream.read(max_bytes + 1)
    except MemoryError as error:
        raise OSError(f"{label}: bounded read exhausted memory") from error
    if len(raw) > max_bytes:
        raise ContractError(f"{label} exceeds {max_bytes} bytes")
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


def _forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (code < 32 or code == 127 or 0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


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
        if any(_forbidden_scalar(character) for character in value):
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
        if not isinstance(key, str) or not KEY_RE.fullmatch(key):
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        _walk_limits(key)
        _walk_limits(item, depth + 1)


def canonical_json(value: object) -> bytes:
    """Return canonical v1 JSON bytes or raise ContractError."""
    try:
        _walk_limits(value)
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("canonical JSON processing exhausted memory") from error


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
            continue
        if character == '"':
            in_string = True
        elif character in "[{":
            depth += 1
            if depth > MAX_DEPTH:
                raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_json(raw: bytes) -> object:
    """Decode strict bounded UTF-8 JSON without requiring canonical whitespace."""
    try:
        if len(raw) > MAX_SET_BYTES:
            raise ContractError(f"JSON input exceeds {MAX_SET_BYTES} bytes")
        text = raw.decode("utf-8")
        _precheck_nesting(text)
        value = json.loads(text, object_pairs_hook=_pairs_object, parse_int=_parse_int,
                           parse_float=_reject_float, parse_constant=_reject_constant)
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("JSON decoding exhausted memory") from error
    except (UnicodeError, ValueError, RecursionError, json.JSONDecodeError) as error:
        raise ContractError(f"invalid UTF-8 JSON: {error}") from error
    try:
        _walk_limits(value)
    except MemoryError as error:
        raise ContractError("JSON validation exhausted memory") from error
    return value


def decode_record_set(raw: bytes) -> list[dict[str, object]]:
    """Strictly decode an exact compact canonical record-set array."""
    if len(raw) > MAX_SET_BYTES:
        raise ContractError(f"record set exceeds {MAX_SET_BYTES} bytes")
    value = decode_json(raw)
    if not isinstance(value, list) or not value:
        raise ContractError("record set must be a non-empty JSON array")
    if not all(isinstance(record, dict) for record in value):
        raise ContractError("record set entries must be JSON objects")
    if canonical_json(value) != raw:
        raise ContractError("record set is not exact compact canonical JSON")
    return value


def canonical_record_payload(record: dict[str, object]) -> bytes:
    """Canonicalize one record after blanking its self digest."""
    try:
        integrity = record.get("integrity")
        if not isinstance(integrity, dict):
            raise ContractError("record.integrity must be an object")
        payload = {**record, "integrity": {**integrity, "canonical_sha256": ""}}
        encoded = canonical_json(payload)
        if len(encoded) + 64 > MAX_RECORD_BYTES:
            raise ContractError(f"record exceeds {MAX_RECORD_BYTES} bytes")
        return encoded
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("record canonicalization exhausted memory") from error


def compute_record_digest(record: dict[str, object]) -> str:
    """Compute the kind-domain-separated self digest for one record."""
    kind = record.get("kind")
    if not isinstance(kind, str) or kind not in DOMAINS:
        raise ContractError(f"unsupported record kind {kind!r}")
    try:
        return hashlib.sha256(DOMAINS[kind] + canonical_record_payload(record)).hexdigest()
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError("record digest processing exhausted memory") from error
