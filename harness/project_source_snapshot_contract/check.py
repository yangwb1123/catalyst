#!/usr/bin/env python3
"""Strict explicit-input and repository golden checker."""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from project_source_snapshot_contract.codec import ContractError  # noqa: E402
from project_source_snapshot_contract.constants import MAX_ENVELOPE_BYTES  # noqa: E402
from project_source_snapshot_contract.fixture import load_golden  # noqa: E402
from project_source_snapshot_contract.validation import decode_production  # noqa: E402


class UsageError(ValueError):
    """Arguments differ from the exact two-mode CLI."""


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _stdin() -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise ContractError("input stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, MAX_ENVELOPE_BYTES + 1 - len(raw)))
        except BlockingIOError as error:
            raise ContractError("input stdin did not reach EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise ContractError("input stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MAX_ENVELOPE_BYTES:
            raise ContractError("input exceeds its byte bound")


def _read_file(path: str) -> bytes:
    if path == "-":
        raw = _stdin()
        if not 1 <= len(raw) <= MAX_ENVELOPE_BYTES:
            raise ContractError("input byte length outside bound")
        return raw
    descriptor = None
    try:
        before_path = os.lstat(path)
        if stat.S_ISLNK(before_path.st_mode) or not stat.S_ISREG(before_path.st_mode):
            raise ContractError("input must be a real regular file")
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) |
                             getattr(os, "O_NOFOLLOW", 0) |
                             getattr(os, "O_NONBLOCK", 0))
        before_fd = os.fstat(descriptor)
        with os.fdopen(descriptor, "rb") as stream:
            descriptor = None
            raw = stream.read(MAX_ENVELOPE_BYTES + 1)
            after_fd = os.fstat(stream.fileno())
        after_path = os.lstat(path)
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot read input: {error}") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    if (_identity(before_path) != _identity(before_fd) or
            _identity(before_fd) != _identity(after_fd) or
            _identity(after_fd) != _identity(after_path)):
        raise ContractError("input changed during read")
    if not 1 <= len(raw) <= MAX_ENVELOPE_BYTES:
        raise ContractError("input byte length outside bound")
    return raw


def _run(arguments: list[str]) -> bytes:
    if len(arguments) != 2 or arguments[0] not in {"--golden", "--input"}:
        raise UsageError("expected exactly --golden ROOT or --input FILE|-")
    if not arguments[1] or arguments[1].startswith("-") and arguments[1] != "-":
        raise UsageError("missing or option-shaped argument value")
    if arguments[0] == "--golden":
        load_golden(Path(arguments[1]))
        return b"Project Source Snapshot v1 golden: OK (authority neutral)"
    decode_production(_read_file(arguments[1]))
    return b"Project Source Snapshot v1: OK (authority neutral)"


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
        print(f"Project Source Snapshot usage error: {error}", file=sys.stderr)
        return 2
    except (ContractError, OSError, ValueError) as error:
        print(f"Project Source Snapshot: ERROR: {error}", file=sys.stderr)
        return 1
    try:
        _emit(output)
    except OSError as error:
        print(f"Project Source Snapshot: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
