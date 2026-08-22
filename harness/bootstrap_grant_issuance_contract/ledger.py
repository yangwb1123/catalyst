"""Complete signed GrantIssuanceLedger snapshot and result validation."""

from __future__ import annotations

from typing import Any

from .authority import key_for_usage
from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, LEDGER_API, LEDGER_DOMAIN, MAX_LEDGER_BYTES,
                        MAX_LEDGER_ENTRIES, MAX_RESULT_BYTES, RESULT_API)
from .documents import (validate_policy, validate_policy_request, validate_receipt,
                        validate_receipt_relations, validate_request)
from .grant import (validate_bootstrap_grant, validate_grant_relations)
from .shape import (integer, require_keys, sha256, validate_profile_id,
                    validate_signature)

LEDGER_FIELDS = {
    "api_version", "canonicalization", "clock_high_water_unix_ms", "entries", "kind",
    "ledger_sha256", "profile_id", "signature", "trust_epoch", "trust_root_sha256",
}
ENTRY_FIELDS = {"grant", "policy", "receipt", "request", "sequence"}
RESULT_FIELDS = {"api_version", "canonicalization", "delivery_disposition", "grant",
                 "kind", "receipt"}


def ledger_sha256(value: dict[str, Any]) -> str:
    return self_digest(LEDGER_DOMAIN, value, "ledger_sha256", MAX_LEDGER_BYTES,
                       "GrantIssuanceLedger", signed=True)


def validate_ledger(value: Any, profile_hash: str,
                    root: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "GrantIssuanceLedger", LEDGER_FIELDS)
    bounded_canonical_json(node, MAX_LEDGER_BYTES, "GrantIssuanceLedger")
    expected = (LEDGER_API, CANONICALIZATION, "GrantIssuanceLedger")
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError("GrantIssuanceLedger envelope drifted from v1")
    validate_profile_id(node["profile_id"], "ledger.profile_id")
    _validate_ledger_authority(node, profile_hash, root)
    entries = node["entries"]
    if not isinstance(entries, list) or not 1 <= len(entries) <= MAX_LEDGER_ENTRIES:
        raise ContractError("ledger entries must contain 1..256 complete records")
    _validate_entries(entries, profile_hash, root, node["clock_high_water_unix_ms"])
    sha256(node["ledger_sha256"], "ledger.ledger_sha256")
    if node["ledger_sha256"] != ledger_sha256(node):
        raise ContractError("GrantIssuanceLedger self digest does not match")
    return node


def _validate_ledger_authority(node: dict[str, Any], profile_hash: str,
                               root: dict[str, Any]) -> None:
    sha256(node["trust_root_sha256"], "ledger.trust_root_sha256")
    integer(node["trust_epoch"], "ledger.trust_epoch", 1, 2**63 - 1)
    integer(node["clock_high_water_unix_ms"], "ledger.clock_high_water_unix_ms",
            0, 2**63 - 1)
    if (node["trust_root_sha256"] != root["root_sha256"] or
            node["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("ledger does not bind the GovernanceTrustRoot")
    signature = validate_signature(node["signature"], "ledger.signature", profile_hash)
    if signature["key_id"] != key_for_usage(root, "grant_issue")["key_id"]:
        raise ContractError("ledger signature does not use grant_issue key")


def _validate_entries(entries: list[Any], profile_hash: str, root: dict[str, Any],
                      clock_high_water: int) -> None:
    prior = None
    record_keys: set[str] = set()
    for index, value in enumerate(entries):
        entry = require_keys(value, f"ledger.entries[{index}]", ENTRY_FIELDS)
        if entry["sequence"] != index + 1:
            raise ContractError("ledger entry sequence must start at one and be contiguous")
        receipt = _validate_entry(entry, profile_hash, root)
        if receipt["ledger_sequence"] != entry["sequence"]:
            raise ContractError("receipt ledger_sequence differs from its entry")
        if receipt["prior_receipt_sha256"] != prior:
            raise ContractError("receipt prior digest chain is not contiguous")
        if receipt["record_key_sha256"] in record_keys:
            raise ContractError("ledger contains a duplicate idempotency record key")
        record_keys.add(receipt["record_key_sha256"])
        prior = receipt["receipt_sha256"]
        observed = max(entry["request"]["requested_at_unix_ms"], receipt["stored_at_unix_ms"])
        if clock_high_water < observed:
            raise ContractError("ledger clock high-water is below an observed timestamp")


def _validate_entry(entry: dict[str, Any], profile_hash: str,
                    root: dict[str, Any]) -> dict[str, Any]:
    policy = validate_policy(entry["policy"], profile_hash)
    request = validate_request(entry["request"], profile_hash)
    receipt = validate_receipt(entry["receipt"], profile_hash)
    validate_policy_request(policy, request)
    validate_receipt_relations(policy, request, receipt)
    _validate_document_key_bindings(policy, request, receipt, root)
    grant = entry["grant"]
    if grant is None:
        if receipt["decision"] != "denied":
            raise ContractError("only a denied ledger entry may have null Grant")
    else:
        grant = validate_bootstrap_grant(grant, profile_hash, root)
        validate_grant_relations(grant, policy, request, receipt)
    return receipt


def _validate_document_key_bindings(policy: dict[str, Any], request: dict[str, Any],
                                    receipt: dict[str, Any], root: dict[str, Any]) -> None:
    expected = ((policy, "policy_sign"), (request, "request_auth"),
                (receipt, "grant_issue"))
    if any(document["signature"]["key_id"] != key_for_usage(root, usage)["key_id"]
           for document, usage in expected):
        raise ContractError("signed artifact uses a key with the wrong root usage")
    if request["subject"] != key_for_usage(root, "request_auth")["principal"]:
        raise ContractError("request-auth principal must equal Request subject")


def validate_result(value: Any, profile_hash: str, root: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapGrantIssuanceResult", RESULT_FIELDS)
    bounded_canonical_json(node, MAX_RESULT_BYTES, "BootstrapGrantIssuanceResult")
    expected = (RESULT_API, CANONICALIZATION, "BootstrapGrantIssuanceResult")
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError("BootstrapGrantIssuanceResult envelope drifted from v1")
    if node["delivery_disposition"] not in ("exact_replay", "stored"):
        raise ContractError("result delivery_disposition is unsupported")
    receipt = validate_receipt(node["receipt"], profile_hash)
    if node["grant"] is None:
        if receipt["decision"] != "denied":
            raise ContractError("result null Grant requires denied receipt")
    else:
        validate_bootstrap_grant(node["grant"], profile_hash, root)
        if receipt["decision"] != "issued":
            raise ContractError("result Grant requires issued receipt")
    return node
