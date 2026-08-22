"""Exact ADR-0081 receipt and state bindings carried into lifecycle requests."""

from __future__ import annotations

from typing import Any

from authenticated_adr_approval_contract.documents import validate_receipt
from authenticated_adr_approval_contract.shape import validate_signature as approval_signature

from .authority import approval_key_for_usage
from .canonical import (ContractError, bounded_canonical_json, self_digest,
                        sha256_bytes)
from .constants import (CANONICALIZATION, MAX_APPROVAL_LEDGER_ENTRIES,
                        MAX_APPROVAL_REVOCATION_SNAPSHOTS, MAX_REQUEST_BYTES,
                        PREREQUISITE_DOMAIN, PROFILE_ID)
from .proposal import validate_proposal_binding
from .shape import integer, require_keys, sha256

PREREQUISITE_API = "forgeos.architecture-decision-acceptance-prerequisite/v1"
PREREQUISITE_FIELDS = {
    "api_version", "approval_trust_epoch", "approval_trust_root_sha256",
    "authorization_ledger_clock_high_water_unix_ms",
    "authorization_ledger_last_sequence", "authorization_ledger_sha256",
    "authorization_ledger_signature", "authorization_receipt", "canonicalization",
    "authorization_receipt_physical_sha256", "kind", "observed_at_unix_ms",
    "prerequisite_sha256", "profile_id", "proposal_binding",
    "revocation_high_water_sequence", "revocation_high_water_sha256",
}


def prerequisite_sha256(value: dict[str, Any]) -> str:
    return self_digest(PREREQUISITE_DOMAIN, value, ("prerequisite_sha256",),
                       MAX_REQUEST_BYTES,
                       "ArchitectureDecisionAcceptancePrerequisite")


def validate_prerequisite(value: Any, profile_hash: str,
                          approval_root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionAcceptancePrerequisite"
    node = require_keys(value, label, PREREQUISITE_FIELDS)
    bounded_canonical_json(node, MAX_REQUEST_BYTES, label)
    expected = (PREREQUISITE_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    binding = validate_proposal_binding(node["proposal_binding"])
    receipt = validate_receipt(node["authorization_receipt"], profile_hash,
                               approval_root)
    _validate_pins(node, approval_root)
    _validate_receipt_relation(node, binding, receipt)
    _validate_ledger_binding(node, profile_hash, approval_root, receipt)
    expected_hash = prerequisite_sha256(node)
    if sha256(node["prerequisite_sha256"],
              "prerequisite.prerequisite_sha256") != expected_hash:
        raise ContractError("acceptance prerequisite self digest does not match")
    return node


def _validate_pins(node: dict[str, Any], root: dict[str, Any]) -> None:
    if (sha256(node["approval_trust_root_sha256"],
               "prerequisite.approval_trust_root_sha256") != root["root_sha256"] or
            node["approval_trust_epoch"] != root["trust_epoch"]):
        raise ContractError("acceptance prerequisite does not bind the approval root")
    integer(node["approval_trust_epoch"], "prerequisite.approval_trust_epoch", 1)
    for field in ("authorization_ledger_sha256",
                  "authorization_receipt_physical_sha256",
                  "revocation_high_water_sha256"):
        sha256(node[field], f"prerequisite.{field}")


def _validate_receipt_relation(node: dict[str, Any], binding: dict[str, Any],
                               receipt: dict[str, Any]) -> None:
    if (receipt["authorization_decision"] != "acceptance_transition_authorized" or
            receipt["reason_codes"] or not receipt["qualifying_approval_ids"]):
        raise ContractError("acceptance prerequisite requires an authorized-shaped receipt")
    if receipt["proposal_binding_sha256"] != binding["proposal_binding_sha256"]:
        raise ContractError("approval receipt does not bind the proposed ADR")
    raw = bounded_canonical_json(receipt, 256 * 1024,
                                 "embedded authorization receipt")
    if node["authorization_receipt_physical_sha256"] != sha256_bytes(raw):
        raise ContractError("acceptance prerequisite does not bind exact receipt bytes")
    observed = integer(node["observed_at_unix_ms"], "prerequisite.observed_at_unix_ms")
    if not receipt["evaluated_at_unix_ms"] <= observed < receipt[
            "authorization_expires_at_unix_ms"]:
        raise ContractError("declared prerequisite observation lies outside receipt validity")


def _validate_ledger_binding(node: dict[str, Any], profile_hash: str,
                             root: dict[str, Any], receipt: dict[str, Any]) -> None:
    last = integer(node["authorization_ledger_last_sequence"],
                   "prerequisite.authorization_ledger_last_sequence", 1,
                   MAX_APPROVAL_LEDGER_ENTRIES)
    clock = integer(node["authorization_ledger_clock_high_water_unix_ms"],
                    "prerequisite.authorization_ledger_clock_high_water_unix_ms")
    revocation = integer(node["revocation_high_water_sequence"],
                         "prerequisite.revocation_high_water_sequence", 1,
                         MAX_APPROVAL_REVOCATION_SNAPSHOTS)
    observed = node["observed_at_unix_ms"]
    if (last < receipt["ledger_sequence"] or
            not receipt["evaluated_at_unix_ms"] <= clock <= observed):
        raise ContractError("approval ledger sequence or clock high-water is impossible")
    if ((receipt["ledger_sequence"] == 1) !=
            (receipt["prior_receipt_sha256"] is None)):
        raise ContractError("approval receipt prior link differs from its own ledger sequence")
    if revocation < receipt["revocation_sequence"]:
        raise ContractError("approval revocation high-water regresses below the receipt")
    if (revocation == receipt["revocation_sequence"] and
            node["revocation_high_water_sha256"] != receipt["revocation_sha256"]):
        raise ContractError("equal revocation sequence requires the exact receipt digest")
    key = approval_key_for_usage(root, "approval_authorization_state_sign")
    approval_signature(node["authorization_ledger_signature"],
                       "prerequisite.authorization_ledger_signature", profile_hash)
    if node["authorization_ledger_signature"]["key_id"] != key["key_id"]:
        raise ContractError("approval ledger binding uses the wrong state-signing key")


__all__ = ["PREREQUISITE_API", "prerequisite_sha256", "validate_prerequisite"]
