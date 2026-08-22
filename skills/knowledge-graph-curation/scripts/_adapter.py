#!/usr/bin/env python3
"""Shared pure adapter for the two closed GraphSnapshot projector wires."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    sys.stderr.write(
        "GraphSnapshot projection rejected: isolated no-bytecode Python (-I -B) is required\n"
    )
    raise SystemExit(1)

import importlib.util
from pathlib import Path
from types import ModuleType

MAX_REQUEST_BYTES = 24 << 20
MAX_ENVELOPE_BYTES = 96 << 20
REQUEST_FIELDS = {
    "api_version", "canonicalization", "graph_observation_base64url",
    "graph_observation_sha256", "project_id", "projector_profile_id",
    "request_sha256", "run_id",
}
CANONICALIZATION = "forgeos.canonical-json/v1"
MODULE_PACKAGE = "module_package"
GO_TEST_SOURCE = "go_test_source"
PROFILE = {
    MODULE_PACKAGE: {
        "script": "project_module_package_snapshot.py",
        "request_api": "forgeos.governance.local-go-graph-snapshot-projection-request/v1",
        "profile_id": "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1",
        "derive": "derive_envelope",
        "rejection": "module/package GraphSnapshot projection rejected",
    },
    GO_TEST_SOURCE: {
        "script": "project_go_test_source_snapshot.py",
        "request_api": "forgeos.governance.local-go-test-source-graph-snapshot-projection-request/v1",
        "profile_id": (
            "adr-0053-selected-go-module-lexical-package-test-source-"
            "partial-graph-snapshot-v1"
        ),
        "derive": "derive_test_source_envelope",
        "rejection": "Go test-source GraphSnapshot projection rejected",
    },
}
PACKAGES = (
    ("_forgeos_bundled_governance_contract_v1", "governance_contract"),
    ("_forgeos_bundled_local_command_observation_producer_v1",
     "local_command_observation_producer"),
    ("_forgeos_bundled_go_package_dependency_graph_observation_producer_v1",
     "go_package_dependency_graph_observation_producer"),
    ("_forgeos_bundled_graph_snapshot_contract_v1", "graph_snapshot_contract"),
)
_contract_module: ModuleType | None = None


class AdapterError(ValueError):
    """A stable portable-projector rejection."""


def _purge_modules() -> None:
    prefixes = tuple(name for pair in PACKAGES for name in pair)
    for name in tuple(sys.modules):
        if any(name == prefix or name.startswith(prefix + ".")
               for prefix in prefixes):
            sys.modules.pop(name, None)


def _load_package(name: str, path: Path) -> ModuleType:
    specification = importlib.util.spec_from_file_location(
        name, path / "__init__.py", submodule_search_locations=[str(path)])
    if specification is None or specification.loader is None:
        raise AdapterError("bundled semantic package loader is unavailable")
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


def _alias_loaded(private: str, canonical: str, package: ModuleType) -> None:
    sys.modules[canonical] = package
    prefix = private + "."
    for name, module in tuple(sys.modules.items()):
        if name.startswith(prefix):
            sys.modules[canonical + name[len(private):]] = module


def _load_contract() -> ModuleType:
    global _contract_module
    if _contract_module is not None:
        return _contract_module
    vendor = Path(__file__).resolve().parent / "_vendor"
    _purge_modules()
    try:
        loaded = None
        for private, canonical in PACKAGES:
            package = _load_package(private, vendor / canonical)
            _alias_loaded(private, canonical, package)
            loaded = package
    except BaseException as error:
        _purge_modules()
        raise AdapterError("bundled semantic closure could not be loaded") from error
    if loaded is None:
        raise AdapterError("bundled semantic closure is empty")
    _contract_module = loaded
    return loaded


def _read_request() -> bytes:
    stream = getattr(sys.stdin, "buffer", None)
    if stream is None:
        raise AdapterError("stdin binary stream is unavailable")
    raw = bytearray()
    while True:
        try:
            chunk = stream.read(min(65_536, MAX_REQUEST_BYTES + 1 - len(raw)))
        except BlockingIOError as error:
            raise AdapterError("stdin did not reach explicit EOF") from error
        if chunk is None or not isinstance(chunk, bytes):
            raise AdapterError("stdin did not return complete bytes through EOF")
        if not chunk:
            return bytes(raw)
        raw.extend(chunk)
        if len(raw) > MAX_REQUEST_BYTES:
            raise AdapterError("request exceeds its byte bound")


def _decode_request(contract: ModuleType, raw: bytes,
                    selected: dict[str, object]) -> dict[str, object]:
    request = contract.decode_canonical(
        raw, max_bytes=MAX_REQUEST_BYTES, label="GraphSnapshot request")
    if not isinstance(request, dict) or set(request) != REQUEST_FIELDS:
        raise AdapterError("request has the wrong exact shape")
    if (request["api_version"] != selected["request_api"] or
            request["canonicalization"] != CANONICALIZATION or
            request["projector_profile_id"] != selected["profile_id"]):
        raise AdapterError("request selected an unsupported projector wire")
    return request


def project(raw: bytes, profile: str) -> bytes:
    """Project one exact existing request object into its unique envelope."""
    selected = PROFILE[profile]
    contract = _load_contract()
    request = _decode_request(contract, raw, selected)
    graph = contract.decode_base64url(request["graph_observation_base64url"])
    derive = getattr(contract, selected["derive"])
    envelope = derive(
        graph, request["graph_observation_sha256"], request["run_id"],
        request["project_id"])
    derived_request = contract.canonical_json(
        envelope["request"], max_bytes=MAX_REQUEST_BYTES)
    if derived_request != raw:
        raise AdapterError("derived envelope request does not equal exact stdin bytes")
    output = contract.canonical_json(envelope, max_bytes=MAX_ENVELOPE_BYTES)
    if len(output) > MAX_ENVELOPE_BYTES:
        raise AdapterError("derived envelope exceeds its byte bound")
    return output + b"\n"


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


def main(profile: str, argv: list[str]) -> int:
    selected = PROFILE[profile]
    if argv:
        sys.stderr.write(
            f"usage: {selected['script']} < REQUEST.json > GRAPH-SNAPSHOT.json\n")
        return 2
    try:
        output = project(_read_request(), profile)
        _write_all(output)
    except (Exception, MemoryError):
        sys.stderr.write(str(selected["rejection"]) + "\n")
        return 1
    return 0
