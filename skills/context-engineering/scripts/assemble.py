#!/usr/bin/env python3
"""Package-local pure adapter for ContextPackage v1 assembly."""

import sys

if not sys.flags.isolated:
    sys.stderr.write(
        "context-engineering assembly rejected: isolated Python (-I) is required\n"
    )
    raise SystemExit(1)

import importlib.util
from pathlib import Path
from types import ModuleType

USAGE = "assemble.py < canonical-request.json > context-package.json"
MAX_REQUEST_BYTES = 20 * 1024 * 1024
MAX_PACKAGE_BYTES = 2 * 1024 * 1024
CONTRACT_MODULE = "_forgeos_bundled_context_package_contract_v1"
_contract_module: ModuleType | None = None


class AdapterError(ValueError):
    """A stable package-local adapter rejection."""


def _load_contract() -> ModuleType:
    global _contract_module
    if _contract_module is not None:
        return _contract_module
    package = Path(__file__).resolve().parent / "_vendor/context_package_contract"
    specification = importlib.util.spec_from_file_location(
        CONTRACT_MODULE, package / "__init__.py",
        submodule_search_locations=[str(package)],
    )
    if specification is None or specification.loader is None:
        raise AdapterError("bundled contract loader is unavailable")
    module = importlib.util.module_from_spec(specification)
    sys.modules[CONTRACT_MODULE] = module
    try:
        specification.loader.exec_module(module)
    except Exception as error:
        sys.modules.pop(CONTRACT_MODULE, None)
        raise AdapterError("bundled contract could not be loaded") from error
    _contract_module = module
    return module


def canonical_json(value: object) -> bytes:
    """Expose the bundled canonical encoder without a sys.path mutation."""
    return _load_contract().canonical_json(value)


def _read_request() -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise AdapterError("stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, MAX_REQUEST_BYTES + 1 - len(raw)))
        except BlockingIOError as error:
            raise AdapterError("stdin did not reach EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise AdapterError("stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MAX_REQUEST_BYTES:
            raise AdapterError("request exceeds its byte bound")


def build(raw: bytes) -> bytes:
    """Assemble and fully revalidate one exact supplied request."""
    contract = _load_contract()
    try:
        request = contract.decode_request(raw)
        counter = contract.Utf8ByteTokenCounter()
        package = contract.assemble(request, counter)
        contract.validate_package(request, package, counter)
        payload = contract.canonical_json(package)
    except contract.ContractError as error:
        raise AdapterError(str(error)) from error
    if len(payload) > MAX_PACKAGE_BYTES:
        raise AdapterError("package exceeds its byte bound")
    return payload + b"\n"


def _write_all(raw: bytes) -> None:
    stream = getattr(sys.stdout, "buffer", None)
    if stream is None:
        raise AdapterError("stdout binary stream is unavailable")
    offset = 0
    while offset < len(raw):
        written = stream.write(raw[offset:])
        remaining = len(raw) - offset
        if (isinstance(written, bool) or not isinstance(written, int) or
                written <= 0 or written > remaining):
            raise AdapterError("stdout made no forward progress")
        offset += written
    stream.flush()


def main(argv: list[str]) -> int:
    if argv:
        print(f"usage: {USAGE}", file=sys.stderr)
        return 2
    try:
        output = build(_read_request())
        _write_all(output)
    except (AdapterError, MemoryError, OSError, ValueError) as error:
        print(f"context-engineering assembly rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
