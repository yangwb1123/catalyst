"""Strict bounded JSON and canonical digest support."""

from __future__ import annotations

import hashlib
import json
import re
from typing import Any

from .constants import (BOOLEAN_TYPE, DOCUMENT_INVALID, INTEGER_TYPE,
                        MAX_ARRAY_ITEMS, MAX_JSON_DEPTH, MAX_OBJECT_FIELDS,
                        MAX_STRING_BYTES, OBJECT_TYPE, TEXT_TYPE, VALUE_INVALID)

_FORBIDDEN = re.compile(
    "[\\x00-\\x1f\\x7f-\\x9f\\ud800-\\udfff\\u061c\\u200e\\u200f"
    "\\u2028-\\u202e\\u2066-\\u2069]"
)
MAX_I64 = (1 << 63) - 1


class ContractError(ValueError):
    """One deterministic contract rejection."""

    def __init__(self, message: str, code: str = VALUE_INVALID) -> None:
        self.code = code
        self.detail = message
        super().__init__(f"{code}: {message}")

    def with_code(self, code: str) -> "ContractError":
        """Return the same diagnostic under one stable public rejection class."""
        return ContractError(self.detail, code)


def decode_canonical(raw: bytes, maximum: int) -> dict[str, Any]:
    """Decode one exact compact canonical JSON object."""
    value = decode_json(raw, maximum)
    if type(value) is not dict:
        raise ContractError("contract root must be a JSON object")
    if canonical_json(value, maximum) != raw:
        raise ContractError("input is not exact compact canonical JSON")
    return value


def decode_json(raw: bytes, maximum: int) -> Any:
    """Decode bounded strict JSON while preserving duplicate rejection."""
    if type(raw) is not bytes:
        raise ContractError("strict JSON input must be immutable bytes")
    if not raw or len(raw) > maximum:
        raise ContractError(f"JSON byte length must be 1..{maximum}")
    try:
        text = raw.decode("utf-8")
        _precheck_depth(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"invalid strict JSON: {error}") from error
    validate_json_value(value, remaining=[maximum])
    return value


def canonical_json(value: Any, maximum: int) -> bytes:
    """Encode one bounded value using exact Platform canonical JSON."""
    try:
        writer = _CanonicalWriter(maximum)
        writer.append(value, 1)
    except ContractError:
        raise
    except (KeyError, MemoryError, RuntimeError, TypeError,
            UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error
    return bytes(writer.output)


def observation_digest(domain: bytes, canonical: bytes) -> str:
    """Return one domain-separated conformance observation digest."""
    return hashlib.sha256(domain + canonical).hexdigest()


def validate_json_value(value: Any, depth: int = 1,
                        remaining: list[int] | None = None) -> None:
    """Validate the shared JSON scalar, depth, and collection profile."""
    if remaining is None:
        remaining = [MAX_STRING_BYTES]
    if remaining[0] == 0:
        raise ContractError("JSON aggregate occurrence budget exceeded")
    remaining[0] -= 1
    if depth > MAX_JSON_DEPTH:
        raise ContractError(f"JSON depth exceeds {MAX_JSON_DEPTH}")
    if value is None or type(value) is bool:
        return
    if type(value) is int:
        if not -MAX_I64 - 1 <= value <= MAX_I64:
            raise ContractError("JSON integer exceeds signed int64")
        return
    if type(value) is str:
        validate_text(value, "JSON string", MAX_STRING_BYTES, False)
        return
    if type(value) is list:
        _validate_array(value, depth, remaining)
        return
    if type(value) is dict:
        _validate_object(value, depth, remaining)
        return
    raise ContractError(f"unsupported JSON value {type(value).__name__}")


def validate_text(value: Any, label: str, maximum: int, nonempty: bool = True) -> str:
    """Return validated bounded UTF-8 text."""
    if type(value) is not str:
        raise ContractError(f"{label} must be text", DOCUMENT_INVALID)
    if len(value) > maximum:
        raise ContractError(f"{label} must be within 1..{maximum} UTF-8 bytes")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"{label} is not valid UTF-8") from error
    if len(encoded) > maximum or nonempty and not value:
        raise ContractError(f"{label} must be within 1..{maximum} UTF-8 bytes")
    if _FORBIDDEN.search(value):
        raise ContractError(f"{label} contains a forbidden Unicode scalar")
    return value


