"""ApprovalRecord v1 shape, identity, target projection, and Grant reference."""

from __future__ import annotations

import copy
import re
from typing import Any

from .canonical import (ContractError, bounded_canonical_json, bounded_digest,
                        decode_canonical)
from .constants import (APPROVAL_API, APPROVAL_DOMAIN, CANONICALIZATION, DISTINCTIONS,
                        EFFECTS, EFFECT_VOCABULARY_SHA256, KIND, MAX_RECORD_BYTES,
                        MAX_TARGET_BYTES, MAX_TTL_MS, TARGET_DOMAIN)
from .shape import (array, enum, integer, principal_key, require_keys, sha256,
                    sorted_unique_strings, text, validate_artifacts, validate_base64url,
                    validate_identifier, validate_principal, validate_principal_array,
                    validate_ref_array)

RECORD_FIELDS = {
    "api_version", "approval_id", "approval_sha256", "approver", "authority_proof",
    "bindings", "canonicalization", "conditions", "decision", "decision_basis",
    "effect_vocabulary_sha256", "kind", "risk_acceptance_refs", "scope",
    "separation_of_duty", "subject", "validity",
}
TARGET_FIELDS = {
    "approver", "authority_binding", "bindings", "conditions", "decision",
    "risk_acceptance_refs", "scope", "separation_of_duty_declaration", "subject",
}
AUTHORITY_SOURCE_FIELDS = {
    "authority_class", "authority_domain", "principal_id", "principal_type",
}
AUTHORITY_BINDING_FIELDS = {
    "authority_source", "key_id", "proof_kind", "proof_profile_id",
    "proof_profile_sha256", "trust_domain", "trust_epoch",
}
SOD_DECLARATION_FIELDS = {
    "implementers", "proof_profile_id", "proof_profile_sha256", "requester",
    "required_distinctions",
}


def _validate_authority_source(value: Any, label: str) -> None:
    node = require_keys(value, label, AUTHORITY_SOURCE_FIELDS)
    authority_class = enum(node["authority_class"], f"{label}.authority_class",
                           ("external_operator", "forgeos_kernel"))
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    principal_type = enum(node["principal_type"], f"{label}.principal_type",
                          ("human", "operator", "service"))
    allowed = ("human", "operator") if authority_class == "external_operator" else ("service",)
    if principal_type not in allowed:
        raise ContractError(f"{label} principal type contradicts authority_class")


def _validate_authority_binding(value: Any, label: str) -> None:
    node = require_keys(value, label, AUTHORITY_BINDING_FIELDS)
    _validate_authority_source(node["authority_source"], f"{label}.authority_source")
    text(node["key_id"], f"{label}.key_id")
    enum(node["proof_kind"], f"{label}.proof_kind", ("attestation", "signature"))
    text(node["proof_profile_id"], f"{label}.proof_profile_id")
    sha256(node["proof_profile_sha256"], f"{label}.proof_profile_sha256")
    text(node["trust_domain"], f"{label}.trust_domain")
    integer(node["trust_epoch"], f"{label}.trust_epoch", 0, 2**63 - 1)


def _validate_authority_proof(value: Any) -> None:
    fields = set(AUTHORITY_BINDING_FIELDS) | {"proof_base64url"}
    node = require_keys(value, "authority_proof", fields)
    binding = {field: node[field] for field in AUTHORITY_BINDING_FIELDS}
    _validate_authority_binding(binding, "authority_proof")
    validate_base64url(node["proof_base64url"], "authority_proof.proof_base64url")


def _validate_scope(value: Any, label: str = "scope") -> None:
    fields = {"change_id", "effect_id", "environment_class", "environment_id", "gate_id",
              "materiality_level", "project_id", "scope_type"}
    node = require_keys(value, label, fields)
    for field in ("change_id", "environment_id", "project_id"):
        text(node[field], f"{label}.{field}")
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "local", "production", "staging", "test"))
    enum(node["materiality_level"], f"{label}.materiality_level",
         ("L0", "L1", "L2", "L3", "L4"))
    scope_type = enum(node["scope_type"], f"{label}.scope_type", ("effect", "gate"))
    if scope_type == "gate":
        text(node["gate_id"], f"{label}.gate_id")
        if node["effect_id"] is not None:
            raise ContractError(f"{label} gate scope requires effect_id=null")
        return
    enum(node["effect_id"], f"{label}.effect_id", EFFECTS)
    if node["gate_id"] is not None:
        raise ContractError(f"{label} effect scope requires gate_id=null")


