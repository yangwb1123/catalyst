"""Deterministic full CognitiveAtom→Transaction→operation golden fixture."""

from __future__ import annotations

import hashlib
from pathlib import Path

from kernel_operational_contract.closure import seal_closure as seal_operational_closure
from kernel_operational_contract.constants import (
    ARTIFACT_RECEIPT_API, ARTIFACT_RECEIPT_KIND, ATTESTATION_FIELDS as OP_ATTESTATIONS,
    CANONICALIZATION as OP_CANONICALIZATION, CLOSURE_API as OP_CLOSURE_API,
    CLOSURE_KIND as OP_CLOSURE_KIND, EVENT_API, EVENT_KIND, EXECUTION_RECEIPT_API,
    EXECUTION_RECEIPT_KIND, INVOCATION_API, INVOCATION_KIND,
    SUCCESS_MARKER as OP_SUCCESS_MARKER,
)
from kernel_operational_contract.records import (
    seal_artifact_receipt, seal_capability_invocation, seal_execution_receipt,
    seal_interaction_event,
)

from .atoms import seal_cognitive_atom
from .closure import seal_closure
from .codec import ContractError, canonical_json, read_bounded_file
from .constants import *  # noqa: F403 - the fixture exercises every frozen constant
from .transaction import seal_decision_transaction

GOLDEN_SHA256 = "93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c"


def _attestations() -> dict[str, bool]:
    return {field: False for field in ATTESTATION_FIELDS}  # noqa: F405


def _op_attestations() -> dict[str, bool]:
    return {field: False for field in OP_ATTESTATIONS}


def _principal(identifier: str, principal_type: str) -> dict[str, object]:
    return {"authority_domain": "forgeos.fixture", "principal_id": identifier,
            "principal_type": principal_type}


def _bindings() -> dict[str, object]:
    return {
        "context_sha256": "1" * 64, "environment_profile_id": "fixture-environment-v1",
        "environment_sha256": "2" * 64, "policy_sha256": "3" * 64,
        "source_profile_id": "fixture-source-v1", "source_revision": "fixture-revision-0090",
        "source_tree_sha256": "4" * 64,
    }


def _task() -> dict[str, object]:
    return {
        "attempt_id": None, "change_id": "change-kernel-decision-v1",
        "environment_class": "development", "environment_id": "fixture-development",
        "node_id": "node-kernel-decision", "project_id": "forgeos", "role": "decision",
        "run_id": "run-kernel-decision-fixture", "target_id": None,
        "task_id": "task-kernel-decision-v1",
    }


def _artifact(kind: str, reference: str, digit: str) -> dict[str, object]:
    return {"artifact_kind": kind, "artifact_ref": reference,
            "artifact_sha256": digit * 64}


def _authority(kind: str, digit: str = "0") -> dict[str, object]:
    if kind == "none":
        reference = None
    elif kind == "approval_record":
        reference = {"approval_id": f"approval-record-{digit * 64}",
                     "approval_sha256": digit * 64,
                     "authority_domain": "forgeos.fixture"}
    elif kind == "architecture_decision":
        reference = {"adr_id": "ADR-9000", "adr_self_sha256": digit * 64}
    else:
        reference = _artifact("governance-contract", "fixture/contract-v1", digit)
    return {"authority_kind": kind, "authority_ref": reference}


def _source(kind: str, digit: str, selector: str | None = None) -> dict[str, object]:
    refs = {
        "artifact": _artifact("source-document", f"fixture/source-{digit}", digit),
        "cognitive_atom_v1": {"atom_id": f"atom-{digit * 64}",
                              "canonical_sha256": ("f" if digit != "f" else "e") * 64},
        "evidence_record": {"canonical_sha256": digit * 64,
                            "record_id": "fixture evidence record"},
        "work_intent": {"work_intent_id": f"work-intent-{digit * 64}",
                        "work_intent_sha256": digit * 64},
    }
    return {"source_kind": kind, "source_phase": "predecision",
            "source_ref": refs[kind], "source_selector": selector}


