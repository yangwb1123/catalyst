"""Bounded exact canonical JSON codec for Kernel operational records."""

from __future__ import annotations

import json
import os
import stat
import unicodedata
from pathlib import Path

from .constants import (KEY_RE, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_FIELDS, MAX_I64,
                        MAX_STRING_BYTES, MIN_I64)


class ContractError(ValueError):
    """Raised when supplied bytes or values violate the frozen contract."""


def read_bounded_file(path: Path, label: str, maximum: int) -> bytes:
    """Read one single-link regular explicit input without following a symlink."""
    try:
        before = path.lstat()
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ContractError(f"{label} must be a single-link regular file")
        nofollow = getattr(os, "O_NOFOLLOW", None)
        if nofollow is None:
            raise ContractError(f"{label} no-follow reads are unsupported on this host")
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | nofollow
        descriptor = os.open(path, flags)
        try:
            opened = os.fstat(descriptor)
            identity = (opened.st_dev, opened.st_ino)
            if (identity != (before.st_dev, before.st_ino) or
                    not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1):
                raise ContractError(f"{label} changed before open or is not single-link regular")
            if opened.st_size > maximum:
                raise ContractError(f"{label} exceeds {maximum} bytes")
            with os.fdopen(descriptor, "rb", closefd=False) as stream:
                raw = stream.read(maximum + 1)
            after = os.fstat(descriptor)
            stable = (opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns,
                      opened.st_nlink, opened.st_mode)
            if stable != (after.st_size, after.st_mtime_ns, after.st_ctime_ns,
                          after.st_nlink, after.st_mode):
                raise ContractError(f"{label} changed while being read")
        finally:
            os.close(descriptor)
    except ContractError:
        raise
    except MemoryError as error:
        raise ContractError(f"{label} read exhausted memory") from error
    except OSError as error:
        raise ContractError(f"{label} cannot be read: {error}") from error
    if len(raw) > maximum:
        raise ContractError(f"{label} exceeds {maximum} bytes")
    return raw


def _reject_number(value: str) -> None:
    raise ContractError(f"non-integer JSON number {value!r} is forbidden")


def _parse_int(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > 19:
        raise ContractError("integer is outside signed int64")
    try:
        parsed = int(value)
    except ValueError as error:
        raise ContractError("integer is outside signed int64") from error
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
    return (unicodedata.category(character) == "Cc" or
            0xD800 <= code <= 0xDFFF or
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
            raise ContractError("string contains a forbidden Unicode scalar")
        if len(value.encode("utf-8")) > MAX_STRING_BYTES:
            raise ContractError(f"string exceeds {MAX_STRING_BYTES} UTF-8 bytes")
        return
    if isinstance(value, list):
        if len(value) > MAX_ARRAY_ITEMS:
            raise ContractError(f"array exceeds {MAX_ARRAY_ITEMS} items")
        for item in value:
            _walk_limits(item, depth + 1)
        return
    if not isinstance(value, dict):
        raise ContractError(f"unsupported JSON value {type(value).__name__}")
    if len(value) > MAX_FIELDS:
        raise ContractError(f"object exceeds {MAX_FIELDS} fields")
    for key, item in value.items():
        if not isinstance(key, str) or KEY_RE.fullmatch(key) is None:
            raise ContractError(f"object key {key!r} is not ASCII snake_case")
        _walk_limits(key)
        _walk_limits(item, depth + 1)


def canonical_json(value: object) -> bytes:
    """Return compact UTF-8 JSON with recursively sorted object keys."""
    try:
        _walk_limits(value)
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(",", ":")).encode("utf-8")
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error


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


def decode_canonical_json(raw: bytes, maximum: int) -> object:
    """Decode exact compact canonical JSON under a document byte ceiling."""
    if not isinstance(raw, bytes):
        raise ContractError("input must be exact bytes")
    if len(raw) > maximum:
        raise ContractError(f"input exceeds {maximum} bytes")
    try:
        text = raw.decode("utf-8")
        _precheck_nesting(text)
        value = json.loads(text, object_pairs_hook=_pairs_object,
                           parse_int=_parse_int, parse_float=_reject_number,
                           parse_constant=_reject_number)
        _walk_limits(value)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"invalid UTF-8 JSON: {error}") from error
    if canonical_json(value) != raw:
        raise ContractError("input is not exact compact canonical JSON")
    return value
