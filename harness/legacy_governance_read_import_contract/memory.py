"""Strict, loss-preserving parser for supplied legacy memory JSONL bytes."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any

from .canonical import ContractError, validate_text
from .constants import (
    MAX_ARRAY_ITEMS, MAX_CONFIDENCE_LEXEME_BYTES, MAX_DEPTH, MAX_FIELDS,
    MAX_MEMORY_ENTRIES, MAX_MEMORY_LINE_BYTES, MAX_SOURCE_REF_BYTES,
)

FIELDS = {
    "_format", "kind", "topic", "detail", "iteration", "source", "confidence",
    "supersedes", "created_at_unix",
}
REQUIRED = {"kind", "topic", "detail", "iteration", "created_at_unix"}
INTEGER_RE = re.compile(r"-?(?:0|[1-9][0-9]*)")
DECIMAL_RE = re.compile(
    r"(-?)(0|[1-9][0-9]*)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?"
)


@dataclass(frozen=True)
class NumberLexeme:
    raw: str


def _number(raw: str) -> NumberLexeme:
    return NumberLexeme(raw)


def _constant(raw: str) -> None:
    raise ContractError(f"non-finite JSON number {raw!r} is forbidden")


def _pairs(values: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in values:
        if key in result:
            raise ContractError(f"duplicate legacy memory key {key!r}")
        result[key] = value
    return result


def _bounded_shape(value: Any, depth: int = 1) -> None:
    if depth > MAX_DEPTH:
        raise ContractError(f"legacy memory JSON depth exceeds {MAX_DEPTH}")
    if isinstance(value, dict):
        if len(value) > MAX_FIELDS:
            raise ContractError(f"legacy memory object fields exceed {MAX_FIELDS}")
        for child in value.values():
            _bounded_shape(child, depth + 1)
    elif isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"legacy memory array items exceed {MAX_ARRAY_ITEMS}")
        for child in value:
            _bounded_shape(child, depth + 1)


def _integer(value: Any, label: str) -> int:
    if not isinstance(value, NumberLexeme) or INTEGER_RE.fullmatch(value.raw) is None:
        raise ContractError(f"{label} must be a JSON integer")
    digits = value.raw[1:] if value.raw.startswith("-") else value.raw
    if len(digits) > 19:
        raise ContractError(f"{label} is outside signed int64")
    try:
        integer = int(value.raw)
    except ValueError as error:
        raise ContractError(f"{label} is outside signed int64") from error
    if not -(2**63) <= integer <= 2**63 - 1:
        raise ContractError(f"{label} is outside signed int64")
    return integer


def _optional_text(entry: dict[str, Any], field: str) -> str | None:
    if field not in entry:
        return None
    return validate_text(entry[field], f"legacy memory {field}", MAX_SOURCE_REF_BYTES)


def _confidence(entry: dict[str, Any]) -> dict[str, str | None]:
    if "confidence" not in entry:
        return {"presence": "omitted", "raw_number_lexeme": None}
    value = entry["confidence"]
    if not isinstance(value, NumberLexeme):
        raise ContractError("legacy memory confidence must be a JSON number")
    if not 1 <= len(value.raw.encode("ascii")) <= MAX_CONFIDENCE_LEXEME_BYTES:
        raise ContractError("legacy memory confidence number lexeme is too long")
    if not _confidence_in_range(value.raw):
        raise ContractError("legacy memory confidence must be in decimal range 0..1")
    return {"presence": "explicit", "raw_number_lexeme": value.raw}


def _confidence_in_range(raw: str) -> bool:
    match = DECIMAL_RE.fullmatch(raw)
    if match is None:
        return False
    sign, integer, fraction, exponent = match.groups()
    fraction = fraction or ""
    digits = (integer + fraction).lstrip("0")
    if not digits:
        return True
    if sign:
        return False
    position = len(digits) - len(fraction) + int(exponent or "0")
    if position < 1:
        return True
    if position > 1 or digits[0] != "1":
        return False
    return not digits[1:].strip("0")


def parse_line(raw: bytes, ordinal: int) -> dict[str, Any]:
    if not 1 <= len(raw) <= MAX_MEMORY_LINE_BYTES:
        raise ContractError(f"legacy memory line {ordinal} exceeds its byte bound")
    try:
        entry = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                           parse_int=_number, parse_float=_number,
                           parse_constant=_constant)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ContractError(f"legacy memory line {ordinal} is invalid JSON: {error}") from error
    if not isinstance(entry, dict):
        raise ContractError(f"legacy memory line {ordinal} must be one JSON object")
    _bounded_shape(entry)
    if unknown := set(entry) - FIELDS:
        raise ContractError(f"legacy memory line {ordinal} has unknown fields {sorted(unknown)!r}")
    if missing := REQUIRED - set(entry):
        raise ContractError(f"legacy memory line {ordinal} lacks fields {sorted(missing)!r}")
    legacy_format = entry.get("_format")
    if "_format" in entry and legacy_format != "forgeos.memory.v1":
        raise ContractError("legacy memory _format must be omitted or forgeos.memory.v1")
    kind = entry["kind"]
    if not isinstance(kind, str) or kind not in {"decision", "gap", "lesson"}:
        raise ContractError("legacy memory kind is outside the frozen vocabulary")
    return {
        "confidence": _confidence(entry),
        "created_at_unix": _integer(entry["created_at_unix"], "created_at_unix"),
        "declared_kind": kind,
        "declared_source": _optional_text(entry, "source"),
        "declared_supersedes": _optional_text(entry, "supersedes"),
        "declared_topic": validate_text(entry["topic"], "legacy memory topic",
                                        MAX_SOURCE_REF_BYTES),
        "detail": validate_text(entry["detail"], "legacy memory detail",
                                MAX_MEMORY_LINE_BYTES),
        "iteration": _integer(entry["iteration"], "iteration"),
        "legacy_format": legacy_format,
    }


def parse_jsonl(raw: bytes) -> list[tuple[int, bytes, dict[str, Any]]]:
    lines = raw[:-1].split(b"\n")
    if len(lines) > MAX_MEMORY_ENTRIES:
        raise ContractError(f"legacy memory entry count exceeds {MAX_MEMORY_ENTRIES}")
    if any(not line for line in lines):
        raise ContractError("legacy memory JSONL must not contain blank lines")
    return [(index, line, parse_line(line, index))
            for index, line in enumerate(lines, start=1)]
