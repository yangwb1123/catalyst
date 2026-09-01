"""Pure Platform Core VerificationRequest/Receipt v1 validation."""

from __future__ import annotations

from typing import Any

from .artifact import (_validate_artifact_references,
                       _validate_artifact_relations,
                       _validate_artifact_values)
from .codec import (ContractError, canonical_json, decode_canonical, exact_object,
                    lower_token, observation_digest, validate_hash, validate_i64,
                    validate_schema_name, validate_document_shape, validate_text)
from .constants import (CANONICALIZATION, MAX_EVIDENCE_REFS,
                        DOCUMENT_INVALID, MAX_RECEIPT_BYTES, MAX_UNIX_MILLISECONDS,
                        MAX_VERIFICATION_CHECKS, RECEIPT_VERSION,
                        REFERENCE_MISMATCH, RELATION_MISMATCH, STATE_INVALID,
                        VERIFICATION_CHECK_REQUEST_FIELDS,
                        VERIFICATION_RECEIPT_TYPE_PROFILE,
                        VERIFICATION_RECEIPT_FIELDS,
                        VERIFICATION_REQUEST_DIGEST_DOMAIN,
                        VERIFICATION_REQUEST_FIELDS,
                        VERIFICATION_REQUEST_TYPE_PROFILE,
                        VERIFICATION_RESULT_FIELDS)
from .execution_receipt import validate_reason_codes
from .identity import validate_typed_id
from .references import (validate_actor_ref, validate_record_ref,
                         validate_scope_references, validate_scope_values)

_STATUSES = {"pass", "fail", "inconclusive", "not_executed"}
_SEVERITY = {"pass": 0, "not_executed": 1, "inconclusive": 2, "fail": 3}


def validate_verification_request(value: Any) -> dict[str, Any]:
    """Return one valid immutable-input verification request."""
    validate_document_shape(value, VERIFICATION_REQUEST_TYPE_PROFILE,
                            "VerificationRequest")
    node = exact_object(value, VERIFICATION_REQUEST_FIELDS,
                        "VerificationRequest")
    _validate_request_values(node)
    canonical_json(node, MAX_RECEIPT_BYTES)
    _validate_input_references(node["scope_ref"], node["input_artifact_ref"])
    _validate_request_relations(node)
    return node


def _validate_request_values(node: dict[str, Any]) -> None:
    if (node["canonicalization"] != CANONICALIZATION or
            validate_i64(node["verification_request_version"],
                         "verification_request_version", 1) != RECEIPT_VERSION):
        raise ContractError("verification request version or canonicalization is unsupported")
    validate_typed_id(node["verification_id"], "ver", "verification_id")
    validate_actor_ref(node["requested_by"])
    _unix_ms(node["requested_at_unix_ms"], "requested_at_unix_ms")
    validate_scope_values(node["scope_ref"])
    _validate_artifact_values(node["input_artifact_ref"])
    _validate_check_requests(node["checks"])


def _validate_request_relations(node: dict[str, Any]) -> None:
    _validate_artifact_relations(node["input_artifact_ref"])
    if node["input_artifact_ref"]["created_at_unix_ms"] > node["requested_at_unix_ms"]:
        raise ContractError("verification input cannot postdate its request",
                            RELATION_MISMATCH)


def validate_verification_receipt(value: Any) -> dict[str, Any]:
    """Return one valid supplied verification observation declaration."""
    validate_document_shape(value, VERIFICATION_RECEIPT_TYPE_PROFILE,
                            "VerificationReceipt")
    node = exact_object(value, VERIFICATION_RECEIPT_FIELDS,
                        "VerificationReceipt")
    _validate_receipt_values(node)
    canonical_json(node, MAX_RECEIPT_BYTES)
    _validate_input_references(node["scope_ref"], node["input_artifact_ref"])
    _validate_receipt_state(node)
    _validate_receipt_relations(node)
    return node


def _validate_receipt_values(node: dict[str, Any]) -> None:
    if (node["canonicalization"] != CANONICALIZATION or
            validate_i64(node["verification_receipt_version"],
                         "verification_receipt_version", 1) != RECEIPT_VERSION):
        raise ContractError("verification receipt version or canonicalization is unsupported")
    validate_typed_id(node["receipt_id"], "rcp", "receipt_id")
    validate_typed_id(node["verification_id"], "ver", "verification_id")
    validate_hash(node["request_sha256"], "request_sha256")
    _validate_producer(node["produced_by"])
    validate_scope_values(node["scope_ref"])
    _validate_artifact_values(node["input_artifact_ref"])
    _unix_ms(node["started_at_unix_ms"], "started_at_unix_ms")
    _unix_ms(node["ended_at_unix_ms"], "ended_at_unix_ms")
    _validate_check_result_values(node["results"])


