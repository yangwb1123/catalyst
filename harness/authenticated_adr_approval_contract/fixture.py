"""Deterministic proof-shaped, explicitly non-authenticated golden bundle."""

from __future__ import annotations

import base64
import copy
import hashlib
from pathlib import Path
from typing import Any

from approval_record_contract import seal_record
from approval_record_contract.constants import EFFECT_VOCABULARY_SHA256
from architecture_decision_record_v2 import validate_document_bytes

from .approvals import expected_artifacts
from .authority import signature_profile_sha256, trust_root_sha256
from .canonical import ContractError, canonical_json, read_bounded_file
from .constants import (
    ADR_V2_SCHEMA_PATH,
    ADR_V2_SCHEMA_SHA256,
    APPROVAL_RECORD_SCHEMA_PATH,
    APPROVAL_RECORD_SCHEMA_SHA256,
    CANONICALIZATION,
    FIXTURE_PATH,
    GOLDEN_SHA256,
    GATE_ID,
    LEDGER_API,
    MAX_GOLDEN_BYTES,
    MAX_PROPOSAL_BYTES,
    POLICY_API,
    PROFILE_ID,
    PROPOSAL_BODY_SHA256,
    PROPOSAL_FIXTURE_PATH,
    PROPOSAL_PHYSICAL_SHA256,
    PROPOSAL_SELF_SHA256,
    RECEIPT_API,
    REQUEST_API,
    RESULT_API,
    REVOCATION_API,
    SIGNATURE_PROFILE_API,
    SIGNATURE_PROFILE_ID,
    SCHEMA_PATH,
    SCHEMA_SHA256,
    TRUST_ROOT_API,
)
from .contract import decode_document, validate_document
from .documents import record_key_sha256, receipt_sha256, request_sha256
from .ledger import ledger_sha256
from .policy import policy_sha256
from .proposal import derive_proposal_binding, encode_proposal_document
from .revocation import revocation_sha256

START = 1_786_744_800_000
REQUESTED = START + 3_600_000
REQUEST_EXPIRES = REQUESTED + 300_000
EVALUATED = REQUESTED + 1_000
END = START + 86_400_000


def _b64(byte: int, size: int) -> str:
    return base64.urlsafe_b64encode(bytes([byte]) * size).decode("ascii").rstrip("=")


def _principal(identifier: str, principal_type: str) -> dict[str, str]:
    return {"authority_domain": "forgeos.fixture.authenticated-adr-approval",
            "principal_id": identifier, "principal_type": principal_type}


def _signature(key_id: str, profile_hash: str, byte: int) -> dict[str, str]:
    return {"key_id": key_id, "profile_id": SIGNATURE_PROFILE_ID,
            "profile_sha256": profile_hash, "signature_base64url": _b64(byte, 64)}


def _signature_profile() -> dict[str, Any]:
    profile = {
        "algorithm": "Ed25519", "api_version": SIGNATURE_PROFILE_API,
        "canonicalization": CANONICALIZATION, "digest_algorithm": "SHA-256",
        "kind": "SignatureProfile",
        "message_preimage": "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
        "profile_id": SIGNATURE_PROFILE_ID, "profile_sha256": "",
        "public_key_encoding": "base64url_unpadded_32_bytes",
        "signature_encoding": "base64url_unpadded_64_bytes",
    }
    profile["profile_sha256"] = signature_profile_sha256(profile)
    return profile


def _root_key(key_id: str, usage: str, principal: dict[str, str],
              byte: int) -> dict[str, Any]:
    return {"key_id": key_id, "principal": principal,
            "public_key_base64url": _b64(byte, 32), "usage": usage}


