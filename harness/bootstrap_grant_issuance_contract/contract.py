"""Top-level golden and strict byte decoding for ADR-0057."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from .authority import validate_signature_profile, validate_trust_root
from .canonical import (ContractError, bounded_canonical_json, decode_canonical,
                        read_bounded_file)
from .constants import FIXTURE, GOLDEN_FILE_SHA256, MAX_GOLDEN_BYTES
from .documents import (validate_policy, validate_policy_request, validate_receipt,
                        validate_receipt_relations, validate_request)
from .grant import validate_bootstrap_grant, validate_grant_relations
from .ledger import validate_ledger, validate_result

GOLDEN_FIELDS = {"grant", "ledger", "policy", "receipt", "request", "result",
                 "signature_profile", "trust_root"}


def validate_document(value: Any) -> dict[str, Any]:
    node = _require_golden_fields(value)
    bounded_canonical_json(node, MAX_GOLDEN_BYTES, "bootstrap issuance document")
    profile = validate_signature_profile(node["signature_profile"])
    profile_hash = profile["profile_sha256"]
    root = validate_trust_root(node["trust_root"], profile_hash)
    policy = validate_policy(node["policy"], profile_hash)
    request = validate_request(node["request"], profile_hash)
    receipt = validate_receipt(node["receipt"], profile_hash)
    validate_policy_request(policy, request)
    validate_receipt_relations(policy, request, receipt)
    grant = _validate_top_grant(node["grant"], policy, request, receipt, profile_hash, root)
    result = validate_result(node["result"], profile_hash, root)
    ledger = validate_ledger(node["ledger"], profile_hash, root)
    _validate_root_relations(root, policy, request)
    _validate_result_relations(result, grant, receipt)
    _validate_ledger_membership(node, ledger)
    return node


def _require_golden_fields(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != GOLDEN_FIELDS:
        raise ContractError("bootstrap issuance document has unexpected fields")
    return value


def _validate_top_grant(value: Any, policy: dict[str, Any], request: dict[str, Any],
                        receipt: dict[str, Any], profile_hash: str,
                        root: dict[str, Any]) -> dict[str, Any] | None:
    if value is None:
        if policy["disposition"] != "deny" or receipt["decision"] != "denied":
            raise ContractError("null Grant requires authenticated Policy denial")
        return None
    if policy["disposition"] != "allow":
        raise ContractError("deny Policy cannot produce a Grant")
    grant = validate_bootstrap_grant(value, profile_hash, root)
    validate_grant_relations(grant, policy, request, receipt)
    return grant


def _validate_root_relations(root: dict[str, Any], policy: dict[str, Any],
                             request: dict[str, Any]) -> None:
    for label, document in (("Policy", policy), ("Request", request)):
        if (document["trust_root_sha256"] != root["root_sha256"] or
                document["trust_epoch"] != root["trust_epoch"]):
            raise ContractError(f"{label} does not bind GovernanceTrustRoot")


def _validate_result_relations(result: dict[str, Any], grant: dict[str, Any] | None,
                               receipt: dict[str, Any]) -> None:
    if result["grant"] != grant or result["receipt"] != receipt:
        raise ContractError("result does not bind exact top-level Grant and Receipt")


def _validate_ledger_membership(document: dict[str, Any], ledger: dict[str, Any]) -> None:
    matches = [entry for entry in ledger["entries"]
               if all(entry[field] == document[field]
                      for field in ("grant", "policy", "receipt", "request"))]
    if len(matches) != 1:
        raise ContractError("top-level issuance artifacts do not identify one ledger entry")
    if (document["result"]["delivery_disposition"] == "stored" and
            matches[0] is not ledger["entries"][-1]):
        raise ContractError("stored result must identify the final newly appended entry")


def decode_document(raw: bytes) -> dict[str, Any]:
    return validate_document(decode_canonical(raw, MAX_GOLDEN_BYTES,
                                              "bootstrap issuance document"))


def load_fixture(repo_root: Path) -> dict[str, Any]:
    path = repo_root / FIXTURE
    raw = read_bounded_file(path, MAX_GOLDEN_BYTES, "bootstrap issuance golden fixture")
    if GOLDEN_FILE_SHA256 and hashlib.sha256(raw).hexdigest() != GOLDEN_FILE_SHA256:
        raise ContractError("bootstrap issuance golden fixture file digest drifted")
    fixture_raw = raw[:-1] if raw.endswith(b"\n") else raw
    return decode_document(fixture_raw)
