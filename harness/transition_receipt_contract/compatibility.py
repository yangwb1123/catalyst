"""Pure ADR-0056 Grant and ADR-0059 ApprovalRecord compatibility helpers."""

from __future__ import annotations

from typing import Any

import approval_record_contract
import capability_grant_contract

from .canonical import ContractError
from .constants import APPROVAL_RESULT, GRANT_RESULT
from .receipt import validate_receipt
from .shape import sorted_nodes


def project_capability_grant_ref(grant: dict[str, Any]) -> dict[str, str]:
    capability_grant_contract.grant.validate_grant(grant)
    issuer = grant["authority_proof"]["issuer"]
    return {
        "authority_domain": issuer["authority_domain"],
        "grant_id": grant["grant_id"],
        "grant_sha256": grant["grant_sha256"],
    }


def _same(left: Any, right: Any, field: str) -> str:
    return f"same_declared_{field}" if left == right else f"{field}_mismatch"


def _grant_bindings(grant: dict[str, Any]) -> dict[str, Any]:
    fields = ("context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
              "risk_sha256", "source_revision", "source_tree_sha256")
    return {field: grant["bindings"][field] for field in fields}


def _receipt_bindings(receipt: dict[str, Any]) -> dict[str, Any]:
    fields = ("context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
              "risk_sha256", "source_revision", "source_tree_sha256")
    return {field: receipt["bindings"][field] for field in fields}


def _compatibility(relations: dict[str, str], result: str) -> dict[str, Any]:
    reasons = sorted(value for value in relations.values() if value.endswith("_mismatch"))
    return {"reason_codes": reasons, "relations": relations, "result": result}


def assess_declared_grant_compatibility(grant: dict[str, Any],
                                        receipt: dict[str, Any]) -> dict[str, Any]:
    capability_grant_contract.grant.validate_grant(grant)
    validate_receipt(receipt)
    declared_at = receipt["transition"]["declared_at_unix_ms"]
    validity = grant["validity"]
    time_matches = validity["not_before_unix_ms"] <= declared_at < validity["expires_at_unix_ms"]
    relations = {
        "actor": _same(receipt["actor"], grant["subject"], "actor"),
        "approval_refs": _same(receipt["approval_refs"], grant["approval_refs"],
                               "approval_refs"),
        "bindings": _same(_receipt_bindings(receipt), _grant_bindings(grant), "bindings"),
        "declared_time": ("same_declared_time" if time_matches
                          else "declared_time_mismatch"),
        "grant_ref": _same(receipt["capability_grant_ref"],
                           project_capability_grant_ref(grant), "grant_ref"),
        "task_binding": _same(receipt["task_binding"], grant["task_binding"],
                              "task_binding"),
    }
    return _compatibility(relations, GRANT_RESULT)


def project_approval_refs(records: list[dict[str, Any]]) -> list[dict[str, str]]:
    if not isinstance(records, list) or len(records) > 32:
        raise ContractError("ApprovalRecord projection count must be 0..32")
    projected = [approval_record_contract.approval_ref(record) for record in records]
    sorted_nodes(projected, "projected ApprovalRefs")
    return projected


def _approval_scope_matches(record: dict[str, Any], receipt: dict[str, Any]) -> bool:
    scope, task = record["scope"], receipt["task_binding"]
    return (scope["project_id"] == task["project_id"] and
            scope["change_id"] == task["change_id"] and
            scope["environment_class"] == task["environment_class"] and
            scope["environment_id"] == task["environment_id"] and
            scope["gate_id"] == receipt["transition"]["gate_id"])


def assess_declared_approval_compatibility(
        records: list[dict[str, Any]], receipt: dict[str, Any]) -> dict[str, Any]:
    validate_receipt(receipt)
    projected = project_approval_refs(records)
    relations = {
        "ref_set": _same(receipt["approval_refs"], projected, "ref_set"),
        "scope": ("same_declared_scope" if all(
            _approval_scope_matches(record, receipt) for record in records)
            else "scope_mismatch"),
    }
    return _compatibility(relations, APPROVAL_RESULT)
