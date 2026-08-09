"""Strict member and primitive validation matching the published JSON Schema."""

from __future__ import annotations

from .constants import (CLAIM_STATES, EVIDENCE_TYPES, HASH_RE, ID_RE, KINDS,
                        LOCATOR_BY_TYPE, MAX_I64, MAX_ITEMS, METADATA_FIELDS, MIN_I64,
                        PRINCIPAL_FIELDS, PRINCIPAL_TYPES, STATUS_FIELDS, TOP_FIELDS)


def object_shape(value: object, fields: set[str], label: str, issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def identifier(value: object, label: str, issues: list[str], *, nullable: bool = False) -> None:
    if nullable and value is None:
        return
    if not isinstance(value, str) or ID_RE.fullmatch(value) is None:
        issues.append(f"{label}: expected bounded identifier")


def text(value: object, label: str, issues: list[str], maximum: int = 4096) -> None:
    if not isinstance(value, str) or not value or len(value) > maximum:
        issues.append(f"{label}: expected non-empty string <= {maximum} characters")


def integer(value: object, label: str, issues: list[str], minimum: int = MIN_I64,
            maximum: int = MAX_I64, *, nullable: bool = False) -> None:
    if nullable and value is None:
        return
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        issues.append(f"{label}: expected integer in [{minimum}, {maximum}]")


def digest(value: object, label: str, issues: list[str], *, nullable: bool = False) -> None:
    if nullable and value is None:
        return

    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        issues.append(f"{label}: expected lowercase bare SHA-256")


def enum(value: object, allowed: set[str], label: str, issues: list[str]) -> None:
    if not isinstance(value, str) or value not in allowed:
        issues.append(f"{label}: expected one of {sorted(allowed)}")


def id_list(value: object, label: str, issues: list[str], *, nonempty: bool = False) -> None:
    if not isinstance(value, list) or len(value) > MAX_ITEMS or (nonempty and not value):
        issues.append(f"{label}: expected {'non-empty ' if nonempty else ''}list <= {MAX_ITEMS}")
        return
    for index, item in enumerate(value):
        identifier(item, f"{label}[{index}]", issues)
    if all(isinstance(item, str) for item in value) and value != sorted(set(value)):
        issues.append(f"{label}: must be sorted and unique")


def _validate_principal(value: object, label: str, issues: list[str]) -> None:
    if not object_shape(value, PRINCIPAL_FIELDS, label, issues):
        return
    enum(value["principal_type"], PRINCIPAL_TYPES, f"{label}.principal_type", issues)
    for field in PRINCIPAL_FIELDS - {"principal_type"}:
        identifier(value[field], f"{label}.{field}", issues)


def _validate_metadata(value: object, label: str, issues: list[str]) -> None:
    if not object_shape(value, METADATA_FIELDS, label, issues):
        return
    for field in {"aggregate_id", "project_id", "record_id", "scope", "source_revision"}:
        identifier(value[field], f"{label}.{field}", issues)
    for field in {"context_sha256", "policy_sha256", "source_tree_sha256"}:
        digest(value[field], f"{label}.{field}", issues)
    integer(value["created_at_unix_ms"], f"{label}.created_at_unix_ms", issues, 0)
    integer(value["sequence"], f"{label}.sequence", issues, 1)
    id_list(value["supersedes_record_ids"], f"{label}.supersedes_record_ids", issues)
    _validate_principal(value["created_by"], f"{label}.created_by", issues)


def validate_status(value: object, allowed: set[str], label: str, issues: list[str]) -> bool:
    if not object_shape(value, STATUS_FIELDS, label, issues):
        return False
    enum(value["state"], allowed, f"{label}.state", issues)
    id_list(value["reason_codes"], f"{label}.reason_codes", issues)
    integer(value["valid_from_unix_ms"], f"{label}.valid_from_unix_ms", issues, 0)
    integer(value["valid_until_unix_ms"], f"{label}.valid_until_unix_ms", issues, 0,
            nullable=True)
    start, end = value["valid_from_unix_ms"], value["valid_until_unix_ms"]
    if type(start) is int and type(end) is int and end <= start:
        issues.append(f"{label}: valid_until_unix_ms must be greater than valid_from_unix_ms")
    return True


def validate_common(record: dict[str, object], label: str, issues: list[str]) -> str | None:
    if not object_shape(record, TOP_FIELDS, label, issues):
        return None
    if record["api_version"] != "forgeos.governance/v1":
        issues.append(f"{label}.api_version: unsupported version")
    enum(record["kind"], KINDS, f"{label}.kind", issues)
    _validate_metadata(record["metadata"], f"{label}.metadata", issues)
    integrity = record["integrity"]
    fields = {"canonical_sha256", "canonicalization"}
    if object_shape(integrity, fields, f"{label}.integrity", issues):
        digest(integrity["canonical_sha256"], f"{label}.integrity.canonical_sha256", issues)
        if integrity["canonicalization"] != "forgeos.canonical-json/v1":
            issues.append(f"{label}.integrity.canonicalization: unsupported format")
    return record["kind"] if isinstance(record["kind"], str) and record["kind"] in KINDS else None


def _validate_collector(value: object, label: str, issues: list[str]) -> None:
    fields = {"collector_id", "collector_type", "collector_version", "parameters_sha256", "run_id"}
    if not object_shape(value, fields, label, issues):
        return
    enum(value["collector_type"], {"human", "operator", "service", "tool"},
         f"{label}.collector_type", issues)
    for field in {"collector_id", "collector_version", "run_id"}:
        identifier(value[field], f"{label}.{field}", issues)
    digest(value["parameters_sha256"], f"{label}.parameters_sha256", issues)


def _validate_locator(value: object, label: str, issues: list[str]) -> None:
    fields = {"content_sha256", "exit_code", "line_end", "line_start", "locator_ref", "locator_type"}
    if not object_shape(value, fields, label, issues):
        return
    digest(value["content_sha256"], f"{label}.content_sha256", issues)
    text(value["locator_ref"], f"{label}.locator_ref", issues)
    enum(value["locator_type"], set(LOCATOR_BY_TYPE.values()), f"{label}.locator_type", issues)
    integer(value["exit_code"], f"{label}.exit_code", issues, nullable=True)
    for field in {"line_start", "line_end"}:
        integer(value[field], f"{label}.{field}", issues, 1, nullable=True)
    start, end = value["line_start"], value["line_end"]
    if (start is None) != (end is None) or (type(start) is int and type(end) is int and end < start):
        issues.append(f"{label}: line_start and line_end must be a valid pair")


def _validate_snapshot(value: object, label: str, issues: list[str]) -> None:
    fields = {"snapshot_id", "snapshot_sha256", "snapshot_type"}
    if not object_shape(value, fields, label, issues):
        return
    identifier(value["snapshot_id"], f"{label}.snapshot_id", issues)
    digest(value["snapshot_sha256"], f"{label}.snapshot_sha256", issues)
    enum(value["snapshot_type"], {"artifact", "external", "repository", "runtime"},
         f"{label}.snapshot_type", issues)


def validate_evidence_shape(record: dict[str, object], label: str, issues: list[str]) -> bool:
    fields = {"artifact_sha256", "collector", "content_role", "directness", "evidence_type",
              "locator", "observed_at_unix_ms", "sensitivity", "source_snapshot",
              "source_trust", "subjects"}
    spec = record["spec"]
    if not object_shape(spec, fields, f"{label}.spec", issues):
        return False
    digest(spec["artifact_sha256"], f"{label}.spec.artifact_sha256", issues, nullable=True)
    _validate_collector(spec["collector"], f"{label}.spec.collector", issues)
    _validate_locator(spec["locator"], f"{label}.spec.locator", issues)
    _validate_snapshot(spec["source_snapshot"], f"{label}.spec.source_snapshot", issues)
    id_list(spec["subjects"], f"{label}.spec.subjects", issues, nonempty=True)
    integer(spec["observed_at_unix_ms"], f"{label}.spec.observed_at_unix_ms", issues, 0)
    enum(spec["evidence_type"], EVIDENCE_TYPES, f"{label}.spec.evidence_type", issues)
    enum(spec["directness"], {"attested", "derived", "direct"}, f"{label}.spec.directness", issues)
    enum(spec["sensitivity"], {"public", "internal", "confidential", "restricted"},
         f"{label}.spec.sensitivity", issues)
    enum(spec["source_trust"], {"untrusted", "observed", "controlled", "authoritative"},
         f"{label}.spec.source_trust", issues)
    enum(spec["content_role"], {"untrusted_data", "trusted_control"},
         f"{label}.spec.content_role", issues)
    return True


def _validate_plan(value: object, label: str, issues: list[str]) -> None:
    if value is None:
        return
    fields = {"due_at_unix_ms", "impact_if_false", "method", "owner_id", "required_evidence_types"}
    if not object_shape(value, fields, label, issues):
        return
    integer(value["due_at_unix_ms"], f"{label}.due_at_unix_ms", issues, 0)
    text(value["impact_if_false"], f"{label}.impact_if_false", issues)
    text(value["method"], f"{label}.method", issues)
    identifier(value["owner_id"], f"{label}.owner_id", issues)
    id_list(value["required_evidence_types"], f"{label}.required_evidence_types", issues,
            nonempty=True)
    if (isinstance(value["required_evidence_types"], list) and
            all(isinstance(item, str) for item in value["required_evidence_types"])):
        unknown = set(value["required_evidence_types"]) - EVIDENCE_TYPES
        if unknown:
            issues.append(f"{label}.required_evidence_types: unknown values {sorted(unknown)}")


def _validate_claim_nested(spec: dict[str, object], label: str, issues: list[str]) -> None:
    owner = spec["owner"]
    if object_shape(owner, {"principal_id", "principal_type"}, f"{label}.spec.owner", issues):
        identifier(owner["principal_id"], f"{label}.spec.owner.principal_id", issues)
        enum(owner["principal_type"], PRINCIPAL_TYPES, f"{label}.spec.owner.principal_type", issues)
    authority = spec["decision_authority"]
    if authority is not None and object_shape(authority, {"adr_ref", "approval_ref"},
                                              f"{label}.spec.decision_authority", issues):
        identifier(authority["adr_ref"], f"{label}.spec.decision_authority.adr_ref", issues)
        identifier(authority["approval_ref"], f"{label}.spec.decision_authority.approval_ref", issues)
    _validate_plan(spec["validation_plan"], f"{label}.spec.validation_plan", issues)


def validate_claim_shape(record: dict[str, object], label: str, issues: list[str]) -> bool:
    fields = {"claim_type", "confidence_micros", "contradicting_evidence_record_ids",
              "decision_authority", "derived_from_claim_record_ids", "object_type",
              "object_value", "owner", "predicate", "queue_ref", "reasoning",
              "review_by_unix_ms", "subject", "supporting_evidence_record_ids", "validation_plan"}
    spec = record["spec"]
    if not object_shape(spec, fields, f"{label}.spec", issues):
        return False
    enum(spec["claim_type"], set(CLAIM_STATES), f"{label}.spec.claim_type", issues)
    for field in {"subject", "predicate"}:
        identifier(spec[field], f"{label}.spec.{field}", issues)
    text(spec["reasoning"], f"{label}.spec.reasoning", issues)
    integer(spec["review_by_unix_ms"], f"{label}.spec.review_by_unix_ms", issues, 0,
            nullable=True)
    for field in {"supporting_evidence_record_ids", "contradicting_evidence_record_ids",
                  "derived_from_claim_record_ids"}:
        id_list(spec[field], f"{label}.spec.{field}", issues)
    enum(spec["object_type"], {"artifact_ref", "boolean", "integer", "null", "string"},
         f"{label}.spec.object_type", issues)
    object_value = spec["object_value"]
    if isinstance(object_value, str) and len(object_value) > 16_384:
        issues.append(f"{label}.spec.object_value: exceeds 16384 characters")
    _validate_claim_nested(spec, label, issues)
    return True