def _validate_bindings(value: Any, label: str = "bindings") -> None:
    fields = {"artifacts", "context_sha256", "impact_sha256", "plan_sha256",
              "policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
    node = require_keys(value, label, fields)
    validate_artifacts(node["artifacts"], f"{label}.artifacts")
    for field in ("context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
                  "risk_sha256", "source_tree_sha256"):
        sha256(node[field], f"{label}.{field}")
    text(node["source_revision"], f"{label}.source_revision")


def _validate_decision_basis(value: Any) -> None:
    node = require_keys(value, "decision_basis",
                        {"rationale_ref", "rationale_sha256", "reason_codes"})
    text(node["rationale_ref"], "decision_basis.rationale_ref", 4096)
    sha256(node["rationale_sha256"], "decision_basis.rationale_sha256")
    reasons = array(node["reason_codes"], "decision_basis.reason_codes", 1, 16)
    for index, reason in enumerate(reasons):
        validate_identifier(reason, f"decision_basis.reason_codes[{index}]")
    if any(left.encode() >= right.encode() for left, right in zip(reasons, reasons[1:])):
        raise ContractError("decision_basis.reason_codes must be UTF-8 sorted and unique")


def _validate_sod_declaration(value: Any, label: str) -> None:
    node = require_keys(value, label, SOD_DECLARATION_FIELDS)
    validate_principal(node["requester"], f"{label}.requester")
    validate_principal_array(node["implementers"], f"{label}.implementers")
    sorted_unique_strings(node["required_distinctions"],
                          f"{label}.required_distinctions", DISTINCTIONS)
    text(node["proof_profile_id"], f"{label}.proof_profile_id")
    sha256(node["proof_profile_sha256"], f"{label}.proof_profile_sha256")


def _validate_sod(value: Any, record: dict[str, Any]) -> None:
    fields = set(SOD_DECLARATION_FIELDS) | {"proof_base64url"}
    node = require_keys(value, "separation_of_duty", fields)
    declaration = {field: node[field] for field in SOD_DECLARATION_FIELDS}
    _validate_sod_declaration(declaration, "separation_of_duty")
    validate_base64url(node["proof_base64url"], "separation_of_duty.proof_base64url")
    _validate_declared_distinctions(record)


def _validate_declared_distinctions(record: dict[str, Any]) -> None:
    _validate_sod_consistency(
        record["approver"], record["subject"], record["scope"],
        record["risk_acceptance_refs"], record["separation_of_duty"])


def _validate_sod_consistency(approver_node: dict[str, Any], subject: dict[str, Any],
                              scope: dict[str, Any], risk_refs: list[Any],
                              sod: dict[str, Any]) -> None:
    approver = principal_key(approver_node)
    distinctions = set(sod["required_distinctions"])
    if ("approver_not_requester" in distinctions and
            approver == principal_key(sod["requester"])):
        raise ContractError("approver_not_requester contradicts declared identities")
    if "approver_not_subject" in distinctions and approver == principal_key(subject):
        raise ContractError("approver_not_subject contradicts declared identities")
    if "approver_not_implementer" in distinctions:
        if any(approver == principal_key(item) for item in sod["implementers"]):
            raise ContractError("approver_not_implementer contradicts declared identities")
    if scope["materiality_level"] in ("L3", "L4"):
        if not sod["implementers"] or distinctions != set(DISTINCTIONS):
            raise ContractError("L3/L4 requires implementers and all SoD distinctions")
    if risk_refs and "approver_not_requester" not in distinctions:
        raise ContractError("RiskAcceptance refs require approver_not_requester")


def _validate_validity(value: Any) -> None:
    fields = {"expires_at_unix_ms", "issued_at_unix_ms", "not_before_unix_ms",
              "revoked_at_unix_ms", "transferable"}
    node = require_keys(value, "validity", fields)
    issued = integer(node["issued_at_unix_ms"], "validity.issued_at_unix_ms", 0, 2**63 - 1)
    starts = integer(node["not_before_unix_ms"], "validity.not_before_unix_ms", 0, 2**63 - 1)
    expires = integer(node["expires_at_unix_ms"], "validity.expires_at_unix_ms", 0, 2**63 - 1)
    if not issued <= starts < expires or expires - issued > MAX_TTL_MS:
        raise ContractError("validity must be ordered within the 24-hour maximum window")
    revoked = node["revoked_at_unix_ms"]
    if revoked is not None:
        integer(revoked, "validity.revoked_at_unix_ms", issued, expires - 1)
    if node["transferable"] is not False:
        raise ContractError("ApprovalRecord must be non-transferable")


def _validate_production_declaration(record: dict[str, Any]) -> None:
    _validate_production_fields(
        record["decision"], record["scope"],
        record["authority_proof"]["authority_source"])


def _validate_production_fields(decision: str, scope: dict[str, Any],
                                source: dict[str, Any]) -> None:
    restricted = (scope["scope_type"] == "effect" and
                  scope["effect_id"] in ("migration.apply", "release.execute") and
                  scope["environment_class"] == "production" and
                  decision == "approve")
    if restricted and source["authority_class"] != "external_operator":
        raise ContractError("production apply/execute approval requires external_operator")


def _validate_identity(record: dict[str, Any], allow_empty: bool) -> None:
    identifier, claimed = record["approval_id"], record["approval_sha256"]
    if allow_empty and identifier == claimed == "":
        return
    sha256(claimed, "approval_sha256")
    if identifier != f"approval-record-{claimed}":
        raise ContractError("ApprovalRecord identity does not match its digest")
    if approval_sha256(record, validate=False) != claimed:
        raise ContractError("ApprovalRecord self digest does not match")


def validate_record(value: Any, allow_empty_identity: bool = False) -> dict[str, Any]:
    record = require_keys(value, "ApprovalRecord", RECORD_FIELDS)
    if record["api_version"] != APPROVAL_API or record["kind"] != KIND:
        raise ContractError("ApprovalRecord API/kind is unsupported; aliases are rejected")
    if record["canonicalization"] != CANONICALIZATION:
        raise ContractError("ApprovalRecord canonicalization is unsupported")
    if record["effect_vocabulary_sha256"] != EFFECT_VOCABULARY_SHA256:
        raise ContractError("ApprovalRecord does not bind the frozen effect vocabulary")
    bounded_canonical_json(record, MAX_RECORD_BYTES, "ApprovalRecord")
    validate_principal(record["approver"], "approver", ("human", "operator"))
    validate_principal(record["subject"], "subject")
    _validate_authority_proof(record["authority_proof"])
    _validate_scope(record["scope"])
    _validate_bindings(record["bindings"])
    enum(record["decision"], "decision", ("abstain", "approve", "reject"))
    _validate_decision_basis(record["decision_basis"])
    validate_ref_array(record["conditions"], "conditions", "condition")
    validate_ref_array(record["risk_acceptance_refs"], "risk_acceptance_refs", "risk")
    _validate_sod(record["separation_of_duty"], record)
    _validate_validity(record["validity"])
    _validate_production_declaration(record)
    _validate_identity(record, allow_empty_identity)
    return record


def approval_sha256(value: dict[str, Any], validate: bool = True) -> str:
    if validate:
        validate_record(value, allow_empty_identity=True)
    bounded_canonical_json(value, MAX_RECORD_BYTES, "ApprovalRecord")
    payload = copy.deepcopy(value)
    payload["approval_id"] = ""
    payload["approval_sha256"] = ""
    payload["authority_proof"]["proof_base64url"] = ""
    payload["separation_of_duty"]["proof_base64url"] = ""
    return bounded_digest(APPROVAL_DOMAIN, payload, MAX_RECORD_BYTES,
                          "ApprovalRecord digest preimage")


def seal_record(candidate: dict[str, Any]) -> dict[str, Any]:
    record = copy.deepcopy(candidate)
    if record.get("approval_id") != "" or record.get("approval_sha256") != "":
        raise ContractError("unsealed ApprovalRecord identity fields must be empty")
    validate_record(record, allow_empty_identity=True)
    claimed = approval_sha256(record, validate=False)
    record["approval_id"] = f"approval-record-{claimed}"
    record["approval_sha256"] = claimed
    validate_record(record)
    return record


def decode_record(raw: bytes) -> dict[str, Any]:
    return validate_record(decode_canonical(raw, MAX_RECORD_BYTES, "ApprovalRecord"))


def declared_target(record: dict[str, Any]) -> dict[str, Any]:
    validate_record(record)
    authority = {field: copy.deepcopy(record["authority_proof"][field])
                 for field in AUTHORITY_BINDING_FIELDS}
    sod = {field: copy.deepcopy(record["separation_of_duty"][field])
           for field in SOD_DECLARATION_FIELDS}
    target = {
        "approver": copy.deepcopy(record["approver"]),
        "authority_binding": authority,
        "bindings": copy.deepcopy(record["bindings"]),
        "conditions": copy.deepcopy(record["conditions"]),
        "decision": record["decision"],
        "risk_acceptance_refs": copy.deepcopy(record["risk_acceptance_refs"]),
        "scope": copy.deepcopy(record["scope"]),
        "separation_of_duty_declaration": sod,
        "subject": copy.deepcopy(record["subject"]),
    }
    validate_target(target)
    return target


def validate_target(value: Any) -> dict[str, Any]:
    target = require_keys(value, "declared target", TARGET_FIELDS)
    bounded_canonical_json(target, MAX_TARGET_BYTES, "declared target")
    validate_principal(target["approver"], "declared target.approver", ("human", "operator"))
    validate_principal(target["subject"], "declared target.subject")
    _validate_authority_binding(target["authority_binding"], "declared target.authority_binding")
    _validate_bindings(target["bindings"], "declared target.bindings")
    validate_ref_array(target["conditions"], "declared target.conditions", "condition")
    enum(target["decision"], "declared target.decision", ("abstain", "approve", "reject"))
    validate_ref_array(target["risk_acceptance_refs"],
                       "declared target.risk_acceptance_refs", "risk")
    _validate_scope(target["scope"], "declared target.scope")
    _validate_sod_declaration(target["separation_of_duty_declaration"],
                              "declared target.separation_of_duty_declaration")
    _validate_sod_consistency(
        target["approver"], target["subject"], target["scope"],
        target["risk_acceptance_refs"], target["separation_of_duty_declaration"])
    _validate_production_fields(
        target["decision"], target["scope"],
        target["authority_binding"]["authority_source"])
    return target


def declared_target_sha256(value: dict[str, Any]) -> str:
    validate_target(value)
    return bounded_digest(TARGET_DOMAIN, value, MAX_TARGET_BYTES, "declared target")


def approval_ref(record: dict[str, Any]) -> dict[str, str]:
    validate_record(record)
    return {
        "approval_id": record["approval_id"],
        "approval_sha256": record["approval_sha256"],
        "authority_domain": record["authority_proof"]["authority_source"]["authority_domain"],
    }


def validate_approval_ref(value: Any) -> dict[str, Any]:
    node = require_keys(value, "CapabilityGrant ApprovalRef",
                        {"approval_id", "approval_sha256", "authority_domain"})
    sha256(node["approval_sha256"], "ApprovalRef.approval_sha256")
    if node["approval_id"] != f"approval-record-{node['approval_sha256']}":
        raise ContractError("CapabilityGrant ApprovalRef identity is inconsistent")
    text(node["authority_domain"], "ApprovalRef.authority_domain")
    return node


def approval_ref_relation(record: dict[str, Any], value: Any) -> str:
    validate_record(record)
    candidate = validate_approval_ref(value)
    return ("same_declared_reference" if candidate == approval_ref(record)
            else "reference_mismatch")
