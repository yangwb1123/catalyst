"""Pure Platform Core ExecutionReceipt v1 validation."""

from __future__ import annotations

import re
from typing import Any

from .artifact import (_validate_artifact_references,
                       _validate_artifact_relations,
                       _validate_artifact_values)
from .codec import (ContractError, canonical_json, exact_object, lower_token,
                    validate_document_shape, validate_i64,
                    validate_schema_name)
from .constants import (CANONICALIZATION, EVENT_RANGE_FIELDS,
                        DOCUMENT_INVALID, EXECUTION_RECEIPT_FIELDS, EXECUTOR_FIELDS,
                        EXECUTION_RECEIPT_TYPE_PROFILE,
                        MAX_EXECUTION_ELAPSED_MS, MAX_OBSERVED_COUNT,
                        MAX_OBSERVED_QUANTITY, MAX_REASON_CODES,
                        MAX_RECEIPT_ARTIFACTS, MAX_RECEIPT_BYTES,
                        MAX_UNIX_MILLISECONDS, OBSERVED_USAGE_FIELDS,
                        RECEIPT_VERSION, REFERENCE_MISMATCH,
                        RELATION_MISMATCH, STATE_INVALID)
from .identity import validate_typed_id
from .references import (validate_actor_ref, validate_entity_ref,
                         validate_record_ref, validate_scope_references,
                         validate_scope_values)

_TERMINAL_ATTEMPT_STATES = {"interrupted", "completed", "failed", "uncertain"}
_ADAPTER_VERSION = re.compile(r"(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})")
_RECORD_ROLES = {
    "approval_ref": "forge.control.approval_record",
    "grant_ref": "forge.control.capability_grant",
}


def validate_execution_receipt(value: Any) -> dict[str, Any]:
    """Return one valid supplied execution observation declaration."""
    validate_document_shape(value, EXECUTION_RECEIPT_TYPE_PROFILE,
                            "ExecutionReceipt")
    node = exact_object(value, EXECUTION_RECEIPT_FIELDS, "ExecutionReceipt")
    if (node["canonicalization"] != CANONICALIZATION or
            validate_i64(node["execution_receipt_version"],
                         "execution_receipt_version", 1) != RECEIPT_VERSION):
        raise ContractError("execution receipt version or canonicalization is unsupported")
    validate_typed_id(node["receipt_id"], "rcp", "receipt_id")
    _validate_time_values(node)
    _validate_executor(node["executor"])
    _validate_declaration_values(node)
    _validate_scope_bindings(node)
    _validate_declaration_references(node)
    _validate_terminal_state(node)
    _validate_relations(node)
    canonical_json(node, MAX_RECEIPT_BYTES)
    return node


def _validate_scope_bindings(node: dict[str, Any]) -> None:
    scope = node["scope_ref"]
    validate_scope_references(scope)
    if (scope["project_snapshot_id"] is None or scope["attempt_id"] is None or
            scope["session_id"] is None or scope["turn_id"] is not None or
            scope["action_id"] is not None):
        raise ContractError(
            "execution receipt requires exact session-level snapshot scope",
            REFERENCE_MISMATCH)
    _exact_ref(node["attempt_ref"], "attempt", scope["attempt_id"], "attempt_ref")
    _exact_ref(node["session_ref"], "session", scope["session_id"], "session_ref")
    _exact_ref(node["source_snapshot_ref"], "project_snapshot",
               scope["project_snapshot_id"], "source_snapshot_ref")


def _exact_ref(value: Any, kind: str, identifier: str, label: str) -> None:
    reference = validate_entity_ref(value, label)
    if reference["entity_type"] != kind or reference["entity_id"] != identifier:
        raise ContractError(f"{label} must exactly match scope_ref",
                            REFERENCE_MISMATCH)


