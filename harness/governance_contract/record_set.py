"""Record-set identity, reference-closure and supersession validation."""

from __future__ import annotations

from .codec import ContractError, canonical_json, decode_record_set
from .constants import MAX_ITEMS, MAX_SET_BYTES
from .semantics import validate_record


def _record_index(records: list[dict[str, object]], issues: list[str]) -> dict[str, dict[str, object]]:
    by_id: dict[str, dict[str, object]] = {}
    identities: set[tuple[object, object, object]] = set()
    record_ids = []
    for record in records:
        metadata = record.get("metadata")
        record_ids.append(metadata.get("record_id") if isinstance(metadata, dict) else None)
    if not all(isinstance(item, str) for item in record_ids) or record_ids != sorted(record_ids):
        issues.append("record set must be sorted by record_id")
    for index, record in enumerate(records):
        metadata = record.get("metadata")
        if not isinstance(metadata, dict):
            continue
        record_id = metadata.get("record_id")
        identity = (record.get("kind"), metadata.get("aggregate_id"), metadata.get("sequence"))
        if isinstance(record_id, str) and record_id in by_id:
            issues.append(f"records[{index}]: duplicate record_id {record_id!r}")
        elif isinstance(record_id, str):
            by_id[record_id] = record
        if (isinstance(identity[0], str) and isinstance(identity[1], str) and
                type(identity[2]) is int):
            if identity in identities:
                issues.append(f"records[{index}]: duplicate kind/aggregate_id/sequence")
            identities.add(identity)
    return by_id


def _validate_supersession(record: dict[str, object], by_id: dict[str, dict[str, object]],
                           label: str, issues: list[str]) -> None:
    metadata = record.get("metadata")
    if not isinstance(metadata, dict) or not isinstance(metadata.get("supersedes_record_ids"), list):
        return
    prior_ids, sequence = metadata["supersedes_record_ids"], metadata.get("sequence")
    if sequence == 1 and prior_ids:
        issues.append(f"{label}: sequence 1 cannot supersede records")
    if type(sequence) is int and sequence > 1 and not prior_ids:
        issues.append(f"{label}: sequence > 1 must supersede an exact prior record")
    prior_sequences = set()
    for prior_id in prior_ids:
        if not isinstance(prior_id, str):
            continue
        prior = by_id.get(prior_id)
        prior_meta = prior.get("metadata") if isinstance(prior, dict) else None
        if not isinstance(prior_meta, dict):
            issues.append(f"{label}: supersedes unknown record {prior_id!r}")
        elif (prior.get("kind") != record.get("kind") or
              prior_meta.get("aggregate_id") != metadata.get("aggregate_id") or
              not (type(prior_meta.get("sequence")) is int and type(sequence) is int and
                   prior_meta["sequence"] < sequence)):
            issues.append(f"{label}: superseded record must be same kind/aggregate and lower sequence")
        else:
            prior_sequences.add(prior_meta["sequence"])
    if type(sequence) is int and sequence > 1 and sequence - 1 not in prior_sequences:
        issues.append(f"{label}: supersedes must include a sequence-1 record")


def _validate_claim_refs(record: dict[str, object], by_id: dict[str, dict[str, object]],
                         label: str, issues: list[str]) -> None:
    if record.get("kind") != "KnowledgeClaim" or not isinstance(record.get("spec"), dict):
        return
    spec, subject = record["spec"], record["spec"].get("subject")
    evidence_fields = {"supporting_evidence_record_ids", "contradicting_evidence_record_ids"}
    for field in evidence_fields:
        refs = spec.get(field)
        if not isinstance(refs, list):
            continue
        for reference in refs:
            if not isinstance(reference, str):
                continue
            target = by_id.get(reference)
            target_spec = target.get("spec") if isinstance(target, dict) else None
            subjects = target_spec.get("subjects") if isinstance(target_spec, dict) else None
            if not isinstance(target, dict) or target.get("kind") != "EvidenceRecord":
                issues.append(f"{label}.spec.{field}: unknown EvidenceRecord {reference!r}")
            elif not isinstance(subjects, list) or subject not in subjects:
                issues.append(f"{label}.spec.{field}: evidence does not cover claim subject")
    derived = spec.get("derived_from_claim_record_ids")
    if not isinstance(derived, list):
        return
    for reference in derived:
        if not isinstance(reference, str):
            continue
        target = by_id.get(reference)
        if not isinstance(target, dict) or target.get("kind") != "KnowledgeClaim":
            issues.append(f"{label}.spec.derived_from_claim_record_ids: unknown KnowledgeClaim {reference!r}")


