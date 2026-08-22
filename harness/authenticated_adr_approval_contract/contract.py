"""Top-level exact candidate bundle validation and strict byte decoding."""

from __future__ import annotations

from typing import Any

from .authority import validate_signature_profile, validate_trust_root
from .canonical import ContractError, bounded_canonical_json, decode_canonical
from .constants import MAX_GOLDEN_BYTES
from .documents import (validate_receipt, validate_receipt_relations,
                        validate_request, validate_result)
from .ledger import validate_ledger
from .policy import validate_policy
from .proposal import (decode_proposal_document, validate_proposal_binding)
from .revocation import validate_revocation
from .shape import require_keys

BUNDLE_FIELDS = {
    "authorization_ledger", "authorization_policy", "authorization_receipt",
    "authorization_request", "authorization_result", "proposal_binding",
    "proposal_document_base64url", "revocation_snapshot", "signature_profile",
    "trust_root",
}


def validate_document(value: Any) -> dict[str, Any]:
    node = require_keys(value, "ADR approval candidate bundle", BUNDLE_FIELDS)
    bounded_canonical_json(node, MAX_GOLDEN_BYTES, "ADR approval candidate bundle")
    signature_profile = validate_signature_profile(node["signature_profile"])
    profile_hash = signature_profile["profile_sha256"]
    root = validate_trust_root(node["trust_root"], profile_hash)
    binding = validate_proposal_binding(node["proposal_binding"])
    _, metadata = decode_proposal_document(node["proposal_document_base64url"], binding,
                                           "proposal_document_base64url")
    policy = validate_policy(node["authorization_policy"], profile_hash, root, metadata)
    if policy["proposal_binding"] != binding:
        raise ContractError("top-level policy differs from exact ProposalBinding")
    snapshot = validate_revocation(node["revocation_snapshot"], profile_hash, root)
    request = validate_request(node["authorization_request"], profile_hash, root,
                               policy, snapshot)
    receipt = validate_receipt(node["authorization_receipt"], profile_hash, root)
    validate_receipt_relations(policy, request, snapshot, receipt, root)
    result = validate_result(node["authorization_result"], profile_hash, root)
    ledger = validate_ledger(node["authorization_ledger"], profile_hash, root)
    _validate_top_relations(node, result, ledger)
    return node


def _validate_top_relations(node: dict[str, Any], result: dict[str, Any],
                            ledger: dict[str, Any]) -> None:
    receipt = node["authorization_receipt"]
    if result["receipt"] != receipt:
        raise ContractError("authorization result does not bind exact top-level receipt")
    snapshot = node["revocation_snapshot"]
    matches = [item for item in ledger["revocation_snapshots"] if item == snapshot]
    if len(matches) != 1:
        raise ContractError("top-level revocation snapshot is not unique in complete ledger")
    entry_matches = [entry for entry in ledger["entries"] if
                     entry["policy"] == node["authorization_policy"] and
                     entry["proposal_document_base64url"] ==
                     node["proposal_document_base64url"] and
                     entry["request"] == node["authorization_request"] and
                     entry["receipt"] == receipt]
    if len(entry_matches) != 1:
        raise ContractError("top-level artifacts do not identify one complete ledger entry")
    if (result["delivery_disposition"] == "stored" and
            entry_matches[0] is not ledger["entries"][-1]):
        raise ContractError("stored result must identify the final appended ledger entry")
    if (result["delivery_disposition"] == "stored" and
            snapshot != ledger["revocation_snapshots"][-1]):
        raise ContractError("stored result must bind the revocation high-water snapshot")


def decode_document(raw: bytes) -> dict[str, Any]:
    return validate_document(decode_canonical(raw, MAX_GOLDEN_BYTES,
                                              "ADR approval candidate bundle"))
