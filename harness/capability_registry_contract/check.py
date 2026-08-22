#!/usr/bin/env python3
"""Package-local strict CLI and physical golden checker for ADR-0068."""

from __future__ import annotations

import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from capability_registry_contract.codec import (  # noqa: E402
    ContractError, canonical_json, read_bounded,
)
from capability_registry_contract.constants import (  # noqa: E402
    MAX_REGISTRY_BYTES, MAX_REQUEST_BYTES,
)
from capability_registry_contract.fixture import load_fixture  # noqa: E402
from capability_registry_contract.physical import validate_physical_registry  # noqa: E402
from capability_registry_contract.resolver import resolve_declared  # noqa: E402
from capability_registry_contract.validation import (  # noqa: E402
    decode_registry, decode_request,
)

SUCCESS = "authority-neutral declared resolution only; NO trust, permission, routing, or effect"


class UsageError(ValueError):
    """CLI syntax differs from the frozen command surface."""


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


def _input(path: str, maximum: int, label: str) -> bytes:
    if path == "-":
        raw = _stdin(maximum, label)
        if not 1 <= len(raw) <= maximum:
            raise ContractError(f"{label} byte length must be 1..{maximum}")
        return raw
    return read_bounded(Path(path), maximum, label)


def _options(arguments: list[str], expected: set[str]) -> dict[str, str]:
    if len(arguments) != len(expected) * 2:
        raise UsageError("missing, repeated, unknown, or positional argument")
    result = {}
    for index in range(0, len(arguments), 2):
        option, value = arguments[index:index + 2]
        if option not in expected or option in result or not value:
            raise UsageError("missing, repeated, unknown, or positional argument")
        result[option] = value
    if set(result) != expected:
        raise UsageError("missing required argument")
    if sum(value == "-" for value in result.values()) > 1:
        raise UsageError("at most one input may be stdin")
    return result


def _golden(arguments: list[str]) -> bytes:
    if len(arguments) != 2 or arguments[0] != "--golden":
        raise UsageError("--golden requires exactly one repository root")
    root = Path(arguments[1])
    fixture = load_fixture(root)
    issues = validate_physical_registry(root, fixture["registry"])
    if issues:
        raise ContractError(issues[0])
    return f"Capability Registry v1 golden: OK ({SUCCESS})".encode()


def _validate(arguments: list[str]) -> bytes:
    options = _options(arguments, {"--registry"})
    registry = decode_registry(_input(
        options["--registry"], MAX_REGISTRY_BYTES, "registry"))
    return canonical_json(registry)


def _resolve(arguments: list[str]) -> bytes:
    options = _options(arguments, {"--registry", "--request"})
    if sum(value == "-" for value in options.values()) != 1:
        raise UsageError("resolve requires exactly one stdin input")
    registry = decode_registry(_input(
        options["--registry"], MAX_REGISTRY_BYTES, "registry"))
    request = decode_request(_input(
        options["--request"], MAX_REQUEST_BYTES, "request"))
    return canonical_json(resolve_declared(registry, request))


def _run(arguments: list[str]) -> bytes:
    if arguments and arguments[0] == "--golden":
        return _golden(arguments)
    if not arguments:
        raise UsageError("missing subcommand")
    subcommand, rest = arguments[0], arguments[1:]
    if subcommand == "validate":
        return _validate(rest)
    if subcommand == "resolve":
        return _resolve(rest)
    raise UsageError("unknown subcommand")


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
        print(f"Capability Registry v1 usage error: {error}", file=sys.stderr)
        return 2
    except (ContractError, OSError, ValueError) as error:
        print(f"Capability Registry v1: ERROR: {error}", file=sys.stderr)
        return 1
    try:
        _emit(output)
    except OSError as error:
        print(f"Capability Registry v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