def _atom(atom_type: str, index: int, source: dict, hardness: str,
          authority: dict, state: str = "declared") -> dict[str, object]:
    subject = f"fixture.atom.{index:02d}"
    confidence = 400_000 + index if atom_type in {"assumption", "hypothesis", "inference"} else None
    return seal_cognitive_atom({
        "api_version": ATOM_API, "atom_id": "", "atom_sha256": "",  # noqa: F405
        "atom_type": atom_type, "attestations": _attestations(), "bindings": _bindings(),
        "canonicalization": CANONICALIZATION, "confidence_micros": confidence,  # noqa: F405
        "declared_authority": authority, "declared_hardness": hardness,
        "effective_hardness": "none", "epistemic_state": state,
        "instruction_allowed": False, "kind": ATOM_KIND,  # noqa: F405
        "proposition": {"object_type": "string", "object_value": f"value-{index:02d}",
                        "predicate": "declares", "subject": subject},
        "scope": {"module": "kernel-decision", "object": subject, "project": "forgeos"},
        "source": source, "task_binding": _task(),
        "validity": {"valid_from_unix_ms": 1_700_000_000_000,
                     "valid_until_unix_ms": None},
    })


def _predecision_atoms() -> list[dict[str, object]]:
    specs = [
        ("acceptance", _source("work_intent", "1", "/intent/success_signals/0"),
         "required", _authority("approval_record", "1"), "declared"),
        ("assumption", _source("artifact", "2", "/assumption"),
         "advisory", _authority("none"), "declared"),
        ("constraint", _source("work_intent", "3", "/intent/external_constraints/0"),
         "contract", _authority("contract_artifact", "3"), "declared"),
        ("constraint", _source("artifact", "4", "/invariant"),
         "invariant", _authority("contract_artifact", "4"), "declared"),
        ("decision", _source("cognitive_atom_v1", "5"),
         "none", _authority("none"), "proposed"),
        ("evidence", _source("evidence_record", "6"),
         "none", _authority("none"), "declared"),
        ("fact", _source("cognitive_atom_v1", "7"),
         "none", _authority("none"), "candidate"),
        ("goal", _source("work_intent", "8", "/intent/goal"),
         "required", _authority("architecture_decision", "8"), "declared"),
        ("hypothesis", _source("artifact", "9", "/hypothesis"),
         "none", _authority("none"), "declared"),
        ("inference", _source("cognitive_atom_v1", "a"),
         "none", _authority("none"), "candidate"),
        ("preference", _source("work_intent", "b", "/intent/non_goals/0"),
         "preferred", _authority("none"), "declared"),
        ("risk", _source("artifact", "c", "/risk"),
         "advisory", _authority("none"), "declared"),
        ("unknown", _source("cognitive_atom_v1", "d"),
         "none", _authority("none"), "open"),
    ]
    return [_atom(atom_type, index, source, hardness, authority, state)
            for index, (atom_type, source, hardness, authority, state) in enumerate(specs, 1)]


def _artifact_receipt_ref(record: dict) -> dict[str, object]:
    return {"artifact_receipt_id": record["artifact_receipt_id"],
            "artifact_receipt_sha256": record["artifact_receipt_sha256"]}


def _invocation_ref(record: dict) -> dict[str, object]:
    return {"invocation_id": record["invocation_id"],
            "invocation_sha256": record["invocation_sha256"]}


def _event_ref(record: dict) -> dict[str, object]:
    return {"event_id": record["event_id"], "event_sha256": record["event_sha256"]}


def _execution_ref(record: dict) -> dict[str, object]:
    return {"execution_receipt_id": record["execution_receipt_id"],
            "execution_receipt_sha256": record["execution_receipt_sha256"]}


def _input_receipt() -> dict[str, object]:
    return seal_artifact_receipt({
        "api_version": ARTIFACT_RECEIPT_API, "artifact": _artifact("request", "fixture/input", "e"),
        "artifact_receipt_id": "", "artifact_receipt_sha256": "",
        "attestations": _op_attestations(), "bindings": _bindings(),
        "canonicalization": OP_CANONICALIZATION, "content_bytes": 16,
        "created_at_unix_ms": 1_700_000_000_000, "kind": ARTIFACT_RECEIPT_KIND,
        "producer": _principal("fixture-input-producer", "human"),
        "producer_invocation_ref": None, "receipt_role": "declared_input",
        "slot": "request", "task_binding": _task(),
    })


def _transaction_options() -> list[dict[str, object]]:
    return [
        {"capability": {"capability_contract_sha256": "5" * 64,
                        "capability_id": "fixture.alternative", "capability_version": "1"},
         "option_id": "alternative", "requested_action_sha256": "6" * 64},
        {"capability": {"capability_contract_sha256": "7" * 64,
                        "capability_id": "fixture.operation", "capability_version": "1"},
         "option_id": "selected", "requested_action_sha256": "8" * 64},
    ]


