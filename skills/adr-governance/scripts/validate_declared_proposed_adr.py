#!/usr/bin/env python3
"""Validate one exact caller-supplied Proposed ADR v2 document."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    sys.stderr.write(
        "proposed ADR v2 validation rejected: isolated no-bytecode Python (-I -B) is required\n"
    )
    raise SystemExit(1)

import importlib.util
from pathlib import Path
from types import ModuleType

MAX_DOCUMENT_BYTES = 262_144
USAGE = "validate_declared_proposed_adr.py ADR-NNNN-slug.md"
SUCCESS = (
    "STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document "
    "bytes only; no identity, ownership, approver, evidence, claim, graph, "
    "acceptance, compliance, persistence, transition, execution, or effect "
    "attestation)"
)
GOVERNANCE_MODULE = "_forgeos_bundled_governance_contract_v1"
CONTRACT_MODULE = "_forgeos_bundled_architecture_decision_record_v2"
_contract_module: ModuleType | None = None


class AdapterError(ValueError):
    """A stable portable-adapter rejection."""


def _purge_modules() -> None:
    prefixes = (GOVERNANCE_MODULE, CONTRACT_MODULE, "governance_contract")
    for name in tuple(sys.modules):
        if any(name == prefix or name.startswith(prefix + ".")
               for prefix in prefixes):
            sys.modules.pop(name, None)


def _load_package(name: str, path: Path) -> ModuleType:
    specification = importlib.util.spec_from_file_location(
        name, path / "__init__.py", submodule_search_locations=[str(path)])
    if specification is None or specification.loader is None:
        raise AdapterError("bundled contract loader is unavailable")
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


def _load_contract() -> ModuleType:
    global _contract_module
    if _contract_module is not None:
        return _contract_module
    vendor = Path(__file__).resolve().parent / "_vendor"
    _purge_modules()
    try:
        governance = _load_package(
            GOVERNANCE_MODULE, vendor / "governance_contract")
        sys.modules["governance_contract"] = governance
        contract = _load_package(
            CONTRACT_MODULE, vendor / "architecture_decision_record_v2")
    except BaseException as error:
        _purge_modules()
        raise AdapterError("bundled contract could not be loaded") from error
    _contract_module = contract
    return contract


def _read_document() -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise AdapterError("stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, MAX_DOCUMENT_BYTES + 1 - len(raw)))
        except BlockingIOError as error:
            raise AdapterError("stdin did not reach EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise AdapterError("stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MAX_DOCUMENT_BYTES:
            raise AdapterError("ADR document exceeds its byte bound")


def validate(raw: bytes, document_name: str) -> None:
    """Validate exact bytes under one caller-supplied lexical basename."""
    _load_contract().validate_document_bytes(raw, document_name)


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
    if len(argv) != 1:
        sys.stderr.write(f"usage: {USAGE}\n")
        return 2
    try:
        validate(_read_document(), argv[0])
        _write_all((SUCCESS + "\n").encode("ascii"))
    except Exception:
        sys.stderr.write("proposed ADR v2 validation rejected\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
