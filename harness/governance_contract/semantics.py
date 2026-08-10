"""Kind-specific shadow semantics without truth or authority attestation."""

from __future__ import annotations

from pathlib import PurePosixPath

from .codec import ContractError, canonical_json, compute_record_digest
from .constants import (CLAIM_STATES, ID_RE, LOCATOR_BY_TYPE, MAX_RECORD_BYTES,
                        SHADOW_STATES)
from .shape import (integer, validate_claim_shape, validate_common,
                    validate_evidence_shape, validate_status)


def _safe_repo_locator(value: object) -> bool:
    if not isinstance(value, str) or value.startswith(("/", "\\")) or "\\" in value:
        return False
    if len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":":
        return False
    path = PurePosixPath(value)
    return bool(path.parts) and str(path) == value and all(part not in {"", ".", ".."}
                                                      for part in path.parts)


def _locator_semantics(spec: dict[str, object], label: str, issues: list[str]) -> None:
    evidence_type, locator = spec["evidence_type"], spec["locator"]
    if not isinstance(evidence_type, str) or evidence_type not in LOCATOR_BY_TYPE or not isinstance(locator, dict):
        return
    locator_type = locator.get("locator_type")
    if locator_type != LOCATOR_BY_TYPE[evidence_type]:
        issues.append(f"{label}.spec.locator: type does not match evidence_type")
    if locator_type == "repo" and not _safe_repo_locator(locator.get("locator_ref")):
        issues.append(f"{label}.spec.locator.locator_ref: unsafe repository path")
    line_values = locator.get("line_start"), locator.get("line_end")
    if locator_type == "repo" and locator.get("exit_code") is not None:
        issues.append(f"{label}.spec.locator: repository locator cannot have exit_code")
    if locator_type == "command" and (locator.get("exit_code") is None or line_values != (None, None)):
        issues.append(f"{label}.spec.locator: command requires exit_code and no line range")
    if locator_type not in {"repo", "command"} and (locator.get("exit_code") is not None or
                                                      line_values != (None, None)):
        issues.append(f"{label}.spec.locator: non-repo/command locator cannot have line or exit data")


def _evidence_origin_semantics(spec: dict[str, object], label: str, issues: list[str]) -> None:
    evidence_type = spec["evidence_type"]
    collector = spec["collector"] if isinstance(spec["collector"], dict) else {}
    collector_type = collector.get("collector_type")
    if not isinstance(spec["source_trust"], str) or spec["source_trust"] not in {"untrusted", "observed"}:
        issues.append(f"{label}.spec.source_trust: controlled/authoritative trust is unavailable")
    if spec["content_role"] != "untrusted_data":
        issues.append(f"{label}.spec.content_role: trusted control is unavailable in shadow mode")
    if evidence_type == "human_attestation" and spec["directness"] != "attested":
        issues.append(f"{label}.spec.directness: human attestation must be attested")
    if evidence_type == "human_attestation" and (not isinstance(collector_type, str) or
                                                   collector_type not in {"human", "operator"}):
        issues.append(f"{label}.spec.collector: human attestation requires human/operator collector")
    if evidence_type == "external_source" and spec["directness"] != "derived":
        issues.append(f"{label}.spec.directness: external source must be derived")
    direct_types = {"repo_locator", "test_run", "gate_result", "runtime_metric", "artifact"}
    if spec["directness"] == "direct" and (not isinstance(evidence_type, str) or
                                             evidence_type not in direct_types or
                                             not isinstance(collector_type, str) or
                                             collector_type not in {"tool", "service"}):
        issues.append(f"{label}.spec.directness: direct evidence requires tool/service observation")


def _evidence_state_semantics(record: dict[str, object], label: str, issues: list[str]) -> None:
    spec, status, metadata = record["spec"], record["status"], record["metadata"]
    state, reasons, artifact = status["state"], status["reason_codes"], spec["artifact_sha256"]
    if not isinstance(state, str):
        return
    if state == "valid" and (artifact is None or reasons != []):
        issues.append(f"{label}: valid evidence requires artifact digest and no reason codes")
    if state == "unavailable" and (artifact is not None or not reasons):
        issues.append(f"{label}: unavailable evidence requires no artifact and a reason code")
    if state in {"invalid", "expired"} and (artifact is None or not reasons):
        issues.append(f"{label}: invalid/expired evidence requires artifact and reason code")
    created, observed = metadata["created_at_unix_ms"], spec["observed_at_unix_ms"]
    valid_from, valid_until = status["valid_from_unix_ms"], status["valid_until_unix_ms"]
    if type(observed) is int and type(created) is int and observed > created:
        issues.append(f"{label}: observed_at_unix_ms cannot exceed created_at_unix_ms")
    if type(observed) is int and type(valid_from) is int and valid_from < observed:
        issues.append(f"{label}: valid_from_unix_ms cannot precede observation")
    if state == "expired" and (type(valid_until) is not int or
                                type(created) is int and valid_until > created):
        issues.append(f"{label}: expired evidence must expire no later than creation")


def _validate_evidence_semantics(record: dict[str, object], label: str,
                                 issues: list[str]) -> None:
    spec = record["spec"]
    _locator_semantics(spec, label, issues)
    _evidence_origin_semantics(spec, label, issues)
    _evidence_state_semantics(record, label, issues)


