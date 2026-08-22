"""Golden fixture loading and verification."""

from __future__ import annotations

from pathlib import Path

from .assembler import assemble, validate_package
from .codec import (ContractError, canonical_json, decode_package, decode_request,
                    read_bounded_file)
from .token_counter import Utf8ByteTokenCounter


def fixture_path(repo_root: Path) -> Path:
    return repo_root / "docs" / "contracts" / "fixtures" / "context-package-v1.json"


def load_fixture(repo_root: Path) -> dict[str, object]:
    raw = read_bounded_file(fixture_path(repo_root), 24 * 1024 * 1024, "context package fixture")
    from .codec import _decode  # local import keeps the public codec surface narrow
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("fixture must end in exactly one repository newline")
    value = _decode(raw[:-1], 24 * 1024 * 1024, "context package fixture")
    if not isinstance(value, dict) or set(value) != {"expected_package", "request"}:
        raise ContractError("fixture must have exact expected_package and request fields")
    return value


def check_golden(repo_root: Path) -> None:
    fixture = load_fixture(repo_root)
    request = decode_request(canonical_json(fixture["request"]))
    package = decode_package(canonical_json(fixture["expected_package"]))
    counter = Utf8ByteTokenCounter()
    if canonical_json(assemble(request, counter)) != canonical_json(package):
        raise ContractError("golden package does not match deterministic assembly")
    validate_package(request, package, counter)
