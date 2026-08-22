"""Exact WorkIntent v1 member and cross-field validation."""

from __future__ import annotations

from .codec import ContractError, canonical_json
from .constants import (API_VERSION, ATTESTATION_FIELDS, CANONICALIZATION, FRESHNESS,
                        HASH_RE, IDENTIFIER_RE, KIND, MATERIALITY_LEVELS, MAX_ARTIFACTS,
                        MAX_I64, MAX_NARRATIVE_ITEMS, MAX_NARRATIVE_TOTAL,
                        MAX_RECORD_REFS, MAX_RECORD_REF_TOTAL, MAX_REFERENCE_BYTES,
                        MAX_SHORT_BYTES, MAX_STRING_BYTES, NARRATIVE_ARRAY_FIELDS,
                        ORIGIN_KINDS, PRINCIPAL_FIELDS, PRINCIPAL_TYPES,
                        SNAPSHOT_TYPES, STATUS, TOP_FIELDS, WORK_TYPES)


def _object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    if not all(isinstance(key, str) for key in value):
        raise ContractError(f"{label} object keys must be strings")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(f"{label} fields differ: missing={sorted(missing)}, "
                            f"unknown={sorted(unknown)}")
    return value


def _text(value: object, label: str, maximum: int) -> str:
    if not isinstance(value, str) or not value:
        raise ContractError(f"{label} must be non-empty UTF-8 text <= {maximum} bytes")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"{label} must be valid UTF-8 text") from error
    if len(encoded) > maximum:
        raise ContractError(f"{label} must be non-empty UTF-8 text <= {maximum} bytes")
    return value