def _transaction(atoms: list[dict], input_receipt: dict) -> dict[str, object]:
    ordered = sorted(atoms, key=lambda item: item["atom_id"].encode("utf-8"))
    goal = next(item for item in ordered if item["atom_type"] == "goal")
    remaining = [item for item in ordered if item is not goal]
    trigger, guards = remaining[:1], remaining[1:]
    atom_ref = lambda item: {"atom_id": item["atom_id"], "atom_sha256": item["atom_sha256"]}
    return seal_decision_transaction({
        "accountable_owner": _principal("fixture-owner", "human"),
        "actor": _principal("fixture-subject", "agent"),
        "api_version": TRANSACTION_API, "attestations": _attestations(),  # noqa: F405
        "bindings": _bindings(),
        "budget": {"max_calls": 8, "max_cost_usd_micros": 1_000,
                   "max_input_tokens": 1_000, "max_network_bytes": 1_000,
                   "max_output_bytes": 1_000, "max_output_tokens": 1_000,
                   "timeout_ms": 5_000},
        "canonicalization": CANONICALIZATION,  # noqa: F405
        "completion_condition": {"condition_ref": "fixture/completion",
                                 "condition_sha256": "9" * 64},
        "compensation": {"applicability": "not_applicable", "capability": None,
                         "requested_action_sha256": None},
        "created_at_unix_ms": 1_700_000_000_500,
        "decision_transaction_id": "", "decision_transaction_sha256": "",
        "goal_atom_ref": atom_ref(goal),
        "guard_atom_refs": sorted([atom_ref(item) for item in guards],
                                  key=lambda item: item["atom_id"].encode("utf-8")),
        "idempotency_key": "idempotency-kernel-decision-v1", "kind": TRANSACTION_KIND,  # noqa: F405
        "options": _transaction_options(),
        "proof_obligations": [{"obligation_id": "completion-evidence",
                               "predicate_sha256": "a" * 64,
                               "required_evidence_kinds": ["execution_receipt"]}],
        "read_artifact_receipt_refs": [_artifact_receipt_ref(input_receipt)],
        "selected_option_id": "selected", "selection_basis_sha256": "b" * 64,
        "task_binding": _task(), "transaction_mode": TRANSACTION_MODE,  # noqa: F405
        "trigger_atom_refs": [atom_ref(item) for item in trigger],
        "verifier": {"capability": {"capability_contract_sha256": "c" * 64,
                                    "capability_id": "fixture.verify", "capability_version": "1"},
                     "independence_basis_sha256": "d" * 64,
                     "principal": _principal("fixture-verifier", "service"),
                     "timeout_ms": 2_000},
        "write_preconditions": [{"expected_sha256": "e" * 64,
                                 "precondition_id": "world-version",
                                 "resource_ref": "fixture/world"}],
        "write_slots": ["result"],
    })


def _invocation(attempt: int, transaction: dict, input_receipt: dict,
                prior: dict | None, requested: int) -> dict[str, object]:
    selected = next(item for item in transaction["options"]
                    if item["option_id"] == transaction["selected_option_id"])
    return seal_capability_invocation({
        "api_version": INVOCATION_API, "attempt": attempt,
        "attestations": _op_attestations(), "bindings": _bindings(),
        "canonicalization": OP_CANONICALIZATION, "capability": selected["capability"],
        "capability_grant_ref": {"authority_domain": "forgeos.fixture",
                                 "grant_id": f"capability-grant-{'f' * 64}",
                                 "grant_sha256": "f" * 64},
        "correlation_id": transaction["decision_transaction_id"],
        "declared_output_slots": ["result"],
        "idempotency_key": transaction["idempotency_key"],
        "input_artifact_receipt_refs": [_artifact_receipt_ref(input_receipt)],
        "invocation_id": "", "invocation_sha256": "", "kind": INVOCATION_KIND,
        "prior_execution_receipt_ref": None if prior is None else _execution_ref(prior),
        "requested_action_sha256": selected["requested_action_sha256"],
        "requested_at_unix_ms": requested, "subject": transaction["actor"],
        "task_binding": _task(),
    })


