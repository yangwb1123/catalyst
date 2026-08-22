"""Strict golden-envelope reassembly for ApprovalRecord v1."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .assessment import validate_assessment, validate_request
from .canonical import ContractError, decode_canonical, read_bounded_file
from .constants import MAX_GOLDEN_BYTES
from .record import approval_ref, declared_target, validate_record

FIXTURE = Path("docs/contracts/fixtures/approval-record-v1.json")
GOLDEN_FIELDS = {
    "approval_record", "assessment_request", "expected_approval_ref",
    "expected_assessment",
}


def validate_golden(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != GOLDEN_FIELDS:
        raise ContractError("ApprovalRecord golden envelope has unexpected fields")
    record = validate_record(value["approval_record"])
    request = validate_request(value["assessment_request"])
    if request["approval_record"] != record:
        raise ContractError("golden root record differs from request record")
    if request["expected_target"] != declared_target(record):
        raise ContractError("golden expected target is not the exact record projection")
    if value["expected_approval_ref"] != approval_ref(record):
        raise ContractError("golden CapabilityGrant ApprovalRef projection drifted")
    validate_assessment(request, value["expected_assessment"])
    return value


def load_golden(repo_root: Path) -> dict[str, Any]:
    raw = read_bounded_file(repo_root / FIXTURE, MAX_GOLDEN_BYTES,
                            "ApprovalRecord golden fixture")
    fixture_raw = raw[:-1] if raw.endswith(b"\n") else raw
    decoded = decode_canonical(fixture_raw, MAX_GOLDEN_BYTES,
                               "ApprovalRecord golden fixture")
    return validate_golden(decoded)