def validate_verification_exchange(request: Any, receipt: Any) -> None:
    """Compare two already supplied records without resolving evidence."""
    request_node = _detached_exchange_value(
        request, VERIFICATION_REQUEST_TYPE_PROFILE, "VerificationRequest",
        VERIFICATION_REQUEST_FIELDS)
    receipt_node = _detached_exchange_value(
        receipt, VERIFICATION_RECEIPT_TYPE_PROFILE, "VerificationReceipt",
        VERIFICATION_RECEIPT_FIELDS)
    _validate_request_values(request_node)
    _validate_receipt_values(receipt_node)
    _validate_input_references(request_node["scope_ref"],
                               request_node["input_artifact_ref"])
    _validate_input_references(receipt_node["scope_ref"],
                               receipt_node["input_artifact_ref"])
    _validate_receipt_state(receipt_node)
    _validate_request_relations(request_node)
    _validate_receipt_relations(receipt_node)
    _validate_exchange_relations(request_node, receipt_node)


def _validate_exchange_relations(request_node: dict[str, Any],
                                 receipt_node: dict[str, Any]) -> None:
    request_digest = observation_digest(
        VERIFICATION_REQUEST_DIGEST_DOMAIN,
        canonical_json(request_node, MAX_RECEIPT_BYTES))
    if (request_node["verification_id"] != receipt_node["verification_id"] or
            receipt_node["request_sha256"] != request_digest):
        raise ContractError("verification receipt does not bind the exact request",
                            RELATION_MISMATCH)
    if (request_node["scope_ref"] != receipt_node["scope_ref"] or
            request_node["input_artifact_ref"] != receipt_node["input_artifact_ref"]):
        raise ContractError("verification request and receipt scope or input differs",
                            RELATION_MISMATCH)
    if receipt_node["started_at_unix_ms"] < request_node["requested_at_unix_ms"]:
        raise ContractError("verification cannot start before its request",
                            RELATION_MISMATCH)
    _compare_checks(request_node["checks"], receipt_node["results"])


def _detached_exchange_value(value: Any, profile: Any, label: str,
                             fields: set[str]) -> dict[str, Any]:
    if type(value) is not dict:
        raise ContractError(f"{label} must be an object", DOCUMENT_INVALID)
    canonical = canonical_json(value, MAX_RECEIPT_BYTES)
    snapshot = decode_canonical(canonical, MAX_RECEIPT_BYTES)
    validate_document_shape(snapshot, profile, label)
    return exact_object(snapshot, fields, label)


def _validate_input_references(scope_value: Any, input_value: Any) -> None:
    scope = scope_value
    validate_scope_references(scope)
    _validate_artifact_references(input_value)
    if scope["project_snapshot_id"] is None or scope["attempt_id"] is None:
        raise ContractError("verification scope requires project snapshot and attempt",
                            REFERENCE_MISMATCH)
    artifact = input_value
    if (artifact["source_snapshot_ref"]["entity_id"] != scope["project_snapshot_id"] or
            artifact["producer_attempt_id"] != scope["attempt_id"]):
        raise ContractError("verification input must match scoped snapshot and attempt",
                            REFERENCE_MISMATCH)


def _validate_check_requests(values: Any) -> None:
    if type(values) is not list:
        raise ContractError("checks must be an array", DOCUMENT_INVALID)
    if not 1 <= len(values) <= MAX_VERIFICATION_CHECKS:
        raise ContractError(
            f"checks cardinality must be 1..{MAX_VERIFICATION_CHECKS}")
    previous = ""
    for value in values:
        node = exact_object(value, VERIFICATION_CHECK_REQUEST_FIELDS,
                            "VerificationCheckRequest")
        _check_id(node["check_id"])
        if node["check_id"] <= previous:
            raise ContractError("checks must be strictly sorted and unique by check_id")
        validate_schema_name(node["check_name"], "check_name")
        if type(node["declared_required"]) is not bool:
            raise ContractError("declared_required must be boolean", DOCUMENT_INVALID)
        previous = node["check_id"]


def _validate_producer(value: Any) -> None:
    actor = validate_actor_ref(value)
    if actor["actor_type"] != "harness":
        raise ContractError(
            "verification receipt producer must declare harness actor_type")


