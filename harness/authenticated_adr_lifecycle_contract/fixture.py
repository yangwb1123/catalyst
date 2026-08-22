"""Deterministic proof-shaped lifecycle fixture with a two-head atomic join."""

from __future__ import annotations

import base64
import copy
import hashlib
from pathlib import Path
from typing import Any

from architecture_decision_record_v2 import validate_document_bytes
from authenticated_adr_approval_contract.documents import receipt_sha256 as approval_receipt_sha
from authenticated_adr_approval_contract.fixture import golden_fixture as approval_golden

from .authority import trust_root_sha256
from .canonical import (ContractError, bounded_canonical_json, physical_pin,
                        read_bounded_file, sha256_bytes)
from .constants import (
    ACCEPTANCE_API, ADR_V2_SCHEMA_PATH, ADR_V2_SCHEMA_SHA256,
    APPROVAL_SCHEMA_PATH, APPROVAL_SCHEMA_SHA256, BUNDLE_API, CANONICALIZATION,
    ENTRY_API, FIXTURE_PATH, GOLDEN_SHA256, LEDGER_API, MAX_GOLDEN_BYTES,
    MAX_PROPOSAL_BYTES, PROFILE_ID,
    PROPOSAL_BODY_SHA256, PROPOSAL_FIXTURE_PATHS, PROPOSAL_PHYSICAL_SHA256,
    PROPOSAL_SELF_SHA256, REQUEST_API, RESULT_API, SCHEMA_PATH, SCHEMA_SHA256,
    SIGNATURE_PROFILE_ID, STATE_API, SUPERSESSION_API, TRUST_ROOT_API,
)
from .contract import decode_document, validate_document
from .documents import (acceptance_sha256, entry_sha256, record_key_sha256,
                        request_sha256, supersession_sha256)
from .ledger import (current_head_set_sha256, ledger_sha256,
                     rebuild_materialized_view, validate_ledger)
from .prerequisite import PREREQUISITE_API, prerequisite_sha256
from .proposal import derive_proposal_binding, encode_proposal_document
from .state import state_sha256

OBSERVED = (1_786_755_600_000, 1_786_755_601_000, 1_786_755_602_000)
APPROVAL_SEQUENCES = (2, 1, 7)


