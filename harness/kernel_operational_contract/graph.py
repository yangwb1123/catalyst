"""Acyclic supplied-record closure validation for the operational core."""

from __future__ import annotations

from .codec import ContractError, canonical_json
from .shape import (artifact_receipt_ref, event_ref, execution_receipt_ref,
                    invocation_ref)


def _unique_map(records: list[dict], id_field: str, label: str) -> dict[str, dict]:
    result: dict[str, dict] = {}
    for record in records:
        identity = record[id_field]
        if identity in result:
            raise ContractError(f"duplicate {label} identity {identity!r}")
        result[identity] = record
    return result


def _exact_ref(record: dict, id_field: str, hash_field: str) -> dict[str, str]:
    return {id_field: record[id_field], hash_field: record[hash_field]}


def _common_context(closure: dict) -> None:
    invocation = closure["capability_invocations"][0]
    expected_binding = invocation["bindings"]
    expected_task = invocation["task_binding"]
    groups = (closure["artifact_receipts"], closure["capability_invocations"],
              closure["interaction_events"], closure["execution_receipts"])
    for records in groups:
        for record in records:
            if record["bindings"] != expected_binding:
                raise ContractError("every record must carry the same opaque bindings")
            if record["task_binding"] != expected_task:
                raise ContractError("every record must carry the same TaskBinding")


def _retry_static(invocation: dict) -> tuple:
    fields = ("bindings", "capability", "capability_grant_ref", "correlation_id",
              "declared_output_slots", "idempotency_key",
              "input_artifact_receipt_refs", "requested_action_sha256", "subject",
              "task_binding")
    return tuple(invocation[field] for field in fields)


def _validate_retry_chain(invocations: list[dict], receipts: list[dict]) -> None:
    if [item["attempt"] for item in invocations] != list(range(1, len(invocations) + 1)):
        raise ContractError("capability_invocations must be ordered contiguous attempts 1..N")
    if [item["attempt"] for item in receipts] != list(range(1, len(receipts) + 1)):
        raise ContractError("execution_receipts must be ordered contiguous attempts 1..N")
    static = _retry_static(invocations[0])
    for index, invocation in enumerate(invocations):
        if _retry_static(invocation) != static:
            raise ContractError("retry invocations must preserve the complete static request")
        receipt = receipts[index]
        if receipt["attempt"] != invocation["attempt"]:
            raise ContractError("invocation and execution receipt attempts must match")
        expected = None if index == 0 else _exact_ref(
            receipts[index - 1], "execution_receipt_id", "execution_receipt_sha256")
        if invocation["prior_execution_receipt_ref"] != expected:
            raise ContractError("invocation prior receipt must be the preceding attempt")
        if receipt["prior_execution_receipt_ref"] != expected:
            raise ContractError("execution receipt prior must equal its invocation prior")
        if index and receipts[index - 1]["outcome"] == "succeeded":
            raise ContractError("a succeeded execution receipt cannot be retried")
        if index and invocation["requested_at_unix_ms"] < receipts[index - 1]["ended_at_unix_ms"]:
            raise ContractError("retry request time precedes the prior attempt end")


def _resolve_inputs(invocation: dict, receipt: dict,
                    artifact_receipts: dict[str, dict], used: set[str]) -> None:
    artifacts: list[dict] = []
    for reference in invocation["input_artifact_receipt_refs"]:
        member = artifact_receipts.get(reference["artifact_receipt_id"])
        if member is None or reference != _exact_ref(
                member, "artifact_receipt_id", "artifact_receipt_sha256"):
            raise ContractError("input artifact receipt reference is unresolved")
        if member["receipt_role"] != "declared_input":
            raise ContractError("invocation inputs must reference declared_input receipts")
        if member["created_at_unix_ms"] > invocation["requested_at_unix_ms"]:
            raise ContractError("input ArtifactReceipt is declared after the invocation request")
        used.add(member["artifact_receipt_id"])
        artifacts.append(member["artifact"])
    encoded = [canonical_json(item) for item in artifacts]
    if len(encoded) != len(set(encoded)):
        raise ContractError("invocation inputs must project distinct ArtifactRefs")
    expected = sorted(artifacts, key=canonical_json)
    if receipt["input_artifacts"] != expected:
        raise ContractError("ExecutionReceipt input_artifacts must exactly project inputs")


def _outputs_for(invocation: dict, receipt: dict, artifacts: list[dict]) -> list[dict]:
    invocation_reference = _exact_ref(invocation, "invocation_id", "invocation_sha256")
    outputs = [item for item in artifacts
               if item["producer_invocation_ref"] == invocation_reference]
    slots = [item["slot"] for item in outputs]
    if len(slots) != len(set(slots)):
        raise ContractError("an invocation may produce at most one ArtifactReceipt per slot")
    if sorted(slots, key=lambda item: item.encode("utf-8")) != invocation["declared_output_slots"]:
        raise ContractError("declared_output_slots must exactly cover output ArtifactReceipts")
    references = [_exact_ref(item, "artifact_receipt_id", "artifact_receipt_sha256")
                  for item in outputs]
    references.sort(key=canonical_json)
    if receipt["output_artifact_receipt_refs"] != references:
        raise ContractError("ExecutionReceipt must reference every exact output receipt")
    for output in outputs:
        if not receipt["started_at_unix_ms"] <= output["created_at_unix_ms"] <= receipt[
                "ended_at_unix_ms"]:
            raise ContractError("output ArtifactReceipt time is outside its execution")
    return outputs


