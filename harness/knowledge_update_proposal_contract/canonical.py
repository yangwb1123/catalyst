"""Strict pre-measured canonical JSON for KnowledgeUpdateProposal v1."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any

from .constants import MAX_ARRAY, MAX_DEPTH, MAX_FIELDS, MAX_GOLDEN_BYTES, MAX_STRING_BYTES


class ContractError(ValueError):
    """Caller data does not satisfy the frozen contract."""


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


def _forbidden(character: str) -> bool:
    code = ord(character)
    return (code <= 0x1F or code == 0x7F or 0xD800 <= code <= 0xDFFF or
            code in {0x061C, 0x200E, 0x200F, 0x2028, 0x2029} or
            0x202A <= code <= 0x202E or 0x2066 <= code <= 0x2069)


def _consume(amount: int, remaining: list[int]) -> None:
    if amount < 0 or amount > remaining[0]:
        raise ContractError("canonical JSON exceeds its configured byte ceiling")
    remaining[0] -= amount


def _measure_string(value: str, remaining: list[int]) -> None:
    if any(_forbidden(character) for character in value):
        raise ContractError("string contains forbidden control, bidi, or surrogate scalar")
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ContractError("string is not Unicode scalar text") from error
    if len(encoded) > MAX_STRING_BYTES:
        raise ContractError(f"string byte length exceeds {MAX_STRING_BYTES}")
    _consume(2 + len(encoded) + value.count('"') + value.count("\\"), remaining)


def _measure_node(value: Any, depth: int, remaining: list[int]) -> None:
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
            _measure_node(child, depth + 1, remaining)
    elif isinstance(value, list):
        if len(value) > MAX_ARRAY:
            raise ContractError(f"array item count exceeds {MAX_ARRAY}")
        _consume(2, remaining)
        for index, child in enumerate(value):
            if index:
                _consume(1, remaining)
            _measure_node(child, depth + 1, remaining)
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


def _measure_bounded(value: Any, maximum: int) -> int:
    remaining = [maximum]
    try:
        _measure_node(value, 1, remaining)
    except RecursionError as error:
        raise ContractError(f"JSON depth exceeds {MAX_DEPTH}") from error
    except MemoryError as error:
        raise ContractError("canonical JSON measurement exhausted memory") from error
    return maximum - remaining[0]


def bounded_canonical_json(value: Any, maximum: int, label: str) -> bytes:
    measured = _measure_bounded(value, maximum)
    if measured < 1:
        raise ContractError(f"{label} canonical byte length must be 1..{maximum}")
    try:
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                             sort_keys=True).encode("utf-8")
    except (MemoryError, RecursionError) as error:
        raise ContractError(f"{label} canonical encoding failed within bounds") from error
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


def bounded_digest(domain: bytes, value: Any, maximum: int, label: str) -> str:
    return hashlib.sha256(domain + bounded_canonical_json(value, maximum, label)).hexdigest()


def read_bounded_file(path: Path, maximum: int, label: str) -> bytes:
    try:
        with path.open("rb") as stream:
            raw = stream.read(maximum + 1)
    except OSError as error:
        raise ContractError(f"cannot read {label}: {error}") from error
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return raw
