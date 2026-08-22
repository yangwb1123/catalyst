"""Shared strict canonical codec for Kernel decision records."""

from __future__ import annotations

from kernel_operational_contract.codec import (
    ContractError, _walk_limits,
    canonical_json as _operational_canonical_json, decode_canonical_json,
    read_bounded_file,
)

from .constants import MAX_CLOSURE_BYTES


def _bounded_add(total: int, increment: int) -> int:
    total += increment
    if total > MAX_CLOSURE_BYTES:
        raise ContractError(
            f"canonical JSON exceeds {MAX_CLOSURE_BYTES} bytes")
    return total


def _string_size(value: str) -> int:
    escaped = sum(character in {'"', "\\"} for character in value)
    return 2 + len(value.encode("utf-8")) + escaped


def _canonical_size(value: object) -> int:
    if value is None:
        return 4
    if value is True:
        return 4
    if value is False:
        return 5
    if isinstance(value, int):
        return len(str(value))
    if isinstance(value, str):
        return _string_size(value)
    if isinstance(value, list):
        total = 2
        for index, item in enumerate(value):
            total = _bounded_add(total, index > 0)
            total = _bounded_add(total, _canonical_size(item))
        return total
    if isinstance(value, dict):
        total = 2
        for index, (key, item) in enumerate(value.items()):
            total = _bounded_add(total, index > 0)
            total = _bounded_add(total, _string_size(key) + 1)
            total = _bounded_add(total, _canonical_size(item))
        return total
    raise ContractError(f"unsupported JSON value {type(value).__name__}")


def canonical_json(value: object) -> bytes:
    """Encode canonical JSON under the ADR-0090 20-MiB document ceiling."""
    try:
        _walk_limits(value)
        _canonical_size(value)
    except ContractError:
        raise
    except (MemoryError, UnicodeError, ValueError, RecursionError) as error:
        raise ContractError(f"canonical JSON failed: {error}") from error
    return _operational_canonical_json(value)

__all__ = ["ContractError", "canonical_json", "decode_canonical_json",
           "read_bounded_file"]
