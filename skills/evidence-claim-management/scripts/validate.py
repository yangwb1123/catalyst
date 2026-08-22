#!/usr/bin/env python3
"""Validate one supplied EvidenceRecord/KnowledgeClaim record set."""

import sys

if not sys.flags.isolated:
    sys.stderr.write(
        "evidence-claim validation rejected: isolated Python (-I) is required\n"
    )
    raise SystemExit(1)

import importlib.util
from pathlib import Path
from types import ModuleType

MAX_RECORD_SET_BYTES = 1_048_576
SUCCESS = "STRUCTURALLY_VALID (shadow; no truth or authority attestation)"
USAGE = "validate.py < canonical-record-set.json"
CONTRACT_MODULE = "_forgeos_bundled_governance_contract_v1"
_contract_module: ModuleType | None = None


class AdapterError(ValueError):
    """A stable portable-adapter rejection."""


def _purge_contract_modules() -> None:
    prefix = CONTRACT_MODULE + "."
    for name in tuple(sys.modules):
        if name == CONTRACT_MODULE or name.startswith(prefix):
            sys.modules.pop(name, None)


def _load_contract() -> ModuleType:
    global _contract_module
    if _contract_module is not None:
        return _contract_module
    package = Path(__file__).resolve().parent / "_vendor/governance_contract"
    specification = importlib.util.spec_from_file_location(
        CONTRACT_MODULE, package / "__init__.py",
        submodule_search_locations=[str(package)],
    )
    if specification is None or specification.loader is None:
        raise AdapterError("bundled contract loader is unavailable")
    _purge_contract_modules()
    module = importlib.util.module_from_spec(specification)
    sys.modules[CONTRACT_MODULE] = module
    try:
        specification.loader.exec_module(module)
    except BaseException as error:
        _purge_contract_modules()
        raise AdapterError("bundled contract could not be loaded") from error
    _contract_module = module
    return module


def _read_record_set() -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise AdapterError("stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, MAX_RECORD_SET_BYTES + 1 - len(raw)))
        except BlockingIOError as error:
            raise AdapterError("stdin did not reach EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise AdapterError("stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MAX_RECORD_SET_BYTES:
            raise AdapterError("record set exceeds its byte bound")


def validate(raw: bytes) -> None:
    """Validate exact supplied bytes without authoring or normalizing records."""
    issues = _load_contract().check_record_set_bytes(raw)
    if issues:
        raise AdapterError("supplied record set is not structurally valid")


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
        sys.stderr.write(f"usage: {USAGE}\n")
        return 2
    try:
        validate(_read_record_set())
        _write_all((SUCCESS + "\n").encode("ascii"))
    except Exception:
        sys.stderr.write("evidence-claim validation rejected\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