def _claim_object_semantics(spec: dict[str, object], label: str, issues: list[str]) -> None:
    object_type, value = spec["object_type"], spec["object_value"]
    matches = {"boolean": type(value) is bool, "integer": type(value) is int,
               "null": value is None, "string": isinstance(value, str),
               "artifact_ref": isinstance(value, str) and ID_RE.fullmatch(value) is not None}
    if isinstance(object_type, str) and object_type in matches and not matches[object_type]:
        issues.append(f"{label}.spec.object_value: does not match object_type {object_type!r}")


def _claim_type_semantics(spec: dict[str, object], state: object, label: str,
                          issues: list[str]) -> None:
    claim_type = spec["claim_type"]
    if not isinstance(claim_type, str):
        return
    if claim_type in CLAIM_STATES and (not isinstance(state, str) or
                                       state not in CLAIM_STATES[claim_type]):
        issues.append(f"{label}.status.state: invalid for claim_type {claim_type!r}")
    elif claim_type in SHADOW_STATES and (not isinstance(state, str) or
                                          state not in SHADOW_STATES[claim_type]):
        issues.append(f"{label}.status.state: authoritative state is unavailable in shadow mode")
    confidence = spec["confidence_micros"]
    if claim_type in {"assumption", "hypothesis", "inference"}:
        integer(confidence, f"{label}.spec.confidence_micros", issues, 0, 1_000_000)
    elif confidence is not None:
        issues.append(f"{label}.spec.confidence_micros: must be null for this claim type")
    plan_required = claim_type in {"assumption", "hypothesis"}
    if plan_required != (spec["validation_plan"] is not None):
        issues.append(f"{label}.spec.validation_plan: {'required' if plan_required else 'must be null'}")
    if claim_type == "unknown":
        if not isinstance(spec["queue_ref"], str) or ID_RE.fullmatch(spec["queue_ref"]) is None:
            issues.append(f"{label}.spec.queue_ref: required bounded identifier for unknown claim")
    elif spec["queue_ref"] is not None:
        issues.append(f"{label}.spec.queue_ref: must be null outside unknown claims")


def _claim_support_semantics(spec: dict[str, object], label: str, issues: list[str]) -> None:
    support = spec["supporting_evidence_record_ids"]
    contradict = spec["contradicting_evidence_record_ids"]
    lists_are_strings = (isinstance(support, list) and isinstance(contradict, list) and
                         all(isinstance(item, str) for item in support + contradict))
    if lists_are_strings and set(support) & set(contradict):
        issues.append(f"{label}: supporting and contradicting evidence must be disjoint")
    claim_type = spec["claim_type"]
    if isinstance(claim_type, str) and claim_type in {"fact", "constraint", "lesson"} and not support:
        issues.append(f"{label}: {claim_type} requires supporting evidence")
    if claim_type == "inference" and not support and not spec["derived_from_claim_record_ids"]:
        issues.append(f"{label}: inference requires supporting evidence or a derived claim")
    if spec["decision_authority"] is not None:
        issues.append(f"{label}.spec.decision_authority: authority is unavailable in shadow mode")


def _claim_time_semantics(record: dict[str, object], label: str, issues: list[str]) -> None:
    spec, status, metadata = record["spec"], record["status"], record["metadata"]
    created, valid_from = metadata["created_at_unix_ms"], status["valid_from_unix_ms"]
    review, plan = spec["review_by_unix_ms"], spec["validation_plan"]
    if type(created) is int and type(valid_from) is int and valid_from < created:
        issues.append(f"{label}: claim valid_from_unix_ms cannot precede creation")
    if type(created) is int and type(review) is int and review < created:
        issues.append(f"{label}: review_by_unix_ms cannot precede creation")
    if isinstance(plan, dict) and type(created) is int and type(plan.get("due_at_unix_ms")) is int:
        if plan["due_at_unix_ms"] <= created:
            issues.append(f"{label}: validation plan due_at_unix_ms must follow creation")


def _validate_claim_semantics(record: dict[str, object], label: str,
                              issues: list[str]) -> None:
    spec = record["spec"]
    _claim_object_semantics(spec, label, issues)
    _claim_type_semantics(spec, record["status"]["state"], label, issues)
    _claim_support_semantics(spec, label, issues)
    _claim_time_semantics(record, label, issues)


def validate_record(record: dict[str, object], index: int, issues: list[str]) -> None:
    """Validate one strict record and its self digest."""
    label = f"records[{index}]"
    issue_count = len(issues)
    kind = validate_common(record, label, issues)
    if kind is None:
        return
    states = ({"valid", "invalid", "unavailable", "expired"} if kind == "EvidenceRecord"
              else set().union(*CLAIM_STATES.values()))
    status_ok = validate_status(record["status"], states, f"{label}.status", issues)
    spec_ok = (validate_evidence_shape if kind == "EvidenceRecord" else validate_claim_shape)(
        record, label, issues)
    if status_ok and spec_ok and len(issues) == issue_count:
        (_validate_evidence_semantics if kind == "EvidenceRecord" else
         _validate_claim_semantics)(record, label, issues)
    try:
        if len(canonical_json(record)) > MAX_RECORD_BYTES:
            issues.append(f"{label}: record exceeds {MAX_RECORD_BYTES} bytes")
        expected = compute_record_digest(record)
        actual = record["integrity"].get("canonical_sha256")
        if actual != expected:
            issues.append(f"{label}.integrity.canonical_sha256: digest mismatch")
    except (ContractError, AttributeError) as error:
        issues.append(f"{label}: cannot compute digest: {error}")
