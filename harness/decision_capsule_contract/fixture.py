"""Deterministic ADR-0092 full structural replay fixture."""

from __future__ import annotations

import hashlib
from pathlib import Path

from kernel_decision_contract import golden_closure as golden_decision_closure

from .capsule import derive_decision_capsule
from .closure import derive_structural_replay_closure
from .codec import ContractError, canonical_json, read_bounded_file
from .constants import FIXTURE_PATH, MAX_CLOSURE_BYTES

GOLDEN_SHA256 = "d54494f49851cc4146905bbd64c0815fe7d79704476c0aeb1113f270d5cbb2d0"


def fixture_reflection_report() -> dict[str, object]:
    return {
        "artifact_kind": "reflection_report",
        "artifact_ref": "fixture/reflection/report-v1",
        "artifact_sha256": "9" * 64,
    }


def golden_structural_replay_closure() -> dict[str, object]:
    capsule = derive_decision_capsule(golden_decision_closure())
    return derive_structural_replay_closure(capsule, [fixture_reflection_report()])


def golden_bytes() -> bytes:
    return canonical_json(golden_structural_replay_closure()) + b"\n"


def load_golden(repo_root: Path) -> dict[str, object]:
    path = repo_root / FIXTURE_PATH
    raw = read_bounded_file(path, "decision capsule golden", MAX_CLOSURE_BYTES)
    if hashlib.sha256(raw).hexdigest() != GOLDEN_SHA256:
        raise ContractError("decision capsule golden physical SHA-256 mismatch")
    if raw != golden_bytes():
        raise ContractError("decision capsule golden differs from deterministic reconstruction")
    from .closure import decode_structural_replay_closure
    return decode_structural_replay_closure(raw[:-1])