def _validate_time_values(node: dict[str, Any]) -> None:
    _unix_ms(node["started_at_unix_ms"], "started_at_unix_ms")
    _unix_ms(node["ended_at_unix_ms"], "ended_at_unix_ms")
    usage = exact_object(node["observed_usage"], OBSERVED_USAGE_FIELDS,
                         "observed_usage")
    _validate_usage(usage)


def _validate_terminal_state(node: dict[str, Any]) -> None:
    terminal = node["terminal_state"]
    if type(terminal) is not str:
        raise ContractError("terminal_state must be text", DOCUMENT_INVALID)
    if terminal not in _TERMINAL_ATTEMPT_STATES:
        raise ContractError(
            f"terminal_state {terminal!r} is not an Attempt terminal state",
            STATE_INVALID)


def _validate_usage(value: dict[str, Any]) -> None:
    bounds = {
        "cost_usd_micros": MAX_OBSERVED_QUANTITY,
        "elapsed_ms": MAX_EXECUTION_ELAPSED_MS,
        "input_tokens": MAX_OBSERVED_QUANTITY,
        "model_calls": MAX_OBSERVED_COUNT,
        "network_bytes": MAX_OBSERVED_QUANTITY,
        "output_bytes": MAX_OBSERVED_QUANTITY,
        "output_tokens": MAX_OBSERVED_QUANTITY,
        "tool_calls": MAX_OBSERVED_COUNT,
    }
    for field, maximum in bounds.items():
        validate_i64(value[field], f"observed_usage.{field}", 0, maximum)


def _validate_executor(value: Any) -> None:
    node = exact_object(value, EXECUTOR_FIELDS, "executor")
    actor = validate_actor_ref(node["actor_ref"])
    if actor["actor_type"] not in {"agent", "service", "system"}:
        raise ContractError("executor actor_type must be agent, service, or system")
    validate_schema_name(node["adapter_id"], "executor.adapter_id")
    version = node["adapter_version"]
    if type(version) is not str:
        raise ContractError("adapter_version must be text", DOCUMENT_INVALID)
    if _ADAPTER_VERSION.fullmatch(version) is None:
        raise ContractError("adapter_version must be canonical bounded major.minor.patch")


def _validate_declaration_values(node: dict[str, Any]) -> None:
    for field in ("approval_ref", "grant_ref"):
        if node[field] is not None:
            validate_record_ref(node[field], field)
    validate_scope_values(node["scope_ref"])
    for field in ("attempt_ref", "session_ref", "source_snapshot_ref"):
        validate_entity_ref(node[field], field)
    _validate_artifact_set_values(node["input_artifact_refs"], "input_artifact_refs")
    _validate_artifact_set_values(node["output_artifact_refs"], "output_artifact_refs")
    _validate_event_range_values(node["event_range"])
    validate_reason_codes(node["reason_codes"], "reason_codes")


def _validate_artifact_set_values(values: Any, label: str) -> None:
    if type(values) is not list:
        raise ContractError(f"{label} must be an array", DOCUMENT_INVALID)
    if len(values) > MAX_RECEIPT_ARTIFACTS:
        raise ContractError(
            f"{label} must be an array of at most {MAX_RECEIPT_ARTIFACTS} items")
    previous = ""
    for artifact in values:
        _validate_artifact_values(artifact)
        if artifact["logical_id"] <= previous:
            raise ContractError(f"{label} must be strictly sorted by logical_id")
        previous = artifact["logical_id"]


def _validate_declaration_references(node: dict[str, Any]) -> None:
    for field, expected in _RECORD_ROLES.items():
        reference = node[field]
        if reference is not None and reference["record_type"] != expected:
            raise ContractError(
                f"{field} must declare record_type {expected}",
                REFERENCE_MISMATCH)
    for field in ("input_artifact_refs", "output_artifact_refs"):
        for artifact in node[field]:
            _validate_artifact_references(artifact)
            if (artifact["source_snapshot_ref"]["entity_id"] !=
                    node["source_snapshot_ref"]["entity_id"]):
                raise ContractError(
                    "execution artifact source snapshot must match receipt",
                    REFERENCE_MISMATCH)
    event_range = node["event_range"]
    if event_range is not None and event_range["aggregate_ref"] != node["attempt_ref"]:
        raise ContractError("event_range aggregate must equal attempt_ref",
                            REFERENCE_MISMATCH)


