#!/usr/bin/env python3
"""Strict ADR-0069 CLI and physical cross-language golden checker."""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from planning_capability_ownership_projection.codec import (  # noqa: E402
    ContractError, canonical_json,
)
from planning_capability_ownership_projection.constants import (  # noqa: E402
    MAX_CATALOG_BYTES, MAX_MAPPING_BYTES, MAX_PROJECTION_BYTES,
)
from planning_capability_ownership_projection.fixture import load_golden  # noqa: E402
from planning_capability_ownership_projection.projection import (  # noqa: E402
    decode_projection, project,
)
from planning_capability_ownership_projection.request import build_request  # noqa: E402


class UsageError(ValueError):
    """Arguments differ from the deliberately small command surface."""


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _stdin(maximum: int, label: str) -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise ContractError(f"{label} stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, maximum + 1 - len(raw)))
        except BlockingIOError as error:
            raise ContractError(f"{label} stdin did not reach EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise ContractError(f"{label} stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > maximum:
            raise ContractError(f"{label} exceeds {maximum} bytes")


def _read(path: str, maximum: int, label: str) -> bytes:
    if path == "-":
        raw = _stdin(maximum, label)
    else:
        descriptor = None
        try:
            before_path = os.lstat(path)
            if stat.S_ISLNK(before_path.st_mode) or not stat.S_ISREG(before_path.st_mode):
                raise ContractError(f"{label} input must be a real regular file")
            descriptor = os.open(
                path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) |
                getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0),
            )
            before_descriptor = os.fstat(descriptor)
            if not stat.S_ISREG(before_descriptor.st_mode):
                raise ContractError(f"{label} input must be a regular file")
            with os.fdopen(descriptor, "rb") as stream:
                descriptor = None
                raw = stream.read(maximum + 1)
                after_descriptor = os.fstat(stream.fileno())
            after_path = os.lstat(path)
            expected = _identity(before_path)
            if (expected != _identity(before_descriptor) or
                    expected != _identity(after_descriptor) or
                    expected != _identity(after_path)):
                raise ContractError(f"{label} input changed during read")
        except ContractError:
            raise
        except OSError as error:
            raise ContractError(f"cannot read {label}: {error}") from error
        finally:
            if descriptor is not None:
                os.close(descriptor)
    if not 1 <= len(raw) <= maximum:
        raise ContractError(f"{label} byte length must be 1..{maximum}")
    return raw


def _options(arguments: list[str], expected: set[str]) -> dict[str, str]:
    if len(arguments) != len(expected) * 2:
        raise UsageError("missing, repeated, unknown, or positional argument")
    result = {}
    for index in range(0, len(arguments), 2):
        option, value = arguments[index:index + 2]
        if (option not in expected or option in result or not value or
                value.startswith("-") and value != "-"):
            raise UsageError("missing, repeated, unknown, or positional argument")
        result[option] = value
    if set(result) != expected:
        raise UsageError("missing required argument")
    return result


def _run(arguments: list[str]) -> bytes:
    if len(arguments) == 2 and arguments[0] == "--golden":
        load_golden(Path(arguments[1]))
        return b"Planning Capability Ownership Projection v1 golden: OK (planning only)"
    if not arguments:
        raise UsageError("missing subcommand")
    command, rest = arguments[0], arguments[1:]
    if command == "project":
        options = _options(rest, {"--catalog", "--mapping"})
        if sum(value == "-" for value in options.values()) != 1:
            raise UsageError("project requires exactly one stdin source")
        request = build_request(_read(options["--catalog"], MAX_CATALOG_BYTES, "catalog"),
                                _read(options["--mapping"], MAX_MAPPING_BYTES, "mapping"))
        return canonical_json(project(request), MAX_PROJECTION_BYTES)
    if command == "validate":
        options = _options(rest, {"--projection"})
        return canonical_json(decode_projection(_read(
            options["--projection"], MAX_PROJECTION_BYTES, "projection")))
    raise UsageError("unknown command")


def _emit(raw: bytes) -> None:
    output, offset = raw + b"\n", 0
    while offset < len(output):
        written = sys.stdout.buffer.write(output[offset:])
        if not isinstance(written, int) or written <= 0:
            raise OSError("stdout short write")
        offset += written
    sys.stdout.buffer.flush()


def main(argv: list[str] | None = None) -> int:
    try:
        output = _run(sys.argv[1:] if argv is None else argv)
    except UsageError as error:
        print(f"Capability ownership usage error: {error}", file=sys.stderr)
        return 2
    except (ContractError, OSError, ValueError) as error:
        print(f"Capability ownership: ERROR: {error}", file=sys.stderr)
        return 1
    try:
        _emit(output)
    except OSError as error:
        print(f"Capability ownership: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