def _event(invocation: dict, transaction: dict, artifact: dict, sequence: int,
           occurred: int, verb: str, cause: dict | None) -> dict[str, object]:
    return seal_interaction_event({
        "actor": _principal("fixture-event-actor", "service"), "api_version": EVENT_API,
        "artifact_refs": [artifact], "attestations": _op_attestations(),
        "bindings": _bindings(), "canonicalization": OP_CANONICALIZATION,
        "causation_event_ref": None if cause is None else _event_ref(cause),
        "confidence_micros": None, "correlation_id": transaction["decision_transaction_id"],
        "event_id": "", "event_sha256": "", "invocation_ref": _invocation_ref(invocation),
        "kind": EVENT_KIND, "logical_sequence": sequence,
        "object_ref": f"fixture/attempt-{invocation['attempt']}/{sequence}",
        "occurred_at_unix_ms": occurred, "target": None, "task_binding": _task(),
        "verb": verb,
    })


def _output_receipt(invocation: dict, artifact: dict, created: int) -> dict[str, object]:
    return seal_artifact_receipt({
        "api_version": ARTIFACT_RECEIPT_API, "artifact": artifact,
        "artifact_receipt_id": "", "artifact_receipt_sha256": "",
        "attestations": _op_attestations(), "bindings": _bindings(),
        "canonicalization": OP_CANONICALIZATION, "content_bytes": 17,
        "created_at_unix_ms": created, "kind": ARTIFACT_RECEIPT_KIND,
        "producer": _principal("fixture-output-producer", "service"),
        "producer_invocation_ref": _invocation_ref(invocation),
        "receipt_role": "declared_output", "slot": "result", "task_binding": _task(),
    })


def _execution(invocation: dict, transaction: dict, events: list[dict], output: dict,
               prior: dict | None, started: int, ended: int, outcome: str) -> dict[str, object]:
    return seal_execution_receipt({
        "api_version": EXECUTION_RECEIPT_API, "attempt": invocation["attempt"],
        "attestations": _op_attestations(), "bindings": _bindings(),
        "canonicalization": OP_CANONICALIZATION,
        "correlation_id": transaction["decision_transaction_id"], "ended_at_unix_ms": ended,
        "event_refs": [_event_ref(item) for item in events],
        "execution_receipt_id": "", "execution_receipt_sha256": "",
        "executor": _principal("fixture-executor", "service"),
        "input_artifacts": [_artifact("request", "fixture/input", "e")],
        "invocation_ref": _invocation_ref(invocation), "kind": EXECUTION_RECEIPT_KIND,
        "observed_usage": {"call_count": 1, "cost_usd_micros": 10,
                           "elapsed_ms": ended - started, "input_tokens": 7,
                           "network_bytes": 9, "output_bytes": 17, "output_tokens": 3},
        "outcome": outcome, "output_artifact_receipt_refs": [_artifact_receipt_ref(output)],
        "prior_execution_receipt_ref": None if prior is None else _execution_ref(prior),
        "reason_codes": [] if outcome == "succeeded" else ["fixture_retryable_failure"],
        "started_at_unix_ms": started, "task_binding": _task(),
    })


def _operational_closure(transaction: dict, input_receipt: dict) -> dict[str, object]:
    input_artifact = input_receipt["artifact"]
    first_artifact = _artifact("execution-output", "fixture/attempt-1", "1")
    final_artifact = _artifact("execution-output", "fixture/attempt-2", "2")
    first = _invocation(1, transaction, input_receipt, None, 1_700_000_001_000)
    event1 = _event(first, transaction, input_artifact, 1, 1_700_000_001_100, "execute", None)
    output1 = _output_receipt(first, first_artifact, 1_700_000_001_150)
    event2 = _event(first, transaction, first_artifact, 2, 1_700_000_001_200, "observe", event1)
    receipt1 = _execution(first, transaction, [event1, event2], output1, None,
                          1_700_000_001_050, 1_700_000_001_400, "failed")
    retry = _invocation(2, transaction, input_receipt, receipt1, 1_700_000_001_500)
    event3 = _event(retry, transaction, input_artifact, 1, 1_700_000_001_600, "execute", None)
    output2 = _output_receipt(retry, final_artifact, 1_700_000_001_650)
    event4 = _event(retry, transaction, final_artifact, 2, 1_700_000_001_700, "verify", event3)
    receipt2 = _execution(retry, transaction, [event3, event4], output2, receipt1,
                          1_700_000_001_550, 1_700_000_001_900, "succeeded")
    candidate = {
        "api_version": OP_CLOSURE_API,
        "artifact_receipts": sorted([input_receipt, output1, output2],
                                    key=lambda item: item["artifact_receipt_id"].encode("utf-8")),
        "artifacts": sorted([input_artifact, first_artifact, final_artifact], key=canonical_json),
        "attestations": _op_attestations(), "canonicalization": OP_CANONICALIZATION,
        "capability_invocations": [first, retry], "closure_id": "", "closure_sha256": "",
        "execution_receipts": [receipt1, receipt2],
        "interaction_events": [event1, event2, event3, event4], "kind": OP_CLOSURE_KIND,
        "result": OP_SUCCESS_MARKER,
    }
    return seal_operational_closure(candidate)