def _trust_root(profile_hash: str) -> dict[str, Any]:
    keys = [
        _root_key("fixture-approver-key-1", "architecture_approval_sign",
                  _principal("operator-architecture-1", "operator"), 1),
        _root_key("fixture-approver-key-2", "architecture_approval_sign",
                  _principal("operator-architecture-2", "operator"), 2),
        _root_key("fixture-policy-key-1", "approval_policy_sign",
                  _principal("service-approval-policy", "service"), 3),
        _root_key("fixture-request-key-1", "approval_request_auth",
                  _principal("operator-architecture-requester", "operator"), 4),
        _root_key("fixture-revocation-key-1", "approval_revocation_sign",
                  _principal("service-approval-revocation", "service"), 5),
        _root_key("fixture-state-key-1", "approval_authorization_state_sign",
                  _principal("service-approval-state", "service"), 6),
    ]
    root = {
        "api_version": TRUST_ROOT_API, "canonicalization": CANONICALIZATION,
        "keys": sorted(keys, key=canonical_json),
        "kind": "ArchitectureDecisionApprovalTrustRoot", "profile_id": PROFILE_ID,
        "root_sha256": "", "signature_profile_sha256": profile_hash,
        "trust_domain": "forgeos.fixture.authenticated-adr-approval", "trust_epoch": 1,
    }
    root["root_sha256"] = trust_root_sha256(root)
    return root


def _approval_profile() -> dict[str, Any]:
    return {
        "change_id": "adr-9002-approval", "context_sha256": "1" * 64,
        "environment_class": "test", "environment_id": "fixture-test",
        "gate_id": GATE_ID, "impact_sha256": "2" * 64,
        "materiality_level": "L4", "plan_sha256": "3" * 64,
        "project_id": "forgeos-fixture", "risk_sha256": "4" * 64,
        "source_revision": "fixture-adr-9002-proposal",
        "source_tree_sha256": "5" * 64,
        "subject": _principal("service-architecture-lifecycle", "service"),
    }


def _roles() -> dict[str, Any]:
    return {
        "approver_bindings": [
            {"approver_ref": "architecture-review", "key_id": "fixture-approver-key-1"},
            {"approver_ref": "governance-review", "key_id": "fixture-approver-key-2"},
        ],
        "implementers": [_principal("developer-architecture-1", "human")],
        "owner_bindings": [
            {"owner_ref": "governance",
             "principal": _principal("owner-governance", "human")},
            {"owner_ref": "runtime-engineering",
             "principal": _principal("owner-runtime-engineering", "human")},
        ],
        "requester": _principal("operator-architecture-requester", "operator"),
    }


def _policy(binding: dict[str, Any], profile_hash: str,
            root: dict[str, Any]) -> dict[str, Any]:
    policy = {
        "api_version": POLICY_API, "approval_record_profile": _approval_profile(),
        "canonicalization": CANONICALIZATION, "disposition": "allow",
        "eligible_approver_key_ids": ["fixture-approver-key-1", "fixture-approver-key-2"],
        "kind": "ArchitectureDecisionApprovalPolicy", "max_request_validity_ms": 300_000,
        "policy_id": "fixture-adr-9002-approval-policy", "policy_sha256": "",
        "profile_id": PROFILE_ID, "proposal_binding": binding,
        "required_distinctions": ["approver_not_implementer", "approver_not_owner",
                                  "approver_not_requester", "approver_not_subject",
                                  "approvers_pairwise_distinct"],
        "roles": _roles(), "signature": _signature("fixture-policy-key-1", profile_hash, 11),
        "threshold": 2, "trust_epoch": 1, "trust_root_sha256": root["root_sha256"],
        "validity": {"expires_at_unix_ms": END, "not_before_unix_ms": START},
        "veto_on_reject": True,
    }
    policy["policy_sha256"] = policy_sha256(policy)
    return policy


def _revocation(profile_hash: str, root: dict[str, Any]) -> dict[str, Any]:
    snapshot = {
        "api_version": REVOCATION_API, "canonicalization": CANONICALIZATION,
        "effective_at_unix_ms": START, "expires_at_unix_ms": END,
        "kind": "ArchitectureDecisionApprovalRevocationSnapshot",
        "prior_revocation_sha256": None, "profile_id": PROFILE_ID,
        "revocation_sequence": 1, "revocation_sha256": "",
        "revoked_approval_ids": [], "revoked_key_ids": [],
        "signature": _signature("fixture-revocation-key-1", profile_hash, 12),
        "trust_epoch": 1, "trust_root_sha256": root["root_sha256"],
    }
    snapshot["revocation_sha256"] = revocation_sha256(snapshot)
    return snapshot


