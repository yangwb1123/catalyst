"""Exact strict-Proposed ADR v2 byte and digest binding."""

from __future__ import annotations

import base64
import hashlib
import re
from typing import Any

from architecture_decision_record_v2 import validate_document_bytes
from governance_contract import ContractError as ADRV2Error

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, MAX_PROPOSAL_BINDING_BYTES,
                        MAX_PROPOSAL_BYTES, PROFILE_ID, PROPOSAL_BINDING_API,
                        PROPOSAL_BINDING_DOMAIN)
from .shape import decode_base64url, require_keys, sha256

PROPOSAL_BINDING_FIELDS = {
    "adr_id", "api_version", "body_sha256", "canonicalization", "document_name",
    "kind", "physical_sha256", "profile_id", "proposal_binding_sha256",
    "self_sha256", "status",
}


def proposal_binding_sha256(value: dict[str, Any]) -> str:
    return self_digest(PROPOSAL_BINDING_DOMAIN, value, ("proposal_binding_sha256",),
                       MAX_PROPOSAL_BINDING_BYTES, "ArchitectureDecisionProposalBinding")


def validate_proposal_binding(value: Any) -> dict[str, Any]:
    label = "ArchitectureDecisionProposalBinding"
    node = require_keys(value, label, PROPOSAL_BINDING_FIELDS)
    bounded_canonical_json(node, MAX_PROPOSAL_BINDING_BYTES, label)
    expected = (PROPOSAL_BINDING_API, CANONICALIZATION, label, PROFILE_ID, "proposed")
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"], node["status"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    adr_match = (re.fullmatch(r"ADR-((?!0000)[0-9]{4})", node["adr_id"])
                 if isinstance(node["adr_id"], str) else None)
    if adr_match is None:
        raise ContractError("proposal_binding.adr_id is malformed")
    name_match = (re.fullmatch(r"ADR-([0-9]{4})-[a-z0-9]+(?:-[a-z0-9]+)*\.md",
                              node["document_name"])
                  if isinstance(node["document_name"], str) else None)
    if name_match is None or name_match.group(1) != adr_match.group(1):
        raise ContractError("proposal_binding.document_name is malformed")
    for field in ("body_sha256", "physical_sha256", "proposal_binding_sha256",
                  "self_sha256"):
        sha256(node[field], f"proposal_binding.{field}")
    if node["proposal_binding_sha256"] != proposal_binding_sha256(node):
        raise ContractError("proposal binding self digest does not match")
    return node


def derive_proposal_binding(proposal_bytes: bytes, document_name: str) -> dict[str, Any]:
    if not isinstance(proposal_bytes, bytes) or not 1 <= len(proposal_bytes) <= MAX_PROPOSAL_BYTES:
        raise ContractError("proposal bytes must be non-empty and at most 262144 bytes")
    try:
        metadata = validate_document_bytes(proposal_bytes, document_name)
    except ADRV2Error as error:
        raise ContractError(f"proposal is not a strict Proposed ADR v2: {error}") from error
    binding = {
        "adr_id": metadata["adr_id"],
        "api_version": PROPOSAL_BINDING_API,
        "body_sha256": metadata["body_sha256"],
        "canonicalization": CANONICALIZATION,
        "document_name": metadata["document_name"],
        "kind": "ArchitectureDecisionProposalBinding",
        "physical_sha256": hashlib.sha256(proposal_bytes).hexdigest(),
        "profile_id": PROFILE_ID,
        "proposal_binding_sha256": "",
        "self_sha256": metadata["self_sha256"],
        "status": "proposed",
    }
    binding["proposal_binding_sha256"] = proposal_binding_sha256(binding)
    return validate_proposal_binding(binding)


def validate_proposal_bytes(proposal_bytes: bytes, binding: dict[str, Any]) -> dict[str, Any]:
    validate_proposal_binding(binding)
    derived = derive_proposal_binding(proposal_bytes, binding["document_name"])
    if derived != binding:
        raise ContractError("exact proposal bytes do not match ProposalBinding")
    return derived


def encode_proposal_document(proposal_bytes: bytes) -> str:
    if not isinstance(proposal_bytes, bytes) or not 1 <= len(proposal_bytes) <= MAX_PROPOSAL_BYTES:
        raise ContractError("proposal bytes must be non-empty and at most 262144 bytes")
    return base64.urlsafe_b64encode(proposal_bytes).decode("ascii").rstrip("=")


def decode_proposal_document(value: Any, binding: dict[str, Any],
                             label: str) -> tuple[bytes, dict[str, Any]]:
    proposal_bytes = decode_base64url(value, label, MAX_PROPOSAL_BYTES)
    validate_proposal_bytes(proposal_bytes, binding)
    try:
        metadata = validate_document_bytes(proposal_bytes, binding["document_name"])
    except ADRV2Error as error:
        raise ContractError(f"proposal is not a strict Proposed ADR v2: {error}") from error
    return proposal_bytes, metadata
