"""Authority-neutral comparison of caller-declared Grant relations."""

from __future__ import annotations

from typing import Any

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        canonical_json, digest)
from .constants import (ACTION_DOMAIN, ASSESSMENT_API, ASSESSMENT_DOMAIN, CANONICALIZATION,
                        MAX_ASSESSMENT_BYTES, MAX_REQUEST_BYTES, MODE, POSITIVE_RELATIONS,
                        RELATION_REASONS, REQUEST_API, REQUEST_DOMAIN, RESULT)
from .grant import validate_grant
from .scope import scope_relation, validate_requested_action
from .shape import (enum, integer, require_keys, sha256, sorted_unique_strings,
                    validate_bindings, validate_capability, validate_principal,
                    validate_task_binding)

REQUEST_FIELDS = {"api_version", "canonicalization", "evaluated_at_unix_ms", "expected",
                  "grant", "request_sha256", "requested_action"}
ASSESSMENT_FIELDS = {
    "api_version", "approval_state", "assessment_mode", "assessment_sha256",
    "authority_proof_state", "authorization_decision", "canonicalization",
    "effect_attestation", "grant_id", "grant_sha256", "permission_attestation",
    "reason_codes", "relations", "request_sha256", "requested_action_sha256",
    "result", "revocation_state", "usage_state",
}
RELATION_VALUES = {
    "binding": ("binding_mismatch", "same_declared_binding"),
    "budget": ("at_or_below_declared_ceiling", "exceeds_declared_ceiling"),
    "capability": ("capability_mismatch", "same_declared_capability"),
    "effect": ("effect_mismatch", "same_declared_effect"),
    "scope": ("covered_by_declaration", "denied_by_declaration", "outside_declared_scope"),
    "subject": ("same_declared_subject", "subject_mismatch"),
    "task": ("same_declared_task", "task_mismatch"),
    "temporal": ("inside_declared_window", "outside_declared_window"),
}
REASON_CODES = tuple(sorted(set(RELATION_REASONS.values())))


def _validate_expected(value: Any) -> dict[str, Any]:
    node = require_keys(value, "expected", {"bindings", "capability", "subject", "task_binding"})
    validate_bindings(node["bindings"], "expected.bindings")
    validate_capability(node["capability"], "expected.capability")
    validate_principal(node["subject"], "expected.subject")
    validate_task_binding(node["task_binding"], "expected.task_binding")
    return node


def request_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_REQUEST_BYTES, "declared assessment request")
    payload = dict(value)
    payload["request_sha256"] = ""
    return bounded_digest(REQUEST_DOMAIN, payload, MAX_REQUEST_BYTES,
                          "declared assessment request")


def validate_request(value: Any) -> dict[str, Any]:
    node = require_keys(value, "declared assessment request", REQUEST_FIELDS)
    if node["api_version"] != REQUEST_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("declared assessment request API or canonicalization is unsupported")
    integer(node["evaluated_at_unix_ms"], "evaluated_at_unix_ms", 0, 2**63 - 1)
    bounded_canonical_json(node, MAX_REQUEST_BYTES, "declared assessment request")
    _validate_expected(node["expected"])
    validate_grant(node["grant"])
    validate_requested_action(node["requested_action"])
    sha256(node["request_sha256"], "request_sha256")
    if request_sha256(node) != node["request_sha256"]:
        raise ContractError("declared assessment request self digest does not match")
    return node


def _budget_relation(grant: dict[str, Any], action: dict[str, Any]) -> str:
    budget, usage = grant["budget"], action["usage"]
    pairs = (("call_count", "max_calls"), ("cost_usd_micros", "max_cost_usd_micros"),
             ("input_tokens", "max_input_tokens"), ("network_bytes", "max_network_bytes"),
             ("output_bytes", "max_output_bytes"), ("output_tokens", "max_output_tokens"),
             ("timeout_ms", "timeout_ms"))
    if all(usage[action_field] <= budget[grant_field] for action_field, grant_field in pairs):
        return "at_or_below_declared_ceiling"
    return "exceeds_declared_ceiling"


def _temporal_relation(grant: dict[str, Any], evaluated_at: int) -> str:
    validity = grant["validity"]
    if evaluated_at < validity["not_before_unix_ms"]:
        return "outside_declared_window"
    if evaluated_at >= validity["expires_at_unix_ms"]:
        return "outside_declared_window"
    return "inside_declared_window"


def _same(left: Any, right: Any, positive: str, negative: str) -> str:
    return positive if left == right else negative


