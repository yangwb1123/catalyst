"""Strict canonical JSON, text, base64url, and digest helpers."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from typing import Any

from .constants import MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_STRING_BYTES


class ContractError(ValueError):
    """Supplied bytes do not satisfy the frozen ADR-0086 contract."""


def _pairs(values: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in values:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _integer(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as error:
        raise ContractError("JSON number is not an integer") from error
    if str(value) != raw or not -(2**63) <= value <= 2**63 - 1:
        raise ContractError("JSON integer is not canonical signed int64")
    return value


def _reject_number(raw: str) -> None:
    raise ContractError(f"non-integer JSON number {raw!r} is forbidden")


_FORBIDDEN_SCALAR_RE = re.compile('[\\x00-\\x1f\\x7f\\ud800-\\udfff\\u061c\\u200e\\u200f\\u2028\\u2029\\u202a-\\u202e\\u2066-\\u2069]')


def forbidden_scalar(character: str) -> bool:
    return _FORBIDDEN_SCALAR_RE.fullmatch(character) is not None


def validate_text(value: Any, label: str, maximum: int = MAX_STRING_BYTES) -> str:
    if not isinstance(value, str):
        raise ContractError(f"{label} must be a string")
    if _FORBIDDEN_SCALAR_RE.search(value) is not None:
        raise ContractError(f"{label} contains a forbidden Unicode scalar")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ContractError(f"{label} is not Unicode scalar text") from error
    if not 1 <= len(encoded) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return value


def _measure(value: Any, depth: int, remaining: list[int]) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if isinstance(value, dict):
        if len(value) > MAX_FIELDS:
            raise ContractError(f"object field count exceeds {MAX_FIELDS}")
        _consume(2, remaining)
        for index, (key, child) in enumerate(value.items()):
            if not isinstance(key, str) or re.fullmatch(r"[a-z][a-z0-9_]*", key) is None:
                raise ContractError(f"object key {key!r} is not ASCII snake_case")
            if index:
                _consume(1, remaining)
            _measure_string(key, remaining)
            _consume(1, remaining)
            _measure(child, depth + 1, remaining)
    elif isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"array item count exceeds {MAX_ARRAY_ITEMS}")
        _consume(2, remaining)
        for index, child in enumerate(value):
            if index:
                _consume(1, remaining)
            _measure(child, depth + 1, remaining)
    elif isinstance(value, str):
        _measure_string(value, remaining)
    elif isinstance(value, int) and not isinstance(value, bool):
        if not -(2**63) <= value <= 2**63 - 1:
            raise ContractError("integer is outside signed int64")
        _consume(len(str(value)), remaining)
    elif value is None:
        _consume(4, remaining)
    elif isinstance(value, bool):
        _consume(4 if value else 5, remaining)
    else:
        raise ContractError(f"unsupported JSON value {type(value).__name__}")


def _measure_string(value: str, remaining: list[int]) -> None:
    if _FORBIDDEN_SCALAR_RE.search(value) is not None:
        raise ContractError("JSON string contains a forbidden Unicode scalar")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ContractError("JSON string is not Unicode scalar text") from error
    if len(encoded) > MAX_STRING_BYTES:
        raise ContractError(f"JSON string byte length exceeds {MAX_STRING_BYTES}")
    _consume(2 + len(encoded) + value.count('"') + value.count("\\"), remaining)


def _consume(amount: int, remaining: list[int]) -> None:
    if amount < 0 or amount > remaining[0]:
        raise ContractError("canonical JSON exceeds its configured byte ceiling")
    remaining[0] -= amount


def canonical_json(value: Any, maximum: int, label: str) -> bytes:
    remaining = [maximum]
    _measure(value, 1, remaining)
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                         sort_keys=True).encode("utf-8")
    if len(encoded) != maximum - remaining[0]:
        raise ContractError(f"{label} canonical byte measurement drifted")
    if not encoded:
        raise ContractError(f"{label} must not be empty")
    return encoded


def decode_canonical(raw: bytes, maximum: int, label: str) -> Any:
    if not isinstance(raw, bytes) or not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                           parse_int=_integer, parse_float=_reject_number,
                           parse_constant=_reject_number)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ContractError(f"{label} is not strict UTF-8 JSON: {error}") from error
    if canonical_json(value, maximum, label) != raw:
        raise ContractError(f"{label} is not exact compact canonical JSON")
    return value


def digest(domain: bytes, value: Any, maximum: int, label: str) -> str:
    return hashlib.sha256(domain + canonical_json(value, maximum, label)).hexdigest()


def self_digest(domain: bytes, value: dict[str, Any], field: str,
                maximum: int, label: str) -> str:
    payload = copy.deepcopy(value)
    payload[field] = ""
    return digest(domain, payload, maximum, f"{label} digest preimage")


def sha256_bytes(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def require_exact_fields(value: Any, fields: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    if set(value) != fields:
        raise ContractError(f"{label} fields must be exactly {sorted(fields)!r}")
    return value
