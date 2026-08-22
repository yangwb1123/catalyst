"""Strict golden-envelope validation for KnowledgeUpdateProposal v1."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .assessment import validate_assessment, validate_request
from .canonical import (ContractError, bounded_canonical_json, decode_canonical,
                        read_bounded_file)
from .compatibility import project_artifact_resources
from .constants import MAX_GOLDEN_BYTES
from .proposal import declared_target, validate_proposal
from .shape import require_keys

FIXTURE = Path("docs/contracts/fixtures/knowledge-update-proposal-v1.json")
GOLDEN_FIELDS = {
    "assessment_request", "expected_artifact_resources", "expected_assessment",
    "expected_capability_grant_ref", "knowledge_update_proposal",
}


def validate_golden(value: Any) -> dict[str, Any]:
    bounded_canonical_json(value, MAX_GOLDEN_BYTES,
                           "KnowledgeUpdateProposal golden fixture")
    node = require_keys(value, "KnowledgeUpdateProposal golden", GOLDEN_FIELDS)
    proposal = validate_proposal(node["knowledge_update_proposal"])
    request = validate_request(node["assessment_request"])
    if request["knowledge_update_proposal"] != proposal:
        raise ContractError("golden request does not embed the exact proposal")
    if request["expected_target"] != declared_target(proposal):
        raise ContractError("golden request target is not the exact proposal projection")
    if proposal["capability_grant_ref"] != node["expected_capability_grant_ref"]:
        raise ContractError("golden CapabilityGrant reference drifted")
    if project_artifact_resources(proposal) != node["expected_artifact_resources"]:
        raise ContractError("golden artifact resource projection drifted")
    validate_assessment(request, node["expected_assessment"])
    return node


def load_golden(repo_root: Path) -> dict[str, Any]:
    raw = read_bounded_file(repo_root / FIXTURE, MAX_GOLDEN_BYTES,
                            "KnowledgeUpdateProposal golden fixture")
    payload = raw[:-1] if raw.endswith(b"\n") else raw
    return validate_golden(decode_canonical(payload, MAX_GOLDEN_BYTES,
                                            "KnowledgeUpdateProposal golden fixture"))
