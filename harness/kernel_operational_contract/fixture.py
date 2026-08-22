"""Deterministic full-DAG golden for the Kernel operational core v1."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .closure import seal_closure
from .codec import ContractError, canonical_json, read_bounded_file
from .constants import *  # noqa: F403 - fixture deliberately uses the frozen vocabulary
from .records import (decode_artifact_receipt, decode_capability_invocation,
                      decode_execution_receipt, decode_interaction_event,
                      seal_artifact_receipt, seal_capability_invocation,
                      seal_execution_receipt, seal_interaction_event)

GOLDEN_SHA256 = "85f8d9887331fe95e52533c228e40b41750f04dfe10f3a7c77e5a4daff785f2f"


def _attestations() -> dict[str, bool]:
    return {field: False for field in ATTESTATION_FIELDS}  # noqa: F405


def _principal(identifier: str, principal_type: str) -> dict[str, object]:
    return {"authority_domain": "forgeos.fixture", "principal_id": identifier,
            "principal_type": principal_type}


def _bindings() -> dict[str, object]:
    return {
        "context_sha256": "1" * 64,
        "environment_profile_id": "fixture-environment-v1",
        "environment_sha256": "2" * 64,
        "policy_sha256": "3" * 64,
        "source_profile_id": "fixture-source-v1",
        "source_revision": "fixture-revision-0088",
        "source_tree_sha256": "4" * 64,
    }


def _task_binding() -> dict[str, object]:
    return {
        "attempt_id": None,
        "change_id": "change-kernel-operational-v1",
        "environment_class": "development",
        "environment_id": "fixture-development",
        "node_id": "node-kernel-operational",
        "project_id": "forgeos",
        "role": "execution",
        "run_id": "run-kernel-operational-fixture",
        "target_id": None,
        "task_id": "task-kernel-operational-v1",
    }


def _artifact(kind: str, reference: str, digit: str) -> dict[str, object]:
    return {"artifact_kind": kind, "artifact_ref": reference,
            "artifact_sha256": digit * 64}


def _invocation_ref(invocation: dict) -> dict[str, object]:
    return {"invocation_id": invocation["invocation_id"],
            "invocation_sha256": invocation["invocation_sha256"]}


def _artifact_receipt_ref(receipt: dict) -> dict[str, object]:
    return {"artifact_receipt_id": receipt["artifact_receipt_id"],
            "artifact_receipt_sha256": receipt["artifact_receipt_sha256"]}


def _event_ref(event: dict) -> dict[str, object]:
    return {"event_id": event["event_id"], "event_sha256": event["event_sha256"]}


def _execution_ref(receipt: dict) -> dict[str, object]:
    return {"execution_receipt_id": receipt["execution_receipt_id"],
            "execution_receipt_sha256": receipt["execution_receipt_sha256"]}


def _artifact_receipt(artifact: dict, content_bytes: int, created: int,
                      producer: dict, role: str, slot: str,
                      invocation: dict | None) -> dict[str, object]:
    return seal_artifact_receipt({
        "api_version": ARTIFACT_RECEIPT_API,  # noqa: F405
        "artifact": artifact,
        "artifact_receipt_id": "",
        "artifact_receipt_sha256": "",
        "attestations": _attestations(),
        "bindings": _bindings(),
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "content_bytes": content_bytes,
        "created_at_unix_ms": created,
        "kind": ARTIFACT_RECEIPT_KIND,  # noqa: F405
        "producer": producer,
        "producer_invocation_ref": None if invocation is None else _invocation_ref(invocation),
        "receipt_role": role,
        "slot": slot,
        "task_binding": _task_binding(),
    })


def _invocation(attempt: int, input_receipt: dict | None,
                prior: dict | None, requested: int) -> dict[str, object]:
    inputs = [] if input_receipt is None else [_artifact_receipt_ref(input_receipt)]
    return seal_capability_invocation({
        "api_version": INVOCATION_API,  # noqa: F405
        "attempt": attempt,
        "attestations": _attestations(),
        "bindings": _bindings(),
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "capability": {"capability_contract_sha256": "5" * 64,
                       "capability_id": "fixture.operation",
                       "capability_version": "1"},
        "capability_grant_ref": {
            "authority_domain": "forgeos.fixture",
            "grant_id": f"capability-grant-{'6' * 64}",
            "grant_sha256": "6" * 64,
        },
        "correlation_id": "correlation-kernel-operational-v1",
        "declared_output_slots": [] if input_receipt is None else ["result"],
        "idempotency_key": "idempotency-kernel-operational-v1",
        "input_artifact_receipt_refs": inputs,
        "invocation_id": "",
        "invocation_sha256": "",
        "kind": INVOCATION_KIND,  # noqa: F405
        "prior_execution_receipt_ref": None if prior is None else _execution_ref(prior),
        "requested_action_sha256": "7" * 64,
        "requested_at_unix_ms": requested,
        "subject": _principal("fixture-subject", "agent"),
        "task_binding": _task_binding(),
    })


def _event(invocation: dict, sequence: int, occurred: int, verb: str,
           artifact: dict, cause: dict | None, actor: dict,
           target: dict | None) -> dict[str, object]:
    return seal_interaction_event({
        "actor": actor,
        "api_version": EVENT_API,  # noqa: F405
        "artifact_refs": [artifact],
        "attestations": _attestations(),
        "bindings": _bindings(),
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "causation_event_ref": None if cause is None else _event_ref(cause),
        "confidence_micros": None if sequence == 1 else 500_000,
        "correlation_id": "correlation-kernel-operational-v1",
        "event_id": "",
        "event_sha256": "",
        "invocation_ref": _invocation_ref(invocation),
        "kind": EVENT_KIND,  # noqa: F405
        "logical_sequence": sequence,
        "object_ref": f"fixture/object/attempt-{invocation['attempt']}/{sequence}",
        "occurred_at_unix_ms": occurred,
        "target": target,
        "task_binding": _task_binding(),
        "verb": verb,
    })


def _execution_receipt(invocation: dict, events: list[dict], output: dict | None,
                       prior: dict | None, started: int, ended: int,
                       outcome: str) -> dict[str, object]:
    output_refs = [] if output is None else [_artifact_receipt_ref(output)]
    inputs = [] if not invocation["input_artifact_receipt_refs"] else [
        _artifact("request", "fixture/input-v1", "a")]
    return seal_execution_receipt({
        "api_version": EXECUTION_RECEIPT_API,  # noqa: F405
        "attempt": invocation["attempt"],
        "attestations": _attestations(),
        "bindings": _bindings(),
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "correlation_id": "correlation-kernel-operational-v1",
        "ended_at_unix_ms": ended,
        "event_refs": [_event_ref(item) for item in events],
        "execution_receipt_id": "",
        "execution_receipt_sha256": "",
        "executor": _principal("fixture-executor", "service"),
        "input_artifacts": inputs,
        "invocation_ref": _invocation_ref(invocation),
        "kind": EXECUTION_RECEIPT_KIND,  # noqa: F405
        "observed_usage": {
            "call_count": invocation["attempt"], "cost_usd_micros": 10,
            "elapsed_ms": ended - started, "input_tokens": 7,
            "network_bytes": 9, "output_bytes": 17, "output_tokens": 3,
        },
        "outcome": outcome,
        "output_artifact_receipt_refs": output_refs,
        "prior_execution_receipt_ref": None if prior is None else _execution_ref(prior),
        "reason_codes": [] if outcome == "succeeded" else ["fixture_retryable_failure"],
        "started_at_unix_ms": started,
        "task_binding": _task_binding(),
    })


def _closure_candidate(artifacts: list[dict], receipts: list[dict],
                       invocations: list[dict], events: list[dict],
                       executions: list[dict]) -> dict[str, object]:
    return {
        "api_version": CLOSURE_API,  # noqa: F405
        "artifact_receipts": sorted(
            receipts, key=lambda item: item["artifact_receipt_id"].encode("utf-8")),
        "artifacts": sorted(artifacts, key=canonical_json),
        "attestations": _attestations(),
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "capability_invocations": invocations,
        "closure_id": "", "closure_sha256": "",
        "execution_receipts": executions,
        "interaction_events": events,
        "kind": CLOSURE_KIND,  # noqa: F405
        "result": SUCCESS_MARKER,  # noqa: F405
    }


def fixture_candidate() -> dict[str, object]:
    """Return the deterministic blank-closure full retry DAG."""
    input_artifact = _artifact("request", "fixture/input-v1", "a")
    first_artifact = _artifact("execution-output", "fixture/attempt-1", "b")
    final_artifact = _artifact("execution-output", "fixture/attempt-2", "c")
    input_receipt = _artifact_receipt(
        input_artifact, 16, 1_700_000_000_000,
        _principal("fixture-input-producer", "human"), "declared_input", "request", None)
    first_invocation = _invocation(1, input_receipt, None, 1_700_000_001_000)
    first_event = _event(first_invocation, 1, 1_700_000_001_100, "execute",
                         input_artifact, None, _principal("fixture-agent", "agent"),
                         _principal("fixture-tool", "service"))
    second_event = _event(first_invocation, 2, 1_700_000_001_200, "observe",
                          first_artifact, first_event,
                          _principal("fixture-tool", "service"), None)
    first_output = _artifact_receipt(
        first_artifact, 17, 1_700_000_001_150,
        _principal("fixture-output-producer", "service"),
        "declared_output", "result", first_invocation)
    first_receipt = _execution_receipt(
        first_invocation, [first_event, second_event], first_output, None,
        1_700_000_001_050, 1_700_000_001_400, "failed")
    retry = _invocation(2, input_receipt, first_receipt, 1_700_000_001_500)
    retry_first_event = _event(retry, 1, 1_700_000_001_600, "execute",
                               input_artifact, None,
                               _principal("fixture-agent", "agent"), None)
    retry_second_event = _event(retry, 2, 1_700_000_001_700, "verify",
                                final_artifact, retry_first_event,
                                _principal("fixture-verifier", "service"),
                                _principal("fixture-subject", "agent"))
    final_output = _artifact_receipt(
        final_artifact, 18, 1_700_000_001_650,
        _principal("fixture-output-producer", "service"),
        "declared_output", "result", retry)
    final_receipt = _execution_receipt(
        retry, [retry_first_event, retry_second_event], final_output, first_receipt,
        1_700_000_001_550, 1_700_000_001_900, "succeeded")
    return _closure_candidate(
        [input_artifact, first_artifact, final_artifact],
        [input_receipt, first_output, final_output], [first_invocation, retry],
        [first_event, second_event, retry_first_event, retry_second_event],
        [first_receipt, final_receipt])


def empty_profile_closure() -> dict[str, object]:
    """Return a sealed no-input/no-output/no-event one-attempt positive profile."""
    invocation = _invocation(1, None, None, 1_700_000_010_000)
    receipt = _execution_receipt(invocation, [], None, None,
                                 1_700_000_010_000, 1_700_000_010_001, "cancelled")
    candidate = {
        "api_version": CLOSURE_API, "artifact_receipts": [], "artifacts": [],  # noqa: F405
        "attestations": _attestations(), "canonicalization": CANONICALIZATION,  # noqa: F405
        "capability_invocations": [invocation], "closure_id": "",
        "closure_sha256": "", "execution_receipts": [receipt],
        "interaction_events": [], "kind": CLOSURE_KIND, "result": SUCCESS_MARKER,  # noqa: F405
    }
    return seal_closure(candidate)


def golden_closure() -> dict[str, object]:
    return seal_closure(fixture_candidate())


def golden_bytes() -> bytes:
    return canonical_json(golden_closure()) + b"\n"


def load_golden(repo_root: Path) -> dict[str, object]:
    raw = read_bounded_file(repo_root / FIXTURE_PATH, "operational golden",  # noqa: F405
                            MAX_CLOSURE_BYTES + 1)  # noqa: F405
    if hashlib.sha256(raw).hexdigest() != GOLDEN_SHA256:
        raise ContractError("operational golden physical SHA-256 mismatch")
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("operational golden must end in exactly one LF")
    from .closure import decode_closure
    closure = decode_closure(raw[:-1])
    if closure != golden_closure() or raw != golden_bytes():
        raise ContractError("operational golden differs from deterministic fixture")
    return closure
