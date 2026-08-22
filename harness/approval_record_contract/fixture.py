"""Deterministic ADR-0059 golden fixture construction."""

from __future__ import annotations

from .assessment import evaluate_declared_assessment, seal_request
from .canonical import canonical_json
from .constants import (APPROVAL_API, CANONICALIZATION, EFFECT_VOCABULARY_SHA256,
                        KIND)
from .record import approval_ref, declared_target, seal_record


def _principal(identifier: str, principal_type: str) -> dict[str, object]:
    return {
        "authority_domain": "forgeos.fixture",
        "principal_id": identifier,
        "principal_type": principal_type,
    }


def _authority_proof() -> dict[str, object]:
    return {
        "authority_source": {
            "authority_class": "external_operator",
            "authority_domain": "forgeos.fixture",
            "principal_id": "operator-release-1",
            "principal_type": "operator",
        },
        "key_id": "fixture-approval-key-1",
        "proof_base64url": "Zml4dHVyZS1hcHByb3ZhbC1hdHRlc3RhdGlvbi0x",
        "proof_kind": "attestation",
        "proof_profile_id": "forgeos.fixture-attestation/v1",
        "proof_profile_sha256": "1" * 64,
        "trust_domain": "forgeos.fixture",
        "trust_epoch": 1,
    }


def _bindings() -> dict[str, object]:
    return {
        "artifacts": [{
            "artifact_kind": "release-package",
            "artifact_ref": "release/package-v1",
            "artifact_sha256": "2" * 64,
        }],
        "context_sha256": "3" * 64,
        "impact_sha256": "4" * 64,
        "plan_sha256": "5" * 64,
        "policy_sha256": "6" * 64,
        "risk_sha256": "7" * 64,
        "source_revision": "fixture-revision-0059",
        "source_tree_sha256": "8" * 64,
    }


def _separation_of_duty() -> dict[str, object]:
    return {
        "implementers": [_principal("implementer-release-1", "agent")],
        "proof_base64url": "Zml4dHVyZS1zZXBhcmF0aW9uLXByb29mLTE",
        "proof_profile_id": "forgeos.fixture-sod/v1",
        "proof_profile_sha256": "c" * 64,
        "requester": _principal("requester-release-1", "agent"),
        "required_distinctions": [
            "approver_not_implementer",
            "approver_not_requester",
            "approver_not_subject",
        ],
    }


def _candidate() -> dict[str, object]:
    return {
        "api_version": APPROVAL_API,
        "approval_id": "",
        "approval_sha256": "",
        "approver": _principal("operator-release-1", "operator"),
        "authority_proof": _authority_proof(),
        "bindings": _bindings(),
        "canonicalization": CANONICALIZATION,
        "conditions": [{
            "condition_id": "canary-observation",
            "condition_ref": "release/conditions/canary-observation",
            "condition_sha256": "9" * 64,
        }],
        "decision": "approve",
        "decision_basis": {
            "rationale_ref": "release/approval-rationale-v1",
            "rationale_sha256": "a" * 64,
            "reason_codes": ["current_evidence_reviewed", "risk_controls_declared"],
        },
        "effect_vocabulary_sha256": EFFECT_VOCABULARY_SHA256,
        "kind": KIND,
        "risk_acceptance_refs": [{
            "authority_domain": "forgeos.fixture.risk",
            "risk_acceptance_id": "risk-acceptance-fixture-1",
            "risk_acceptance_sha256": "b" * 64,
        }],
        "scope": {
            "change_id": "change-0059-fixture",
            "effect_id": "release.execute",
            "environment_class": "production",
            "environment_id": "fixture-production",
            "gate_id": None,
            "materiality_level": "L3",
            "project_id": "forgeos",
            "scope_type": "effect",
        },
        "separation_of_duty": _separation_of_duty(),
        "subject": _principal("release-executor-1", "service"),
        "validity": {
            "expires_at_unix_ms": 1_700_003_600_000,
            "issued_at_unix_ms": 1_700_000_000_000,
            "not_before_unix_ms": 1_700_000_000_000,
            "revoked_at_unix_ms": None,
            "transferable": False,
        },
    }


def golden_fixture() -> dict[str, object]:
    record = seal_record(_candidate())
    target = declared_target(record)
    request = seal_request(record, target, 1_700_000_001_000)
    return {
        "approval_record": record,
        "assessment_request": request,
        "expected_approval_ref": approval_ref(record),
        "expected_assessment": evaluate_declared_assessment(request),
    }


def main() -> int:
    print(canonical_json(golden_fixture()).decode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
