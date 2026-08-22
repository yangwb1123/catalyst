"""Pure declared TransitionReceipt assessment without authority or state mutation."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        canonical_json, decode_canonical)
from .constants import (ASSESSMENT_API, ASSESSMENT_DOMAIN, CANONICALIZATION, EDGES,
                        MAX_ASSESSMENT_BYTES, MAX_REQUEST_BYTES, MODE, REQUEST_API,
                        REQUEST_DOMAIN, RESULT, REWORK_TARGETS)
from .receipt import (declared_target, declared_target_sha256, validate_receipt,
                      validate_target)
from .shape import enum, integer, require_keys, sha256
from .vocabulary import transition_vocabulary

REQUEST_FIELDS = {
    "api_version", "canonicalization", "evaluated_at_unix_ms", "expected_target",
    "expected_target_sha256", "previous_receipt", "request_sha256", "transition_receipt",
}
RELATION_VALUES = {
    "target": ("same_declared_target", "target_mismatch"),
    "edge": ("listed_declared_edge", "unlisted_declared_edge"),
    "chain": ("initial_declared_chain", "same_declared_predecessor", "predecessor_mismatch"),
    "continuity": ("same_declared_state_continuity", "state_continuity_mismatch"),
    "preconditions": ("declared_pass_or_na_only", "declared_fail_or_unknown_present"),
    "applicability": ("internally_consistent_declared_applicability",),
    "recovery": ("internally_consistent_declared_recovery", "rework_or_resume_mismatch"),
    "temporal": ("nondecreasing_declared_time", "temporal_declaration_mismatch"),
}
NEGATIVE_RELATIONS = tuple(sorted({values[-1] for values in RELATION_VALUES.values()
                                   if len(values) > 1}))
ASSESSMENT_FIELDS = {
    "api_version", "approval_state", "assessment_mode", "assessment_sha256",
    "authorization_decision", "canonicalization", "completion_attestation",
    "controller_authentication_state", "effect_attestation", "evidence_state",
    "execution_attestation", "expected_target_sha256", "grant_state", "ledger_state",
    "permission_attestation", "persistence_attestation", "policy_decision",
    "precondition_truth_state", "reason_codes", "receipt_id", "receipt_sha256",
    "relations", "request_sha256", "result", "transition_attestation",
    "transition_vocabulary_sha256", "waiver_state",
}


def request_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_REQUEST_BYTES, "Transition assessment request")
    payload = copy.deepcopy(value)
    payload["request_sha256"] = ""
    return bounded_digest(REQUEST_DOMAIN, payload, MAX_REQUEST_BYTES,
                          "Transition assessment request digest preimage")


def validate_request(value: Any, allow_empty_digest: bool = False) -> dict[str, Any]:
    node = require_keys(value, "Transition assessment request", REQUEST_FIELDS)
    if node["api_version"] != REQUEST_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("Transition assessment request API/canonicalization is unsupported")
    bounded_canonical_json(node, MAX_REQUEST_BYTES, "Transition assessment request")
    integer(node["evaluated_at_unix_ms"], "evaluated_at_unix_ms")
    validate_receipt(node["transition_receipt"])
    if node["previous_receipt"] is not None:
        validate_receipt(node["previous_receipt"])
    validate_target(node["expected_target"])
    sha256(node["expected_target_sha256"], "expected_target_sha256")
    if declared_target_sha256(node["expected_target"]) != node["expected_target_sha256"]:
        raise ContractError("expected target digest does not match")
    if allow_empty_digest and node["request_sha256"] == "":
        return node
    sha256(node["request_sha256"], "request_sha256")
    if request_sha256(node) != node["request_sha256"]:
        raise ContractError("Transition assessment request self digest does not match")
    return node


def seal_request(receipt: dict[str, Any], target: dict[str, Any], previous: Any,
                 evaluated_at_unix_ms: int) -> dict[str, Any]:
    validate_receipt(receipt)
    validate_target(target)
    if previous is not None:
        validate_receipt(previous)
    integer(evaluated_at_unix_ms, "evaluated_at_unix_ms")
    node = {
        "api_version": REQUEST_API,
        "canonicalization": CANONICALIZATION,
        "evaluated_at_unix_ms": evaluated_at_unix_ms,
        "expected_target": copy.deepcopy(target),
        "expected_target_sha256": declared_target_sha256(target),
        "previous_receipt": copy.deepcopy(previous),
        "request_sha256": "",
        "transition_receipt": copy.deepcopy(receipt),
    }
    validate_request(node, allow_empty_digest=True)
    node["request_sha256"] = request_sha256(node)
    return validate_request(node)


def decode_request(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_REQUEST_BYTES, "Transition assessment request")
    return validate_request(value)


def _chain_relation(current: dict[str, Any], previous: Any) -> str:
    transition = current["transition"]
    initial = (current["sequence"] == 1 and previous is None and
               current["previous_receipt_id"] is None and
               current["previous_receipt_sha256"] is None and
               transition["from_state"] == "DRAFT")
    if initial:
        return "initial_declared_chain"
    if previous is None or current["sequence"] != previous["sequence"] + 1:
        return "predecessor_mismatch"
    same_identity = (current["previous_receipt_id"] == previous["receipt_id"] and
                     current["previous_receipt_sha256"] == previous["receipt_sha256"])
    current_task, previous_task = current["task_binding"], previous["task_binding"]
    same_scope = (current["work_id"] == previous["work_id"] and
                  current_task["project_id"] == previous_task["project_id"] and
                  current_task["change_id"] == previous_task["change_id"])
    return "same_declared_predecessor" if same_identity and same_scope else "predecessor_mismatch"


def _continuity_relation(current: dict[str, Any], previous: Any) -> str:
    source = current["transition"]["from_state"]
    consistent = source == "DRAFT" if previous is None else (
        previous["transition"]["to_state"] == source)
    return ("same_declared_state_continuity" if consistent
            else "state_continuity_mismatch")


def _edge_relation(current: dict[str, Any], previous: Any) -> str:
    transition = current["transition"]
    source, target = transition["from_state"], transition["to_state"]
    listed = target in EDGES.get(source, ())
    if (previous is not None and source in ("NEEDS_INFO", "BLOCKED") and
            previous["transition"]["to_state"] == source):
        listed = listed or target == previous["transition"]["resume_state"]
    return "listed_declared_edge" if listed else "unlisted_declared_edge"


def _applicability_relation(current: dict[str, Any]) -> str:
    # Strict receipt validation rejects every intrinsic applicability contradiction
    # before assessment, so this relation has no negative runtime state in v1.
    del current
    return "internally_consistent_declared_applicability"


def _entry_recovery(transition: dict[str, Any], previous: Any) -> bool:
    source, target = transition["from_state"], transition["to_state"]
    rework = transition["rework_target"]
    resume = transition["resume_state"]
    if (rework is not None) != (target == "CHANGES_REQUESTED"):
        return False
    if rework is not None and rework not in REWORK_TARGETS:
        return False
    if (resume is not None) != (target in ("NEEDS_INFO", "BLOCKED")):
        return False
    inherited = (source == "NEEDS_INFO" and target == "BLOCKED" and previous is not None)
    expected_resume = previous["transition"]["resume_state"] if inherited else source
    return resume is None or resume == expected_resume


def _exit_recovery(transition: dict[str, Any], previous: Any) -> bool:
    source, target = transition["from_state"], transition["to_state"]
    if source == "CHANGES_REQUESTED":
        return (previous is not None and
                target in (previous["transition"]["rework_target"], "BLOCKED",
                           "REJECTED", "SUPERSEDED"))
    if source in ("NEEDS_INFO", "BLOCKED"):
        escalations = (("BLOCKED", "REJECTED", "SUPERSEDED") if source == "NEEDS_INFO"
                       else ("REJECTED", "SUPERSEDED"))
        return previous is not None and target in (
            previous["transition"]["resume_state"], *escalations)
    return True


def _recovery_relation(current: dict[str, Any], previous: Any) -> str:
    transition = current["transition"]
    consistent = (_entry_recovery(transition, previous) and
                  _exit_recovery(transition, previous))
    return ("internally_consistent_declared_recovery" if consistent
            else "rework_or_resume_mismatch")


def _relations(request: dict[str, Any]) -> dict[str, str]:
    current, previous = request["transition_receipt"], request["previous_receipt"]
    declared_at = current["transition"]["declared_at_unix_ms"]
    temporal = declared_at <= request["evaluated_at_unix_ms"]
    if previous is not None:
        temporal = temporal and previous["transition"]["declared_at_unix_ms"] <= declared_at
    return {
        "applicability": _applicability_relation(current),
        "chain": _chain_relation(current, previous),
        "continuity": _continuity_relation(current, previous),
        "edge": _edge_relation(current, previous),
        "preconditions": ("declared_pass_or_na_only" if all(
            item["result"] in ("PASS", "NA") for item in current["preconditions"])
            else "declared_fail_or_unknown_present"),
        "recovery": _recovery_relation(current, previous),
        "target": ("same_declared_target" if request["expected_target"] ==
                   declared_target(current) else "target_mismatch"),
        "temporal": ("nondecreasing_declared_time" if temporal
                     else "temporal_declaration_mismatch"),
    }


def _reason_codes(relations: dict[str, str]) -> list[str]:
    return sorted(value for value in relations.values() if value in NEGATIVE_RELATIONS)


def assessment_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_ASSESSMENT_BYTES, "Transition declared assessment")
    payload = copy.deepcopy(value)
    payload["assessment_sha256"] = ""
    return bounded_digest(ASSESSMENT_DOMAIN, payload, MAX_ASSESSMENT_BYTES,
                          "Transition declared assessment digest preimage")


def evaluate_declared_assessment(value: Any) -> dict[str, Any]:
    request = validate_request(value)
    receipt = request["transition_receipt"]
    relations = _relations(request)
    assessment = {
        "api_version": ASSESSMENT_API,
        "approval_state": "not_evaluated",
        "assessment_mode": MODE,
        "assessment_sha256": "",
        "authorization_decision": "none",
        "canonicalization": CANONICALIZATION,
        "completion_attestation": False,
        "controller_authentication_state": "not_evaluated",
        "effect_attestation": False,
        "evidence_state": "not_evaluated",
        "execution_attestation": False,
        "expected_target_sha256": request["expected_target_sha256"],
        "grant_state": "not_evaluated",
        "ledger_state": "not_evaluated",
        "permission_attestation": False,
        "persistence_attestation": False,
        "policy_decision": "none",
        "precondition_truth_state": "not_evaluated",
        "reason_codes": _reason_codes(relations),
        "receipt_id": receipt["receipt_id"],
        "receipt_sha256": receipt["receipt_sha256"],
        "relations": relations,
        "request_sha256": request["request_sha256"],
        "result": RESULT,
        "transition_attestation": False,
        "transition_vocabulary_sha256": receipt["transition_vocabulary_sha256"],
        "waiver_state": "not_evaluated",
    }
    assessment["assessment_sha256"] = assessment_sha256(assessment)
    return validate_assessment_shape(assessment)


def _validate_relations(value: Any) -> dict[str, str]:
    node = require_keys(value, "relations", set(RELATION_VALUES))
    for field, allowed in RELATION_VALUES.items():
        enum(node[field], f"relations.{field}", allowed)
    return node


def validate_assessment_shape(value: Any) -> dict[str, Any]:
    node = require_keys(value, "Transition declared assessment", ASSESSMENT_FIELDS)
    bounded_canonical_json(node, MAX_ASSESSMENT_BYTES, "Transition declared assessment")
    if node["api_version"] != ASSESSMENT_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("Transition assessment API/canonicalization is unsupported")
    if node["assessment_mode"] != MODE or node["result"] != RESULT:
        raise ContractError("Transition assessment escalates its contract-only boundary")
    states = ("approval_state", "controller_authentication_state", "evidence_state",
              "grant_state", "ledger_state", "precondition_truth_state", "waiver_state")
    if any(node[field] != "not_evaluated" for field in states):
        raise ContractError("Transition authority states must remain not_evaluated")
    if node["policy_decision"] != "none" or node["authorization_decision"] != "none":
        raise ContractError("Transition contract cannot make an authority decision")
    attestations = ("completion_attestation", "effect_attestation", "execution_attestation",
                    "permission_attestation", "persistence_attestation",
                    "transition_attestation")
    if any(node[field] is not False for field in attestations):
        raise ContractError("Transition contract cannot emit an attestation")
    for field in ("assessment_sha256", "expected_target_sha256", "receipt_sha256",
                  "request_sha256", "transition_vocabulary_sha256"):
        sha256(node[field], field)
    if node["transition_vocabulary_sha256"] != transition_vocabulary()["vocabulary_sha256"]:
        raise ContractError("assessment does not bind the frozen Transition vocabulary")
    if node["receipt_id"] != f"transition-receipt-{node['receipt_sha256']}":
        raise ContractError("assessment TransitionReceipt identity is inconsistent")
    relations = _validate_relations(node["relations"])
    if node["reason_codes"] != _reason_codes(relations):
        raise ContractError("assessment reason_codes do not match relations")
    if assessment_sha256(node) != node["assessment_sha256"]:
        raise ContractError("Transition declared assessment self digest does not match")
    return node


def decode_assessment(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_ASSESSMENT_BYTES, "Transition declared assessment")
    return validate_assessment_shape(value)


def validate_assessment(request: Any, assessment: Any) -> None:
    validate_assessment_shape(assessment)
    if canonical_json(evaluate_declared_assessment(request)) != canonical_json(assessment):
        raise ContractError("assessment is not the exact authority-neutral reassembly")
