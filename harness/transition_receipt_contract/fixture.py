"""Deterministic ADR-0060 TransitionReceipt golden fixture construction."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import approval_record_contract
import capability_grant_contract

from .assessment import evaluate_declared_assessment, seal_request
from .canonical import canonical_json
from .compatibility import project_approval_refs, project_capability_grant_ref
from .constants import CANONICALIZATION, RECEIPT_API, RECEIPT_KIND
from .receipt import declared_target, seal_receipt
from .vocabulary import transition_vocabulary


def _bindings(grant: dict[str, Any]) -> dict[str, Any]:
    value = {field: grant["bindings"][field] for field in (
        "context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
        "risk_sha256", "source_revision", "source_tree_sha256")}
    value["artifacts"] = [{
        "artifact_kind": "transition-input",
        "artifact_ref": "governance/transition-fixture-v1",
        "artifact_sha256": "8" * 64,
    }]
    return value


def _candidate(grant: dict[str, Any], approval_refs: list[dict[str, str]]) -> dict[str, Any]:
    return {
        "api_version": RECEIPT_API,
        "actor": grant["subject"],
        "applicability": {
            "decision": "applicable", "evidence_refs": [], "reason_codes": [],
            "stage_id": "NEEDS_EVIDENCE",
        },
        "approval_refs": approval_refs,
        "bindings": _bindings(grant),
        "canonicalization": CANONICALIZATION,
        "capability_grant_ref": project_capability_grant_ref(grant),
        "declared_controller": {
            "authority_domain": "forgeos.kernel.fixture",
            "principal_id": "fixture-controller",
            "principal_type": "service",
        },
        "kind": RECEIPT_KIND,
        "preconditions": [{
            "evidence_refs": [{"canonical_sha256": "9" * 64,
                               "record_id": "evidence-fixture-transition-intake"}],
            "precondition_id": "intake_evidence_declared",
            "reason_codes": ["fixture_evidence_reference_present"],
            "result": "PASS",
        }],
        "previous_receipt_id": None,
        "previous_receipt_sha256": None,
        "reason_codes": ["declared_initial_intake"],
        "receipt_id": "",
        "receipt_sha256": "",
        "sequence": 1,
        "task_binding": grant["task_binding"],
        "transition": {
            "declared_at_unix_ms": 1_700_000_001_000,
            "from_state": "DRAFT",
            "gate_id": None,
            "resume_state": None,
            "rework_target": None,
            "to_state": "NEEDS_EVIDENCE",
        },
        "transition_vocabulary_sha256": transition_vocabulary()["vocabulary_sha256"],
        "waiver_refs": [],
        "work_id": "work-transition-fixture-0060",
    }


def golden_fixture(repo_root: Path) -> dict[str, Any]:
    grant = capability_grant_contract.load_golden(repo_root)["grant"]
    approval = approval_record_contract.load_golden(repo_root)["approval_record"]
    approval_refs = project_approval_refs([approval])
    receipt = seal_receipt(_candidate(grant, approval_refs))
    target = declared_target(receipt)
    request = seal_request(receipt, target, None, 1_700_000_001_000)
    return {
        "assessment_request": request,
        "expected_approval_refs": approval_refs,
        "expected_assessment": evaluate_declared_assessment(request),
        "expected_capability_grant_ref": project_capability_grant_ref(grant),
        "transition_receipt": receipt,
        "transition_vocabulary": transition_vocabulary(),
    }


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    print(canonical_json(golden_fixture(root)).decode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

