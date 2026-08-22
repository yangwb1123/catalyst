"""Complete signed candidate ledger snapshot and append-chain relations."""

from __future__ import annotations

from typing import Any

from .authority import key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, LEDGER_API, LEDGER_DOMAIN,
                        MAX_LEDGER_BYTES, MAX_LEDGER_ENTRIES, PROFILE_ID)
from .documents import (validate_receipt, validate_receipt_relations,
                        validate_request)
from .policy import validate_policy
from .proposal import decode_proposal_document, validate_proposal_binding
from .revocation import validate_revocation_chain
from .shape import (array, integer, require_keys, sha256, validate_signature)

LEDGER_FIELDS = {
    "api_version", "canonicalization", "clock_high_water_unix_ms", "entries", "kind",
    "ledger_sha256", "profile_id", "revocation_high_water_sequence",
    "revocation_high_water_sha256", "revocation_snapshots", "signature", "trust_epoch",
    "trust_root_sha256",
}
ENTRY_FIELDS = {"policy", "proposal_document_base64url", "receipt", "request", "sequence"}


def ledger_sha256(value: dict[str, Any]) -> str:
    return self_digest(LEDGER_DOMAIN, value, ("ledger_sha256",), MAX_LEDGER_BYTES,
                       "ArchitectureDecisionApprovalAuthorizationLedger", signed=True)


