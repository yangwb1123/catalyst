"""Golden cross-language fixture validation for the universal scaffold check."""

from __future__ import annotations

from pathlib import Path

from .codec import (ContractError, canonical_json, canonical_record_payload,
                    compute_record_digest, decode_json, read_bounded_file)
from .constants import DOMAINS
from .record_set import validate_record_set

FIXTURE_PATH = Path("docs/contracts/fixtures/governance-evidence-claim-v1.json")
WRAPPER_FIELDS = {"api_version", "canonicalization", "records", "schema_ref"}
ENTRY_FIELDS = {"digest_domain", "expected", "record"}
EXPECTED_FIELDS = {"canonical_payload_json", "canonical_record_json", "canonical_sha256"}


def _exact_fields(value: object, expected: set[str], label: str, issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - expected, expected - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def _validate_entry(entry: object, index: int, issues: list[str]) -> dict[str, object] | None:
    label = f"golden.records[{index}]"
    if not _exact_fields(entry, ENTRY_FIELDS, label, issues):
        return None
    expected, record = entry["expected"], entry["record"]
    if not _exact_fields(expected, EXPECTED_FIELDS, f"{label}.expected", issues):
        return None
    if not isinstance(record, dict):
        issues.append(f"{label}.record: expected object")
        return None
    kind = record.get("kind")
    domain = DOMAINS.get(kind) if isinstance(kind, str) else None
    if domain is None:
        issues.append(f"{label}.record.kind: unsupported kind")
        return record
    if entry["digest_domain"] != domain[:-1].decode("ascii"):
        issues.append(f"{label}.digest_domain: does not match record kind")
    try:
        payload = canonical_record_payload(record).decode("utf-8")
        full = canonical_json(record).decode("utf-8")
        digest = compute_record_digest(record)
    except ContractError as error:
        issues.append(f"{label}: cannot canonicalize: {error}")
        return record
    comparisons = {
        "canonical_payload_json": payload,
        "canonical_record_json": full,
        "canonical_sha256": digest,
    }
    for field, actual in comparisons.items():
        if expected[field] != actual:
            issues.append(f"{label}.expected.{field}: golden value mismatch")
    integrity = record.get("integrity")
    if not isinstance(integrity, dict) or integrity.get("canonical_sha256") != digest:
        issues.append(f"{label}.record.integrity.canonical_sha256: golden digest mismatch")
    return record


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    fixture_path = repo_root / FIXTURE_PATH
    try:
        value = decode_json(read_bounded_file(fixture_path, label="golden fixture"))
    except (OSError, ContractError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    issues: list[str] = []
    if not _exact_fields(value, WRAPPER_FIELDS, "golden", issues):
        return issues
    if value["api_version"] != "forgeos.governance-golden/v1":
        issues.append("golden.api_version: unsupported version")
    if value["canonicalization"] != "forgeos.canonical-json/v1":
        issues.append("golden.canonicalization: unsupported format")
    if value["schema_ref"] != "docs/contracts/governance-evidence-claim-v1.schema.json":
        issues.append("golden.schema_ref: unexpected schema reference")
    elif not (repo_root / value["schema_ref"]).is_file():
        issues.append("golden.schema_ref: referenced schema does not exist")
    entries = value["records"]
    if not isinstance(entries, list) or not entries:
        issues.append("golden.records: expected non-empty list")
        return issues
    records = []
    for index, entry in enumerate(entries):
        record = _validate_entry(entry, index, issues)
        if record is not None:
            records.append(record)
    if len(records) == len(entries):
        issues.extend(f"golden semantic: {issue}" for issue in validate_record_set(records))
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate golden bytes and return a controlled issue on memory exhaustion."""
    try:
        return _validate_golden_fixture(repo_root)
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