def validate_schema_name(value: Any, label: str) -> str:
    """Return one validated lowercase schema namespace."""
    text = validate_text(value, label, 128)
    parts = text.split(".")
    if not 3 <= len(parts) <= 6 or any(
            not 1 <= len(part) <= 32 or not lower_token(part) for part in parts):
        raise ContractError(f"{label} must have three to six lowercase segments")
    return text


def validate_hash(value: Any, label: str) -> str:
    """Return one validated lowercase bare SHA-256."""
    if type(value) is not str:
        raise ContractError(f"{label} must be text", DOCUMENT_INVALID)
    if not re.fullmatch(r"[a-f0-9]{64}", value):
        raise ContractError(f"{label} must be a lowercase bare SHA-256")
    return value


def validate_i64(value: Any, label: str, minimum: int = -MAX_I64 - 1,
                 maximum: int = MAX_I64) -> int:
    """Return an exact Python int inside the requested signed-i64 range."""
    if type(value) is not int:
        raise ContractError(f"{label} must be a signed int64", DOCUMENT_INVALID)
    if not minimum <= value <= maximum:
        raise ContractError(f"{label} must be a signed int64 in {minimum}..{maximum}")
    return value


def lower_token(value: str) -> bool:
    """Whether value is one nonempty lowercase snake token."""
    return re.fullmatch(r"[a-z][a-z0-9_]*", value) is not None


def exact_object(value: Any, fields: set[str], label: str) -> dict[str, Any]:
    """Return an object only when its field set is exact."""
    if (type(value) is not dict or len(value) != len(fields) or
            any(type(key) is not str or key not in fields for key in value)):
        raise ContractError(
            f"{label} does not have its exact required fields", DOCUMENT_INVALID)
    return value


def validate_document_shape(value: Any, profile: Any, label: str) -> None:
    """Validate one complete exact object/array/scalar type profile."""
    if type(profile) is dict:
        if type(value) is not dict:
            raise ContractError(f"{label} must be an object", DOCUMENT_INVALID)
        if len(value) > MAX_OBJECT_FIELDS:
            raise ContractError(
                f"{label} exceeds {MAX_OBJECT_FIELDS} fields", DOCUMENT_INVALID)
        if (len(value) != len(profile) or
                any(type(key) is not str or key not in profile for key in value)):
            raise ContractError(
                f"{label} does not have its exact required fields", DOCUMENT_INVALID)
        for field, child_profile in profile.items():
            validate_document_shape(value[field], child_profile,
                                    f"{label}.{field}")
        return
    if type(profile) is tuple:
        kind, child_profile = profile
        if kind == "nullable" and value is None:
            return
        if kind == "array":
            if type(value) is not list:
                raise ContractError(f"{label} must be an array", DOCUMENT_INVALID)
            if len(value) > MAX_ARRAY_ITEMS:
                raise ContractError(
                    f"{label} exceeds {MAX_ARRAY_ITEMS} items", VALUE_INVALID)
            for index, child in enumerate(value):
                validate_document_shape(child, child_profile, f"{label}[{index}]")
            return
        validate_document_shape(value, child_profile, label)
        return
    valid = ((profile == TEXT_TYPE and type(value) is str) or
             (profile == INTEGER_TYPE and type(value) is int) or
             (profile == BOOLEAN_TYPE and type(value) is bool) or
             (profile == OBJECT_TYPE and type(value) is dict))
    if not valid:
        raise ContractError(f"{label} has the wrong JSON scalar type", DOCUMENT_INVALID)