def _relations(request: dict[str, Any]) -> dict[str, str]:
    grant, expected, action = request["grant"], request["expected"], request["requested_action"]
    return {
        "binding": _same(expected["bindings"], grant["bindings"],
                         "same_declared_binding", "binding_mismatch"),
        "budget": _budget_relation(grant, action),
        "capability": _same(expected["capability"], grant["capability"],
                            "same_declared_capability", "capability_mismatch"),
        "effect": _same(action["effect_id"], grant["scope"]["effect_id"],
                        "same_declared_effect", "effect_mismatch"),
        "scope": scope_relation(grant["scope"], action),
        "subject": _same(expected["subject"], grant["subject"],
                         "same_declared_subject", "subject_mismatch"),
        "task": _same(expected["task_binding"], grant["task_binding"],
                      "same_declared_task", "task_mismatch"),
        "temporal": _temporal_relation(grant, request["evaluated_at_unix_ms"]),
    }


def assessment_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_ASSESSMENT_BYTES, "declared assessment")
    payload = dict(value)
    payload["assessment_sha256"] = ""
    return bounded_digest(ASSESSMENT_DOMAIN, payload, MAX_ASSESSMENT_BYTES,
                          "declared assessment")


def evaluate_declared_assessment(value: Any) -> dict[str, Any]:
    request = validate_request(value)
    relations = _relations(request)
    reasons = _reason_codes(relations)
    grant = request["grant"]
    assessment = {
        "api_version": ASSESSMENT_API, "approval_state": "not_evaluated",
        "assessment_mode": MODE, "assessment_sha256": "",
        "authority_proof_state": "not_evaluated", "authorization_decision": "none",
        "canonicalization": CANONICALIZATION, "effect_attestation": False,
        "grant_id": grant["grant_id"], "grant_sha256": grant["grant_sha256"],
        "permission_attestation": False, "reason_codes": reasons, "relations": relations,
        "request_sha256": request["request_sha256"],
        "requested_action_sha256": digest(ACTION_DOMAIN, request["requested_action"]),
        "result": RESULT, "revocation_state": "not_evaluated", "usage_state": "not_evaluated",
    }
    assessment["assessment_sha256"] = assessment_sha256(assessment)
    bounded_canonical_json(assessment, MAX_ASSESSMENT_BYTES, "declared assessment")
    return assessment


def _reason_codes(relations: dict[str, str]) -> list[str]:
    reasons = set()
    for field, relation in relations.items():
        if relation == POSITIVE_RELATIONS[field]:
            continue
        if field == "scope" and relations["effect"] == "effect_mismatch":
            continue
        reasons.add(RELATION_REASONS[relation])
    return sorted(reasons)


def _validate_relations(value: Any) -> None:
    node = require_keys(value, "relations", set(RELATION_VALUES))
    for field, allowed in RELATION_VALUES.items():
        enum(node[field], f"relations.{field}", allowed)


def validate_assessment_shape(value: Any) -> dict[str, Any]:
    node = require_keys(value, "declared assessment", ASSESSMENT_FIELDS)
    if node["api_version"] != ASSESSMENT_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("declared assessment API or canonicalization is unsupported")
    fixed = ("approval_state", "authority_proof_state", "revocation_state", "usage_state")
    if any(node[field] != "not_evaluated" for field in fixed):
        raise ContractError("all authority lifecycle states must remain not_evaluated")
    if node["assessment_mode"] != MODE or node["authorization_decision"] != "none":
        raise ContractError("assessment must remain authority-neutral with no decision")
    if node["permission_attestation"] is not False or node["effect_attestation"] is not False:
        raise ContractError("assessment cannot attest permission or an effect")
    if node["result"] != RESULT:
        raise ContractError("declared assessment result escalates the contract-only boundary")
    bounded_canonical_json(node, MAX_ASSESSMENT_BYTES, "declared assessment")
    for field in ("assessment_sha256", "grant_sha256", "request_sha256",
                  "requested_action_sha256"):
        sha256(node[field], field)
    if node["grant_id"] != f"capability-grant-{node['grant_sha256']}":
        raise ContractError("assessment Grant identity is inconsistent")
    _validate_relations(node["relations"])
    if (node["relations"]["effect"] == "effect_mismatch" and
            node["relations"]["scope"] != "outside_declared_scope"):
        raise ContractError("effect_mismatch requires scope outside_declared_scope")
    sorted_unique_strings(node["reason_codes"], "reason_codes", REASON_CODES) if node[
        "reason_codes"] else None
    expected_reasons = _reason_codes(node["relations"])
    if node["reason_codes"] != expected_reasons:
        raise ContractError("reason_codes do not match the declared relations")
    if assessment_sha256(node) != node["assessment_sha256"]:
        raise ContractError("declared assessment self digest does not match")
    return node


def validate_assessment(request: Any, assessment: Any) -> None:
    validate_assessment_shape(assessment)
    expected = evaluate_declared_assessment(request)
    if canonical_json(expected) != canonical_json(assessment):
        raise ContractError("assessment is not the exact authority-neutral reassembly")
