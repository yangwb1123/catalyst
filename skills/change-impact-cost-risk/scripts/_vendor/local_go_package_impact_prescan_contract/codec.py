"""Bounded canonical JSON, Base64URL, and digest primitives for ADR-0062."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import unicodedata

from governance_contract import ContractError

from .constants import (BASE64URL_RE, MAX_ARRAY_ITEMS, MAX_DEPTH,
                        MAX_ENVELOPE_BYTES, MAX_FIELDS, MAX_GRAPH_BASE64URL_BYTES,
                        MAX_GRAPH_BYTES, MAX_I64, MIN_I64)


def forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (unicodedata.category(character) == "Cc" or
            0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _valid_key(key: object) -> bool:
    return (isinstance(key, str) and bool(key) and "a" <= key[0] <= "z" and
            all(char == "_" or "a" <= char <= "z" or "0" <= char <= "9"
                for char in key))


def _walk(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"ADR-0062 JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("ADR-0062 integer is outside signed int64")
        return
    if isinstance(value, str):
        _walk_string(value)
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"ADR-0062 array exceeds {MAX_ARRAY_ITEMS} items")
        for item in value:
            _walk(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported ADR-0062 JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"ADR-0062 object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not _valid_key(key):
            raise ContractError(f"ADR-0062 key {key!r} is not ASCII snake_case")
        _walk(item, depth + 1)


def _walk_string(value: str) -> None:
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"ADR-0062 string is not valid UTF-8: {error}") from error
    if len(encoded) > MAX_GRAPH_BASE64URL_BYTES:
        raise ContractError("ADR-0062 string exceeds the largest closed field bound")
    if any(forbidden_scalar(character) for character in value):
        raise ContractError("ADR-0062 string contains forbidden Unicode scalar")


def canonical_json(value: object, *, max_bytes: int = MAX_ENVELOPE_BYTES) -> bytes:
    try:
        _walk(value)
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":")).encode("utf-8")
        if len(raw) > max_bytes:
            raise ContractError(f"ADR-0062 canonical JSON exceeds {max_bytes} bytes")
        return raw
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"ADR-0062 canonical JSON failed: {error}") from error


def _reject_number(value: str) -> None:
    raise ContractError(f"floating or non-finite ADR-0062 number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("ADR-0062 integer is outside signed int64")
    parsed = int(value)
    if not MIN_I64 <= parsed <= MAX_I64:
        raise ContractError("ADR-0062 integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate ADR-0062 JSON key {key!r}")
        result[key] = value
    return result


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
                raise ContractError(f"ADR-0062 JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_canonical(raw: bytes, *, max_bytes: int, label: str) -> object:
    try:
        if len(raw) > max_bytes:
            raise ContractError(f"{label} exceeds {max_bytes} bytes")
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
        _walk(value)
        if canonical_json(value, max_bytes=max_bytes) != raw:
            raise ContractError(f"{label} is not exact compact canonical JSON")
        return value
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError,
            json.JSONDecodeError) as error:
        raise ContractError(f"invalid {label} UTF-8 JSON: {error}") from error


def decode_base64url(value: object) -> bytes:
    if (not isinstance(value, str) or not 3 <= len(value) <= MAX_GRAPH_BASE64URL_BYTES or
            BASE64URL_RE.fullmatch(value) is None):
        raise ContractError("request graph observation is not bounded unpadded Base64URL")
    try:
        encoded = value.encode("ascii")
        padding = b"=" * ((4 - len(encoded) % 4) % 4)
        raw = base64.b64decode(encoded + padding, altchars=b"-_", validate=True)
    except (UnicodeError, ValueError, binascii.Error) as error:
        raise ContractError(f"request graph observation Base64URL is invalid: {error}") from error
    if len(raw) > MAX_GRAPH_BYTES:
        raise ContractError(f"decoded graph observation exceeds {MAX_GRAPH_BYTES} bytes")
    if base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii") != value:
        raise ContractError("request graph observation Base64URL is not canonical")
    return raw


def domain_digest(domain: bytes, value: object, *, max_bytes: int) -> str:
    return hashlib.sha256(domain + canonical_json(value, max_bytes=max_bytes)).hexdigest()


def self_digest(domain: bytes, value: dict[str, object], field: str,
                *, max_bytes: int) -> str:
    preimage = dict(value)
    preimage[field] = ""
    return domain_digest(domain, preimage, max_bytes=max_bytes)
