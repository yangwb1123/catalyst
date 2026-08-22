"""Complete append-only ledger validation and deterministic view reconstruction."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import ContractError, bounded_canonical_json, digest_value, self_digest
from .constants import (CANONICALIZATION, HEAD_DOMAIN, LEDGER_API, LEDGER_DOMAIN,
                        MAX_DECISIONS, MAX_ENTRIES, MAX_LEDGER_BYTES,
                        MAX_VIEW_BYTES, PROFILE_ID, VIEW_API, VIEW_DOMAIN)
from .documents import record_key_sha256, validate_entry_shape
from .shape import adr_id, integer, require_keys, sha256, sorted_unique

LEDGER_FIELDS = {
    "api_version", "canonicalization", "current_head_set_sha256", "entries", "kind",
    "last_sequence", "ledger_sha256", "profile_id",
}
VIEW_FIELDS = {
    "api_version", "canonicalization", "current_head_set_sha256", "decisions",
    "head_adr_ids", "kind", "last_sequence", "ledger_sha256", "profile_id",
    "view_sha256",
}
DECISION_FIELDS = {
    "acceptance_id", "acceptance_sha256", "accepted_at_unix_ms", "adr_id",
    "authorization_receipt_physical_sha256", "authorization_receipt_sha256",
    "document_name", "expires_at_unix_ms", "proposal_binding_sha256",
    "proposed_at_unix_ms", "source_body_sha256", "source_physical_sha256",
    "source_self_sha256", "status", "superseded_at_unix_ms", "superseded_by",
    "supersession_receipt_sha256", "supersedes",
}


def ledger_sha256(value: dict[str, Any]) -> str:
    return self_digest(LEDGER_DOMAIN, value, ("ledger_sha256",), MAX_LEDGER_BYTES,
                       "ArchitectureDecisionLifecycleLedger")


def view_sha256(value: dict[str, Any]) -> str:
    return self_digest(VIEW_DOMAIN, value, ("view_sha256",), MAX_VIEW_BYTES,
                       "ArchitectureDecisionLifecycleMaterializedView")


def current_head_set_sha256(state: dict[str, dict[str, Any]]) -> str:
    heads = [
        {"adr_id": item["adr_id"],
         "proposal_binding_sha256": item["proposal_binding_sha256"]}
        for item in state.values() if item["status"] == "accepted"
    ]
    heads.sort(key=lambda item: item["adr_id"].encode("utf-8"))
    return digest_value(HEAD_DOMAIN, heads, MAX_VIEW_BYTES,
                        "structural current-head set")


def validate_ledger(value: Any, profile_hash: str, lifecycle_root: dict[str, Any],
                    approval_root: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    label = "ArchitectureDecisionLifecycleLedger"
    node = require_keys(value, label, LEDGER_FIELDS)
    bounded_canonical_json(node, MAX_LEDGER_BYTES, label)
    _validate_ledger_envelope(node)
    entries = node["entries"]
    if not isinstance(entries, list) or not 1 <= len(entries) <= MAX_ENTRIES:
        raise ContractError(f"lifecycle ledger requires 1..{MAX_ENTRIES} entries")
    state: dict[str, dict[str, Any]] = {}
    previous_entry = None
    previous_ledger = None
    previous_time = None
    record_keys: set[str] = set()
    approval_receipts: set[str] = set()
    for index, raw_entry in enumerate(entries, 1):
        entry, metadata = validate_entry_shape(raw_entry, profile_hash,
                                               lifecycle_root, approval_root)
        _validate_entry_cas(entry, index, previous_entry, previous_ledger, state)
        previous_time = _apply_entry(entry, metadata, state, record_keys,
                                     approval_receipts, previous_time)
        previous_entry = entry
        previous_ledger = _prefix_ledger(node, entries[:index],
                                         current_head_set_sha256(state))
    _validate_final_ledger(node, previous_ledger, state)
    return node, state


def _validate_ledger_envelope(node: dict[str, Any]) -> None:
    expected = (LEDGER_API, CANONICALIZATION,
                "ArchitectureDecisionLifecycleLedger", PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError("lifecycle ledger envelope drifted from v1")
    integer(node["last_sequence"], "ledger.last_sequence", 1, MAX_ENTRIES)
    sha256(node["current_head_set_sha256"], "ledger.current_head_set_sha256")
    sha256(node["ledger_sha256"], "ledger.ledger_sha256")


def _validate_entry_cas(entry: dict[str, Any], sequence: int,
                        previous_entry: dict[str, Any] | None,
                        previous_ledger: dict[str, Any] | None,
                        state: dict[str, dict[str, Any]]) -> None:
    request = entry["request"]
    expected_entry = None if previous_entry is None else previous_entry["entry_sha256"]
    expected_ledger = None if previous_ledger is None else previous_ledger["ledger_sha256"]
    if entry["sequence"] != sequence or request["expected_next_sequence"] != sequence:
        raise ContractError("lifecycle ledger sequence is not contiguous")
    if entry["prior_entry_sha256"] != expected_entry:
        raise ContractError("lifecycle entry prior digest chain is broken")
    if request["expected_ledger_sha256"] != expected_ledger:
        raise ContractError("request expected ledger digest differs from exact prefix")
    if request["expected_current_head_set_sha256"] != current_head_set_sha256(state):
        raise ContractError("request expected current-head set differs from rebuilt prefix")


def _apply_entry(entry: dict[str, Any], metadata: dict[str, Any],
                 state: dict[str, dict[str, Any]], record_keys: set[str],
                 approval_receipts: set[str], previous_time: int | None) -> int:
    request = entry["request"]
    prerequisite = request["acceptance_prerequisite"]
    binding = prerequisite["proposal_binding"]
    new_adr = binding["adr_id"]
    if new_adr in state:
        raise ContractError("an ADR may be accepted only once")
    _validate_targets(request["supersession_targets"], state)
    record_key = record_key_sha256(request["idempotency_key"])
    approval_physical = prerequisite["authorization_receipt_physical_sha256"]
    if record_key in record_keys or approval_physical in approval_receipts:
        raise ContractError("idempotency key or exact approval receipt is reused")
    record_keys.add(record_key)
    approval_receipts.add(approval_physical)
    acceptance = entry["acceptance_receipt"]
    accepted = _validate_entry_time(entry, metadata, previous_time)
    state[new_adr] = _new_decision(metadata, binding, prerequisite, acceptance)
    _apply_supersessions(entry, state, acceptance)
    resulting = current_head_set_sha256(state)
    _validate_entry_relations(entry, resulting, record_key)
    _validate_graph(state)
    return accepted


def _validate_targets(targets: list[dict[str, Any]],
                      state: dict[str, dict[str, Any]]) -> None:
    for target in targets:
        adr = target["adr_id"]
        if adr not in state:
            raise ContractError(f"supersession target {adr} is missing or legacy")
        current = state[adr]
        if current["status"] != "accepted":
            raise ContractError(f"supersession target {adr} is already superseded")
        expected = {
            "acceptance_id": current["acceptance_id"],
            "acceptance_sha256": current["acceptance_sha256"],
            "proposal_binding_sha256": current["proposal_binding_sha256"],
        }
        if any(target[field] != value for field, value in expected.items()):
            raise ContractError(f"supersession target {adr} exact row binding differs")


def _validate_entry_time(entry: dict[str, Any], metadata: dict[str, Any],
                         previous_time: int | None) -> int:
    request = entry["request"]
    acceptance = entry["acceptance_receipt"]
    prerequisite = request["acceptance_prerequisite"]
    approval = prerequisite["authorization_receipt"]
    accepted = acceptance["accepted_at_unix_ms"]
    if accepted != request["requested_at_unix_ms"] or accepted != prerequisite[
            "observed_at_unix_ms"]:
        raise ContractError("acceptance must use the exact prerequisite observation time")
    if accepted < metadata["proposed_at_unix_ms"]:
        raise ContractError("acceptance time precedes the immutable proposal")
    if approval["evaluated_at_unix_ms"] < metadata["proposed_at_unix_ms"]:
        raise ContractError("approval evaluation time precedes the immutable proposal")
    if metadata["expires_at_unix_ms"] is not None and accepted >= metadata[
            "expires_at_unix_ms"]:
        raise ContractError("immutable proposal expired before acceptance")
    if (metadata["expires_at_unix_ms"] is not None and
            approval["authorization_expires_at_unix_ms"] >
            metadata["expires_at_unix_ms"]):
        raise ContractError("approval authorization extends beyond proposal expiry")
    if not approval["evaluated_at_unix_ms"] <= accepted < request["expires_at_unix_ms"]:
        raise ContractError("acceptance lies outside approval/request time relations")
    if previous_time is not None and accepted < previous_time:
        raise ContractError("lifecycle acceptance observation time regressed")
    return accepted


def _new_decision(metadata: dict[str, Any], binding: dict[str, Any],
                  prerequisite: dict[str, Any], acceptance: dict[str, Any],
                  ) -> dict[str, Any]:
    return {
        "acceptance_id": acceptance["acceptance_id"],
        "acceptance_sha256": acceptance["acceptance_sha256"],
        "accepted_at_unix_ms": acceptance["accepted_at_unix_ms"],
        "adr_id": binding["adr_id"],
        "authorization_receipt_physical_sha256":
            prerequisite["authorization_receipt_physical_sha256"],
        "authorization_receipt_sha256": prerequisite["authorization_receipt"]
        ["receipt_sha256"],
        "document_name": binding["document_name"],
        "expires_at_unix_ms": metadata["expires_at_unix_ms"],
        "proposal_binding_sha256": binding["proposal_binding_sha256"],
        "proposed_at_unix_ms": metadata["proposed_at_unix_ms"],
        "source_body_sha256": binding["body_sha256"],
        "source_physical_sha256": binding["physical_sha256"],
        "source_self_sha256": binding["self_sha256"],
        "status": "accepted", "superseded_at_unix_ms": None,
        "superseded_by": [], "supersession_receipt_sha256": None,
        "supersedes": list(metadata["supersedes"]),
    }


def _apply_supersessions(entry: dict[str, Any], state: dict[str, dict[str, Any]],
                         acceptance: dict[str, Any]) -> None:
    for receipt in entry["supersession_receipts"]:
        target = state[receipt["target_adr_id"]]
        expected = {
            "target_acceptance_id": target["acceptance_id"],
            "target_proposal_binding_sha256": target["proposal_binding_sha256"],
            "superseded_by_acceptance_id": acceptance["acceptance_id"],
            "superseded_by_adr_id": acceptance["adr_id"],
            "superseded_by_proposal_binding_sha256":
                acceptance["proposal_binding_sha256"],
        }
        if any(receipt[field] != value for field, value in expected.items()):
            raise ContractError("supersession receipt target/new-decision binding differs")
        target["status"] = "superseded"
        target["superseded_at_unix_ms"] = receipt["superseded_at_unix_ms"]
        target["superseded_by"] = [acceptance["adr_id"]]
        target["supersession_receipt_sha256"] = receipt["receipt_sha256"]


def _validate_entry_relations(entry: dict[str, Any], resulting: str,
                              record_key: str) -> None:
    request = entry["request"]
    prerequisite = request["acceptance_prerequisite"]
    binding = prerequisite["proposal_binding"]
    approval = prerequisite["authorization_receipt"]
    acceptance = entry["acceptance_receipt"]
    expected = {
        "adr_id": binding["adr_id"],
        "authorization_receipt_physical_sha256":
            prerequisite["authorization_receipt_physical_sha256"],
        "authorization_receipt_sha256": approval["receipt_sha256"],
        "ledger_sequence": entry["sequence"],
        "proposal_binding_sha256": binding["proposal_binding_sha256"],
        "record_key_sha256": record_key, "request_sha256": request["request_sha256"],
        "supersedes": [target["adr_id"] for target in request["supersession_targets"]],
    }
    if any(acceptance[field] != value for field, value in expected.items()):
        raise ContractError("acceptance receipt differs from request and prerequisite")
    if entry["resulting_current_head_set_sha256"] != resulting:
        raise ContractError("entry resulting current-head set differs from rebuilt state")
    for receipt in entry["supersession_receipts"]:
        _validate_supersession_relation(receipt, entry, acceptance)


def _validate_supersession_relation(receipt: dict[str, Any], entry: dict[str, Any],
                                    acceptance: dict[str, Any]) -> None:
    expected = {
        "ledger_sequence": entry["sequence"],
        "request_sha256": entry["request"]["request_sha256"],
        "superseded_at_unix_ms": acceptance["accepted_at_unix_ms"],
        "trust_epoch": acceptance["trust_epoch"],
        "trust_root_sha256": acceptance["trust_root_sha256"],
    }
    if any(receipt[field] != value for field, value in expected.items()):
        raise ContractError("supersession receipt differs from its atomic entry")


def _validate_graph(state: dict[str, dict[str, Any]]) -> None:
    if len(state) > MAX_DECISIONS:
        raise ContractError(f"materialized state exceeds {MAX_DECISIONS} decisions")
    for decision in state.values():
        if any(target not in state for target in decision["supersedes"]):
            raise ContractError("materialized supersedes edge is dangling")
    visiting: set[str] = set()
    visited: set[str] = set()
    for item in state:
        _visit(item, state, visiting, visited)


def _visit(adr: str, state: dict[str, dict[str, Any]], visiting: set[str],
           visited: set[str]) -> None:
    if adr in visiting:
        raise ContractError("lifecycle supersession graph contains a cycle")
    if adr in visited:
        return
    visiting.add(adr)
    for target in state[adr]["supersedes"]:
        _visit(target, state, visiting, visited)
    visiting.remove(adr)
    visited.add(adr)


def _prefix_ledger(template: dict[str, Any], entries: list[dict[str, Any]],
                   head: str) -> dict[str, Any]:
    prefix = {field: copy.deepcopy(template[field]) for field in LEDGER_FIELDS}
    prefix["entries"] = copy.deepcopy(entries)
    prefix["current_head_set_sha256"] = head
    prefix["last_sequence"] = len(entries)
    prefix["ledger_sha256"] = ""
    prefix["ledger_sha256"] = ledger_sha256(prefix)
    return prefix


def _validate_final_ledger(node: dict[str, Any], rebuilt: dict[str, Any] | None,
                           state: dict[str, dict[str, Any]]) -> None:
    if node["last_sequence"] != len(node["entries"]):
        raise ContractError("ledger last_sequence differs from complete entries")
    if node["current_head_set_sha256"] != current_head_set_sha256(state):
        raise ContractError("ledger current-head set differs from rebuilt state")
    if node["ledger_sha256"] != ledger_sha256(node):
        raise ContractError("lifecycle ledger self digest does not match")
    if rebuilt is None or rebuilt["ledger_sha256"] != node["ledger_sha256"]:
        raise ContractError("complete lifecycle ledger differs from final prefix")


def rebuild_materialized_view(ledger: dict[str, Any],
                              state: dict[str, dict[str, Any]]) -> dict[str, Any]:
    decisions = [copy.deepcopy(state[key]) for key in sorted(state)]
    heads = sorted((key for key, item in state.items() if item["status"] == "accepted"),
                   key=lambda item: item.encode("utf-8"))
    view = {
        "api_version": VIEW_API, "canonicalization": CANONICALIZATION,
        "current_head_set_sha256": ledger["current_head_set_sha256"],
        "decisions": decisions, "head_adr_ids": heads,
        "kind": "ArchitectureDecisionLifecycleMaterializedView",
        "last_sequence": ledger["last_sequence"], "ledger_sha256": ledger["ledger_sha256"],
        "profile_id": PROFILE_ID, "view_sha256": "",
    }
    view["view_sha256"] = view_sha256(view)
    return view


def validate_materialized_view(value: Any, ledger: dict[str, Any],
                               state: dict[str, dict[str, Any]]) -> dict[str, Any]:
    label = "ArchitectureDecisionLifecycleMaterializedView"
    node = require_keys(value, label, VIEW_FIELDS)
    bounded_canonical_json(node, MAX_VIEW_BYTES, label)
    _validate_view_shape(node)
    if node != rebuild_materialized_view(ledger, state):
        raise ContractError("materialized lifecycle view is not exact ledger rebuild")
    return node


def _validate_view_shape(node: dict[str, Any]) -> None:
    expected = (VIEW_API, CANONICALIZATION,
                "ArchitectureDecisionLifecycleMaterializedView", PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"],
              node["profile_id"])
    if actual != expected:
        raise ContractError("materialized lifecycle view envelope drifted from v1")
    integer(node["last_sequence"], "view.last_sequence", 1, MAX_ENTRIES)
    for field in ("current_head_set_sha256", "ledger_sha256", "view_sha256"):
        sha256(node[field], f"view.{field}")
    _validate_decisions(node["decisions"])
    heads = sorted_unique(node["head_adr_ids"], "view.head_adr_ids", MAX_DECISIONS)
    for index, item in enumerate(heads):
        adr_id(item, f"view.head_adr_ids[{index}]")


def _validate_decisions(value: Any) -> None:
    if not isinstance(value, list) or not 1 <= len(value) <= MAX_DECISIONS:
        raise ContractError("materialized decisions exceed the closed bound")
    identifiers = []
    for index, decision in enumerate(value):
        node = require_keys(decision, f"view.decisions[{index}]", DECISION_FIELDS)
        identifiers.append(adr_id(node["adr_id"], f"view.decisions[{index}].adr_id"))
        if node["status"] not in {"accepted", "superseded"}:
            raise ContractError("materialized decision status is unsupported")
        sorted_unique(node["supersedes"], f"view.decisions[{index}].supersedes",
                      MAX_DECISIONS)
        sorted_unique(node["superseded_by"], f"view.decisions[{index}].superseded_by", 1)
    expected = sorted(set(identifiers), key=lambda item: item.encode("utf-8"))
    if identifiers != expected:
        raise ContractError("materialized decisions must be sorted and unique by ADR ID")