def _approval_record(index: int, policy: dict[str, Any],
                     profile_hash: str) -> dict[str, Any]:
    approver = _principal(f"operator-architecture-{index}", "operator")
    key_id = f"fixture-approver-key-{index}"
    candidate = {
        "api_version": "forgeos.approval-record/v1", "approval_id": "",
        "approval_sha256": "", "approver": approver,
        "authority_proof": {
            "authority_source": {"authority_class": "external_operator", **approver},
            "key_id": key_id, "proof_base64url": _b64(20 + index, 64),
            "proof_kind": "signature", "proof_profile_id": SIGNATURE_PROFILE_ID,
            "proof_profile_sha256": profile_hash,
            "trust_domain": "forgeos.fixture.authenticated-adr-approval", "trust_epoch": 1,
        },
        "bindings": _approval_bindings(policy), "canonicalization": CANONICALIZATION,
        "conditions": [], "decision": "approve",
        "decision_basis": {
            "rationale_ref": f"fixture/adr-9002/rationale/operator-{index}",
            "rationale_sha256": str(index + 5) * 64,
            "reason_codes": ["architecture_decision_reviewed"],
        },
        "effect_vocabulary_sha256": EFFECT_VOCABULARY_SHA256, "kind": "ApprovalRecord",
        "risk_acceptance_refs": [], "scope": _approval_scope(policy),
        "separation_of_duty": {
            "implementers": copy.deepcopy(policy["roles"]["implementers"]),
            "proof_base64url": _b64(30 + index, 64),
            "proof_profile_id": SIGNATURE_PROFILE_ID,
            "proof_profile_sha256": profile_hash,
            "requester": copy.deepcopy(policy["roles"]["requester"]),
            "required_distinctions": ["approver_not_implementer",
                                      "approver_not_requester", "approver_not_subject"],
        },
        "subject": copy.deepcopy(policy["approval_record_profile"]["subject"]),
        "validity": {"expires_at_unix_ms": END, "issued_at_unix_ms": START,
                     "not_before_unix_ms": START, "revoked_at_unix_ms": None,
                     "transferable": False},
    }
    return seal_record(candidate)


def _approval_bindings(policy: dict[str, Any]) -> dict[str, Any]:
    profile = policy["approval_record_profile"]
    return {
        "artifacts": expected_artifacts(policy["proposal_binding"]),
        "context_sha256": profile["context_sha256"],
        "impact_sha256": profile["impact_sha256"], "plan_sha256": profile["plan_sha256"],
        "policy_sha256": policy["policy_sha256"], "risk_sha256": profile["risk_sha256"],
        "source_revision": profile["source_revision"],
        "source_tree_sha256": profile["source_tree_sha256"],
    }


def _approval_scope(policy: dict[str, Any]) -> dict[str, Any]:
    profile = policy["approval_record_profile"]
    return {
        "change_id": profile["change_id"], "effect_id": None,
        "environment_class": profile["environment_class"],
        "environment_id": profile["environment_id"], "gate_id": GATE_ID,
        "materiality_level": "L4", "project_id": profile["project_id"],
        "scope_type": "gate",
    }


def _request(records: list[dict[str, Any]], policy: dict[str, Any],
             snapshot: dict[str, Any], profile_hash: str,
             root: dict[str, Any]) -> dict[str, Any]:
    request = {
        "api_version": REQUEST_API, "approval_records": records,
        "canonicalization": CANONICALIZATION, "expected_ledger_sha256": None,
        "expected_next_sequence": 1, "expires_at_unix_ms": REQUEST_EXPIRES,
        "idempotency_key": "fixture-adr-9002-request-0001",
        "kind": "ArchitectureDecisionApprovalAuthorizationRequest",
        "policy_sha256": policy["policy_sha256"], "profile_id": PROFILE_ID,
        "proposal_binding": policy["proposal_binding"], "request_id": "",
        "request_sha256": "", "requested_at_unix_ms": REQUESTED,
        "requester": copy.deepcopy(policy["roles"]["requester"]),
        "revocation_sequence": 1, "revocation_sha256": snapshot["revocation_sha256"],
        "signature": _signature("fixture-request-key-1", profile_hash, 13),
        "trust_epoch": 1, "trust_root_sha256": root["root_sha256"],
    }
    digest = request_sha256(request)
    request["request_id"] = f"architecture-decision-approval-request-{digest}"
    request["request_sha256"] = digest
    return request


