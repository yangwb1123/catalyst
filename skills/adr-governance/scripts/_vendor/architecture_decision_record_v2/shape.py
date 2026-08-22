"""Exact proposed-only ArchitectureDecisionRecord v2 shape and semantics."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import validate_text
from .constants import (
    ADR_ID_RE, ALTERNATIVE_FIELDS, API_VERSION, CANONICALIZATION,
    DOCUMENT_NAME_RE, GRAPH_NODE_RE, HASH_RE, IDENTIFIER_RE,
    IMPLEMENTATION_REF_RE, KIND, MAX_ARRAY_ITEMS, MAX_DOCUMENT_NAME_BYTES, MAX_ID_BYTES,
    MAX_IMPLEMENTATION_REF_BYTES, MAX_I64, MAX_LINE_NUMBER, MAX_TITLE_BYTES,
    NARRATIVE_FIELDS, REQUIRED_SET_FIELDS, REVISIT_FIELDS, RISK_FIELDS,
    SET_FIELDS, STATUS, TOP_FIELDS, VALIDATION_FIELDS,
)


def _exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(f"{label} fields differ; unknown={sorted(unknown)}, missing={sorted(missing)}")
    return value


def _identifier(value: object, label: str) -> str:
    if not isinstance(value, str) or IDENTIFIER_RE.fullmatch(value) is None:
        raise ContractError(f"{label} must match the ADR-0045 bounded identifier grammar")
    if len(value.encode("utf-8")) > MAX_ID_BYTES:
        raise ContractError(f"{label} exceeds {MAX_ID_BYTES} UTF-8 bytes")
    return value


def _sorted_unique(value: object, label: str, *, nonempty: bool = False) -> list[str]:
    if not isinstance(value, list) or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError(f"{label} must be an array with at most {MAX_ARRAY_ITEMS} items")
    if nonempty and not value:
        raise ContractError(f"{label} must not be empty")
    if not all(isinstance(item, str) for item in value):
        raise ContractError(f"{label} entries must be strings")
    if value != sorted(set(value), key=lambda item: item.encode("utf-8")):
        raise ContractError(f"{label} must be raw-UTF-8 sorted and unique")
    return value


def _validate_ref_sets(metadata: dict[str, object]) -> None:
    for field in SET_FIELDS:
        values = _sorted_unique(metadata[field], field, nonempty=field in REQUIRED_SET_FIELDS)
        if field == "affected_node_ids":
            pattern = GRAPH_NODE_RE
        elif field in {"superseded_by", "supersedes"}:
            pattern = ADR_ID_RE
        elif field == "implementation_refs":
            pattern = IMPLEMENTATION_REF_RE
        else:
            pattern = IDENTIFIER_RE
        for index, value in enumerate(values):
            if pattern.fullmatch(value) is None:
                raise ContractError(f"{field}[{index}] has an invalid reference")
            if field == "implementation_refs":
                _validate_implementation_ref(value, f"{field}[{index}]")


def _validate_implementation_ref(value: str, label: str) -> None:
    path = value.split("#", 1)[0]
    if path.startswith("/") or "\\" in path:
        raise ContractError(f"{label} must be a canonical repository-relative path")
    if any(segment in {"", ".", ".."} for segment in path.split("/")):
        raise ContractError(f"{label} contains a non-canonical path segment")
    if path.split("/", 1)[0] in {".git", ".forge"}:
        raise ContractError(f"{label} targets forbidden local control state")
    if len(value.encode("utf-8")) > MAX_IMPLEMENTATION_REF_BYTES:
        raise ContractError(f"{label} exceeds {MAX_IMPLEMENTATION_REF_BYTES} UTF-8 bytes")
    if "#L" in value and int(value.rsplit("#L", 1)[1]) > MAX_LINE_NUMBER:
        raise ContractError(f"{label} line number exceeds signed int32")


def _validate_narrative_list(value: object, label: str) -> None:
    if not isinstance(value, list) or not value or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError(f"{label} must be a non-empty bounded narrative array")
    for index, item in enumerate(value):
        validate_text(item, f"{label}[{index}]")


def _validate_alternatives(value: object) -> None:
    if not isinstance(value, list) or not value or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError("alternatives must be a non-empty bounded array")
    ids, dispositions = [], set()
    for index, item in enumerate(value):
        node = _exact_object(item, ALTERNATIVE_FIELDS, f"alternatives[{index}]")
        ids.append(_identifier(node["alternative_id"], f"alternatives[{index}].alternative_id"))
        disposition = node["disposition"]
        if disposition not in {"candidate", "rejected"}:
            raise ContractError(f"alternatives[{index}].disposition is invalid")
        dispositions.add(disposition)
        for field in {"description", "rationale"}:
            validate_text(node[field], f"alternatives[{index}].{field}")
    _require_sorted_ids(ids, "alternatives")
    if dispositions != {"candidate", "rejected"}:
        raise ContractError("alternatives require at least one candidate and one rejected entry")


def _require_sorted_ids(ids: list[str], label: str) -> None:
    if ids != sorted(set(ids), key=lambda item: item.encode("utf-8")):
        raise ContractError(f"{label} must be sorted and unique by identifier")


def _validate_risks(value: object) -> None:
    if not isinstance(value, list) or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError("risks must be a bounded array")
    ids = []
    for index, item in enumerate(value):
        node = _exact_object(item, RISK_FIELDS, f"risks[{index}]")
        ids.append(_identifier(node["risk_id"], f"risks[{index}].risk_id"))
        for field in {"description", "mitigation"}:
            validate_text(node[field], f"risks[{index}].{field}")
    _require_sorted_ids(ids, "risks")


def _validate_validation(value: object, owner_refs: list[str]) -> None:
    if not isinstance(value, list) or not value or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError("validation_plan must be a non-empty bounded array")
    ids = []
    for index, item in enumerate(value):
        node = _exact_object(item, VALIDATION_FIELDS, f"validation_plan[{index}]")
        ids.append(_identifier(node["validation_id"], f"validation_plan[{index}].validation_id"))
        owner = _identifier(node["owner_ref"], f"validation_plan[{index}].owner_ref")
        if owner not in owner_refs:
            raise ContractError(f"validation_plan[{index}].owner_ref is not a declared owner")
        for field in {"description", "due_trigger", "success_criteria"}:
            validate_text(node[field], f"validation_plan[{index}].{field}")
        _validate_narrative_list(
            node["evidence_required"], f"validation_plan[{index}].evidence_required")
    _require_sorted_ids(ids, "validation_plan")


def _validate_revisit(value: object) -> None:
    if not isinstance(value, list) or not value or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError("revisit_triggers must be a non-empty bounded array")
    ids = []
    for index, item in enumerate(value):
        node = _exact_object(item, REVISIT_FIELDS, f"revisit_triggers[{index}]")
        ids.append(_identifier(node["trigger_id"], f"revisit_triggers[{index}].trigger_id"))
        for field in {"condition"}:
            validate_text(node[field], f"revisit_triggers[{index}].{field}")
        _validate_narrative_list(
            node["evidence_required"], f"revisit_triggers[{index}].evidence_required")
    _require_sorted_ids(ids, "revisit_triggers")


def _validate_consequences(value: object) -> None:
    if not isinstance(value, list) or not value or len(value) > MAX_ARRAY_ITEMS:
        raise ContractError("consequences must be a non-empty bounded narrative array")
    for index, item in enumerate(value):
        validate_text(item, f"consequences[{index}]")


def _validate_identity(metadata: dict[str, object], document_name: str) -> None:
    if metadata["document_name"] != document_name:
        raise ContractError("document_name does not equal the physical basename")
    if len(document_name.encode("utf-8")) > MAX_DOCUMENT_NAME_BYTES:
        raise ContractError(f"document_name exceeds {MAX_DOCUMENT_NAME_BYTES} UTF-8 bytes")
    match = DOCUMENT_NAME_RE.fullmatch(document_name)
    if match is None or match.group(1) == "0000":
        raise ContractError("document filename is not canonical ADR v2 syntax")
    expected_adr = f"ADR-{match.group(1)}"
    if metadata["adr_id"] != expected_adr:
        raise ContractError("adr_id does not match the document filename sequence")
    if not isinstance(metadata["adr_id"], str) or ADR_ID_RE.fullmatch(metadata["adr_id"]) is None:
        raise ContractError("adr_id must be ADR-NNNN")


def _validate_constants(metadata: dict[str, object]) -> None:
    expected = {
        "api_version": API_VERSION, "canonicalization": CANONICALIZATION,
        "kind": KIND, "status": STATUS, "accepted_at_unix_ms": None,
        "acceptance_id": None, "superseded_by": [],
    }
    for field, value in expected.items():
        if metadata[field] != value:
            raise ContractError(f"{field} must equal the proposed-only v2 constant {value!r}")
    timestamp = metadata["proposed_at_unix_ms"]
    if type(timestamp) is not int or not 0 <= timestamp <= MAX_I64:
        raise ContractError("proposed_at_unix_ms must be a non-negative signed int64")
    expires = metadata["expires_at_unix_ms"]
    if expires is not None and (type(expires) is not int or
                                not timestamp < expires <= MAX_I64):
        raise ContractError("expires_at_unix_ms must be null or later than proposed_at_unix_ms")


def validate_metadata(metadata: object, document_name: str) -> dict[str, object]:
    node = _exact_object(metadata, TOP_FIELDS, "frontmatter")
    _validate_constants(node)
    _validate_identity(node, document_name)
    validate_text(node["title"], "title", MAX_TITLE_BYTES)
    for field in NARRATIVE_FIELDS:
        validate_text(node[field], field)
    for field in {"body_sha256", "self_sha256"}:
        if not isinstance(node[field], str) or HASH_RE.fullmatch(node[field]) is None:
            raise ContractError(f"{field} must be a lowercase bare SHA-256")
    _validate_ref_sets(node)
    if node["adr_id"] in node["supersedes"]:
        raise ContractError("supersedes cannot contain the current adr_id")
    _validate_alternatives(node["alternatives"])
    _validate_consequences(node["consequences"])
    _validate_risks(node["risks"])
    _validate_validation(node["validation_plan"], node["owner_refs"])
    _validate_revisit(node["revisit_triggers"])
    return node
