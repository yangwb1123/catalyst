"""TransitionReceipt shape, identity, and declared-target projection."""

from __future__ import annotations

import copy
from typing import Any

from .canonical import ContractError, bounded_canonical_json, bounded_digest, decode_canonical
from .constants import (CANONICALIZATION, MAX_RECEIPT_BYTES, MAX_TARGET_BYTES,
                        MAX_TOTAL_EVIDENCE_REFS, MAX_TOTAL_REASON_CODES, RECEIPT_API,
                        RECEIPT_DOMAIN, RECEIPT_KIND, REWORK_TARGETS, STATES,
                        TARGET_DOMAIN)
from .shape import (array, enum, integer, nullable_sha256, reasons, require_keys, sha256,
                    sorted_nodes, text, validate_artifacts, validate_authority_refs,
                    validate_evidence_refs, validate_principal, validate_task_binding)
from .vocabulary import transition_vocabulary

RECEIPT_FIELDS = {
    "api_version", "actor", "applicability", "approval_refs", "bindings",
    "canonicalization", "capability_grant_ref", "declared_controller", "kind",
    "preconditions", "previous_receipt_id", "previous_receipt_sha256", "reason_codes",
    "receipt_id", "receipt_sha256", "sequence", "task_binding", "transition",
    "transition_vocabulary_sha256", "waiver_refs", "work_id",
}
TARGET_FIELDS = RECEIPT_FIELDS - {
    "api_version", "canonicalization", "kind", "receipt_id", "receipt_sha256",
}


