"""Logical validation of the shared physical cross-language golden."""

from __future__ import annotations

from pathlib import Path
import hashlib

from .codec import ContractError, decode_canonical
from .constants import (
    FIXTURE_API, FIXTURE_PATH, FROZEN_FIXTURE_BYTES, FROZEN_FIXTURE_SHA256,
    MAX_GOLDEN_BYTES,
)
from .resolver import validate_assessment
from .filesystem import assert_snapshots, guard_root, read_regular, stable_root
from .validation import validate_registry, validate_request

CASE_IDS = {
    "legacy_repository_reader_not_registered",
    "registered_key_digest_mismatch",
    "resolved_exact",
}


def _mapping(value: object, label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != CASE_IDS:
        raise ContractError(f"golden.{label}: expected exact cross-language cases")
    return value


def validate_fixture(value: object) -> dict[str, object]:
    fields = {"api_version", "assessments", "fixture_semantics", "registry", "requests"}
    if not isinstance(value, dict) or set(value) != fields:
        raise ContractError("golden: unknown or missing fields")
    if value["api_version"] != FIXTURE_API:
        raise ContractError("golden.api_version: unsupported fixture")
    if value["fixture_semantics"] != (
            "exact_cross_language_declared_resolution_without_authority"):
        raise ContractError("golden.fixture_semantics: authority boundary drifted")
    registry = validate_registry(value["registry"])
    requests = _mapping(value["requests"], "requests")
    assessments = _mapping(value["assessments"], "assessments")
    expected_resolutions = {
        "legacy_repository_reader_not_registered": "capability_id_not_found",
        "registered_key_digest_mismatch": "capability_contract_digest_mismatch",
        "resolved_exact": "resolved_exact",
    }
    for case_id in sorted(CASE_IDS):
        request = validate_request(requests[case_id])
        assessment = validate_assessment(registry, request, assessments[case_id])
        if assessment["resolution"] != expected_resolutions[case_id]:
            raise ContractError(f"golden.{case_id}: frozen resolution drifted")
    return value


def load_fixture(repo_root: Path) -> dict[str, object]:
    root, root_identity = stable_root(repo_root)
    guard = guard_root(root, root_identity)
    raw = read_regular(root, str(FIXTURE_PATH), MAX_GOLDEN_BYTES, guard)
    assert_snapshots(guard, "repository root")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("Capability Registry v1 golden requires exactly one terminal LF")
    if (len(raw) != FROZEN_FIXTURE_BYTES or
            hashlib.sha256(raw).hexdigest() != FROZEN_FIXTURE_SHA256):
        raise ContractError("Capability Registry v1 golden physical digest drifted")
    payload = raw[:-1]
    if b"\n" in payload:
        raise ContractError("Capability Registry v1 golden contains noncanonical framing")
    return validate_fixture(decode_canonical(
        payload, max_bytes=MAX_GOLDEN_BYTES, label="Capability Registry v1 golden"))


def validate_golden_fixture(repo_root: Path) -> list[str]:
    try:
        load_fixture(repo_root)
        return []
    except (ContractError, OSError, ValueError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    except (MemoryError, RecursionError) as error:
        return [f"{FIXTURE_PATH}: bounded processing failed: {error}"]
