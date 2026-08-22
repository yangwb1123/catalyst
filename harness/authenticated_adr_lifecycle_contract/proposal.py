"""Exact immutable Proposed ADR v2 source bindings."""

from __future__ import annotations

import base64
from typing import Any

from architecture_decision_record_v2 import validate_document_bytes
from authenticated_adr_approval_contract.proposal import (
    derive_proposal_binding,
    validate_proposal_binding,
)
from governance_contract import ContractError as ADRV2Error

from .canonical import ContractError
from .constants import MAX_PROPOSAL_BYTES
from .shape import decode_base64url


def encode_proposal_document(raw: bytes) -> str:
    if not isinstance(raw, bytes) or not 1 <= len(raw) <= MAX_PROPOSAL_BYTES:
        raise ContractError("proposal bytes must be 1..262144 bytes")
    return base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=")


def decode_proposal_document(value: Any, binding: dict[str, Any],
                             label: str) -> tuple[bytes, dict[str, Any]]:
    raw = decode_base64url(value, label, MAX_PROPOSAL_BYTES)
    expected = derive_proposal_binding(raw, binding["document_name"])
    if expected != validate_proposal_binding(binding):
        raise ContractError(f"{label} does not match the exact ProposalBinding")
    try:
        metadata = validate_document_bytes(raw, binding["document_name"])
    except ADRV2Error as error:
        raise ContractError(f"{label} is not a strict Proposed ADR v2: {error}") from error
    return raw, metadata


__all__ = [
    "decode_proposal_document", "derive_proposal_binding",
    "encode_proposal_document", "validate_proposal_binding",
]
