"""Bounded strict canonical JSON and domain-separated ADR v2 digests."""

from __future__ import annotations
import re

import hashlib
import json

from governance_contract import ContractError

from .constants import (
    BODY_DOMAIN, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_FRONTMATTER_BYTES,
    MAX_I64, MAX_NARRATIVE_BYTES, SELF_DOMAIN,
)


_FORBIDDEN_SCALAR_RE = re.compile('[\\x00-\\x1f\\x7f-\\x9f\\ud800-\\udfff\\u061c\\u200e\\u200f\\u2028\\u2029\\u202a-\\u202e\\u2066-\\u2069]')


def forbidden_scalar(character: str) -> bool:
    return _FORBIDDEN_SCALAR_RE.fullmatch(character) is not None


def validate_text(value: object, label: str, maximum: int = MAX_NARRATIVE_BYTES,
                  *, nonempty: bool = True) -> str:
    if not isinstance(value, str):
        raise ContractError(f"{label} must be a string")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"{label} is not valid UTF-8") from error
    if nonempty and not value.strip():
        raise ContractError(f"{label} must not be blank")
    if len(encoded) > maximum:
        raise ContractError(f"{label} exceeds {maximum} UTF-8 bytes")
    if _FORBIDDEN_SCALAR_RE.search(value) is not None:
        raise ContractError(f"{label} contains a forbidden Unicode scalar")
    return value


def _walk(value: object, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"ADR v2 JSON depth exceeds {MAX_DEPTH}")
    if value is None or isinstance(value, bool):
        return
    if isinstance(value, int):
        if not -MAX_I64 - 1 <= value <= MAX_I64:
            raise ContractError("ADR v2 integer is outside signed int64")
        return
    if isinstance(value, str):
        validate_text(value, "ADR v2 JSON string", nonempty=False)
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"ADR v2 array exceeds {MAX_ARRAY_ITEMS} items")
        for item in value:
            _walk(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported ADR v2 JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"ADR v2 object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not isinstance(key, str):
            raise ContractError("ADR v2 object key must be a string")
        validate_text(key, "ADR v2 JSON key")
        _walk(item, depth + 1)


def canonical_json(value: object) -> bytes:
    try:
        _walk(value)
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"ADR v2 canonical JSON failed: {error}") from error
    if len(raw) > MAX_FRONTMATTER_BYTES:
        raise ContractError(f"ADR v2 frontmatter exceeds {MAX_FRONTMATTER_BYTES} bytes")
    return raw


def _reject_number(value: str) -> None:
    raise ContractError(f"floating or non-finite ADR v2 number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("ADR v2 integer is outside signed int64")
    parsed = int(value)
    if not -MAX_I64 - 1 <= parsed <= MAX_I64:
        raise ContractError("ADR v2 integer is outside signed int64")
    return parsed


def _pairs_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate ADR v2 JSON key {key!r}")
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
                raise ContractError(f"ADR v2 JSON depth exceeds {MAX_DEPTH}")
        elif character in "]}":
            depth -= 1


def decode_canonical(raw: bytes) -> dict[str, object]:
    if len(raw) > MAX_FRONTMATTER_BYTES:
        raise ContractError(f"ADR v2 frontmatter exceeds {MAX_FRONTMATTER_BYTES} bytes")
    try:
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"invalid ADR v2 UTF-8 JSON: {error}") from error
    if not isinstance(value, dict):
        raise ContractError("ADR v2 frontmatter must be a JSON object")
    if canonical_json(value) != raw:
        raise ContractError("ADR v2 frontmatter is not exact compact canonical JSON")
    return value


def body_digest(body: bytes) -> str:
    return hashlib.sha256(BODY_DOMAIN + body).hexdigest()


def self_digest(metadata: dict[str, object], body: bytes) -> str:
    blanked = {**metadata, "self_sha256": ""}
    return hashlib.sha256(SELF_DOMAIN + canonical_json(blanked) + b"\0" + body).hexdigest()
