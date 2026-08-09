#!/usr/bin/env python3
"""Resolve and type-check BackendDecisionPackage evidence artifacts."""
import hashlib
import re

from backend_decision_contract import (
    EVIDENCE_CLASSES,
    EVIDENCE_CLASS_PRODUCERS,
    EVIDENCE_CLASS_PROOF_TYPES,
    EVIDENCE_CLASS_RESULTS,
    MAX_EVIDENCE_BYTES,
    PROOF_TYPES,
)
from engineering_check_support import repo_path_issue, unique_id_issues, unknown_field_issues


DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
GOOD_RESULTS = {"observed", "passed"}
EVIDENCE_FIELDS = {
    "id", "kind", "evidence_class", "proof_types", "subject_type", "subject_id",
    "locator", "content_sha256", "source_revision", "producer", "producer_id", "result",
}
SUBJECT_TYPES = {"decision", "readiness", "assumption", "risk", "review", "applicability"}


def _shape(record, label):
    if not isinstance(record, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(record, EVIDENCE_FIELDS, label)
    missing = EVIDENCE_FIELDS - set(record)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _string_list(value, label):
    if not isinstance(value, list) or not value:
        return [f"{label}: expected non-empty list"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    return [f"{label}: values must be unique"] if len(value) != len(set(value)) else []


def _stream_sha256(path):
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            total += len(chunk)
            if total > MAX_EVIDENCE_BYTES:
                raise ValueError(f"evidence file exceeds {MAX_EVIDENCE_BYTES} bytes while reading")
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def _class_issues(record, label, proof_types):
    evidence_class = record.get("evidence_class")
    issues = []
    allowed_types = EVIDENCE_CLASS_PROOF_TYPES.get(evidence_class, set()) \
        if isinstance(evidence_class, str) else set()
    if not proof_types or not proof_types <= allowed_types:
        issues.append(f"{label}: proof types do not match evidence_class")
    producer = record.get("producer")
    allowed_producers = EVIDENCE_CLASS_PRODUCERS.get(evidence_class, set()) \
        if isinstance(evidence_class, str) else set()
    if not isinstance(producer, str) or producer not in allowed_producers:
        issues.append(f"{label}.producer: does not match evidence_class")
    result = record.get("result")
    allowed_results = EVIDENCE_CLASS_RESULTS.get(evidence_class, set()) \
        if isinstance(evidence_class, str) else set()
    if not isinstance(result, str) or result not in allowed_results:
        issues.append(f"{label}.result: does not match evidence_class")
    return issues


def evidence_record_issues(record, label, repo_root, source_revision):
    issues = _shape(record, label)
    if not isinstance(record, dict):
        return issues
    for field, maximum in (("id", 128), ("subject_id", 128), ("locator", 512),
                           ("source_revision", 256), ("producer_id", 128)):
        value = record.get(field)
        if not isinstance(value, str) or not value.strip() or len(value) > maximum:
            issues.append(f"{label}.{field}: must be non-empty string <= {maximum} characters")
    if record.get("kind") != "repository_file":
        issues.append(f"{label}.kind: v1 supports only repository_file")
    if not isinstance(record.get("evidence_class"), str) or record.get("evidence_class") not in EVIDENCE_CLASSES:
        issues.append(f"{label}.evidence_class: invalid")
    if not isinstance(record.get("subject_type"), str) or record.get("subject_type") not in SUBJECT_TYPES:
        issues.append(f"{label}.subject_type: invalid")
    proof_types = record.get("proof_types")
    issues.extend(_string_list(proof_types, f"{label}.proof_types"))
    known = set(proof_types) if isinstance(proof_types, list) and all(isinstance(x, str) for x in proof_types) else set()
    if known - PROOF_TYPES:
        issues.append(f"{label}.proof_types: contains unknown proof type")
    issues.extend(_class_issues(record, label, known))
    result = record.get("result")
    if not isinstance(result, str) or result not in GOOD_RESULTS | {"failed", "not_executed", "inconclusive"}:
        issues.append(f"{label}.result: invalid")
    if record.get("source_revision") != source_revision:
        issues.append(f"{label}.source_revision: does not match package source_revision")
    issues.extend(_resolved_file_issues(record, label, repo_root))
    return issues


def _resolved_file_issues(record, label, repo_root):
    digest, locator = record.get("content_sha256"), record.get("locator")
    issues = []
    if not isinstance(digest, str) or not DIGEST.fullmatch(digest):
        issues.append(f"{label}.content_sha256: requires sha256:<64 lowercase hex>")
    issue = repo_path_issue(repo_root, locator, f"{label}.locator")
    if issue:
        return issues + [issue]
    target = repo_root / locator
    try:
        size = target.stat().st_size
        if not target.is_file():
            return issues + [f"{label}.locator: must resolve to a regular file"]
        if size > MAX_EVIDENCE_BYTES:
            return issues + [f"{label}.locator: evidence file exceeds {MAX_EVIDENCE_BYTES} bytes"]
        actual = _stream_sha256(target)
    except (OSError, ValueError) as exc:
        return issues + [f"{label}.locator: cannot read evidence file ({exc})"]
    if isinstance(digest, str) and DIGEST.fullmatch(digest) and digest != actual:
        issues.append(f"{label}.content_sha256: does not match current file bytes")
    return issues


def build_evidence_index(package, repo_root, label):
    records, issues = package.get("evidence"), []
    id_issues, _ = unique_id_issues(records, label, "evidence")
    issues.extend(id_issues)
    if not isinstance(records, list) or not records or len(records) > 256:
        return issues + [f"{label}.evidence: expected 1..256 records"], {}
    evidence, locator_classes = {}, {}
    for index, record in enumerate(records):
        issues.extend(evidence_record_issues(record, f"{label}.evidence[{index}]", repo_root,
                                             package.get("source_revision")))
        record_id = record.get("id") if isinstance(record, dict) else None
        if isinstance(record_id, str) and record_id.strip():
            evidence[record_id] = record
        locator = record.get("locator") if isinstance(record, dict) else None
        evidence_class = record.get("evidence_class") if isinstance(record, dict) else None
        if isinstance(locator, str) and isinstance(evidence_class, str):
            prior = locator_classes.setdefault(locator, evidence_class)
            if prior != evidence_class:
                issues.append(f"{label}.evidence[{index}]: one locator cannot claim multiple evidence classes")
    return issues, evidence


def proof_refs_issues(refs, label, evidence, *, positive=False, non_empty=False):
    if not isinstance(refs, list) or (non_empty and not refs):
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list"]
    if not all(isinstance(ref, str) and ref.strip() for ref in refs):
        return [f"{label}: values must be non-empty strings"]
    issues = [f"{label}: values must be unique"] if len(refs) != len(set(refs)) else []
    missing = set(refs) - set(evidence)
    if missing:
        issues.append(f"{label}: unknown evidence ids {sorted(missing)}")
    if positive:
        bad = [ref for ref in refs
               if not isinstance(evidence.get(ref, {}).get("result"), str)
               or evidence.get(ref, {}).get("result") not in GOOD_RESULTS]
        if bad:
            issues.append(f"{label}: requires observed/passed evidence, got {sorted(bad)}")
    return issues


def subject_proof_issues(refs, label, evidence, subject_type, subject_id, required_types):
    issues = proof_refs_issues(refs, label, evidence, positive=True, non_empty=True)
    if not isinstance(refs, list) or not all(isinstance(ref, str) for ref in refs):
        return issues
    matched_types = set()
    for ref in refs:
        record = evidence.get(ref)
        if not isinstance(record, dict):
            continue
        if record.get("subject_type") != subject_type or record.get("subject_id") != subject_id:
            issues.append(f"{label}: evidence {ref!r} is bound to a different subject")
            continue
        types = record.get("proof_types")
        if isinstance(types, list) and all(isinstance(item, str) for item in types):
            matched_types.update(types)
    missing = set(required_types) - matched_types
    if missing:
        issues.append(f"{label}: missing required proof types {sorted(missing)}")
    return issues