def _hash(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must be a lowercase bare SHA-256")
    return value


def _unix_ms(value: object, label: str, nullable: bool = False) -> None:
    if nullable and value is None:
        return
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= MAX_I64:
        raise ContractError(f"{label} must be a nonnegative signed-int64 integer")


def _enum(value: object, allowed: set[str], label: str) -> None:
    if not isinstance(value, str) or value not in allowed:
        raise ContractError(f"{label} is unsupported")


def _principal(value: object, label: str) -> None:
    member = _object(value, PRINCIPAL_FIELDS, label)
    _text(member["authority_domain"], f"{label}.authority_domain", MAX_SHORT_BYTES)
    _text(member["principal_id"], f"{label}.principal_id", MAX_SHORT_BYTES)
    _enum(member["principal_type"], PRINCIPAL_TYPES, f"{label}.principal_type")


def _attestations(value: object) -> None:
    member = _object(value, ATTESTATION_FIELDS, "attestations")
    if any(item is not False for item in member.values()):
        raise ContractError("every WorkIntent attestation must be exactly false")


def _binding(value: object) -> None:
    member = _object(value, {"change_id", "project_id", "run_id"}, "binding")
    _text(member["change_id"], "binding.change_id", MAX_SHORT_BYTES)
    _text(member["project_id"], "binding.project_id", MAX_SHORT_BYTES)
    if member["run_id"] is not None:
        _text(member["run_id"], "binding.run_id", MAX_SHORT_BYTES)


def _narrative_list(value: object, label: str, nonempty: bool) -> list[str]:
    if not isinstance(value, list) or len(value) > MAX_NARRATIVE_ITEMS:
        raise ContractError(f"{label} must contain at most {MAX_NARRATIVE_ITEMS} items")
    if nonempty and not value:
        raise ContractError(f"{label} must be non-empty")
    result = [_text(item, f"{label}[{index}]", MAX_STRING_BYTES)
              for index, item in enumerate(value)]
    if len(result) != len(set(result)):
        raise ContractError(f"{label} must preserve unique authored entries")
    return result


def _intent(value: object) -> None:
    fields = {"deadline_unix_ms", "external_constraints", "goal", "non_goals",
              "open_questions", "scope", "success_signals", "work_type"}
    member = _object(value, fields, "intent")
    _unix_ms(member["deadline_unix_ms"], "intent.deadline_unix_ms", nullable=True)
    _text(member["goal"], "intent.goal", MAX_STRING_BYTES)
    total = 0
    for field in NARRATIVE_ARRAY_FIELDS:
        items = _narrative_list(member[field], f"intent.{field}",
                                field in {"scope", "success_signals"})
        total += len(items)
    if total > MAX_NARRATIVE_TOTAL:
        raise ContractError(f"intent narrative arrays exceed {MAX_NARRATIVE_TOTAL} total items")
    _enum(member["work_type"], WORK_TYPES, "intent.work_type")


def _materiality(value: object) -> None:
    member = _object(value, {"basis", "level"}, "materiality")
    if member["basis"] != "caller_declaration_only":
        raise ContractError("materiality.basis must be caller_declaration_only")
    _enum(member["level"], MATERIALITY_LEVELS, "materiality.level")


def _origin(value: object) -> None:
    member = _object(value, {"origin_kind", "origin_ref"}, "origin")
    _enum(member["origin_kind"], ORIGIN_KINDS, "origin.origin_kind")
    if member["origin_ref"] is not None:
        _text(member["origin_ref"], "origin.origin_ref", MAX_REFERENCE_BYTES)


def _record_refs(value: object, label: str) -> list[dict[str, object]]:
    fields = {"canonical_sha256", "record_id"}
    if not isinstance(value, list) or len(value) > MAX_RECORD_REFS:
        raise ContractError(f"{label} must contain at most {MAX_RECORD_REFS} items")
    result: list[dict[str, object]] = []
    for index, item in enumerate(value):
        member = _object(item, fields, f"{label}[{index}]")
        record_id = member["record_id"]
        if not isinstance(record_id, str) or IDENTIFIER_RE.fullmatch(record_id) is None:
            raise ContractError(
                f"{label}[{index}].record_id must match the ADR-0045 identifier grammar")
        _hash(member["canonical_sha256"], f"{label}[{index}].canonical_sha256")
        result.append(member)
    keys = [item["record_id"].encode("utf-8") for item in result]
    if keys != sorted(keys) or len(keys) != len(set(keys)):
        raise ContractError(f"{label} must be strictly sorted and unique by record_id UTF-8")
    return result


def _artifacts(value: object) -> None:
    fields = {"artifact_kind", "artifact_ref", "artifact_sha256"}
    if not isinstance(value, list) or len(value) > MAX_ARTIFACTS:
        raise ContractError(f"local_artifact_declarations must contain <= {MAX_ARTIFACTS} items")
    members, pairs = [], set()
    for index, item in enumerate(value):
        member = _object(item, fields, f"local_artifact_declarations[{index}]")
        kind = _text(member["artifact_kind"], "artifact_kind", MAX_SHORT_BYTES)
        ref = _text(member["artifact_ref"], "artifact_ref", MAX_REFERENCE_BYTES)
        _hash(member["artifact_sha256"], "artifact_sha256")
        if (kind, ref) in pairs:
            raise ContractError("artifact kind/ref pairs must be unique")
        pairs.add((kind, ref))
        members.append(member)
    encoded = [canonical_json(member) for member in members]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError("artifacts must be strictly sorted and unique by canonical bytes")


def _snapshot(value: object) -> None:
    if value is None:
        return
    member = _object(value, {"snapshot_id", "snapshot_sha256", "snapshot_type"},
                     "local_source_snapshot_declaration")
    if not isinstance(member["snapshot_id"], str) or IDENTIFIER_RE.fullmatch(
            member["snapshot_id"]) is None:
        raise ContractError("snapshot_id must match the ADR-0045 identifier grammar")
    _hash(member["snapshot_sha256"], "snapshot_sha256")
    _enum(member["snapshot_type"], SNAPSHOT_TYPES, "snapshot_type")


def _references(value: object) -> None:
    fields = {"claim_record_refs", "evidence_record_refs",
              "local_artifact_declarations", "local_source_snapshot_declaration"}
    member = _object(value, fields, "references")
    claims = _record_refs(member["claim_record_refs"], "claim_record_refs")
    evidence = _record_refs(member["evidence_record_refs"], "evidence_record_refs")
    if len(claims) + len(evidence) > MAX_RECORD_REF_TOTAL:
        raise ContractError(
            f"claim and evidence record refs exceed {MAX_RECORD_REF_TOTAL} total items")
    claim_ids = {item["record_id"] for item in claims}
    if claim_ids & {item["record_id"] for item in evidence}:
        raise ContractError("claim and evidence record IDs must be mutually disjoint")
    _artifacts(member["local_artifact_declarations"])
    _snapshot(member["local_source_snapshot_declaration"])


def validate_shape(value: object, *, allow_blank_identity: bool = False) -> dict[str, object]:
    """Validate exact wire shape and bounded, authority-neutral declarations."""
    record = _object(value, TOP_FIELDS, "WorkIntent")
    expected = {"api_version": API_VERSION, "canonicalization": CANONICALIZATION,
                "freshness": FRESHNESS, "kind": KIND, "status": STATUS}
    for field, required in expected.items():
        if record[field] != required:
            raise ContractError(f"{field} must be {required!r}")
    _attestations(record["attestations"])
    _binding(record["binding"])
    _unix_ms(record["declared_at_unix_ms"], "declared_at_unix_ms")
    if record["declared_owner"] is not None:
        _principal(record["declared_owner"], "declared_owner")
    _principal(record["requester"], "requester")
    _intent(record["intent"])
    _materiality(record["materiality"])
    _origin(record["origin"])
    _references(record["references"])
    if allow_blank_identity and record["work_intent_id"] == record["work_intent_sha256"] == "":
        return record
    _hash(record["work_intent_sha256"], "work_intent_sha256")
    expected_id = f"work-intent-{record['work_intent_sha256']}"
    if record["work_intent_id"] != expected_id:
        raise ContractError("work_intent_id must bind the declared digest")
    return record