def _receipt(request: dict[str, Any], policy: dict[str, Any],
             snapshot: dict[str, Any], profile_hash: str,
             root: dict[str, Any]) -> dict[str, Any]:
    approvals = sorted(record["approval_id"] for record in request["approval_records"])
    receipt = {
        "api_version": RECEIPT_API,
        "authorization_decision": "acceptance_transition_authorized",
        "authorization_expires_at_unix_ms": REQUEST_EXPIRES,
        "canonicalization": CANONICALIZATION, "evaluated_at_unix_ms": EVALUATED,
        "kind": "ArchitectureDecisionApprovalAuthorizationReceipt", "ledger_sequence": 1,
        "policy_sha256": policy["policy_sha256"], "prior_receipt_sha256": None,
        "profile_id": PROFILE_ID,
        "proposal_binding_sha256": policy["proposal_binding"]["proposal_binding_sha256"],
        "qualifying_approval_ids": approvals, "reason_codes": [], "receipt_id": "",
        "receipt_sha256": "", "record_key_sha256": record_key_sha256(
            request["idempotency_key"]), "request_sha256": request["request_sha256"],
        "revocation_sequence": 1, "revocation_sha256": snapshot["revocation_sha256"],
        "signature": _signature("fixture-state-key-1", profile_hash, 14),
        "trust_epoch": 1, "trust_root_sha256": root["root_sha256"],
    }
    digest = receipt_sha256(receipt)
    receipt["receipt_id"] = f"architecture-decision-approval-receipt-{digest}"
    receipt["receipt_sha256"] = digest
    return receipt


def _ledger(policy: dict[str, Any], encoded_proposal: str, request: dict[str, Any],
            receipt: dict[str, Any], snapshot: dict[str, Any], profile_hash: str,
            root: dict[str, Any]) -> dict[str, Any]:
    ledger = {
        "api_version": LEDGER_API, "canonicalization": CANONICALIZATION,
        "clock_high_water_unix_ms": EVALUATED,
        "entries": [{"policy": policy, "proposal_document_base64url": encoded_proposal,
                     "receipt": receipt, "request": request, "sequence": 1}],
        "kind": "ArchitectureDecisionApprovalAuthorizationLedger", "ledger_sha256": "",
        "profile_id": PROFILE_ID, "revocation_high_water_sequence": 1,
        "revocation_high_water_sha256": snapshot["revocation_sha256"],
        "revocation_snapshots": [snapshot],
        "signature": _signature("fixture-state-key-1", profile_hash, 15),
        "trust_epoch": 1, "trust_root_sha256": root["root_sha256"],
    }
    ledger["ledger_sha256"] = ledger_sha256(ledger)
    return ledger


