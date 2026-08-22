"""Shared cross-language ADR-0069 golden construction and validation."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .codec import ContractError, canonical_json, decode_canonical
from .constants import (
    API_GOLDEN, CANONICAL, FIXTURE_PATH, FIXTURE_SHA256, MAX_PROJECTION_BYTES,
)
from .physical import build_current, read_regular, stable_root
from .projection import validate_projection
from .request import validate_request
from .shapes import exact_object, fixed

GOLDEN_FIELDS = {"api_version", "canonicalization", "projection", "request"}
MAX_GOLDEN_BYTES = MAX_PROJECTION_BYTES + 2_097_152


def golden_value(repo_root: Path) -> dict[str, object]:
    request, projection = build_current(repo_root)
    return {
        "api_version": API_GOLDEN,
        "canonicalization": CANONICAL,
        "projection": projection,
        "request": request,
    }


def validate_golden(value: object) -> dict[str, object]:
    golden = exact_object(value, GOLDEN_FIELDS, "golden")
    fixed(golden["api_version"], API_GOLDEN, "golden.api_version")
    fixed(golden["canonicalization"], CANONICAL, "golden.canonicalization")
    request = validate_request(golden["request"])[0]
    projection = validate_projection(golden["projection"])
    if canonical_json(projection["request"]) != canonical_json(request):
        raise ContractError("golden projection does not embed the exact golden request")
    return golden


def load_golden(repo_root: Path) -> dict[str, object]:
    root, root_identity = stable_root(repo_root)
    snapshots = {root: root_identity}
    raw = read_regular(root, FIXTURE_PATH, MAX_GOLDEN_BYTES + 1, snapshots)
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("golden fixture must have exactly one terminal LF")
    if FIXTURE_SHA256 == "TO_BE_GENERATED":
        raise ContractError("golden fixture physical pin is not frozen")
    if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA256:
        raise ContractError("golden fixture physical SHA-256 drifted")
    value = decode_canonical(raw[:-1], MAX_GOLDEN_BYTES, "ownership projection golden")
    golden = validate_golden(value)
    expected = golden_value(root)
    if canonical_json(golden) != canonical_json(expected):
        raise ContractError("golden fixture differs from current exact-source reassembly")
    return golden


if __name__ == "__main__":
    import sys
    root = Path(sys.argv[1] if len(sys.argv) == 2 else ".")
    sys.stdout.buffer.write(canonical_json(golden_value(root)) + b"\n")