class _CanonicalWriter:
    def __init__(self, maximum: int) -> None:
        self.maximum = maximum
        self.output = bytearray()
        self.remaining_nodes = maximum

    def append(self, value: Any, depth: int) -> None:
        if depth > MAX_JSON_DEPTH:
            raise ContractError(f"JSON depth exceeds {MAX_JSON_DEPTH}")
        if self.remaining_nodes == 0:
            raise ContractError("JSON aggregate occurrence budget exceeded")
        self.remaining_nodes -= 1
        if value is None:
            self.write(b"null")
        elif type(value) is bool:
            self.write(b"true" if value else b"false")
        elif type(value) is int:
            if not -MAX_I64 - 1 <= value <= MAX_I64:
                raise ContractError("JSON integer exceeds signed int64")
            self.write(str(value).encode("ascii"))
        elif type(value) is str:
            self.append_string(value, "JSON string", False)
        elif type(value) is list:
            self.append_array(value, depth)
        elif type(value) is dict:
            self.append_object(value, depth)
        else:
            raise ContractError(f"unsupported JSON value {type(value).__name__}")

    def append_array(self, value: list[Any], depth: int) -> None:
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"JSON array exceeds {MAX_ARRAY_ITEMS} items")
        self.write(b"[")
        for index, child in enumerate(value):
            if index:
                self.write(b",")
            self.append(child, depth + 1)
        self.write(b"]")

    def append_object(self, value: dict[Any, Any], depth: int) -> None:
        if len(value) > MAX_OBJECT_FIELDS:
            raise ContractError(f"JSON object exceeds {MAX_OBJECT_FIELDS} fields")
        for key in value:
            validate_text(key, "JSON object key", MAX_STRING_BYTES)
        self.write(b"{")
        for index, key in enumerate(sorted(value)):
            if index:
                self.write(b",")
            self.append_string(key, "JSON object key")
            self.write(b":")
            self.append(value[key], depth + 1)
        self.write(b"}")

    def append_string(self, value: Any, label: str, nonempty: bool = True) -> None:
        text = validate_text(value, label, MAX_STRING_BYTES, nonempty)
        encoded = text.encode("utf-8").replace(b"\\", b"\\\\").replace(b'"', b'\\"')
        self.write(b'"' + encoded + b'"')

    def write(self, value: bytes) -> None:
        if len(value) > self.maximum - len(self.output):
            raise ContractError(f"canonical JSON exceeds {self.maximum} bytes")
        self.output.extend(value)


def _validate_array(value: list[Any], depth: int, remaining: list[int]) -> None:
    if len(value) > MAX_ARRAY_ITEMS:
        raise ContractError(f"JSON array exceeds {MAX_ARRAY_ITEMS} items")
    for child in value:
        validate_json_value(child, depth + 1, remaining)


def _validate_object(value: dict[Any, Any], depth: int, remaining: list[int]) -> None:
    if len(value) > MAX_OBJECT_FIELDS:
        raise ContractError(f"JSON object exceeds {MAX_OBJECT_FIELDS} fields")
    for key, child in value.items():
        validate_text(key, "JSON object key", MAX_STRING_BYTES)
        validate_json_value(child, depth + 1, remaining)


def _pairs_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _parse_int(value: str) -> int:
    parsed = int(value)
    if str(parsed) != value or not -MAX_I64 - 1 <= parsed <= MAX_I64:
        raise ContractError("JSON integer is not canonical signed int64")
    return parsed


def _reject_number(value: str) -> None:
    raise ContractError(f"floating or non-finite JSON number {value!r} is forbidden")


def _precheck_depth(text: str) -> None:
    depth, quoted, escaped = 0, False, False
    for character in text:
        if quoted:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quoted = False
        elif character == '"':
            quoted = True
        elif character in "[{":
            depth += 1
            if depth > MAX_JSON_DEPTH:
                raise ContractError(f"JSON depth exceeds {MAX_JSON_DEPTH}")
        elif character in "]}":
            depth -= 1