def _events_for(invocation: dict, receipt: dict, events: list[dict]) -> list[dict]:
    reference = _exact_ref(invocation, "invocation_id", "invocation_sha256")
    members = [item for item in events if item["invocation_ref"] == reference]
    sequences = [item["logical_sequence"] for item in members]
    if sequences != list(range(1, len(members) + 1)):
        raise ContractError("events must be ordered contiguous logical sequences per invocation")
    expected_refs = [_exact_ref(item, "event_id", "event_sha256") for item in members]
    if receipt["event_refs"] != expected_refs:
        raise ContractError("ExecutionReceipt event_refs must exactly cover its events")
    for index, event in enumerate(members):
        cause = None if index == 0 else expected_refs[index - 1]
        if event["causation_event_ref"] != cause:
            raise ContractError("event causation must be the immediately preceding event")
        if not receipt["started_at_unix_ms"] <= event["occurred_at_unix_ms"] <= receipt[
                "ended_at_unix_ms"]:
            raise ContractError("event time is outside its execution")
        if index and event["occurred_at_unix_ms"] < members[index - 1][
                "occurred_at_unix_ms"]:
            raise ContractError("event times must be nondecreasing by logical sequence")
    return members


def _validate_attempt(invocation: dict, receipt: dict, artifact_map: dict[str, dict],
                      artifacts: list[dict], events: list[dict], used: set[str]) -> list[dict]:
    invocation_reference = _exact_ref(invocation, "invocation_id", "invocation_sha256")
    if receipt["invocation_ref"] != invocation_reference:
        raise ContractError("ExecutionReceipt must reference its matching invocation")
    if receipt["correlation_id"] != invocation["correlation_id"]:
        raise ContractError("receipt and invocation correlation_id differ")
    if receipt["started_at_unix_ms"] < invocation["requested_at_unix_ms"]:
        raise ContractError("execution starts before its invocation request")
    _resolve_inputs(invocation, receipt, artifact_map, used)
    outputs = _outputs_for(invocation, receipt, artifacts)
    used.update(item["artifact_receipt_id"] for item in outputs)
    attempt_events = _events_for(invocation, receipt, events)
    for event in attempt_events:
        if event["correlation_id"] != invocation["correlation_id"]:
            raise ContractError("event and invocation correlation_id differ")
    return attempt_events


def _validate_artifact_inventory(closure: dict) -> None:
    receipts = closure["artifact_receipts"]
    receipt_artifacts = [item["artifact"] for item in receipts]
    event_artifacts: list[dict] = []
    for event in closure["interaction_events"]:
        for artifact in event["artifact_refs"]:
            matching = [receipt for receipt in receipts
                        if receipt["artifact"] == artifact and
                        receipt["created_at_unix_ms"] <= event["occurred_at_unix_ms"]]
            if not matching:
                raise ContractError(
                    "every Event ArtifactRef needs a non-future included ArtifactReceipt")
            event_artifacts.append(artifact)
    projected = receipt_artifacts + event_artifacts
    for receipt in closure["execution_receipts"]:
        projected.extend(receipt["input_artifacts"])
    expected = sorted({canonical_json(item): item for item in projected}.values(),
                      key=canonical_json)
    if closure["artifacts"] != expected:
        raise ContractError("closure artifacts must exactly equal all referenced ArtifactRefs")


def validate_reference_graph(closure: dict) -> None:
    """Validate the one-correlation retry DAG without consulting external state."""
    _common_context(closure)
    invocations = closure["capability_invocations"]
    receipts = closure["execution_receipts"]
    if len(invocations) != len(receipts):
        raise ContractError("each invocation requires exactly one ExecutionReceipt")
    artifact_map = _unique_map(closure["artifact_receipts"],
                               "artifact_receipt_id", "ArtifactReceipt")
    _unique_map(invocations, "invocation_id", "CapabilityInvocation")
    _unique_map(closure["interaction_events"], "event_id", "InteractionEvent")
    _unique_map(receipts, "execution_receipt_id", "ExecutionReceipt")
    _validate_retry_chain(invocations, receipts)
    used_receipts: set[str] = set()
    flattened_events: list[dict] = []
    for invocation, receipt in zip(invocations, receipts, strict=True):
        flattened_events.extend(_validate_attempt(
            invocation, receipt, artifact_map, closure["artifact_receipts"],
            closure["interaction_events"], used_receipts))
    if flattened_events != closure["interaction_events"]:
        raise ContractError("interaction_events must be ordered by attempt then sequence")
    if used_receipts != set(artifact_map):
        raise ContractError("ArtifactReceipt inventory contains an orphan")
    _validate_artifact_inventory(closure)