def validate_ledger(value: Any, profile_hash: str,
                    root: dict[str, Any]) -> dict[str, Any]:
    label = "ArchitectureDecisionApprovalAuthorizationLedger"
    node = require_keys(value, label, LEDGER_FIELDS)
    bounded_canonical_json(node, MAX_LEDGER_BYTES, label)
    expected = (LEDGER_API, CANONICALIZATION, label, PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError(f"{label} envelope drifted from v1")
    _validate_ledger_authority(node, profile_hash, root)
    snapshots = validate_revocation_chain(node["revocation_snapshots"], profile_hash, root)
    _validate_revocation_high_water(node, snapshots)
    if node["clock_high_water_unix_ms"] < snapshots[-1]["effective_at_unix_ms"]:
        raise ContractError("ledger clock high-water is below revocation high-water time")
    if node["signature"]["key_id"] in snapshots[-1]["revoked_key_ids"]:
        raise ContractError("ledger signing key is revoked at revocation high-water")
    entries = array(node["entries"], "ledger.entries", 1, MAX_LEDGER_ENTRIES)
    _validate_entries(entries, snapshots, profile_hash, root,
                      node["clock_high_water_unix_ms"])
    sha256(node["ledger_sha256"], "ledger.ledger_sha256")
    if node["ledger_sha256"] != ledger_sha256(node):
        raise ContractError("ledger self digest does not match")
    return node


def _validate_ledger_authority(node: dict[str, Any], profile_hash: str,
                               root: dict[str, Any]) -> None:
    if (sha256(node["trust_root_sha256"], "ledger.trust_root_sha256") !=
            root["root_sha256"] or node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("ledger does not bind the supplied trust root")
    integer(node["trust_epoch"], "ledger.trust_epoch", 1, 2**63 - 1)
    integer(node["clock_high_water_unix_ms"], "ledger.clock_high_water_unix_ms",
            0, 2**63 - 1)
    signature = validate_signature(node["signature"], "ledger.signature", profile_hash)
    expected = key_for_usage(root, "approval_authorization_state_sign")["key_id"]
    if signature["key_id"] != expected:
        raise ContractError("ledger signature uses the wrong root key usage")


def _validate_revocation_high_water(node: dict[str, Any],
                                    snapshots: list[dict[str, Any]]) -> None:
    latest = snapshots[-1]
    if (node["revocation_high_water_sequence"] != latest["revocation_sequence"] or
            sha256(node["revocation_high_water_sha256"],
                   "ledger.revocation_high_water_sha256") != latest["revocation_sha256"]):
        raise ContractError("ledger revocation high-water differs from complete snapshot chain")


def _validate_entries(entries: list[Any], snapshots: list[dict[str, Any]],
                      profile_hash: str, root: dict[str, Any], clock_high_water: int) -> None:
    prior_receipt = None
    record_keys: set[str] = set()
    authorized_proposals: set[str] = set()
    prior_revocation_sequence = 0
    for index, value in enumerate(entries):
        entry = require_keys(value, f"ledger.entries[{index}]", ENTRY_FIELDS)
        if entry["sequence"] != index + 1:
            raise ContractError("ledger entry sequence must start at one and be contiguous")
        receipt = _validate_entry(entry, snapshots, profile_hash, root)
        _validate_entry_chain(entry, receipt, prior_receipt, record_keys,
                              authorized_proposals, clock_high_water)
        current_revocation = entry["request"]["revocation_sequence"]
        if current_revocation < prior_revocation_sequence:
            raise ContractError("ledger request revocation sequences must be nondecreasing")
        prior_revocation_sequence = current_revocation
        prior_receipt = receipt["receipt_sha256"]


def _validate_entry(entry: dict[str, Any], snapshots: list[dict[str, Any]],
                    profile_hash: str, root: dict[str, Any]) -> dict[str, Any]:
    raw_request = entry["request"]
    if not isinstance(raw_request, dict) or "proposal_binding" not in raw_request:
        raise ContractError("ledger request has no ProposalBinding")
    binding = validate_proposal_binding(raw_request["proposal_binding"])
    _, metadata = decode_proposal_document(entry["proposal_document_base64url"], binding,
                                           "ledger proposal document")
    policy = validate_policy(entry["policy"], profile_hash, root, metadata)
    if policy["proposal_binding"] != binding:
        raise ContractError("ledger policy differs from exact proposal binding")
    snapshot = _snapshot_for_request(raw_request, snapshots)
    request = validate_request(raw_request, profile_hash, root, policy, snapshot)
    receipt = validate_receipt(entry["receipt"], profile_hash, root)
    validate_receipt_relations(policy, request, snapshot, receipt, root)
    return receipt


def _snapshot_for_request(request: dict[str, Any],
                          snapshots: list[dict[str, Any]]) -> dict[str, Any]:
    if "revocation_sequence" not in request or "revocation_sha256" not in request:
        raise ContractError("request has no revocation snapshot reference")
    matches = [snapshot for snapshot in snapshots
               if snapshot["revocation_sequence"] == request["revocation_sequence"] and
               snapshot["revocation_sha256"] == request["revocation_sha256"]]
    if len(matches) != 1:
        raise ContractError("request revocation reference is absent from complete ledger state")
    return matches[0]


def _validate_entry_chain(entry: dict[str, Any], receipt: dict[str, Any],
                          prior_receipt: str | None, record_keys: set[str],
                          authorized_proposals: set[str], clock_high_water: int) -> None:
    sequence = entry["sequence"]
    request = entry["request"]
    if receipt["ledger_sequence"] != sequence or request["expected_next_sequence"] != sequence:
        raise ContractError("ledger entry, request, and receipt sequence differ")
    if receipt["prior_receipt_sha256"] != prior_receipt:
        raise ContractError("ledger receipt prior digest chain is not contiguous")
    record_key = receipt["record_key_sha256"]
    if record_key in record_keys:
        raise ContractError("ledger contains a duplicate idempotency record key")
    record_keys.add(record_key)
    if receipt["authorization_decision"] == "acceptance_transition_authorized":
        proposal = receipt["proposal_binding_sha256"]
        if proposal in authorized_proposals:
            raise ContractError("ledger contains two authorized receipts for one proposal")
        authorized_proposals.add(proposal)
    observed = max(request["requested_at_unix_ms"], receipt["evaluated_at_unix_ms"])
    if clock_high_water < observed:
        raise ContractError("ledger clock high-water is below an observed declared timestamp")
