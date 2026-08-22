"""KnowledgeClaim create/supersede declarations and shadow lifecycle continuity."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError
from .constants import MAX_MUTATIONS, SHADOW_TRANSITIONS, STABLE_CLAIM_FIELDS
from .shape import (array, enum, identifier, reasons, require_keys, text,
                    validate_claim_ref)

MUTATION_FIELDS = {
    "after_claim_ref", "before_claim_ref", "operation", "rationale", "reason_codes",
    "target_aggregate_id", "target_kind",
}


def _metadata(record: dict[str, Any], label: str) -> dict[str, Any]:
    value = record.get("metadata")
    if not isinstance(value, dict):
        raise ContractError(f"{label}.metadata must be an object")
    return value


def _claim(record: Any, label: str) -> dict[str, Any]:
    if not isinstance(record, dict) or record.get("kind") != "KnowledgeClaim":
        raise ContractError(f"{label} must reference a KnowledgeClaim")
    return record


def _ref_matches(reference: dict[str, Any], record: dict[str, Any], label: str) -> None:
    metadata, integrity = record["metadata"], record.get("integrity")
    if not isinstance(integrity, dict):
        raise ContractError(f"{label} referenced record integrity is malformed")
    if (reference["record_id"] != metadata.get("record_id") or
            reference["canonical_sha256"] != integrity.get("canonical_sha256")):
        raise ContractError(f"{label} does not bind the exact referenced record")


def _validate_create(after: dict[str, Any], mutation: dict[str, Any], label: str) -> None:
    metadata = _metadata(after, f"{label}.after")
    if mutation["before_claim_ref"] is not None:
        raise ContractError(f"{label} create requires null before_claim_ref")
    if metadata.get("sequence") != 1 or metadata.get("supersedes_record_ids") != []:
        raise ContractError(f"{label} create requires sequence 1 with no supersedes")


def _stable_identity(record: dict[str, Any]) -> tuple[Any, ...]:
    metadata, spec = record["metadata"], record["spec"]
    return (
        record["kind"], metadata["aggregate_id"], metadata["project_id"],
        metadata["scope"], *(spec[field] for field in STABLE_CLAIM_FIELDS),
    )


def _validate_supersede(after: dict[str, Any], before: dict[str, Any],
                        mutation: dict[str, Any], label: str) -> None:
    after_meta, before_meta = _metadata(after, f"{label}.after"), _metadata(
        before, f"{label}.before")
    if _stable_identity(after) != _stable_identity(before):
        raise ContractError(f"{label} supersede changes ADR-0054 stable semantic identity")
    if after_meta.get("sequence") != before_meta.get("sequence") + 1:
        raise ContractError(f"{label} supersede must name the immediate predecessor")
    prior_ids = after_meta.get("supersedes_record_ids")
    if (not isinstance(prior_ids, list) or
            before_meta.get("record_id") not in prior_ids):
        raise ContractError(f"{label} after Claim must include its immediate predecessor")
    created_after, created_before = (after_meta.get("created_at_unix_ms"),
                                     before_meta.get("created_at_unix_ms"))
    if type(created_after) is not int or type(created_before) is not int or created_after < created_before:
        raise ContractError(f"{label} successor creation time cannot move backwards")
    claim_type = after["spec"].get("claim_type")
    transition = (before["status"].get("state"), after["status"].get("state"))
    if transition not in SHADOW_TRANSITIONS.get(claim_type, set()):
        raise ContractError(f"{label} state change is outside ADR-0054 shadow lifecycle")
    before_ref = mutation["before_claim_ref"]
    if before_ref is None:
        raise ContractError(f"{label} supersede requires before_claim_ref")
    _ref_matches(before_ref, before, f"{label}.before_claim_ref")


def validate_mutation_shapes(value: Any, label: str = "mutations") -> list[dict[str, Any]]:
    nodes = array(value, label, 1, MAX_MUTATIONS)
    after_ids: set[str] = set()
    before_ids: set[str] = set()
    for index, value in enumerate(nodes):
        item_label = f"{label}[{index}]"
        node = require_keys(value, item_label, MUTATION_FIELDS)
        after = validate_claim_ref(node["after_claim_ref"],
                                   f"{item_label}.after_claim_ref")
        before = validate_claim_ref(node["before_claim_ref"],
                                    f"{item_label}.before_claim_ref", True)
        enum(node["operation"], f"{item_label}.operation", ("create", "supersede"))
        if ((node["operation"] == "create") != (before is None)):
            raise ContractError(
                f"{item_label} operation and before_claim_ref disagree")
        if after["record_id"] in after_ids:
            raise ContractError(f"{item_label}.after_claim_ref is reused")
        after_ids.add(after["record_id"])
        if before is not None:
            if before["record_id"] == after["record_id"]:
                raise ContractError(
                    f"{item_label} before_claim_ref cannot equal after_claim_ref")
            if before["record_id"] in before_ids:
                raise ContractError(f"{item_label}.before_claim_ref forks across mutations")
            before_ids.add(before["record_id"])
        text(node["rationale"], f"{item_label}.rationale", 4096)
        reasons(node["reason_codes"], f"{item_label}.reason_codes", 1)
        identifier(node["target_aggregate_id"], f"{item_label}.target_aggregate_id")
        if node["target_kind"] != "KnowledgeClaim":
            raise ContractError(f"{item_label}.target_kind must be KnowledgeClaim")
    aggregate_ids = [node["target_aggregate_id"] for node in nodes]
    if any(left.encode() >= right.encode()
           for left, right in zip(aggregate_ids, aggregate_ids[1:])):
        raise ContractError(
            f"{label} must be strictly target_aggregate_id UTF-8 sorted and unique")
    overlap = after_ids.intersection(before_ids)
    if overlap:
        raise ContractError(
            f"{label} after_claim_ref and before_claim_ref sets must be disjoint")
    return nodes


def validate_lifecycle(mutations: list[dict[str, Any]],
                       by_id: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    """Bind every mutation to exact Claims and reject forks or lifecycle widening."""
    seen_after: set[str] = set()
    consumed_before: set[str] = set()
    seen_positions: set[tuple[str, int]] = set()
    after_records = []
    for index, mutation in enumerate(mutations):
        label = f"mutations[{index}]"
        after_ref = mutation["after_claim_ref"]
        after_id = after_ref["record_id"]
        after = _claim(by_id.get(after_id), f"{label}.after_claim_ref")
        _ref_matches(after_ref, after, f"{label}.after_claim_ref")
        metadata = after["metadata"]
        if mutation["target_aggregate_id"] != metadata["aggregate_id"]:
            raise ContractError(f"{label}.target_aggregate_id does not match after Claim")
        position = (metadata["aggregate_id"], metadata["sequence"])
        if after_id in seen_after or position in seen_positions:
            raise ContractError("mutations contain duplicate after Claims or aggregate positions")
        seen_after.add(after_id)
        seen_positions.add(position)
        if mutation["operation"] == "create":
            _validate_create(after, mutation, label)
        else:
            before_ref = mutation["before_claim_ref"]
            before_id = before_ref["record_id"] if isinstance(before_ref, dict) else ""
            before = _claim(by_id.get(before_id), f"{label}.before_claim_ref")
            if before_id in consumed_before:
                raise ContractError("mutations declare a supersession fork")
            consumed_before.add(before_id)
            _validate_supersede(after, before, mutation, label)
        after_records.append(after)
    return after_records
