"""Dependency-free strict canonical JSON and domain-digest helpers."""

from __future__ import annotations

import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from .constants import MAX_GOLDEN_BYTES

MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ARRAY_ITEMS = 256
MAX_STRING_BYTES = 512 * 1024


class ContractError(ValueError):
    """Explicit bytes do not satisfy the frozen structural candidate."""


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


def _forbidden_scalar(character: str) -> bool:
    return _FORBIDDEN_SCALAR_RE.fullmatch(character) is not None


def _measure_string(value: str, remaining: list[int]) -> None:
    if _FORBIDDEN_SCALAR_RE.search(value) is not None:
        raise ContractError("string contains forbidden control, bidi, or surrogate scalar")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ContractError("string is not Unicode scalar text") from error
    if len(encoded) > MAX_STRING_BYTES:
        raise ContractError(f"string byte length exceeds {MAX_STRING_BYTES}")
    _consume(2 + len(encoded) + value.count('"') + value.count("\\"), remaining)


def _consume(amount: int, remaining: list[int]) -> None:
    if amount < 0 or amount > remaining[0]:
        raise ContractError("canonical JSON exceeds its configured byte ceiling")
    remaining[0] -= amount


def _measure_object(value: dict[Any, Any], depth: int, remaining: list[int]) -> None:
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
        _measure_node(child, depth + 1, remaining)


def _measure_array(value: list[Any], depth: int, remaining: list[int]) -> None:
    if len(value) > MAX_ARRAY_ITEMS:
        raise ContractError(f"array item count exceeds {MAX_ARRAY_ITEMS}")
    _consume(2, remaining)
    for index, child in enumerate(value):
        if index:
            _consume(1, remaining)
        _measure_node(child, depth + 1, remaining)


def _measure_node(value: Any, depth: int, remaining: list[int]) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}")
    if isinstance(value, dict):
        _measure_object(value, depth, remaining)
    elif isinstance(value, list):
        _measure_array(value, depth, remaining)
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


def bounded_canonical_json(value: Any, maximum: int, label: str) -> bytes:
    remaining = [maximum]
    _measure_node(value, 1, remaining)
    measured = maximum - remaining[0]
    if measured < 1:
        raise ContractError(f"{label} canonical byte length must be 1..{maximum}")
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                         sort_keys=True).encode("utf-8")
    if len(encoded) != measured:
        raise ContractError(f"{label} canonical byte measurement drifted")
    return encoded


def canonical_json(value: Any) -> bytes:
    return bounded_canonical_json(value, MAX_GOLDEN_BYTES, "canonical JSON")


def decode_canonical(raw: bytes, maximum: int, label: str) -> Any:
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                           parse_int=_integer, parse_float=_reject_number,
                           parse_constant=_reject_number)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ContractError(f"{label} is not strict UTF-8 JSON: {error}") from error
    if bounded_canonical_json(value, maximum, label) != raw:
        raise ContractError(f"{label} is not exact compact canonical JSON")
    return value


def self_digest(domain: bytes, value: dict[str, Any], fields: tuple[str, ...],
                maximum: int, label: str, signed: bool = False) -> str:
    bounded_canonical_json(value, maximum, label)
    payload = copy.deepcopy(value)
    for field in fields:
        payload[field] = ""
    if signed:
        try:
            payload["signature"]["signature_base64url"] = ""
        except (KeyError, TypeError) as error:
            raise ContractError(f"{label} has no closed signature object") from error
    encoded = bounded_canonical_json(payload, maximum, f"{label} digest preimage")
    return hashlib.sha256(domain + encoded).hexdigest()


def signature_message(domain: bytes, digest_hex: str) -> bytes:
    if not isinstance(domain, bytes) or not domain.endswith(b"\0"):
        raise ContractError("signature domain must be NUL-terminated bytes")
    if not isinstance(digest_hex, str):
        raise ContractError("signature digest must be lowercase SHA-256 hex")
    try:
        raw = bytes.fromhex(digest_hex)
    except ValueError as error:
        raise ContractError("signature digest must be lowercase SHA-256 hex") from error
    if len(raw) != 32 or raw.hex() != digest_hex:
        raise ContractError("signature digest must be lowercase SHA-256 hex")
    return domain + raw


def read_bounded_file(path: Path, maximum: int, label: str) -> bytes:
    try:
        with path.open("rb") as stream:
            raw = stream.read(maximum + 1)
    except OSError as error:
        raise ContractError(f"cannot read {label}: {error}") from error
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return raw