def _hash(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def _b64(byte: int, size: int) -> str:
    return base64.urlsafe_b64encode(bytes([byte]) * size).decode("ascii").rstrip("=")


def _principal(identifier: str, principal_type: str) -> dict[str, str]:
    return {"authority_domain": "forgeos.fixture.authenticated-adr-lifecycle",
            "principal_id": identifier, "principal_type": principal_type}


def _signature(key_id: str, profile_hash: str, byte: int) -> dict[str, str]:
    return {"key_id": key_id, "profile_id": SIGNATURE_PROFILE_ID,
            "profile_sha256": profile_hash, "signature_base64url": _b64(byte, 64)}


def _lifecycle_root(profile_hash: str) -> dict[str, Any]:
    keys = [
        {"key_id": "fixture-lifecycle-request-key-1",
         "principal": _principal("operator-lifecycle-requester", "operator"),
         "public_key_base64url": _b64(21, 32),
         "usage": "architecture_decision_lifecycle_request_auth"},
        {"key_id": "fixture-lifecycle-state-key-1",
         "principal": _principal("service-lifecycle-state", "service"),
         "public_key_base64url": _b64(22, 32),
         "usage": "architecture_decision_lifecycle_state_sign"},
    ]
    keys.sort(key=lambda item: bounded_canonical_json(item, 64 * 1024, "root key"))
    root = {
        "api_version": TRUST_ROOT_API, "canonicalization": CANONICALIZATION,
        "keys": keys, "kind": "ArchitectureDecisionLifecycleTrustRoot",
        "profile_id": PROFILE_ID, "root_sha256": "",
        "signature_profile_sha256": profile_hash,
        "trust_domain": "forgeos.fixture.authenticated-adr-lifecycle",
        "trust_epoch": 1,
    }
    root["root_sha256"] = trust_root_sha256(root)
    return root


def _proposal_inputs(repo_root: Path) -> list[tuple[bytes, dict[str, Any], dict[str, Any]]]:
    values = []
    for index, path in enumerate(PROPOSAL_FIXTURE_PATHS):
        raw = physical_pin(repo_root / path, PROPOSAL_PHYSICAL_SHA256[index],
                           MAX_PROPOSAL_BYTES, f"lifecycle proposal {path.name}")
        metadata = validate_document_bytes(raw, path.name)
        if (metadata["body_sha256"] != PROPOSAL_BODY_SHA256[index] or
                metadata["self_sha256"] != PROPOSAL_SELF_SHA256[index]):
            raise ContractError(f"lifecycle proposal digest pins differ: {path.name}")
        values.append((raw, metadata, derive_proposal_binding(raw, path.name)))
    return values


def _approval_receipt(template: dict[str, Any], binding: dict[str, Any], index: int,
                      observed: int, profile_hash: str) -> dict[str, Any]:
    receipt = copy.deepcopy(template)
    sequence = APPROVAL_SEQUENCES[index]
    receipt.update(
        authorization_decision="acceptance_transition_authorized",
        authorization_expires_at_unix_ms=observed + 300_000,
        evaluated_at_unix_ms=observed - 100,
        ledger_sequence=sequence, policy_sha256=_hash(f"policy-{index}"),
        prior_receipt_sha256=None if sequence == 1 else _hash(f"prior-{index}"),
        proposal_binding_sha256=binding["proposal_binding_sha256"], reason_codes=[],
        receipt_id="", receipt_sha256="", record_key_sha256=_hash(f"approval-key-{index}"),
        request_sha256=_hash(f"approval-request-{index}"),
        signature=_signature("fixture-state-key-1", profile_hash, 40 + index),
    )
    receipt["receipt_sha256"] = approval_receipt_sha(receipt)
    receipt["receipt_id"] = f"architecture-decision-approval-receipt-{receipt['receipt_sha256']}"
    return receipt


def _prerequisite(binding: dict[str, Any], receipt: dict[str, Any], index: int,
                  observed: int, approval_root: dict[str, Any],
                  profile_hash: str) -> dict[str, Any]:
    raw_receipt = bounded_canonical_json(receipt, 256 * 1024,
                                         "fixture authorization receipt")
    value = {
        "api_version": PREREQUISITE_API,
        "approval_trust_epoch": approval_root["trust_epoch"],
        "approval_trust_root_sha256": approval_root["root_sha256"],
        "authorization_ledger_clock_high_water_unix_ms": observed,
        "authorization_ledger_last_sequence": 10,
        "authorization_ledger_sha256": _hash(f"approval-ledger-{index}"),
        "authorization_ledger_signature":
            _signature("fixture-state-key-1", profile_hash, 50 + index),
        "authorization_receipt": receipt,
        "authorization_receipt_physical_sha256": sha256_bytes(raw_receipt),
        "canonicalization": CANONICALIZATION,
        "kind": "ArchitectureDecisionAcceptancePrerequisite",
        "observed_at_unix_ms": observed, "prerequisite_sha256": "",
        "profile_id": PROFILE_ID, "proposal_binding": binding,
        "revocation_high_water_sequence": receipt["revocation_sequence"],
        "revocation_high_water_sha256": receipt["revocation_sha256"],
    }
    value["prerequisite_sha256"] = prerequisite_sha256(value)
    return value


def _target_refs(target_ids: list[str], rows: dict[str, dict[str, Any]],
                 ) -> list[dict[str, str]]:
    return [
        {"acceptance_id": rows[item]["acceptance_id"],
         "acceptance_sha256": rows[item]["acceptance_sha256"], "adr_id": item,
         "proposal_binding_sha256": rows[item]["proposal_binding_sha256"]}
        for item in target_ids
    ]


def _request(raw: bytes, prerequisite: dict[str, Any], targets: list[dict[str, str]],
             sequence: int, expected_ledger: str | None, expected_head: str,
             root: dict[str, Any], profile_hash: str, observed: int) -> dict[str, Any]:
    value = {
        "acceptance_prerequisite": prerequisite, "api_version": REQUEST_API,
        "canonicalization": CANONICALIZATION,
        "expected_current_head_set_sha256": expected_head,
        "expected_ledger_sha256": expected_ledger, "expected_next_sequence": sequence,
        "expires_at_unix_ms": observed + 300_000,
        "idempotency_key": f"fixture-lifecycle-request-{sequence:04d}",
        "kind": "ArchitectureDecisionLifecycleTransitionRequest",
        "profile_id": PROFILE_ID, "proposal_document_base64url":
            encode_proposal_document(raw), "request_id": "", "request_sha256": "",
        "requested_at_unix_ms": observed,
        "signature": _signature("fixture-lifecycle-request-key-1", profile_hash,
                                60 + sequence),
        "supersession_targets": targets, "trust_epoch": root["trust_epoch"],
        "trust_root_sha256": root["root_sha256"],
    }
    value["request_sha256"] = request_sha256(value)
    value["request_id"] = f"architecture-decision-lifecycle-request-{value['request_sha256']}"
    return value


def _acceptance(request: dict[str, Any], sequence: int, root: dict[str, Any],
                profile_hash: str, observed: int) -> dict[str, Any]:
    prerequisite = request["acceptance_prerequisite"]
    binding = prerequisite["proposal_binding"]
    approval = prerequisite["authorization_receipt"]
    value = {
        "acceptance_id": "", "acceptance_sha256": "",
        "accepted_at_unix_ms": observed, "adr_id": binding["adr_id"],
        "api_version": ACCEPTANCE_API,
        "authorization_receipt_physical_sha256":
            prerequisite["authorization_receipt_physical_sha256"],
        "authorization_receipt_sha256": approval["receipt_sha256"],
        "canonicalization": CANONICALIZATION,
        "kind": "ArchitectureDecisionLifecycleAcceptanceReceipt",
        "ledger_sequence": sequence, "profile_id": PROFILE_ID,
        "proposal_binding_sha256": binding["proposal_binding_sha256"],
        "record_key_sha256": record_key_sha256(request["idempotency_key"]),
        "request_sha256": request["request_sha256"],
        "signature": _signature("fixture-lifecycle-state-key-1", profile_hash,
                                70 + sequence),
        "supersedes": [item["adr_id"] for item in request["supersession_targets"]],
        "trust_epoch": root["trust_epoch"], "trust_root_sha256": root["root_sha256"],
    }
    value["acceptance_sha256"] = acceptance_sha256(value)
    value["acceptance_id"] = f"architecture-decision-acceptance-{value['acceptance_sha256']}"
    return value


def _supersession(target: dict[str, str], acceptance: dict[str, Any],
                  request: dict[str, Any], sequence: int, root: dict[str, Any],
                  profile_hash: str, index: int) -> dict[str, Any]:
    value = {
        "api_version": SUPERSESSION_API, "canonicalization": CANONICALIZATION,
        "kind": "ArchitectureDecisionLifecycleSupersessionReceipt",
        "ledger_sequence": sequence, "profile_id": PROFILE_ID,
        "receipt_id": "", "receipt_sha256": "",
        "request_sha256": request["request_sha256"],
        "signature": _signature("fixture-lifecycle-state-key-1", profile_hash,
                                80 + index),
        "superseded_at_unix_ms": acceptance["accepted_at_unix_ms"],
        "superseded_by_acceptance_id": acceptance["acceptance_id"],
        "superseded_by_adr_id": acceptance["adr_id"],
        "superseded_by_proposal_binding_sha256":
            acceptance["proposal_binding_sha256"],
        "target_acceptance_id": target["acceptance_id"],
        "target_adr_id": target["adr_id"],
        "target_proposal_binding_sha256": target["proposal_binding_sha256"],
        "trust_epoch": root["trust_epoch"], "trust_root_sha256": root["root_sha256"],
    }
    value["receipt_sha256"] = supersession_sha256(value)
    value["receipt_id"] = f"architecture-decision-supersession-{value['receipt_sha256']}"
    return value


def _minimal_head_state(rows: dict[str, dict[str, Any]],
                        heads: set[str]) -> dict[str, dict[str, Any]]:
    return {item: {"adr_id": item, "proposal_binding_sha256":
                   rows[item]["proposal_binding_sha256"],
                   "status": "accepted" if item in heads else "superseded"}
            for item in rows}


def _sealed_ledger(entries: list[dict[str, Any]], head: str) -> dict[str, Any]:
    value = {
        "api_version": LEDGER_API, "canonicalization": CANONICALIZATION,
        "current_head_set_sha256": head, "entries": copy.deepcopy(entries),
        "kind": "ArchitectureDecisionLifecycleLedger",
        "last_sequence": len(entries), "ledger_sha256": "", "profile_id": PROFILE_ID,
    }
    value["ledger_sha256"] = ledger_sha256(value)
    return value


def _entries(inputs: list[tuple[bytes, dict[str, Any], dict[str, Any]]],
             profile_hash: str, approval_root: dict[str, Any],
             lifecycle_root: dict[str, Any],
             approval_receipt_template: dict[str, Any]) -> list[dict[str, Any]]:
    entries, rows, heads = [], {}, set()
    prior_ledger = None
    for index, (raw, metadata, binding) in enumerate(inputs):
        sequence, observed = index + 1, OBSERVED[index]
        targets = _target_refs(list(metadata["supersedes"]), rows)
        prerequisite = _prerequisite(
            binding, _approval_receipt(approval_receipt_template, binding, index,
                                       observed, profile_hash), index, observed,
            approval_root, profile_hash)
        expected_head = current_head_set_sha256(_minimal_head_state(rows, heads))
        request = _request(raw, prerequisite, targets, sequence,
                           None if prior_ledger is None else prior_ledger["ledger_sha256"],
                           expected_head, lifecycle_root, profile_hash, observed)
        acceptance = _acceptance(request, sequence, lifecycle_root,
                                 profile_hash, observed)
        receipts = [_supersession(target, acceptance, request, sequence,
                                  lifecycle_root, profile_hash, 10 * sequence + offset)
                    for offset, target in enumerate(targets)]
        rows[binding["adr_id"]] = acceptance
        heads.difference_update(item["adr_id"] for item in targets)
        heads.add(binding["adr_id"])
        resulting = current_head_set_sha256(_minimal_head_state(rows, heads))
        entry = {"acceptance_receipt": acceptance, "api_version": ENTRY_API,
                 "canonicalization": CANONICALIZATION, "entry_sha256": "",
                 "kind": "ArchitectureDecisionLifecycleLedgerEntry",
                 "prior_entry_sha256": None if not entries else entries[-1]["entry_sha256"],
                 "profile_id": PROFILE_ID, "request": request,
                 "resulting_current_head_set_sha256": resulting,
                 "sequence": sequence, "supersession_receipts": receipts}
        entry["entry_sha256"] = entry_sha256(entry)
        entries.append(entry)
        prior_ledger = _sealed_ledger(entries, resulting)
    return entries


def fixture_candidate(repo_root: Path) -> dict[str, Any]:
    approval = approval_golden(repo_root)
    profile = copy.deepcopy(approval["signature_profile"])
    approval_root = copy.deepcopy(approval["trust_root"])
    approval_receipt_template = copy.deepcopy(approval["authorization_receipt"])
    lifecycle_root = _lifecycle_root(profile["profile_sha256"])
    entries = _entries(_proposal_inputs(repo_root), profile["profile_sha256"],
                       approval_root, lifecycle_root, approval_receipt_template)
    ledger = _sealed_ledger(entries, entries[-1]["resulting_current_head_set_sha256"])
    ledger, rebuilt = validate_ledger(ledger, profile["profile_sha256"],
                                      lifecycle_root, approval_root)
    view = rebuild_materialized_view(ledger, rebuilt)
    state = {"api_version": STATE_API, "canonicalization": CANONICALIZATION,
             "kind": "ArchitectureDecisionLifecycleState", "ledger": ledger,
             "materialized_view": view, "profile_id": PROFILE_ID,
             "signature": _signature("fixture-lifecycle-state-key-1",
                                     profile["profile_sha256"], 99),
             "state_sha256": "", "trust_epoch": lifecycle_root["trust_epoch"],
             "trust_root_sha256": lifecycle_root["root_sha256"]}
    state["state_sha256"] = state_sha256(state)
    result = {"api_version": RESULT_API, "canonicalization": CANONICALIZATION,
              "delivery_disposition": "stored", "entry_sha256":
                  entries[-1]["entry_sha256"],
              "kind": "ArchitectureDecisionLifecycleTransitionResult",
              "ledger_sha256": ledger["ledger_sha256"],
              "materialized_view_sha256": view["view_sha256"],
              "receipt": copy.deepcopy(entries[-1]["acceptance_receipt"]),
              "state_sha256": state["state_sha256"]}
    return {"api_version": BUNDLE_API, "approval_trust_root": approval_root,
            "canonicalization": CANONICALIZATION,
            "kind": "AuthenticatedArchitectureDecisionLifecycleBundle",
            "lifecycle_result": result, "lifecycle_state": state,
            "lifecycle_trust_root": lifecycle_root, "profile_id": PROFILE_ID,
            "signature_profile": profile}


def golden_fixture(repo_root: Path) -> dict[str, Any]:
    return validate_document(fixture_candidate(repo_root))


def golden_bytes(repo_root: Path) -> bytes:
    return bounded_canonical_json(golden_fixture(repo_root), MAX_GOLDEN_BYTES,
                                  "lifecycle golden") + b"\n"


def load_golden(repo_root: Path) -> dict[str, Any]:
    physical_pin(repo_root / APPROVAL_SCHEMA_PATH, APPROVAL_SCHEMA_SHA256,
                 2 * 1024 * 1024, "ADR approval JSON Schema dependency")
    physical_pin(repo_root / ADR_V2_SCHEMA_PATH, ADR_V2_SCHEMA_SHA256,
                 2 * 1024 * 1024, "ADR v2 JSON Schema dependency")
    physical_pin(repo_root / SCHEMA_PATH, SCHEMA_SHA256, 2 * 1024 * 1024,
                 "ADR lifecycle JSON Schema")
    raw = physical_pin(repo_root / FIXTURE_PATH, GOLDEN_SHA256,
                       MAX_GOLDEN_BYTES + 1, "ADR lifecycle golden")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("ADR lifecycle golden must end in exactly one LF")
    document = decode_document(raw[:-1])
    if document != golden_fixture(repo_root) or raw != golden_bytes(repo_root):
        raise ContractError("ADR lifecycle golden differs from deterministic fixture")
    return document


__all__ = ["fixture_candidate", "golden_bytes", "golden_fixture", "load_golden"]