def _supersession_cycle_issues(records: list[dict[str, object]], issues: list[str]) -> None:
    graph = {}
    for record in records:
        metadata = record.get("metadata")
        if isinstance(metadata, dict) and isinstance(metadata.get("record_id"), str):
            refs = metadata.get("supersedes_record_ids")
            graph[metadata["record_id"]] = refs if isinstance(refs, list) else []
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> bool:
        if node in visiting:
            return True
        if node in visited:
            return False
        visiting.add(node)
        cyclic = any(isinstance(prior, str) and prior in graph and visit(prior)
                     for prior in graph.get(node, []))
        visiting.remove(node)
        visited.add(node)
        return cyclic

    if any(visit(node) for node in graph if node not in visited):
        issues.append("record set contains a supersession cycle")


def _derivation_cycle_issues(records: list[dict[str, object]], issues: list[str]) -> None:
    graph = {}
    for record in records:
        metadata, spec = record.get("metadata"), record.get("spec")
        if (record.get("kind") == "KnowledgeClaim" and isinstance(metadata, dict) and
                isinstance(spec, dict) and isinstance(metadata.get("record_id"), str)):
            refs = spec.get("derived_from_claim_record_ids")
            graph[metadata["record_id"]] = refs if isinstance(refs, list) else []
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> bool:
        if node in visiting:
            return True
        if node in visited:
            return False
        visiting.add(node)
        cyclic = any(isinstance(prior, str) and prior in graph and visit(prior)
                     for prior in graph.get(node, []))
        visiting.remove(node)
        visited.add(node)
        return cyclic

    if any(visit(node) for node in graph if node not in visited):
        issues.append("record set contains a claim derivation cycle")


def _validate_record_set(records: list[dict[str, object]]) -> list[str]:
    if not isinstance(records, list) or not records or len(records) > MAX_ITEMS:
        return [f"record set must contain 1..{MAX_ITEMS} records"]
    try:
        if len(canonical_json(records)) > MAX_SET_BYTES:
            return [f"record set exceeds {MAX_SET_BYTES} bytes"]
    except ContractError as error:
        return [str(error)]
    issues: list[str] = []
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            issues.append(f"records[{index}]: expected object")
        else:
            validate_record(record, index, issues)
    valid_records = [record for record in records if isinstance(record, dict)]
    by_id = _record_index(valid_records, issues)
    for index, record in enumerate(records):
        if isinstance(record, dict):
            _validate_supersession(record, by_id, f"records[{index}]", issues)
            _validate_claim_refs(record, by_id, f"records[{index}]", issues)
    _supersession_cycle_issues(valid_records, issues)
    _derivation_cycle_issues(valid_records, issues)
    return issues


def validate_record_set(records: list[dict[str, object]]) -> list[str]:
    """Return bounded issues without granting truth, authority or completion."""
    try:
        return _validate_record_set(records)
    except MemoryError:
        return ["record-set validation exhausted memory"]


def check_record_set_bytes(raw: bytes) -> list[str]:
    """Decode and validate canonical record-set bytes, returning bounded issues."""
    try:
        return validate_record_set(decode_record_set(raw))
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["record-set processing exhausted memory"]
