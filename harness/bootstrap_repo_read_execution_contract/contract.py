"""Top-level ADR-0058 golden and strict instance validation."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

import bootstrap_grant_issuance_contract as issuance_contract

from .authority import validate_execution_root, validate_signature_profile
from .canonical import (ContractError, bounded_canonical_json, decode_canonical,
                        read_bounded_file)
from .constants import (FIXTURE, GOLDEN_FILE_SHA256, MAX_GOLDEN_BYTES,
                        MAX_INVOCATION_BYTES, MAX_LEDGER_BYTES, MAX_POLICY_BYTES)
from .documents import (invocation_sha256, policy_sha256, validate_invocation,
                        validate_policy)
from .ledger import lookup_usage_group, validate_ledger, validate_receipt
from .manifest import validate_manifest
from .results import validate_delivery, validate_metadata, validate_result
from .shape import sha256

FIELDS = {
    "completed_receipt", "effect_intent_receipt", "execution_policy",
    "execution_result", "execution_trust_root", "expected_manifest", "first_delivery",
    "grant", "grant_issuance_receipt", "invocation", "issuance_ledger",
    "issuance_policy", "issuance_request", "issuance_trust_root", "reserved_receipt",
    "result_metadata", "signature_profile", "usage_ledger",
}


def validate_document(value: Any) -> dict[str, Any]:
    node = _require_fields(value)
    bounded_canonical_json(node, MAX_GOLDEN_BYTES,
                           "bootstrap repo-read execution document")
    issued = _validate_issuance(node)
    profile = validate_signature_profile(node["signature_profile"])
    profile_hash = profile["profile_sha256"]
    root = validate_execution_root(node["execution_trust_root"], profile_hash,
                                   issued["root"])
    manifest = validate_manifest(node["expected_manifest"])
    policy = validate_policy(node["execution_policy"], profile_hash, root,
                             manifest, issued)
    invocation = validate_invocation(node["invocation"], profile_hash, root,
                                     policy, manifest, issued)
    receipts = _validate_receipts(node, profile_hash, root, policy, invocation,
                                  manifest)
    result = validate_result(node["execution_result"], manifest, policy, invocation)
    metadata = validate_metadata(node["result_metadata"], result)
    _validate_completed_result(receipts[-1], result, metadata)
    validate_delivery(node["first_delivery"], receipts[-1], metadata)
    if node["first_delivery"]["execution_result"] != result:
        raise ContractError("first delivery differs from exact raw execution result")
    validate_ledger(node["usage_ledger"], profile_hash, root, issued["root"],
                    issued["ledger"])
    _validate_top_ledger_membership(node, receipts)
    return node


def _require_fields(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != FIELDS:
        raise ContractError("bootstrap repo-read execution document has unexpected fields")
    return value


def _validate_issuance(node: dict[str, Any]) -> dict[str, Any]:
    result = {
        "api_version": "forgeos.bootstrap-grant-issuance-result/v1",
        "canonicalization": "forgeos.canonical-json/v1",
        "delivery_disposition": "exact_replay",
        "grant": node["grant"],
        "kind": "BootstrapGrantIssuanceResult",
        "receipt": node["grant_issuance_receipt"],
    }
    document = {
        "grant": node["grant"], "ledger": node["issuance_ledger"],
        "policy": node["issuance_policy"], "receipt": node["grant_issuance_receipt"],
        "request": node["issuance_request"], "result": result,
        "signature_profile": node["signature_profile"],
        "trust_root": node["issuance_trust_root"],
    }
    issuance_contract.validate_document(document)
    return {"grant": node["grant"], "ledger": node["issuance_ledger"],
            "policy": node["issuance_policy"], "receipt": node["grant_issuance_receipt"],
            "request": node["issuance_request"], "root": node["issuance_trust_root"]}


def _validate_receipts(node: dict[str, Any], profile_hash: str, root: dict[str, Any],
                       policy: dict[str, Any], invocation: dict[str, Any],
                       manifest: dict[str, Any]) -> list[dict[str, Any]]:
    receipts = [validate_receipt(node[field], profile_hash, root, policy,
                                 invocation, manifest)
                for field in ("reserved_receipt", "effect_intent_receipt",
                              "completed_receipt")]
    expected_states = ["reserved_no_repo_io", "effect_intent", "completed"]
    if [receipt["state"] for receipt in receipts] != expected_states:
        raise ContractError("golden receipts must show reserved, intent, completed states")
    prior = None
    for sequence, receipt in enumerate(receipts, 1):
        if (receipt["ledger_sequence"] != sequence or
                receipt["prior_usage_receipt_sha256"] != prior):
            raise ContractError("top-level UsageReceipt chain is not contiguous")
        prior = receipt["receipt_sha256"]
    if receipts[1]["reservation_receipt_sha256"] != receipts[0]["receipt_sha256"]:
        raise ContractError("effect intent does not bind reservation")
    if (receipts[2]["reservation_receipt_sha256"] != receipts[0]["receipt_sha256"] or
            receipts[2]["effect_intent_receipt_sha256"] != receipts[1]["receipt_sha256"]):
        raise ContractError("completed receipt does not bind pre-effect transitions")
    return receipts


def _validate_completed_result(receipt: dict[str, Any], result: dict[str, Any],
                               metadata: dict[str, Any]) -> None:
    sha256(receipt["execution_result_sha256"], "completed execution_result_sha256")
    if (receipt["execution_result_sha256"] != result["execution_result_sha256"] or
            receipt["result_metadata_sha256"] != metadata["metadata_sha256"]):
        raise ContractError("completed receipt does not bind result and durable metadata")


def _validate_top_ledger_membership(node: dict[str, Any],
                                    receipts: list[dict[str, Any]]) -> None:
    entries = node["usage_ledger"]["entries"]
    if len(entries) != 3:
        raise ContractError("golden UsageLedger must contain exactly three transitions")
    if [entry["receipt"] for entry in entries] != receipts:
        raise ContractError("top-level receipts differ from UsageLedger transitions")
    if (entries[0]["execution_policy"] != node["execution_policy"] or
            entries[0]["invocation"] != node["invocation"] or
            entries[0]["manifest"] != node["expected_manifest"] or
            entries[2]["result_metadata"] != node["result_metadata"]):
        raise ContractError("top-level inputs or metadata differ from UsageLedger group")


def decode_document(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_GOLDEN_BYTES,
                             "bootstrap repo-read execution document")
    return validate_document(value)


def decode_usage_ledger(raw: bytes, signature_profile: dict[str, Any],
                        issuance_root: dict[str, Any], issuance_ledger: dict[str, Any],
                        execution_root: dict[str, Any]) -> dict[str, Any]:
    """Strictly decode a self-contained usage ledger plus its public authorities."""
    profile = validate_signature_profile(signature_profile)
    profile_hash = profile["profile_sha256"]
    pinned_issuance = issuance_contract.validate_trust_root(issuance_root, profile_hash)
    root = validate_execution_root(execution_root, profile_hash, pinned_issuance)
    value = decode_canonical(raw, MAX_LEDGER_BYTES,
                             "bootstrap repo-read usage ledger")
    return validate_ledger(value, profile_hash, root, pinned_issuance, issuance_ledger)


def terminal_replay(ledger: dict[str, Any], execution_policy_sha256: str,
                    invocation_sha256: str) -> dict[str, Any] | None:
    """Return content-free terminal replay material from an already validated ledger."""
    for digest, label in ((execution_policy_sha256, "execution_policy_sha256"),
                          (invocation_sha256, "invocation_sha256")):
        sha256(digest, label)
    group = lookup_usage_group(ledger, execution_policy_sha256, invocation_sha256)
    return _terminal_material(group)


def terminal_replay_from_documents(ledger: dict[str, Any], policy_raw: bytes,
                                   invocation_raw: bytes) -> dict[str, Any] | None:
    """Strict-decode exact canonical inputs, then perform content-free ledger lookup."""
    policy = decode_canonical(policy_raw, MAX_POLICY_BYTES, "execution policy lookup")
    invocation = decode_canonical(invocation_raw, MAX_INVOCATION_BYTES,
                                  "execution invocation lookup")
    if not isinstance(policy, dict) or not isinstance(invocation, dict):
        raise ContractError("replay lookup inputs must both be objects")
    policy_digest = policy.get("execution_policy_sha256")
    invocation_digest = invocation.get("invocation_sha256")
    sha256(policy_digest, "execution_policy_sha256")
    sha256(invocation_digest, "invocation_sha256")
    if policy_sha256(policy) != policy_digest or invocation_sha256(invocation) != invocation_digest:
        raise ContractError("replay lookup input self digest does not match")
    group = lookup_usage_group(ledger, policy_digest, invocation_digest)
    if group is not None and (group["execution_policy"] != policy or
                              group["invocation"] != invocation):
        raise ContractError("replay lookup digest matched different canonical documents")
    return _terminal_material(group)


def _terminal_material(group: dict[str, Any] | None) -> dict[str, Any] | None:
    if group is None or group["receipt"]["state"] not in (
            "completed", "failed_consumed", "quarantined"):
        return None
    return {"execution_result": None, "receipt": group["receipt"],
            "result_metadata": group["result_metadata"]}


def load_fixture(repo_root: Path) -> dict[str, Any]:
    path = repo_root / FIXTURE
    raw = read_bounded_file(path, MAX_GOLDEN_BYTES,
                            "bootstrap repo-read execution golden fixture")
    if GOLDEN_FILE_SHA256 and hashlib.sha256(raw).hexdigest() != GOLDEN_FILE_SHA256:
        raise ContractError("bootstrap repo-read execution fixture file digest drifted")
    fixture_raw = raw[:-1] if raw.endswith(b"\n") else raw
    return decode_document(fixture_raw)
