"""Authority-neutral comparison of caller-declared ApprovalRecord fields."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        canonical_json, decode_canonical)
from .constants import (ASSESSMENT_API, ASSESSMENT_DOMAIN, CANONICALIZATION,
                        MAX_ASSESSMENT_BYTES, MAX_REQUEST_BYTES, MODE,
                        POSITIVE_RELATIONS, RELATION_REASONS, REQUEST_API,
                        REQUEST_DOMAIN, RESULT)
from .record import (declared_target, declared_target_sha256, validate_record,
                     validate_target)
from .shape import enum, integer, require_keys, sha256

REQUEST_FIELDS = {
    "api_version", "approval_record", "canonicalization", "evaluated_at_unix_ms",
    "expected_target", "expected_target_sha256", "request_sha256",
}
ASSESSMENT_FIELDS = {
    "api_version", "approval_id", "approval_sha256", "approver_identity_state",
    "assessment_mode", "assessment_sha256", "authority_proof_state",
    "authorization_decision", "canonicalization", "condition_satisfaction_state",
    "effect_attestation", "effective_approval_state", "expected_target_sha256",
    "permission_attestation", "persistence_attestation", "policy_decision",
    "reason_codes", "relations", "request_sha256", "result",
    "revocation_registry_state", "risk_acceptance_state",
    "separation_of_duty_proof_state", "transition_attestation",
}
RELATION_VALUES = {
    "approver": ("approver_mismatch", "same_declared_approver"),
    "authority_binding": ("authority_binding_mismatch", "same_declared_authority_binding"),
    "binding": ("binding_mismatch", "same_declared_binding"),
    "conditions": ("conditions_mismatch", "same_declared_conditions"),
    "decision": ("decision_mismatch", "same_declared_decision"),
    "revocation": ("declared_revocation_time_not_reached",
                   "declared_revocation_time_reached"),
    "risk_acceptance": ("risk_acceptance_mismatch",
                        "same_declared_risk_acceptance_refs"),
    "scope": ("same_declared_scope", "scope_mismatch"),
    "separation_of_duty": ("same_declared_separation_of_duty",
                           "separation_of_duty_mismatch"),
    "subject": ("same_declared_subject", "subject_mismatch"),
    "temporal": ("inside_declared_window", "outside_declared_window"),
}
REASON_CODES = tuple(sorted(set(RELATION_REASONS.values())))


def request_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_REQUEST_BYTES, "approval assessment request")
    payload = copy.deepcopy(value)
    payload["request_sha256"] = ""
    return bounded_digest(REQUEST_DOMAIN, payload, MAX_REQUEST_BYTES,
                          "approval assessment request digest preimage")


def validate_request(value: Any, allow_empty_digest: bool = False) -> dict[str, Any]:
    request = require_keys(value, "approval assessment request", REQUEST_FIELDS)
    if request["api_version"] != REQUEST_API or request["canonicalization"] != CANONICALIZATION:
        raise ContractError("approval assessment request API/canonicalization is unsupported")
    bounded_canonical_json(request, MAX_REQUEST_BYTES, "approval assessment request")
    integer(request["evaluated_at_unix_ms"], "evaluated_at_unix_ms", 0, 2**63 - 1)
    validate_record(request["approval_record"])
    validate_target(request["expected_target"])
    sha256(request["expected_target_sha256"], "expected_target_sha256")
    if declared_target_sha256(request["expected_target"]) != request["expected_target_sha256"]:
        raise ContractError("expected target digest does not match")
    if allow_empty_digest and request["request_sha256"] == "":
        return request
    sha256(request["request_sha256"], "request_sha256")
    if request_sha256(request) != request["request_sha256"]:
        raise ContractError("approval assessment request self digest does not match")
    return request


def seal_request(record: dict[str, Any], expected_target: dict[str, Any],
                 evaluated_at_unix_ms: int) -> dict[str, Any]:
    request = {
        "api_version": REQUEST_API,
        "approval_record": copy.deepcopy(record),
        "canonicalization": CANONICALIZATION,
        "evaluated_at_unix_ms": evaluated_at_unix_ms,
        "expected_target": copy.deepcopy(expected_target),
        "expected_target_sha256": declared_target_sha256(expected_target),
        "request_sha256": "",
    }
    validate_request(request, allow_empty_digest=True)
    request["request_sha256"] = request_sha256(request)
    validate_request(request)
    return request


def decode_request(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_REQUEST_BYTES, "approval assessment request")
    return validate_request(value)


def _same(left: Any, right: Any, positive: str, negative: str) -> str:
    return positive if left == right else negative


def _relations(request: dict[str, Any]) -> dict[str, str]:
    expected = request["expected_target"]
    actual = declared_target(request["approval_record"])
    validity = request["approval_record"]["validity"]
    evaluated = request["evaluated_at_unix_ms"]
    revoked = validity["revoked_at_unix_ms"]
    return {
        "approver": _same(expected["approver"], actual["approver"],
                          "same_declared_approver", "approver_mismatch"),
        "authority_binding": _same(expected["authority_binding"],
                                   actual["authority_binding"],
                                   "same_declared_authority_binding",
                                   "authority_binding_mismatch"),
        "binding": _same(expected["bindings"], actual["bindings"],
                         "same_declared_binding", "binding_mismatch"),
        "conditions": _same(expected["conditions"], actual["conditions"],
                            "same_declared_conditions", "conditions_mismatch"),
        "decision": _same(expected["decision"], actual["decision"],
                          "same_declared_decision", "decision_mismatch"),
        "revocation": ("declared_revocation_time_reached" if revoked is not None and
                       evaluated >= revoked else "declared_revocation_time_not_reached"),
        "risk_acceptance": _same(expected["risk_acceptance_refs"],
                                 actual["risk_acceptance_refs"],
                                 "same_declared_risk_acceptance_refs",
                                 "risk_acceptance_mismatch"),
        "scope": _same(expected["scope"], actual["scope"],
                       "same_declared_scope", "scope_mismatch"),
        "separation_of_duty": _same(expected["separation_of_duty_declaration"],
                                    actual["separation_of_duty_declaration"],
                                    "same_declared_separation_of_duty",
                                    "separation_of_duty_mismatch"),
        "subject": _same(expected["subject"], actual["subject"],
                         "same_declared_subject", "subject_mismatch"),
        "temporal": ("inside_declared_window" if
                     validity["not_before_unix_ms"] <= evaluated <
                     validity["expires_at_unix_ms"] else "outside_declared_window"),
    }


def _reason_codes(relations: dict[str, str]) -> list[str]:
    reasons = [RELATION_REASONS[relation] for field, relation in relations.items()
               if relation != POSITIVE_RELATIONS[field]]
    return sorted(set(reasons))


def assessment_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_ASSESSMENT_BYTES, "approval declared assessment")
    payload = copy.deepcopy(value)
    payload["assessment_sha256"] = ""
    return bounded_digest(ASSESSMENT_DOMAIN, payload, MAX_ASSESSMENT_BYTES,
                          "approval declared assessment digest preimage")


def evaluate_declared_assessment(value: Any) -> dict[str, Any]:
    request = validate_request(value)
    record = request["approval_record"]
    relations = _relations(request)
    assessment = {
        "api_version": ASSESSMENT_API,
        "approval_id": record["approval_id"],
        "approval_sha256": record["approval_sha256"],
        "approver_identity_state": "not_evaluated",
        "assessment_mode": MODE,
        "assessment_sha256": "",
        "authority_proof_state": "not_evaluated",
        "authorization_decision": "none",
        "canonicalization": CANONICALIZATION,
        "condition_satisfaction_state": "not_evaluated",
        "effect_attestation": False,
        "effective_approval_state": "not_evaluated",
        "expected_target_sha256": request["expected_target_sha256"],
        "permission_attestation": False,
        "persistence_attestation": False,
        "policy_decision": "none",
        "reason_codes": _reason_codes(relations),
        "relations": relations,
        "request_sha256": request["request_sha256"],
        "result": RESULT,
        "revocation_registry_state": "not_evaluated",
        "risk_acceptance_state": "not_evaluated",
        "separation_of_duty_proof_state": "not_evaluated",
        "transition_attestation": False,
    }
    assessment["assessment_sha256"] = assessment_sha256(assessment)
    validate_assessment_shape(assessment)
    return assessment


def _validate_relations(value: Any) -> None:
    relations = require_keys(value, "relations", set(RELATION_VALUES))
    for field, allowed in RELATION_VALUES.items():
        enum(relations[field], f"relations.{field}", allowed)


def _validate_reason_codes(value: Any, relations: dict[str, str]) -> None:
    if not isinstance(value, list) or len(value) > len(REASON_CODES):
        raise ContractError("reason_codes must be a bounded array")
    if any(item not in REASON_CODES for item in value):
        raise ContractError("reason_codes contains an unsupported value")
    if any(left.encode() >= right.encode() for left, right in zip(value, value[1:])):
        raise ContractError("reason_codes must be strictly UTF-8 sorted and unique")
    if value != _reason_codes(relations):
        raise ContractError("reason_codes do not match declared relations")


def validate_assessment_shape(value: Any) -> dict[str, Any]:
    assessment = require_keys(value, "approval declared assessment", ASSESSMENT_FIELDS)
    if (assessment["api_version"] != ASSESSMENT_API or
            assessment["canonicalization"] != CANONICALIZATION):
        raise ContractError("approval assessment API/canonicalization is unsupported")
    if assessment["assessment_mode"] != MODE:
        raise ContractError("assessment mode escalates the contract-only boundary")
    state_fields = ("approver_identity_state", "authority_proof_state",
                    "condition_satisfaction_state", "effective_approval_state",
                    "revocation_registry_state", "risk_acceptance_state",
                    "separation_of_duty_proof_state")
    if any(assessment[field] != "not_evaluated" for field in state_fields):
        raise ContractError("authority lifecycle states must remain not_evaluated")
    if assessment["policy_decision"] != "none" or assessment["authorization_decision"] != "none":
        raise ContractError("contract-only assessment cannot make a policy decision")
    bool_fields = ("effect_attestation", "permission_attestation",
                   "persistence_attestation", "transition_attestation")
    if any(assessment[field] is not False for field in bool_fields):
        raise ContractError("contract-only assessment cannot emit an attestation")
    if assessment["result"] != RESULT:
        raise ContractError("assessment result escalates the contract-only boundary")
    bounded_canonical_json(assessment, MAX_ASSESSMENT_BYTES,
                           "approval declared assessment")
    for field in ("approval_sha256", "assessment_sha256", "expected_target_sha256",
                  "request_sha256"):
        sha256(assessment[field], field)
    if assessment["approval_id"] != f"approval-record-{assessment['approval_sha256']}":
        raise ContractError("assessment ApprovalRecord identity is inconsistent")
    _validate_relations(assessment["relations"])
    _validate_reason_codes(assessment["reason_codes"], assessment["relations"])
    if assessment_sha256(assessment) != assessment["assessment_sha256"]:
        raise ContractError("approval declared assessment self digest does not match")
    return assessment


def decode_assessment(raw: bytes) -> dict[str, Any]:
    value = decode_canonical(raw, MAX_ASSESSMENT_BYTES, "approval declared assessment")
    return validate_assessment_shape(value)


def validate_assessment(request: Any, assessment: Any) -> None:
    validate_assessment_shape(assessment)
    expected = evaluate_declared_assessment(request)
    if canonical_json(expected) != canonical_json(assessment):
        raise ContractError("assessment is not the exact authority-neutral reassembly")

