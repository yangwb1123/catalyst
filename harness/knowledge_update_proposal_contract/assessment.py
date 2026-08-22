"""Pure declared KnowledgeUpdateProposal assessment without authority or mutation."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        canonical_json, decode_canonical)
from .constants import (ASSESSMENT_API, ASSESSMENT_DOMAIN, CANONICALIZATION,
                        MAX_ASSESSMENT_BYTES, MAX_REQUEST_BYTES, MODE, REQUEST_API,
                        REQUEST_DOMAIN, RESULT)
from .proposal import (declared_target, declared_target_sha256, validate_proposal,
                       validate_target)
from .shape import enum, integer, reasons, require_keys, sha256

REQUEST_FIELDS = {
    "api_version", "canonicalization", "evaluated_at_unix_ms", "expected_target",
    "expected_target_sha256", "knowledge_update_proposal", "request_sha256",
}
RELATION_VALUES = {
    "binding": ("same_declared_binding", "binding_mismatch"),
    "grant_ref": ("same_declared_grant_ref", "grant_ref_mismatch"),
    "mutations": ("same_declared_mutations", "mutations_mismatch"),
    "proposer": ("same_declared_proposer", "proposer_mismatch"),
    "record_set": ("same_declared_record_set", "record_set_mismatch"),
    "scope": ("same_declared_scope", "scope_mismatch"),
    "task_binding": ("same_declared_task_binding", "task_binding_mismatch"),
    "temporal": ("nonfuture_declared_submission", "future_declared_submission"),
}
RELATION_REASONS = {
    "binding_mismatch": "binding_mismatch",
    "grant_ref_mismatch": "grant_ref_mismatch",
    "mutations_mismatch": "mutations_mismatch",
    "proposer_mismatch": "proposer_mismatch",
    "record_set_mismatch": "record_set_mismatch",
    "scope_mismatch": "scope_mismatch",
    "task_binding_mismatch": "task_binding_mismatch",
    "future_declared_submission": "temporal_declaration_mismatch",
}
ASSESSMENT_FIELDS = {
    "api_version", "assessment_mode", "assessment_sha256", "authorization_decision",
    "canonicalization", "conflict_state", "context_state", "current_knowledge_state",
    "effect_attestation", "evidence_state", "execution_attestation",
    "expected_target_sha256", "freshness_state", "grant_state",
    "knowledge_adoption_attestation", "permission_attestation",
    "persistence_attestation", "policy_decision", "proposal_id", "proposal_sha256",
    "proposer_authentication_state", "reason_codes", "relations", "request_sha256",
    "result", "truth_attestation",
}


def request_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_REQUEST_BYTES, "KnowledgeUpdate assessment request")
    payload = copy.deepcopy(value)
    payload["request_sha256"] = ""
    return bounded_digest(REQUEST_DOMAIN, payload, MAX_REQUEST_BYTES,
                          "KnowledgeUpdate assessment request digest preimage")


def validate_request(value: Any, allow_empty_digest: bool = False) -> dict[str, Any]:
    bounded_canonical_json(value, MAX_REQUEST_BYTES, "KnowledgeUpdate assessment request")
    node = require_keys(value, "KnowledgeUpdate assessment request", REQUEST_FIELDS)
    if node["api_version"] != REQUEST_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("KnowledgeUpdate assessment request identity is unsupported")
    integer(node["evaluated_at_unix_ms"], "evaluated_at_unix_ms")
    validate_proposal(node["knowledge_update_proposal"])
    validate_target(node["expected_target"])
    sha256(node["expected_target_sha256"], "expected_target_sha256")
    if declared_target_sha256(node["expected_target"]) != node["expected_target_sha256"]:
        raise ContractError("expected target digest does not match")
    if allow_empty_digest and node["request_sha256"] == "":
        return node
    sha256(node["request_sha256"], "request_sha256")
    if request_sha256(node) != node["request_sha256"]:
        raise ContractError("KnowledgeUpdate assessment request self digest does not match")
    return node


def seal_request(proposal: dict[str, Any], target: dict[str, Any],
                 evaluated_at_unix_ms: int) -> dict[str, Any]:
    validate_proposal(proposal)
    validate_target(target)
    integer(evaluated_at_unix_ms, "evaluated_at_unix_ms")
    node = {
        "api_version": REQUEST_API,
        "canonicalization": CANONICALIZATION,
        "evaluated_at_unix_ms": evaluated_at_unix_ms,
        "expected_target": copy.deepcopy(target),
        "expected_target_sha256": declared_target_sha256(target),
        "knowledge_update_proposal": copy.deepcopy(proposal),
        "request_sha256": "",
    }
    validate_request(node, allow_empty_digest=True)
    node["request_sha256"] = request_sha256(node)
    return validate_request(node)


def decode_request(raw: bytes) -> dict[str, Any]:
    return validate_request(decode_canonical(raw, MAX_REQUEST_BYTES,
                                             "KnowledgeUpdate assessment request"))


def _same(left: Any, right: Any, field: str) -> str:
    return f"same_declared_{field}" if left == right else f"{field}_mismatch"


def _relations(request: dict[str, Any]) -> dict[str, str]:
    proposal, target = request["knowledge_update_proposal"], request["expected_target"]
    return {
        "binding": _same(proposal["bindings"], target["bindings"], "binding"),
        "grant_ref": _same(proposal["capability_grant_ref"],
                           target["capability_grant_ref"], "grant_ref"),
        "mutations": _same(proposal["mutations"], target["mutations"], "mutations"),
        "proposer": _same(proposal["proposer"], target["proposer"], "proposer"),
        "record_set": _same(proposal["record_set_sha256"],
                            target["record_set_sha256"], "record_set"),
        "scope": _same(proposal["knowledge_scope"], target["knowledge_scope"], "scope"),
        "task_binding": _same(proposal["task_binding"], target["task_binding"],
                              "task_binding"),
        "temporal": ("nonfuture_declared_submission" if
                     proposal["submitted_at_unix_ms"] <= request["evaluated_at_unix_ms"]
                     else "future_declared_submission"),
    }


def _reason_codes(relations: dict[str, str]) -> list[str]:
    return sorted({RELATION_REASONS[value] for value in relations.values()
                   if value in RELATION_REASONS})


def assessment_sha256(value: dict[str, Any]) -> str:
    bounded_canonical_json(value, MAX_ASSESSMENT_BYTES, "KnowledgeUpdate declared assessment")
    payload = copy.deepcopy(value)
    payload["assessment_sha256"] = ""
    return bounded_digest(ASSESSMENT_DOMAIN, payload, MAX_ASSESSMENT_BYTES,
                          "KnowledgeUpdate declared assessment digest preimage")


def evaluate_declared_assessment(value: Any) -> dict[str, Any]:
    request = validate_request(value)
    proposal = request["knowledge_update_proposal"]
    relations = _relations(request)
    assessment = {
        "api_version": ASSESSMENT_API,
        "assessment_mode": MODE,
        "assessment_sha256": "",
        "authorization_decision": "none",
        "canonicalization": CANONICALIZATION,
        "conflict_state": "not_evaluated",
        "context_state": "not_evaluated",
        "current_knowledge_state": "not_evaluated",
        "effect_attestation": False,
        "evidence_state": "not_evaluated",
        "execution_attestation": False,
        "expected_target_sha256": request["expected_target_sha256"],
        "freshness_state": "not_evaluated",
        "grant_state": "not_evaluated",
        "knowledge_adoption_attestation": False,
        "permission_attestation": False,
        "persistence_attestation": False,
        "policy_decision": "none",
        "proposal_id": proposal["proposal_id"],
        "proposal_sha256": proposal["proposal_sha256"],
        "proposer_authentication_state": "not_evaluated",
        "reason_codes": _reason_codes(relations),
        "relations": relations,
        "request_sha256": request["request_sha256"],
        "result": RESULT,
        "truth_attestation": False,
    }
    assessment["assessment_sha256"] = assessment_sha256(assessment)
    return validate_assessment_shape(assessment)


def _validate_relations(value: Any) -> dict[str, str]:
    node = require_keys(value, "relations", set(RELATION_VALUES))
    for field, allowed in RELATION_VALUES.items():
        enum(node[field], f"relations.{field}", allowed)
    return node


def validate_assessment_shape(value: Any) -> dict[str, Any]:
    bounded_canonical_json(value, MAX_ASSESSMENT_BYTES, "KnowledgeUpdate declared assessment")
    node = require_keys(value, "KnowledgeUpdate declared assessment", ASSESSMENT_FIELDS)
    if node["api_version"] != ASSESSMENT_API or node["canonicalization"] != CANONICALIZATION:
        raise ContractError("KnowledgeUpdate assessment identity is unsupported")
    if node["assessment_mode"] != MODE or node["result"] != RESULT:
        raise ContractError("KnowledgeUpdate assessment escalates its contract-only boundary")
    states = ("conflict_state", "context_state", "current_knowledge_state",
              "evidence_state", "freshness_state", "grant_state",
              "proposer_authentication_state")
    if any(node[field] != "not_evaluated" for field in states):
        raise ContractError("KnowledgeUpdate evaluator states must remain not_evaluated")
    if node["authorization_decision"] != "none" or node["policy_decision"] != "none":
        raise ContractError("KnowledgeUpdate contract cannot make an authority decision")
    attestations = ("effect_attestation", "execution_attestation",
                    "knowledge_adoption_attestation", "permission_attestation",
                    "persistence_attestation", "truth_attestation")
    if any(node[field] is not False for field in attestations):
        raise ContractError("KnowledgeUpdate contract cannot emit an attestation")
    for field in ("assessment_sha256", "expected_target_sha256", "proposal_sha256",
                  "request_sha256"):
        sha256(node[field], field)
    if node["proposal_id"] != f"knowledge-update-proposal-{node['proposal_sha256']}":
        raise ContractError("assessment KnowledgeUpdateProposal identity is inconsistent")
    relations = _validate_relations(node["relations"])
    expected_reasons = _reason_codes(relations)
    reasons(node["reason_codes"], "reason_codes", 0, len(RELATION_REASONS))
    if node["reason_codes"] != expected_reasons:
        raise ContractError("assessment reason_codes do not match relations")
    if assessment_sha256(node) != node["assessment_sha256"]:
        raise ContractError("KnowledgeUpdate declared assessment self digest does not match")
    return node


def decode_assessment(raw: bytes) -> dict[str, Any]:
    return validate_assessment_shape(decode_canonical(
        raw, MAX_ASSESSMENT_BYTES, "KnowledgeUpdate declared assessment"))


def validate_assessment(request: Any, assessment: Any) -> None:
    validate_assessment_shape(assessment)
    if canonical_json(evaluate_declared_assessment(request)) != canonical_json(assessment):
        raise ContractError("assessment is not the exact authority-neutral reassembly")
