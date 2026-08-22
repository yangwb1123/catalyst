"""Signed usage receipts and complete content-free UsageLedger."""

from __future__ import annotations

from typing import Any

from bootstrap_grant_issuance_contract.grant import grant_envelope_sha256
from bootstrap_grant_issuance_contract.ledger import (
    validate_ledger as validate_issuance_ledger,
)

from .authority import key_for_usage
from .canonical import (ContractError, bounded_canonical_json, canonical_json,
                        self_digest)
from .constants import (CANONICALIZATION, FAILED_REASONS, LEDGER_API, LEDGER_DOMAIN,
                        MAX_ENTRIES, MAX_LEDGER_BYTES, MAX_METADATA_BYTES,
                        MAX_RECEIPT_BYTES, ORPHAN_ENTRY_OVERHEAD_BYTES, PROFILE_ID,
                        QUARANTINE_REASONS, RECEIPT_API, RECEIPT_DOMAIN,
                        RESERVATION_ENTRY_OVERHEAD_BYTES, STATES)
from .documents import validate_invocation, validate_policy
from .manifest import validate_manifest
from .results import validate_metadata
from .shape import (integer, record_key_sha256, require_keys, sha256,
                    validate_signature)

RECEIPT_FIELDS = {
    "api_version", "canonicalization", "effect_intent_receipt_sha256",
    "execution_policy_sha256", "execution_result_sha256", "execution_trust_epoch",
    "execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
    "grant_issuance_receipt_sha256", "grant_sha256", "idempotency_record_key_sha256",
    "invocation_id", "invocation_sha256", "issuance_trust_epoch",
    "issuance_trust_root_sha256", "kind", "ledger_sequence", "manifest_sha256",
    "prior_usage_receipt_sha256", "profile_id", "reason_code", "receipt_sha256",
    "recorded_at_unix_ms", "requested_action_sha256", "reservation_receipt_sha256",
    "result_metadata_sha256", "signature", "state",
}
LEDGER_FIELDS = {
    "api_version", "canonicalization", "clock_high_water_unix_ms", "entries", "kind",
    "ledger_sha256", "profile_id", "signature", "trust_epoch", "trust_root_sha256",
}
ENTRY_FIELDS = {
    "execution_policy", "invocation", "manifest", "receipt", "result_metadata",
    "sequence",
}


def receipt_sha256(value: dict[str, Any]) -> str:
    return self_digest(RECEIPT_DOMAIN, value, "receipt_sha256", MAX_RECEIPT_BYTES,
                       "BootstrapRepoReadUsageReceipt", signed=True)


def ledger_sha256(value: dict[str, Any]) -> str:
    return self_digest(LEDGER_DOMAIN, value, "ledger_sha256", MAX_LEDGER_BYTES,
                       "BootstrapRepoReadUsageLedger", signed=True)