def _validate_receipt_relations(node: dict[str, Any]) -> None:
    _validate_artifact_relations(node["input_artifact_ref"])
    started = node["started_at_unix_ms"]
    ended = node["ended_at_unix_ms"]
    if ended < started or node["input_artifact_ref"]["created_at_unix_ms"] > started:
        raise ContractError("verification interval or input timing is invalid",
                            RELATION_MISMATCH)
    for result in node["results"]:
        _validate_check_result_relations(result)
    derived = _derive_verification_status(node["results"])
    if node["overall_status"] != derived:
        raise ContractError(f"overall_status must be derived as {derived}",
                            RELATION_MISMATCH)


def _validate_receipt_state(node: dict[str, Any]) -> None:
    _validate_status(node["overall_status"], "overall_status")
    for result in node["results"]:
        _validate_status(result["status"], "verification status")


def _validate_check_result_values(values: Any) -> None:
    if type(values) is not list:
        raise ContractError("results must be an array", DOCUMENT_INVALID)
    if not 1 <= len(values) <= MAX_VERIFICATION_CHECKS:
        raise ContractError(
            f"results cardinality must be 1..{MAX_VERIFICATION_CHECKS}")
    previous = ""
    for value in values:
        node = _validate_check_result_value(value)
        if node["check_id"] <= previous:
            raise ContractError("results must be strictly sorted and unique by check_id")
        previous = node["check_id"]


def _validate_check_result_value(value: Any) -> dict[str, Any]:
    node = exact_object(value, VERIFICATION_RESULT_FIELDS,
                        "VerificationCheckResult")
    _check_id(node["check_id"])
    applicability = node["applicability"]
    if applicability not in {"applicable", "not_applicable"}:
        raise ContractError(f"applicability {applicability!r} is unsupported")
    if node["applicability_reason"] is not None:
        validate_text(node["applicability_reason"], "applicability_reason", 512)
    validate_reason_codes(node["reason_codes"], "result.reason_codes")
    _validate_evidence_refs(node["evidence_refs"])
    return node


def _validate_check_result_relations(node: dict[str, Any]) -> None:
    _validate_applicability(node)
    if (node["status"] == "pass") != (len(node["reason_codes"]) == 0):
        raise ContractError("pass requires no reasons; other statuses require reasons",
                            RELATION_MISMATCH)


def _derive_verification_status(values: list[dict[str, Any]]) -> str:
    overall, applicable = "pass", False
    for value in values:
        if value["applicability"] == "applicable":
            applicable = True
            if _SEVERITY[value["status"]] > _SEVERITY[overall]:
                overall = value["status"]
    return overall if applicable else "not_executed"


def _validate_status(value: Any, label: str) -> None:
    if type(value) is not str:
        raise ContractError(f"{label} must be text", DOCUMENT_INVALID)
    if value not in _STATUSES:
        raise ContractError(f"{label} {value!r} is unsupported", STATE_INVALID)


def _validate_applicability(node: dict[str, Any]) -> None:
    applicability, reason = node["applicability"], node["applicability_reason"]
    if type(applicability) is not str:
        raise ContractError("applicability must be text", DOCUMENT_INVALID)
    if applicability == "applicable":
        if reason is not None:
            raise ContractError("applicable result requires null applicability_reason",
                                RELATION_MISMATCH)
    elif applicability == "not_applicable":
        if node["status"] != "not_executed" or reason is None:
            raise ContractError("not_applicable requires not_executed and a reason",
                                RELATION_MISMATCH)
        validate_text(reason, "applicability_reason", 512)
    else:
        raise ContractError(f"applicability {applicability!r} is unsupported")


def _validate_evidence_refs(values: Any) -> None:
    if type(values) is not list:
        raise ContractError("evidence_refs must be an array", DOCUMENT_INVALID)
    if len(values) > MAX_EVIDENCE_REFS:
        raise ContractError(
            f"evidence_refs must have at most {MAX_EVIDENCE_REFS} items")
    previous = ""
    for value in values:
        reference = validate_record_ref(value, "evidence_ref")
        if reference["record_id"] <= previous:
            raise ContractError("evidence_refs must be strictly sorted and unique")
        previous = reference["record_id"]


def _compare_checks(requests: list[dict[str, Any]],
                    results: list[dict[str, Any]]) -> None:
    if (len(requests) != len(results) or
            any(request["check_id"] != result["check_id"]
                for request, result in zip(requests, results))):
        raise ContractError(
            "verification result set must exactly cover requested checks",
            RELATION_MISMATCH)


def _check_id(value: Any) -> None:
    if type(value) is not str:
        raise ContractError("check_id must be text", DOCUMENT_INVALID)
    if len(value) > 64 or not lower_token(value):
        raise ContractError("check_id must be a bounded lowercase token")


def _unix_ms(value: Any, label: str) -> int:
    return validate_i64(value, label, 1, MAX_UNIX_MILLISECONDS)
