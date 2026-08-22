"""Exact reachable ADR-0045 record closure for proposed Claim mutations."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError


def record_index(records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for record in records:
        metadata = record.get("metadata")
        record_id = metadata.get("record_id") if isinstance(metadata, dict) else None
        if not isinstance(record_id, str) or record_id in result:
            raise ContractError("records require unique validated record_id values")
        result[record_id] = record
    return result


def _references(record: dict[str, Any]) -> list[str]:
    metadata = record["metadata"]
    refs = list(metadata["supersedes_record_ids"])
    if record["kind"] == "KnowledgeClaim":
        spec = record["spec"]
        refs.extend(spec["supporting_evidence_record_ids"])
        refs.extend(spec["contradicting_evidence_record_ids"])
        refs.extend(spec["derived_from_claim_record_ids"])
    return refs


def validate_exact_closure(records: list[dict[str, Any]], after_records: list[dict[str, Any]],
                           by_id: dict[str, dict[str, Any]]) -> None:
    reachable: set[str] = set()
    pending = [record["metadata"]["record_id"] for record in after_records]
    work = 0
    while pending:
        record_id = pending.pop()
        if record_id in reachable:
            continue
        record = by_id.get(record_id)
        if record is None:
            raise ContractError(f"record closure references unknown record {record_id!r}")
        reachable.add(record_id)
        pending.extend(_references(record))
        work += 1
        if work > len(records):
            raise ContractError("record closure traversal exceeded the bounded record set")
    actual = {record["metadata"]["record_id"] for record in records}
    if reachable != actual:
        orphaned = sorted(actual - reachable)
        raise ContractError(f"records must be the exact reachable closure; orphaned={orphaned!r}")