def _validate_relations(node: dict[str, Any]) -> None:
    elapsed = node["ended_at_unix_ms"] - node["started_at_unix_ms"]
    if not 0 <= elapsed <= MAX_EXECUTION_ELAPSED_MS:
        raise ContractError("execution wall interval is invalid or exceeds maximum",
                            RELATION_MISMATCH)
    if node["observed_usage"]["elapsed_ms"] != elapsed:
        raise ContractError(
            "observed_usage.elapsed_ms must equal execution wall interval",
            RELATION_MISMATCH)
    for artifact in node["input_artifact_refs"]:
        _validate_artifact_relations(artifact)
        _validate_artifact_relation(artifact, node, False)
    for artifact in node["output_artifact_refs"]:
        _validate_artifact_relations(artifact)
        _validate_artifact_relation(artifact, node, True)
    _validate_event_range_relation(node["event_range"])
    if (node["terminal_state"] == "completed") != (len(node["reason_codes"]) == 0):
        raise ContractError(
            "completed requires no reasons; other terminals require reasons",
            RELATION_MISMATCH)


def _validate_artifact_relation(
        artifact: dict[str, Any], receipt: dict[str, Any], output: bool) -> None:
    if not output and artifact["created_at_unix_ms"] > receipt["started_at_unix_ms"]:
        raise ContractError("input artifact cannot postdate execution start",
                            RELATION_MISMATCH)
    if output and (artifact["producer_attempt_id"] != receipt["attempt_ref"]["entity_id"] or
                   not receipt["started_at_unix_ms"] <= artifact["created_at_unix_ms"] <=
                   receipt["ended_at_unix_ms"]):
        raise ContractError(
            "output artifact must be produced by the attempt during execution",
            RELATION_MISMATCH)


def _validate_event_range_values(value: Any) -> None:
    if value is None:
        return
    node = exact_object(value, EVENT_RANGE_FIELDS, "event_range")
    validate_entity_ref(node["aggregate_ref"], "event_range.aggregate_ref")
    validate_typed_id(node["first_event_id"], "evt", "event_range.first_event_id")
    validate_typed_id(node["last_event_id"], "evt", "event_range.last_event_id")
    validate_i64(node["first_sequence"], "first_sequence", 1)
    validate_i64(node["last_sequence"], "last_sequence", 1)


def _validate_event_range_relation(value: Any) -> None:
    if value is None:
        return
    if value["last_sequence"] < value["first_sequence"]:
        raise ContractError("event_range sequence interval is reversed",
                            RELATION_MISMATCH)
    if (
            (value["first_sequence"] == value["last_sequence"]) !=
            (value["first_event_id"] == value["last_event_id"])):
        raise ContractError(
            "event_range identity and sequence cardinality disagree",
            RELATION_MISMATCH)


def validate_reason_codes(values: Any, label: str) -> None:
    """Validate one exact sorted set of bounded reason tokens."""
    if type(values) is not list:
        raise ContractError(f"{label} must be an array", DOCUMENT_INVALID)
    if len(values) > MAX_REASON_CODES:
        raise ContractError(
            f"{label} must be an array of at most {MAX_REASON_CODES} items")
    previous = ""
    for value in values:
        if type(value) is not str:
            raise ContractError(f"{label} entries must be text", DOCUMENT_INVALID)
        if len(value) > 64 or not lower_token(value) or value <= previous:
            raise ContractError(f"{label} must be strictly sorted unique lowercase tokens")
        previous = value


def _unix_ms(value: Any, label: str) -> int:
    return validate_i64(value, label, 1, MAX_UNIX_MILLISECONDS)
