"""ApprovalRecord v1 profile constraints for exact proposal authorization."""

from __future__ import annotations

from typing import Any

from approval_record_contract import ContractError as ApprovalRecordError
from approval_record_contract.record import validate_record

from .authority import key_by_id
from .canonical import ContractError, canonical_json
from .constants import (APPROVAL_RECORD_DISTINCTIONS, GATE_ID, MAX_APPROVALS,
                        SIGNATURE_PROFILE_ID)
from .shape import fixed_base64url, principal_identity, sorted_unique_nodes


def expected_artifacts(binding: dict[str, Any]) -> list[dict[str, str]]:
    artifacts = [
        {"artifact_kind": "architecture-decision-proposal-body-v2",
         "artifact_ref": f"{binding['document_name']}#body",
         "artifact_sha256": binding["body_sha256"]},
        {"artifact_kind": "architecture-decision-proposal-physical-v2",
         "artifact_ref": binding["document_name"],
         "artifact_sha256": binding["physical_sha256"]},
        {"artifact_kind": "architecture-decision-proposal-self-v2",
         "artifact_ref": binding["adr_id"],
         "artifact_sha256": binding["self_sha256"]},
    ]
    return sorted(artifacts, key=canonical_json)


def validate_approval_records(value: Any, policy: dict[str, Any], root: dict[str, Any],
                              snapshot: dict[str, Any], evaluated_at: int) -> list[dict[str, Any]]:
    records = sorted_unique_nodes(value, "request.approval_records", 0, MAX_APPROVALS)
    for index, record in enumerate(records):
        _validate_record(record, f"request.approval_records[{index}]", policy, root,
                         snapshot, evaluated_at)
    identities = [principal_identity(record["approver"]) for record in records]
    if len(set(identities)) != len(records):
        raise ContractError("approval records must use pairwise-distinct approvers")
    if [record["approval_id"] for record in records] != sorted(
            record["approval_id"] for record in records):
        raise ContractError("approval records must be sorted by approval_id")
    return records


def _validate_record(record: Any, label: str, policy: dict[str, Any],
                     root: dict[str, Any], snapshot: dict[str, Any],
                     evaluated_at: int) -> None:
    try:
        node = validate_record(record)
    except ApprovalRecordError as error:
        raise ContractError(f"{label} is not an exact ApprovalRecord v1: {error}") from error
    _validate_authority(node, policy, root)
    _validate_scope_and_bindings(node, policy)
    _validate_sod(node, policy)
    _validate_decision(node)
    _validate_time_and_revocation(node, policy, snapshot, evaluated_at)


def _validate_authority(record: dict[str, Any], policy: dict[str, Any],
                        root: dict[str, Any]) -> None:
    proof = record["authority_proof"]
    if proof["proof_kind"] != "signature" or proof["proof_profile_id"] != SIGNATURE_PROFILE_ID:
        raise ContractError("ApprovalRecord authority proof must use the signature profile")
    fixed_base64url(proof["proof_base64url"],
                    "ApprovalRecord authority_proof.proof_base64url", 64)
    if proof["proof_profile_sha256"] != root["signature_profile_sha256"]:
        raise ContractError("ApprovalRecord authority proof profile digest differs")
    if (proof["trust_domain"] != root["trust_domain"] or
            proof["trust_epoch"] != root["trust_epoch"]):
        raise ContractError("ApprovalRecord authority proof does not bind the trust root")
    key = key_by_id(root, proof["key_id"])
    if (proof["key_id"] not in policy["eligible_approver_key_ids"] or
            key["usage"] != "architecture_approval_sign"):
        raise ContractError("ApprovalRecord signer is not an eligible approval key")
    source = proof["authority_source"]
    if source["authority_class"] != "external_operator":
        raise ContractError("ApprovalRecord authority must be external_operator")
    source_principal = {field: source[field] for field in
                        ("authority_domain", "principal_id", "principal_type")}
    if source_principal != record["approver"] or key["principal"] != record["approver"]:
        raise ContractError("ApprovalRecord approver, authority source, and root key differ")