def _post_atom(atom_type: str, index: int, kind: str, reference: dict,
               selector: str, valid_from: int) -> dict[str, object]:
    subject = f"fixture.atom.{index:02d}"
    return seal_cognitive_atom({
        "api_version": ATOM_API, "atom_id": "", "atom_sha256": "",  # noqa: F405
        "atom_type": atom_type, "attestations": _attestations(), "bindings": _bindings(),
        "canonicalization": CANONICALIZATION, "confidence_micros": None,  # noqa: F405
        "declared_authority": _authority("none"), "declared_hardness": "none",
        "effective_hardness": "none", "epistemic_state": "declared",
        "instruction_allowed": False, "kind": ATOM_KIND,  # noqa: F405
        "proposition": {"object_type": "string", "object_value": f"value-{index:02d}",
                        "predicate": "declares", "subject": subject},
        "scope": {"module": "kernel-decision", "object": subject, "project": "forgeos"},
        "source": {"source_kind": kind, "source_phase": "postdecision",
                   "source_ref": reference, "source_selector": selector},
        "task_binding": _task(),
        "validity": {"valid_from_unix_ms": valid_from, "valid_until_unix_ms": None},
    })


def _postdecision_atoms(operational: dict) -> list[dict[str, object]]:
    invocation = operational["capability_invocations"][0]
    output = next(item for item in operational["artifact_receipts"]
                  if item["receipt_role"] == "declared_output")
    event = operational["interaction_events"][0]
    receipt = operational["execution_receipts"][-1]
    return [
        _post_atom("actor", 14, "capability_invocation", _invocation_ref(invocation),
                   "/subject", invocation["requested_at_unix_ms"]),
        _post_atom("object", 15, "artifact_receipt", _artifact_receipt_ref(output),
                   "/artifact", output["created_at_unix_ms"]),
        _post_atom("operation", 16, "interaction_event", _event_ref(event),
                   "/verb", event["occurred_at_unix_ms"]),
        _post_atom("observation", 17, "execution_receipt", _execution_ref(receipt),
                   "/outcome", receipt["ended_at_unix_ms"]),
    ]


def fixture_candidate() -> dict[str, object]:
    pre = _predecision_atoms()
    input_receipt = _input_receipt()
    transaction = _transaction(pre, input_receipt)
    operational = _operational_closure(transaction, input_receipt)
    atoms = sorted(pre + _postdecision_atoms(operational),
                   key=lambda item: item["atom_id"].encode("utf-8"))
    return {
        "api_version": CLOSURE_API, "attestations": _attestations(),  # noqa: F405
        "canonicalization": CANONICALIZATION, "closure_id": "", "closure_sha256": "",  # noqa: F405
        "cognitive_atoms": atoms, "decision_transaction": transaction,
        "kind": CLOSURE_KIND, "operational_closure": operational,  # noqa: F405
        "result": SUCCESS_MARKER,  # noqa: F405
    }


def golden_closure() -> dict[str, object]:
    return seal_closure(fixture_candidate())


def golden_bytes() -> bytes:
    return canonical_json(golden_closure()) + b"\n"


def load_golden(repo_root: Path) -> dict[str, object]:
    path = repo_root / "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
    raw = read_bounded_file(path, "kernel decision golden", MAX_CLOSURE_BYTES)  # noqa: F405
    if hashlib.sha256(raw).hexdigest() != GOLDEN_SHA256:
        raise ContractError("kernel decision golden physical SHA-256 mismatch")
    if raw != golden_bytes():
        raise ContractError("kernel decision golden differs from deterministic reconstruction")
    from .closure import decode_closure
    return decode_closure(raw[:-1])
