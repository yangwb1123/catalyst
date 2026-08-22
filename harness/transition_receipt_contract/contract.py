"""Strict golden-envelope validation for TransitionReceipt v1."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .assessment import validate_assessment, validate_request
from .canonical import ContractError, decode_canonical, read_bounded_file
from .constants import MAX_GOLDEN_BYTES
from .receipt import declared_target, validate_receipt
from .vocabulary import validate_vocabulary

FIXTURE = Path("docs/contracts/fixtures/transition-receipt-v1.json")
GOLDEN_FIELDS = {
    "assessment_request", "expected_approval_refs", "expected_assessment",
    "expected_capability_grant_ref", "transition_receipt", "transition_vocabulary",
}


def validate_golden(value: Any) -> dict[str, Any]:
    if (not isinstance(value, dict) or len(value) != len(GOLDEN_FIELDS) or
            any(key not in value for key in GOLDEN_FIELDS)):
        raise ContractError("TransitionReceipt golden envelope has unexpected fields")
    vocabulary = validate_vocabulary(value["transition_vocabulary"])
    receipt = validate_receipt(value["transition_receipt"])
    request = validate_request(value["assessment_request"])
    if receipt["transition_vocabulary_sha256"] != vocabulary["vocabulary_sha256"]:
        raise ContractError("golden TransitionReceipt vocabulary binding drifted")
    if request["transition_receipt"] != receipt or request["previous_receipt"] is not None:
        raise ContractError("golden request does not embed the exact initial receipt")
    if request["expected_target"] != declared_target(receipt):
        raise ContractError("golden target is not the exact TransitionReceipt projection")
    if receipt["approval_refs"] != value["expected_approval_refs"]:
        raise ContractError("golden ApprovalRef projection drifted")
    if receipt["capability_grant_ref"] != value["expected_capability_grant_ref"]:
        raise ContractError("golden CapabilityGrant ref projection drifted")
    validate_assessment(request, value["expected_assessment"])
    return value


def load_golden(repo_root: Path) -> dict[str, Any]:
    raw = read_bounded_file(repo_root / FIXTURE, MAX_GOLDEN_BYTES,
                            "TransitionReceipt golden fixture")
    payload = raw[:-1] if raw.endswith(b"\n") else raw
    return validate_golden(decode_canonical(payload, MAX_GOLDEN_BYTES,
                                            "TransitionReceipt golden fixture"))