def fixture_candidate(proposal_bytes: bytes) -> dict[str, Any]:
    profile = _signature_profile()
    root = _trust_root(profile["profile_sha256"])
    binding = derive_proposal_binding(proposal_bytes,
                                      PROPOSAL_FIXTURE_PATH.name)
    policy = _policy(binding, profile["profile_sha256"], root)
    snapshot = _revocation(profile["profile_sha256"], root)
    records = sorted((_approval_record(1, policy, profile["profile_sha256"]),
                      _approval_record(2, policy, profile["profile_sha256"])),
                     key=lambda record: record["approval_id"])
    request = _request(records, policy, snapshot, profile["profile_sha256"], root)
    receipt = _receipt(request, policy, snapshot, profile["profile_sha256"], root)
    encoded = encode_proposal_document(proposal_bytes)
    result = {"api_version": RESULT_API, "canonicalization": CANONICALIZATION,
              "delivery_disposition": "stored",
              "kind": "ArchitectureDecisionApprovalAuthorizationResult",
              "receipt": copy.deepcopy(receipt)}
    return {
        "authorization_ledger": _ledger(policy, encoded, request, receipt, snapshot,
                                         profile["profile_sha256"], root),
        "authorization_policy": policy, "authorization_receipt": receipt,
        "authorization_request": request, "authorization_result": result,
        "proposal_binding": binding, "proposal_document_base64url": encoded,
        "revocation_snapshot": snapshot, "signature_profile": profile, "trust_root": root,
    }


def _bind_entry_to_snapshot_for_tests(entry: dict[str, Any],
                                      snapshot: dict[str, Any]) -> None:
    request = entry["request"]
    request.update(revocation_sequence=snapshot["revocation_sequence"],
                   revocation_sha256=snapshot["revocation_sha256"],
                   request_id="", request_sha256="")
    request["request_sha256"] = request_sha256(request)
    request["request_id"] = f"architecture-decision-approval-request-{request['request_sha256']}"
    receipt = entry["receipt"]
    receipt.update(request_sha256=request["request_sha256"],
                   record_key_sha256=record_key_sha256(request["idempotency_key"]),
                   revocation_sequence=snapshot["revocation_sequence"],
                   revocation_sha256=snapshot["revocation_sha256"],
                   receipt_id="", receipt_sha256="")
    receipt["receipt_sha256"] = receipt_sha256(receipt)
    receipt["receipt_id"] = f"architecture-decision-approval-receipt-{receipt['receipt_sha256']}"


def _reseal_two_entry_ledger_for_tests(node: dict[str, Any]) -> dict[str, Any]:
    ledger = node["authorization_ledger"]
    latest = ledger["revocation_snapshots"][-1]
    latest["revocation_sha256"] = revocation_sha256(latest)
    _bind_entry_to_snapshot_for_tests(ledger["entries"][-1], latest)
    ledger.update(revocation_high_water_sequence=latest["revocation_sequence"],
                  revocation_high_water_sha256=latest["revocation_sha256"])
    ledger["ledger_sha256"] = ledger_sha256(ledger)
    return ledger


def _two_entry_candidate_for_tests(candidate: dict[str, Any]) -> dict[str, Any]:
    """Expand a copied genesis fixture to exercise non-genesis ledger relations."""
    node = copy.deepcopy(candidate)
    ledger = node["authorization_ledger"]
    first_snapshot = ledger["revocation_snapshots"][0]
    snapshot = copy.deepcopy(first_snapshot)
    snapshot.update(revocation_sequence=2,
                    prior_revocation_sha256=first_snapshot["revocation_sha256"])
    snapshot["revocation_sha256"] = revocation_sha256(snapshot)
    request = copy.deepcopy(node["authorization_request"])
    request.update(approval_records=[], expected_ledger_sha256=ledger["ledger_sha256"],
                   expected_next_sequence=2,
                   idempotency_key="fixture-adr-9002-request-0002",
                   requested_at_unix_ms=node["authorization_receipt"][
                       "evaluated_at_unix_ms"] + 1,
                   expires_at_unix_ms=node["authorization_receipt"][
                       "evaluated_at_unix_ms"] + 300_001,
                   revocation_sequence=2, revocation_sha256=snapshot["revocation_sha256"],
                   request_id="", request_sha256="")
    request["request_sha256"] = request_sha256(request)
    request["request_id"] = f"architecture-decision-approval-request-{request['request_sha256']}"
    receipt = copy.deepcopy(node["authorization_receipt"])
    receipt.update(authorization_decision="acceptance_transition_not_authorized",
                   authorization_expires_at_unix_ms=request["expires_at_unix_ms"],
                   evaluated_at_unix_ms=request["requested_at_unix_ms"] + 1,
                   ledger_sequence=2,
                   prior_receipt_sha256=node["authorization_receipt"]["receipt_sha256"],
                   qualifying_approval_ids=[],
                   reason_codes=["insufficient_authenticated_approvals"],
                   record_key_sha256=record_key_sha256(request["idempotency_key"]),
                   request_sha256=request["request_sha256"], revocation_sequence=2,
                   revocation_sha256=snapshot["revocation_sha256"],
                   receipt_id="", receipt_sha256="")
    receipt["receipt_sha256"] = receipt_sha256(receipt)
    receipt["receipt_id"] = f"architecture-decision-approval-receipt-{receipt['receipt_sha256']}"
    entry = {"policy": copy.deepcopy(node["authorization_policy"]),
             "proposal_document_base64url": node["proposal_document_base64url"],
             "receipt": copy.deepcopy(receipt), "request": copy.deepcopy(request),
             "sequence": 2}
    ledger.update(clock_high_water_unix_ms=receipt["evaluated_at_unix_ms"],
                  revocation_high_water_sequence=2,
                  revocation_high_water_sha256=snapshot["revocation_sha256"])
    ledger["entries"].append(entry)
    ledger["revocation_snapshots"].append(copy.deepcopy(snapshot))
    ledger["ledger_sha256"] = ledger_sha256(ledger)
    node.update(authorization_receipt=receipt, authorization_request=request,
                revocation_snapshot=snapshot)
    node["authorization_result"]["receipt"] = copy.deepcopy(receipt)
    return node


