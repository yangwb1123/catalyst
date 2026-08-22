"""Top-level strict lifecycle structural bundle validation."""

from __future__ import annotations

from typing import Any

from .authority import (validate_approval_trust_root, validate_independent_roots,
                        validate_lifecycle_trust_root, validate_signature_profile)
from .canonical import ContractError, bounded_canonical_json, decode_canonical
from .constants import (BUNDLE_API, CANONICALIZATION, MAX_GOLDEN_BYTES, PROFILE_ID)
from .documents import validate_acceptance, validate_result
from .shape import require_keys
from .state import validate_state

BUNDLE_FIELDS = {
    "api_version", "approval_trust_root", "canonicalization", "kind",
    "lifecycle_result", "lifecycle_state", "lifecycle_trust_root", "profile_id",
    "signature_profile",
}


def validate_document(value: Any) -> dict[str, Any]:
    label = "AuthenticatedArchitectureDecisionLifecycleBundle"
    node = require_keys(value, label, BUNDLE_FIELDS)
    bounded_canonical_json(node, MAX_GOLDEN_BYTES, label)
    expected = (BUNDLE_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    profile = validate_signature_profile(node["signature_profile"])
    profile_hash = profile["profile_sha256"]
    approval_root = validate_approval_trust_root(node["approval_trust_root"],
                                                 profile_hash)
    lifecycle_root = validate_lifecycle_trust_root(node["lifecycle_trust_root"],
                                                   profile_hash)
    validate_independent_roots(lifecycle_root, approval_root)
    state, _ = validate_state(node["lifecycle_state"], profile_hash,
                              lifecycle_root, approval_root)
    result = validate_result(node["lifecycle_result"])
    receipt = validate_acceptance(result["receipt"], profile_hash, lifecycle_root)
    _validate_result_relations(result, receipt, state)
    return node


def _validate_result_relations(result: dict[str, Any], receipt: dict[str, Any],
                               state: dict[str, Any]) -> None:
    ledger = state["ledger"]
    view = state["materialized_view"]
    matches = [entry for entry in ledger["entries"]
               if entry["entry_sha256"] == result["entry_sha256"] and
               entry["acceptance_receipt"] == receipt]
    if len(matches) != 1:
        raise ContractError("lifecycle result does not identify one exact ledger entry")
    expected = {
        "ledger_sha256": ledger["ledger_sha256"],
        "materialized_view_sha256": view["view_sha256"],
        "state_sha256": state["state_sha256"],
    }
    if any(result[field] != value for field, value in expected.items()):
        raise ContractError("lifecycle result does not bind the exact state image")
    if result["delivery_disposition"] == "stored" and matches[0] is not ledger[
            "entries"][-1]:
        raise ContractError("stored lifecycle result must identify final appended entry")


def decode_document(raw: bytes) -> dict[str, Any]:
    return validate_document(decode_canonical(raw, MAX_GOLDEN_BYTES,
                                              "ADR lifecycle candidate bundle"))


__all__ = ["decode_document", "validate_document"]
