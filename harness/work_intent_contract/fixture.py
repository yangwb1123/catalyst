"""Deterministic WorkIntent v1 candidate golden fixture."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .codec import ContractError, canonical_json, read_bounded_file
from .constants import (API_VERSION, ATTESTATION_FIELDS, CANONICALIZATION,
                        FIXTURE_PATH, FRESHNESS, KIND, MAX_RECORD_BYTES, STATUS)
from .record import decode_work_intent, seal_work_intent

GOLDEN_SHA256 = "8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b"


def _principal(identifier: str, principal_type: str) -> dict[str, object]:
    return {"authority_domain": "forgeos.fixture", "principal_id": identifier,
            "principal_type": principal_type}


def fixture_candidate() -> dict[str, object]:
    """Return the one deterministic blank-identity declaration."""
    return {
        "api_version": API_VERSION,
        "attestations": {field: False for field in ATTESTATION_FIELDS},
        "binding": {"change_id": "change-work-intent-1", "project_id": "forgeos",
                    "run_id": None},
        "canonicalization": CANONICALIZATION,
        "declared_at_unix_ms": 1_700_000_000_000,
        "declared_owner": _principal("owner-work-intent-1", "human"),
        "freshness": FRESHNESS,
        "intent": {
            "deadline_unix_ms": 1_699_999_999_000,
            "external_constraints": ["Preserve the public compatibility surface."],
            "goal": "Publish an authority-neutral WorkIntent v1 candidate contract.",
            "non_goals": ["Do not authorize execution or effects."],
            "open_questions": ["Which authenticated route may later consume it?"],
            "scope": ["The lexical WorkIntent declaration and self-digest."],
            "success_signals": ["Python rejects every non-canonical instance."],
            "work_type": "architecture_evolution",
        },
        "kind": KIND,
        "materiality": {"basis": "caller_declaration_only", "level": "L2"},
        "origin": {"origin_kind": "user_request", "origin_ref": "request/work-intent-v1"},
        "references": {
            "claim_record_refs": [{"canonical_sha256": "1" * 64,
                                   "record_id": "claim-work-intent-contract"}],
            "evidence_record_refs": [{"canonical_sha256": "2" * 64,
                                      "record_id": "evidence-work-intent-review"}],
            "local_artifact_declarations": [{
                "artifact_kind": "json-schema",
                "artifact_ref": "docs/contracts/work-intent-v1.schema.json",
                "artifact_sha256": "3" * 64,
            }],
            "local_source_snapshot_declaration": {
                "snapshot_id": "repository:work-intent-v1-candidate",
                "snapshot_sha256": "4" * 64,
                "snapshot_type": "repository",
            },
        },
        "requester": _principal("requester-work-intent-1", "agent"),
        "status": STATUS,
        "work_intent_id": "",
        "work_intent_sha256": "",
    }


def golden_fixture() -> dict[str, object]:
    """Return the deterministic sealed record."""
    return seal_work_intent(fixture_candidate())


def golden_bytes() -> bytes:
    """Return exact physical fixture bytes: canonical instance plus one LF."""
    return canonical_json(golden_fixture()) + b"\n"


def load_golden(repo_root: Path) -> dict[str, object]:
    """Verify physical pin, one-LF framing, exact bytes, and semantic identity."""
    raw = read_bounded_file(repo_root / FIXTURE_PATH, "WorkIntent golden",
                            MAX_RECORD_BYTES + 1)
    if hashlib.sha256(raw).hexdigest() != GOLDEN_SHA256:
        raise ContractError("WorkIntent golden physical SHA-256 mismatch")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("WorkIntent golden must end in exactly one LF")
    record = decode_work_intent(raw[:-1])
    if record != golden_fixture() or raw != golden_bytes():
        raise ContractError("WorkIntent golden differs from the deterministic fixture")
    return record