def _proposal_bytes(repo_root: Path) -> bytes:
    raw = read_bounded_file(repo_root / PROPOSAL_FIXTURE_PATH, MAX_PROPOSAL_BYTES,
                            "ADR approval proposal fixture")
    if hashlib.sha256(raw).hexdigest() != PROPOSAL_PHYSICAL_SHA256:
        raise ContractError("proposal fixture physical SHA-256 mismatch")
    metadata = validate_document_bytes(raw, PROPOSAL_FIXTURE_PATH.name)
    if (metadata["body_sha256"] != PROPOSAL_BODY_SHA256 or
            metadata["self_sha256"] != PROPOSAL_SELF_SHA256):
        raise ContractError("proposal fixture body or self SHA-256 mismatch")
    return raw


def golden_fixture(repo_root: Path) -> dict[str, Any]:
    return validate_document(fixture_candidate(_proposal_bytes(repo_root)))


def golden_bytes(repo_root: Path) -> bytes:
    return canonical_json(golden_fixture(repo_root)) + b"\n"


def load_golden(repo_root: Path) -> dict[str, Any]:
    dependencies = ((APPROVAL_RECORD_SCHEMA_PATH, APPROVAL_RECORD_SCHEMA_SHA256),
                    (ADR_V2_SCHEMA_PATH, ADR_V2_SCHEMA_SHA256))
    for path, expected in dependencies:
        raw_dependency = read_bounded_file(repo_root / path, 1024 * 1024, str(path))
        if hashlib.sha256(raw_dependency).hexdigest() != expected:
            raise ContractError(f"referenced Schema physical SHA-256 mismatch: {path}")
    schema = read_bounded_file(repo_root / SCHEMA_PATH, 1024 * 1024,
                               "ADR approval JSON Schema")
    if hashlib.sha256(schema).hexdigest() != SCHEMA_SHA256:
        raise ContractError("ADR approval JSON Schema physical SHA-256 mismatch")
    raw = read_bounded_file(repo_root / FIXTURE_PATH, MAX_GOLDEN_BYTES + 1,
                            "ADR approval golden fixture")
    if hashlib.sha256(raw).hexdigest() != GOLDEN_SHA256:
        raise ContractError("ADR approval golden physical SHA-256 mismatch")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("ADR approval golden must end in exactly one LF")
    document = decode_document(raw[:-1])
    if document != golden_fixture(repo_root) or raw != golden_bytes(repo_root):
        raise ContractError("ADR approval golden differs from deterministic fixture")
    return document