def _validate_bindings(value: Any, label: str) -> None:
    fields = {"artifacts", "context_sha256", "impact_sha256", "plan_sha256",
              "policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
    node = require_keys(value, label, fields)
    validate_artifacts(node["artifacts"], f"{label}.artifacts")
    for field in ("context_sha256", "policy_sha256", "source_tree_sha256"):
        sha256(node[field], f"{label}.{field}")
    for field in ("impact_sha256", "plan_sha256", "risk_sha256"):
        nullable_sha256(node[field], f"{label}.{field}")
    text(node["source_revision"], f"{label}.source_revision")


def _validate_grant_ref(value: Any, label: str) -> None:
    node = require_keys(value, label, {"authority_domain", "grant_id", "grant_sha256"})
    text(node["authority_domain"], f"{label}.authority_domain")
    sha256(node["grant_sha256"], f"{label}.grant_sha256")
    if node["grant_id"] != f"capability-grant-{node['grant_sha256']}":
        raise ContractError(f"{label} identity is inconsistent")


def _validate_transition(value: Any, label: str) -> None:
    fields = {"declared_at_unix_ms", "from_state", "gate_id", "resume_state",
              "rework_target", "to_state"}
    node = require_keys(value, label, fields)
    integer(node["declared_at_unix_ms"], f"{label}.declared_at_unix_ms")
    enum(node["from_state"], f"{label}.from_state", STATES)
    enum(node["to_state"], f"{label}.to_state", STATES)
    for field in ("gate_id",):
        if node[field] is not None:
            text(node[field], f"{label}.{field}")
    for field in ("resume_state", "rework_target"):
        if node[field] is not None:
            enum(node[field], f"{label}.{field}", STATES)


def _validate_applicability(value: Any, label: str) -> tuple[int, int]:
    fields = {"decision", "evidence_refs", "reason_codes", "stage_id"}
    node = require_keys(value, label, fields)
    text(node["stage_id"], f"{label}.stage_id")
    decision = enum(node["decision"], f"{label}.decision",
                    ("applicable", "not_applicable"))
    declared_reasons = reasons(node["reason_codes"], f"{label}.reason_codes", 16)
    evidence_count = validate_evidence_refs(node["evidence_refs"], f"{label}.evidence_refs")
    if decision == "applicable" and declared_reasons:
        raise ContractError("applicable stage requires empty applicability reason_codes")
    if decision == "not_applicable" and (not declared_reasons or not evidence_count):
        raise ContractError("not_applicable stage requires reason and evidence declarations")
    return len(declared_reasons), evidence_count


def _validate_preconditions(value: Any, label: str) -> tuple[int, int]:
    nodes = array(value, label, 1, 64)
    reason_count = evidence_count = 0
    fields = {"evidence_refs", "precondition_id", "reason_codes", "result"}
    for index, value in enumerate(nodes):
        item_label = f"{label}[{index}]"
        node = require_keys(value, item_label, fields)
        text(node["precondition_id"], f"{item_label}.precondition_id")
        enum(node["result"], f"{item_label}.result", ("PASS", "FAIL", "NA", "UNKNOWN"))
        reason_count += len(reasons(node["reason_codes"], f"{item_label}.reason_codes", 16))
        evidence_count += validate_evidence_refs(node["evidence_refs"],
                                                  f"{item_label}.evidence_refs")
    sorted_nodes(nodes, label)
    return reason_count, evidence_count


def _validate_core(node: dict[str, Any], label: str) -> None:
    validate_principal(node["actor"], f"{label}.actor")
    validate_principal(node["declared_controller"], f"{label}.declared_controller", True)
    validate_task_binding(node["task_binding"], f"{label}.task_binding")
    _validate_bindings(node["bindings"], f"{label}.bindings")
    _validate_grant_ref(node["capability_grant_ref"], f"{label}.capability_grant_ref")
    validate_authority_refs(node["approval_refs"], f"{label}.approval_refs", "approval")
    validate_authority_refs(node["waiver_refs"], f"{label}.waiver_refs", "waiver")
    pre_reasons, pre_evidence = _validate_preconditions(node["preconditions"],
                                                        f"{label}.preconditions")
    app_reasons, app_evidence = _validate_applicability(
        node["applicability"], f"{label}.applicability")
    _validate_transition(node["transition"], f"{label}.transition")
    _validate_target_applicability(node, label)
    _validate_intrinsic_recovery(node["transition"], label)
    root_reasons = reasons(node["reason_codes"], f"{label}.reason_codes", 256)
    if len(root_reasons) + pre_reasons + app_reasons > MAX_TOTAL_REASON_CODES:
        raise ContractError(f"{label} exceeds total reason-code ceiling")
    if pre_evidence + app_evidence > MAX_TOTAL_EVIDENCE_REFS:
        raise ContractError(f"{label} exceeds total evidence-reference ceiling")


def _validate_target_applicability(node: dict[str, Any], label: str) -> None:
    applicability = node["applicability"]
    if applicability["stage_id"] != node["transition"]["to_state"]:
        raise ContractError(f"{label} applicability stage must equal transition.to_state")
    decision = applicability["decision"]
    if decision == "applicable" and applicability["reason_codes"]:
        raise ContractError(f"{label} applicable declaration requires no reasons")
    if decision == "not_applicable" and (
            not applicability["reason_codes"] or not applicability["evidence_refs"]):
        raise ContractError(f"{label} not_applicable declaration requires reason and evidence")


def _validate_intrinsic_recovery(transition: dict[str, Any], label: str) -> None:
    source, target = transition["from_state"], transition["to_state"]
    rework, resume = transition["rework_target"], transition["resume_state"]
    if (rework is not None) != (target == "CHANGES_REQUESTED"):
        raise ContractError(f"{label} rework_target must exist exactly on CHANGES_REQUESTED entry")
    if rework is not None and rework not in REWORK_TARGETS:
        raise ContractError(f"{label} rework_target is outside the frozen six-state set")
    if (resume is not None) != (target in ("NEEDS_INFO", "BLOCKED")):
        raise ContractError(f"{label} resume_state must exist exactly on suspended-state entry")
    if resume is not None and not (source == "NEEDS_INFO" and target == "BLOCKED"):
        if resume != source:
            raise ContractError(f"{label} resume_state must preserve from_state")


def _validate_predecessor_fields(node: dict[str, Any], label: str) -> None:
    sequence = integer(node["sequence"], f"{label}.sequence", 1)
    for field in ("previous_receipt_id", "previous_receipt_sha256"):
        value = node[field]
        if value is not None:
            if field.endswith("sha256"):
                sha256(value, f"{label}.{field}")
            else:
                text(value, f"{label}.{field}")
    identifier, digest = node["previous_receipt_id"], node["previous_receipt_sha256"]
    if sequence == 1:
        if identifier is not None or digest is not None:
            raise ContractError(f"{label} initial sequence requires null predecessor fields")
        if node["transition"]["from_state"] != "DRAFT":
            raise ContractError(f"{label} initial sequence must declare from_state=DRAFT")
    elif identifier is None or digest is None:
        raise ContractError(f"{label} subsequent sequence requires both predecessor fields")
    elif identifier != f"transition-receipt-{digest}":
        raise ContractError(f"{label} predecessor identity pair is inconsistent")
    text(node["work_id"], f"{label}.work_id")
    expected = transition_vocabulary()["vocabulary_sha256"]
    if node["transition_vocabulary_sha256"] != expected:
        raise ContractError(f"{label} does not bind the frozen Transition vocabulary")


def receipt_sha256(value: dict[str, Any], validate: bool = True) -> str:
    if validate:
        validate_receipt(value, allow_empty_identity=True)
    bounded_canonical_json(value, MAX_RECEIPT_BYTES, "TransitionReceipt")
    payload = copy.deepcopy(value)
    payload["receipt_id"] = ""
    payload["receipt_sha256"] = ""
    return bounded_digest(RECEIPT_DOMAIN, payload, MAX_RECEIPT_BYTES,
                          "TransitionReceipt digest preimage")


def validate_receipt(value: Any, allow_empty_identity: bool = False) -> dict[str, Any]:
    node = require_keys(value, "TransitionReceipt", RECEIPT_FIELDS)
    if node["api_version"] != RECEIPT_API or node["kind"] != RECEIPT_KIND:
        raise ContractError("TransitionReceipt API/kind is unsupported; aliases are rejected")
    if node["canonicalization"] != CANONICALIZATION:
        raise ContractError("TransitionReceipt canonicalization is unsupported")
    bounded_canonical_json(node, MAX_RECEIPT_BYTES, "TransitionReceipt")
    _validate_core(node, "TransitionReceipt")
    _validate_predecessor_fields(node, "TransitionReceipt")
    if allow_empty_identity and node["receipt_id"] == node["receipt_sha256"] == "":
        return node
    sha256(node["receipt_sha256"], "receipt_sha256")
    if node["receipt_id"] != f"transition-receipt-{node['receipt_sha256']}":
        raise ContractError("TransitionReceipt identity is inconsistent")
    if receipt_sha256(node, False) != node["receipt_sha256"]:
        raise ContractError("TransitionReceipt self digest does not match")
    return node


def seal_receipt(candidate: dict[str, Any]) -> dict[str, Any]:
    validate_receipt(candidate, allow_empty_identity=True)
    if candidate.get("receipt_id") != "" or candidate.get("receipt_sha256") != "":
        raise ContractError("unsealed TransitionReceipt identity fields must be empty")
    node = copy.deepcopy(candidate)
    digest = receipt_sha256(node, False)
    node["receipt_id"] = f"transition-receipt-{digest}"
    node["receipt_sha256"] = digest
    return validate_receipt(node)


def decode_receipt(raw: bytes) -> dict[str, Any]:
    return validate_receipt(decode_canonical(raw, MAX_RECEIPT_BYTES, "TransitionReceipt"))


def declared_target(receipt: dict[str, Any]) -> dict[str, Any]:
    validate_receipt(receipt)
    target = {key: copy.deepcopy(receipt[key]) for key in TARGET_FIELDS}
    return validate_target(target)


def validate_target(value: Any) -> dict[str, Any]:
    node = require_keys(value, "declared target", TARGET_FIELDS)
    bounded_canonical_json(node, MAX_TARGET_BYTES, "declared target")
    _validate_core(node, "declared target")
    _validate_predecessor_fields(node, "declared target")
    return node


def declared_target_sha256(value: dict[str, Any]) -> str:
    validate_target(value)
    return bounded_digest(TARGET_DOMAIN, value, MAX_TARGET_BYTES,
                          "Transition declared target")
