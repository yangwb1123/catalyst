"""Bounded exact compact canonical JSON and digest primitives."""

from __future__ import annotations

import hashlib
import json
import re
import unicodedata

from .constants import (
    MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_I64, MAX_MANIFEST_BYTES,
    MAX_SHORT_TEXT_BYTES, MIN_I64,
)


class ContractError(ValueError):
    """Input bytes or values violate the frozen contract."""


def forbidden_scalar(character: str) -> bool:
    code = ord(character)
    return (unicodedata.category(character) == "Cc" or code == 0x7F or
            0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029, 0xFEFF} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _walk(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not MIN_I64 <= value <= MAX_I64:
            raise ContractError("integer outside signed int64")
        return
    if isinstance(value, str):
        if len(value.encode("utf-8")) > MAX_MANIFEST_BYTES:
            raise ContractError("string exceeds global UTF-8 byte limit")
        if any(forbidden_scalar(character) for character in value):
            raise ContractError("string contains forbidden Unicode scalar")
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError("array exceeds item limit")
        for item in value:
            _walk(item, depth + 1)
        return
    if not isinstance(value, dict) or len(value) > MAX_FIELDS:
        raise ContractError("expected bounded JSON object")
    for key, item in value.items():
        if not isinstance(key, str) or re.fullmatch(r"[a-z][a-z0-9_]*", key) is None:
            raise ContractError(f"invalid canonical object key {key!r}")
        _walk(item, depth + 1)


def canonical_json(value: object, maximum: int | None = None) -> bytes:
    try:
        _walk(value)
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except (MemoryError, RecursionError, UnicodeError, ValueError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error
    if maximum is not None and len(raw) > maximum:
        raise ContractError(f"canonical JSON exceeds {maximum} bytes")
    return raw


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _integer(text: str) -> int:
    if re.fullmatch(r"0|-[1-9][0-9]*|[1-9][0-9]*", text) is None:
        raise ContractError("noncanonical JSON integer")
    if len(text) > 20:
        raise ContractError("integer outside signed int64")
    value = int(text)
    if not MIN_I64 <= value <= MAX_I64:
        raise ContractError("integer outside signed int64")
    return value


def _number(text: str) -> None:
    raise ContractError(f"floating or non-finite number {text!r} forbidden")


def decode_canonical(raw: bytes, maximum: int, label: str) -> object:
    if not isinstance(raw, bytes) or not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                           parse_int=_integer, parse_float=_number,
                           parse_constant=_number)
        _walk(value)
    except ContractError:
        raise
    except (json.JSONDecodeError, MemoryError, RecursionError, UnicodeError) as error:
        raise ContractError(f"invalid {label}: {error}") from error
    if canonical_json(value, maximum) != raw:
        raise ContractError(f"{label} is not exact compact canonical JSON")
    return value


def domain_digest(domain: str, value: object, maximum: int = MAX_MANIFEST_BYTES) -> str:
    preimage = domain.encode("utf-8") + b"\0" + canonical_json(value, maximum)
    return hashlib.sha256(preimage).hexdigest()


def path_digest(path: str, domain: str) -> str:
    return hashlib.sha256(domain.encode("utf-8") + b"\0" + path.encode("utf-8")).hexdigest()


def short_text(value: object, label: str) -> str:
    if (not isinstance(value, str) or not value or
            len(value.encode("utf-8")) > MAX_SHORT_TEXT_BYTES):
        raise ContractError(f"{label}: expected bounded nonempty text")
    if any(forbidden_scalar(character) for character in value):
        raise ContractError(f"{label}: forbidden Unicode scalar")
    return value