def validate_receipt(value: Any, profile_hash: str, root: dict[str, Any],
                     policy: dict[str, Any], invocation: dict[str, Any],
                     manifest: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadUsageReceipt", RECEIPT_FIELDS)
    bounded_canonical_json(node, MAX_RECEIPT_BYTES, "BootstrapRepoReadUsageReceipt")
    _validate_receipt_shape(node, profile_hash)
    _validate_receipt_authority(node, root)
    _validate_receipt_relations(node, policy, invocation, manifest)
    _validate_receipt_state(node)
    if node["receipt_sha256"] != receipt_sha256(node):
        raise ContractError("UsageReceipt self digest does not match")
    return node


def _validate_receipt_shape(node: dict[str, Any], profile_hash: str) -> None:
    expected = (RECEIPT_API, CANONICALIZATION, "BootstrapRepoReadUsageReceipt", PROFILE_ID)
    if (node["api_version"], node["canonicalization"], node["kind"],
            node["profile_id"]) != expected:
        raise ContractError("UsageReceipt envelope drifted from v1")
    if node["state"] not in STATES:
        raise ContractError("UsageReceipt state is unsupported")
    for field in ("execution_policy_sha256", "execution_trust_root_sha256",
                  "grant_envelope_sha256", "grant_issuance_receipt_sha256", "grant_sha256",
                  "idempotency_record_key_sha256", "invocation_sha256",
                  "issuance_trust_root_sha256", "manifest_sha256", "receipt_sha256",
                  "requested_action_sha256"):
        sha256(node[field], f"receipt.{field}")
    for field in ("effect_intent_receipt_sha256", "execution_result_sha256",
                  "prior_usage_receipt_sha256", "reservation_receipt_sha256",
                  "result_metadata_sha256"):
        if node[field] is not None:
            sha256(node[field], f"receipt.{field}")
    integer(node["ledger_sequence"], "receipt.ledger_sequence", 1, MAX_ENTRIES)
    integer(node["recorded_at_unix_ms"], "receipt.recorded_at_unix_ms", 0, 2**63 - 1)
    for field in ("execution_trust_epoch", "issuance_trust_epoch"):
        integer(node[field], f"receipt.{field}", 1, 2**63 - 1)
    validate_signature(node["signature"], "receipt.signature", profile_hash)


def _validate_receipt_authority(node: dict[str, Any], root: dict[str, Any]) -> None:
    if (node["execution_trust_root_sha256"] != root["root_sha256"] or
            node["execution_trust_epoch"] != root["trust_epoch"] or
            node["issuance_trust_root_sha256"] != root["issuance_trust_root_sha256"] or
            node["issuance_trust_epoch"] != root["issuance_trust_epoch"]):
        raise ContractError("UsageReceipt root binding is invalid")
    expected = key_for_usage(root, "execution_receipt_sign")["key_id"]
    if node["signature"]["key_id"] != expected:
        raise ContractError("UsageReceipt must use execution_receipt_sign key")


def _validate_receipt_relations(node: dict[str, Any], policy: dict[str, Any],
                                invocation: dict[str, Any], manifest: dict[str, Any]) -> None:
    expected = {
        "execution_policy_sha256": policy["execution_policy_sha256"],
        "execution_trust_epoch": policy["execution_trust_epoch"],
        "execution_trust_root_sha256": policy["execution_trust_root_sha256"],
        "grant_envelope_sha256": policy["grant_envelope_sha256"],
        "grant_id": policy["grant_id"],
        "grant_issuance_receipt_sha256": policy["grant_issuance_receipt_sha256"],
        "grant_sha256": policy["grant_sha256"],
        "idempotency_record_key_sha256": record_key_sha256(policy["idempotency_key"]),
        "invocation_id": invocation["invocation_id"],
        "invocation_sha256": invocation["invocation_sha256"],
        "issuance_trust_epoch": policy["issuance_trust_epoch"],
        "issuance_trust_root_sha256": policy["issuance_trust_root_sha256"],
        "manifest_sha256": manifest["manifest_sha256"],
        "requested_action_sha256": policy["requested_action_sha256"],
    }
    if any(node[field] != wanted for field, wanted in expected.items()):
        raise ContractError("UsageReceipt differs from Policy, Invocation, or manifest")


def _validate_receipt_state(node: dict[str, Any]) -> None:
    state = node["state"]
    result_fields = (node["execution_result_sha256"], node["result_metadata_sha256"])
    if state == "reserved_no_repo_io":
        if any(value is not None for value in result_fields) or any(
                node[field] is not None for field in ("reservation_receipt_sha256",
                                                      "effect_intent_receipt_sha256",
                                                      "reason_code")):
            raise ContractError("reserved_no_repo_io receipt fields are invalid")
    elif state == "effect_intent":
        if node["reservation_receipt_sha256"] is None or any(
                value is not None for value in result_fields + (
                    node["effect_intent_receipt_sha256"], node["reason_code"])):
            raise ContractError("effect_intent receipt fields are invalid")
    elif state == "completed":
        if (node["reservation_receipt_sha256"] is None or
                node["effect_intent_receipt_sha256"] is None or
                any(value is None for value in result_fields) or node["reason_code"] is not None):
            raise ContractError("completed receipt fields are invalid")
    elif state == "failed_consumed":
        _validate_failed_receipt(node, FAILED_REASONS, require_intent=True)
    else:
        _validate_quarantine_receipt(node)


def _validate_failed_receipt(node: dict[str, Any], reasons: tuple[str, ...],
                             *, require_intent: bool) -> None:
    if (node["reservation_receipt_sha256"] is None or
            (require_intent and node["effect_intent_receipt_sha256"] is None) or
            node["execution_result_sha256"] is not None or
            node["result_metadata_sha256"] is not None or node["reason_code"] not in reasons):
        raise ContractError(f"{node['state']} receipt fields are invalid")


def _validate_quarantine_receipt(node: dict[str, Any]) -> None:
    _validate_failed_receipt(node, QUARANTINE_REASONS, require_intent=False)
    reason = node["reason_code"]
    has_intent = node["effect_intent_receipt_sha256"] is not None
    if reason == "orphaned_reserved_no_repo_io" and has_intent:
        raise ContractError("reserved orphan quarantine cannot bind effect intent")
    if reason == "orphaned_effect_intent" and not has_intent:
        raise ContractError("effect-intent orphan quarantine must bind effect intent")


def validate_ledger(value: Any, profile_hash: str, root: dict[str, Any],
                    issuance_root: dict[str, Any],
                    issuance_ledger: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadUsageLedger", LEDGER_FIELDS)
    bounded_canonical_json(node, MAX_LEDGER_BYTES, "BootstrapRepoReadUsageLedger")
    _validate_ledger_envelope(node, profile_hash, root)
    validate_issuance_ledger(issuance_ledger, profile_hash, issuance_root)
    issued = _issued_index(issuance_ledger, issuance_root)
    entries = node["entries"]
    if not isinstance(entries, list) or not 1 <= len(entries) <= MAX_ENTRIES:
        raise ContractError("usage ledger entries must contain 1..256 transitions")
    _validate_entries(entries, profile_hash, root, issued,
                      node["clock_high_water_unix_ms"])
    _validate_capacity(node)
    if node["ledger_sha256"] != ledger_sha256(node):
        raise ContractError("UsageLedger self digest does not match")
    return node


def _validate_ledger_envelope(node: dict[str, Any], profile_hash: str,
                              root: dict[str, Any]) -> None:
    expected = (LEDGER_API, CANONICALIZATION, "BootstrapRepoReadUsageLedger", PROFILE_ID)
    if (node["api_version"], node["canonicalization"], node["kind"],
            node["profile_id"]) != expected:
        raise ContractError("UsageLedger envelope drifted from v1")
    integer(node["clock_high_water_unix_ms"], "ledger.clock_high_water", 0, 2**63 - 1)
    integer(node["trust_epoch"], "ledger.trust_epoch", 1, 2**63 - 1)
    sha256(node["trust_root_sha256"], "ledger.trust_root_sha256")
    sha256(node["ledger_sha256"], "ledger.ledger_sha256")
    signature = validate_signature(node["signature"], "ledger.signature", profile_hash)
    key = key_for_usage(root, "execution_receipt_sign")
    if (node["trust_root_sha256"] != root["root_sha256"] or
            node["trust_epoch"] != root["trust_epoch"] or signature["key_id"] != key["key_id"]):
        raise ContractError("UsageLedger execution receipt authority binding is invalid")


def _issued_index(ledger: dict[str, Any], root: dict[str, Any]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for entry in ledger["entries"]:
        grant = entry["grant"]
        if grant is None:
            continue
        envelope = grant_envelope_sha256(grant)
        if envelope in result:
            raise ContractError("issuance ledger contains duplicate Grant envelopes")
        result[envelope] = {
            "grant": grant, "ledger": ledger, "policy": entry["policy"],
            "receipt": entry["receipt"], "request": entry["request"], "root": root,
        }
    return result


def _validate_entries(entries: list[Any], profile_hash: str, root: dict[str, Any],
                      issued: dict[str, dict[str, Any]], high_water: int) -> None:
    prior: str | None = None
    prior_time: int | None = None
    active: dict[str, Any] | None = None
    envelopes: set[str] = set()
    record_keys: set[str] = set()
    for index, value in enumerate(entries):
        entry = require_keys(value, f"usage_ledger.entries[{index}]", ENTRY_FIELDS)
        if entry["sequence"] != index + 1:
            raise ContractError("usage ledger sequence must start at one and be contiguous")
        if _entry_state(entry) == "reserved_no_repo_io":
            receipt, group = _validate_reservation(entry, profile_hash, root, issued)
            _validate_new_group(group, active, envelopes, record_keys, index + 1)
        else:
            receipt, group = _validate_continuation(entry, profile_hash, root, active)
        _validate_entry_chain(receipt, index + 1, prior, active)
        active = _advance_group(group, receipt)
        prior = receipt["receipt_sha256"]
        if prior_time is not None and receipt["recorded_at_unix_ms"] < prior_time:
            raise ContractError("usage receipt recorded time must not move backward")
        prior_time = receipt["recorded_at_unix_ms"]
        if high_water < receipt["recorded_at_unix_ms"]:
            raise ContractError("usage ledger high-water is below receipt time")
        if high_water < group["invocation"]["requested_at_unix_ms"]:
            raise ContractError("usage ledger high-water is below invocation request time")


def _entry_state(entry: dict[str, Any]) -> str | None:
    receipt = entry["receipt"]
    return receipt.get("state") if isinstance(receipt, dict) else None


def _validate_reservation(entry: dict[str, Any], profile_hash: str,
                          root: dict[str, Any], issued_index: dict[str, dict[str, Any]],
                          ) -> tuple[dict[str, Any], dict[str, Any]]:
    if any(entry[field] is None for field in ("execution_policy", "invocation", "manifest")):
        raise ContractError("reserved ledger entry must embed Policy, Invocation, and manifest")
    if entry["result_metadata"] is not None:
        raise ContractError("reserved ledger entry cannot contain result metadata")
    manifest = validate_manifest(entry["manifest"])
    raw_policy = entry["execution_policy"]
    envelope = raw_policy.get("grant_envelope_sha256") if isinstance(raw_policy, dict) else None
    issued = issued_index.get(envelope)
    if issued is None:
        raise ContractError("reserved entry does not bind one issued Grant record")
    policy = validate_policy(raw_policy, profile_hash, root, manifest, issued)
    invocation = validate_invocation(entry["invocation"], profile_hash, root,
                                     policy, manifest, issued)
    receipt = validate_receipt(entry["receipt"], profile_hash, root, policy,
                               invocation, manifest)
    recorded = receipt["recorded_at_unix_ms"]
    if not invocation["requested_at_unix_ms"] <= recorded < invocation["expires_at_unix_ms"]:
        raise ContractError("reservation time is outside Invocation freshness")
    return receipt, {"invocation": invocation, "manifest": manifest,
                     "policy": policy, "reserved": receipt["receipt_sha256"],
                     "intent": None, "state": receipt["state"]}


def _validate_continuation(entry: dict[str, Any], profile_hash: str,
                           root: dict[str, Any], active: dict[str, Any] | None,
                           ) -> tuple[dict[str, Any], dict[str, Any]]:
    if active is None:
        raise ContractError("usage ledger continuation has no active reservation")
    if any(entry[field] is not None for field in ("execution_policy", "invocation", "manifest")):
        raise ContractError("non-reserved entries must not duplicate input documents")
    receipt = validate_receipt(entry["receipt"], profile_hash, root, active["policy"],
                               active["invocation"], active["manifest"])
    state = receipt["state"]
    if state == "completed":
        if entry["result_metadata"] is None:
            raise ContractError("completed entry requires durable result metadata")
        _validate_terminal_metadata(entry["result_metadata"], receipt, active)
    elif entry["result_metadata"] is not None:
        raise ContractError("only completed entry may store result metadata")
    return receipt, active


def _validate_terminal_metadata(value: Any, receipt: dict[str, Any],
                                group: dict[str, Any]) -> None:
    metadata = validate_metadata(value)
    manifest = group["manifest"]
    expected_reads = [{key: item[key] for key in ("content_bytes", "content_sha256", "path")}
                      for item in manifest["entries"]]
    if (metadata["manifest_sha256"] != manifest["manifest_sha256"] or
            metadata["reads"] != expected_reads or
            metadata["execution_result_sha256"] != receipt["execution_result_sha256"] or
            metadata["metadata_sha256"] != receipt["result_metadata_sha256"]):
        raise ContractError("completed metadata differs from receipt or reserved manifest")
    timeout = group["invocation"]["requested_action"]["usage"]["timeout_ms"]
    if metadata["observed_usage"]["elapsed_ms"] > timeout:
        raise ContractError("completed metadata exceeds cooperative timeout budget")


def _validate_new_group(group: dict[str, Any], active: dict[str, Any] | None,
                        envelopes: set[str], record_keys: set[str], sequence: int) -> None:
    if active is not None:
        raise ContractError("usage ledger cannot reserve while another group is active")
    if sequence > MAX_ENTRIES - 2:
        raise ContractError("reservation lacks capacity for intent and terminal receipts")
    envelope = group["policy"]["grant_envelope_sha256"]
    record_key = record_key_sha256(group["policy"]["idempotency_key"])
    if envelope in envelopes or record_key in record_keys:
        raise ContractError("usage ledger reuses a Grant envelope or idempotency record key")
    envelopes.add(envelope)
    record_keys.add(record_key)


def _validate_entry_chain(receipt: dict[str, Any], sequence: int, prior: str | None,
                          active: dict[str, Any] | None) -> None:
    if receipt["ledger_sequence"] != sequence or receipt["prior_usage_receipt_sha256"] != prior:
        raise ContractError("usage receipt sequence or prior chain is invalid")
    if active is None:
        if receipt["state"] != "reserved_no_repo_io":
            raise ContractError("usage state group must begin with reservation")
        return
    allowed = {"reserved_no_repo_io": ("effect_intent", "quarantined"),
               "effect_intent": ("completed", "failed_consumed", "quarantined")}
    if receipt["state"] not in allowed[active["state"]]:
        raise ContractError("usage state transition is invalid")
    if receipt["reservation_receipt_sha256"] != active["reserved"]:
        raise ContractError("usage receipt reservation binding is invalid")
    expected_intent = active["intent"] if active["state"] == "effect_intent" else None
    if receipt["effect_intent_receipt_sha256"] != expected_intent:
        raise ContractError("usage receipt effect-intent binding is invalid")
    _validate_quarantine_transition(receipt, active["state"])


def _validate_quarantine_transition(receipt: dict[str, Any], prior_state: str) -> None:
    if receipt["state"] != "quarantined":
        return
    allowed = {"reserved_no_repo_io": ("orphaned_reserved_no_repo_io",),
               "effect_intent": ("effect_outcome_uncertain", "orphaned_effect_intent")}
    if receipt["reason_code"] not in allowed[prior_state]:
        raise ContractError("quarantine reason does not match orphaned state")


def _advance_group(group: dict[str, Any], receipt: dict[str, Any]) -> dict[str, Any] | None:
    state = receipt["state"]
    if state in ("completed", "failed_consumed", "quarantined"):
        return None
    group["state"] = state
    if state == "effect_intent":
        group["intent"] = receipt["receipt_sha256"]
    return group


def _validate_capacity(ledger: dict[str, Any]) -> None:
    envelope = dict(ledger)
    envelope["entries"] = []
    prefix_bytes = len(canonical_json(envelope))
    for index, entry in enumerate(ledger["entries"]):
        if entry["receipt"]["state"] == "reserved_no_repo_io":
            documents = sum(len(canonical_json(entry[field]))
                            for field in ("execution_policy", "invocation", "manifest"))
            reserve = (documents + 3 * MAX_RECEIPT_BYTES + MAX_METADATA_BYTES +
                       RESERVATION_ENTRY_OVERHEAD_BYTES)
            current_bytes = 0 if index == 0 else prefix_bytes
            if current_bytes + reserve > MAX_LEDGER_BYTES:
                raise ContractError("reservation did not preflight future ledger byte capacity")
        prefix_bytes += len(canonical_json(entry)) + (1 if index else 0)
    if prefix_bytes != len(canonical_json(ledger)):
        raise ContractError("usage ledger capacity accounting drifted")
    if (_has_active_tail(ledger["entries"]) and
            prefix_bytes + MAX_RECEIPT_BYTES + ORPHAN_ENTRY_OVERHEAD_BYTES > MAX_LEDGER_BYTES):
        raise ContractError("active tail lacks capacity for orphan quarantine")


def _has_active_tail(entries: list[dict[str, Any]]) -> bool:
    return entries[-1]["receipt"]["state"] in ("reserved_no_repo_io", "effect_intent")


def lookup_usage_group(ledger: dict[str, Any], execution_policy_sha256: str,
                       invocation_sha256: str) -> dict[str, Any] | None:
    """Look up one validated group without consulting repository state or a clock."""
    match: dict[str, Any] | None = None
    for entry in ledger["entries"]:
        receipt = entry["receipt"]
        if (receipt["execution_policy_sha256"] == execution_policy_sha256 and
                receipt["invocation_sha256"] == invocation_sha256):
            if entry["execution_policy"] is not None:
                match = {"execution_policy": entry["execution_policy"],
                         "invocation": entry["invocation"], "manifest": entry["manifest"]}
            if match is not None:
                match.update({"receipt": receipt, "result_metadata": entry["result_metadata"]})
    return match


__all__ = [
    "ledger_sha256", "lookup_usage_group", "receipt_sha256", "validate_ledger",
    "validate_receipt",
]