def _validate_scope_and_bindings(record: dict[str, Any], policy: dict[str, Any]) -> None:
    profile = policy["approval_record_profile"]
    expected_scope = {
        "change_id": profile["change_id"], "effect_id": None,
        "environment_class": profile["environment_class"],
        "environment_id": profile["environment_id"], "gate_id": GATE_ID,
        "materiality_level": "L4", "project_id": profile["project_id"],
        "scope_type": "gate",
    }
    if record["scope"] != expected_scope:
        raise ContractError("ApprovalRecord scope differs from the fixed policy gate")
    expected_bindings = {
        "artifacts": expected_artifacts(policy["proposal_binding"]),
        "context_sha256": profile["context_sha256"],
        "impact_sha256": profile["impact_sha256"],
        "plan_sha256": profile["plan_sha256"],
        "policy_sha256": policy["policy_sha256"],
        "risk_sha256": profile["risk_sha256"],
        "source_revision": profile["source_revision"],
        "source_tree_sha256": profile["source_tree_sha256"],
    }
    if record["bindings"] != expected_bindings:
        raise ContractError("ApprovalRecord bindings differ from exact policy/proposal values")
    if record["subject"] != profile["subject"]:
        raise ContractError("ApprovalRecord subject differs from policy profile")
    if record["conditions"] or record["risk_acceptance_refs"]:
        raise ContractError("ApprovalRecord conditions and RiskAcceptance refs must be empty")


def _validate_sod(record: dict[str, Any], policy: dict[str, Any]) -> None:
    sod = record["separation_of_duty"]
    fixed_base64url(sod["proof_base64url"],
                    "ApprovalRecord separation_of_duty.proof_base64url", 64)
    if (sod["requester"] != policy["roles"]["requester"] or
            sod["implementers"] != policy["roles"]["implementers"] or
            tuple(sod["required_distinctions"]) != APPROVAL_RECORD_DISTINCTIONS):
        raise ContractError("ApprovalRecord SoD declarations differ from signed policy roles")
    if (sod["proof_profile_id"] != SIGNATURE_PROFILE_ID or
            sod["proof_profile_sha256"] != policy["signature"]["profile_sha256"]):
        raise ContractError("ApprovalRecord SoD proof profile differs")
    approver = principal_identity(record["approver"])
    owners = {principal_identity(item["principal"])
              for item in policy["roles"]["owner_bindings"]}
    if approver in owners:
        raise ContractError("ApprovalRecord approver equals a signed proposal owner")


def _validate_decision(record: dict[str, Any]) -> None:
    expected = {
        "abstain": ["architecture_decision_abstained"],
        "approve": ["architecture_decision_reviewed"],
        "reject": ["architecture_decision_rejected"],
    }
    if record["decision_basis"]["reason_codes"] != expected[record["decision"]]:
        raise ContractError("ApprovalRecord decision reason code differs from v1 profile")


def _validate_time_and_revocation(record: dict[str, Any], policy: dict[str, Any],
                                  snapshot: dict[str, Any], evaluated_at: int) -> None:
    validity = record["validity"]
    policy_validity = policy["validity"]
    if (validity["issued_at_unix_ms"] < policy_validity["not_before_unix_ms"] or
            validity["expires_at_unix_ms"] > policy_validity["expires_at_unix_ms"] or
            not validity["not_before_unix_ms"] <= evaluated_at <
            validity["expires_at_unix_ms"]):
        raise ContractError("ApprovalRecord is outside the declared policy/evaluation window")
    if validity["revoked_at_unix_ms"] is not None:
        raise ContractError("ApprovalRecord embedded revoked_at_unix_ms must be null")
    proof_key = record["authority_proof"]["key_id"]
    if (record["approval_id"] in snapshot["revoked_approval_ids"] or
            proof_key in snapshot["revoked_key_ids"]):
        raise ContractError("ApprovalRecord or its key appears in the supplied revocation snapshot")


def declared_outcome(policy: dict[str, Any], records: list[dict[str, Any]]) -> tuple[str, list[str]]:
    approvals = sorted(record["approval_id"] for record in records
                       if record["decision"] == "approve")
    if policy["disposition"] == "deny":
        return "acceptance_transition_not_authorized", []
    if any(record["decision"] == "reject" for record in records):
        return "acceptance_transition_not_authorized", approvals
    if len(approvals) < policy["threshold"]:
        return "acceptance_transition_not_authorized", approvals
    return "acceptance_transition_authorized", approvals


def declared_reason_codes(policy: dict[str, Any], records: list[dict[str, Any]]) -> list[str]:
    if policy["disposition"] == "deny":
        return ["policy_denied"]
    if any(record["decision"] == "reject" for record in records):
        return ["authenticated_reject"]
    approvals = sum(record["decision"] == "approve" for record in records)
    return [] if approvals >= policy["threshold"] else ["insufficient_authenticated_approvals"]
