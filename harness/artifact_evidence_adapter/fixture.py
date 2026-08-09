"""Golden validation for the Artifact Evidence adapter v1."""

from __future__ import annotations

from pathlib import Path

from governance_contract import ContractError, canonical_json as governance_json
from governance_contract.codec import read_bounded_file

from .adapter import (adapt_request, compute_request_digest, compute_source_digest,
                      validate_evidence_record, validate_request)
from .codec import canonical_artifact, canonical_json, decode_json
from .constants import MAX_REQUEST_BYTES, SUCCESS

FIXTURE_PATH = Path("docs/contracts/fixtures/artifact-evidence-adapter-v1.json")
WRAPPER_FIELDS = {"api_version", "expected", "request"}
EXPECTED_FIELDS = {
    "canonical_evidence_record_json", "canonical_request_json", "canonical_source_json",
    "evidence_record_sha256", "request_sha256", "result", "source_snapshot_sha256",
}


def _exact_fields(value: object, fields: set[str], label: str,
                  issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def _computed_expected(request: dict[str, object]) -> dict[str, object]:
    record = adapt_request(request)
    return {
        "canonical_evidence_record_json": governance_json(record).decode("utf-8"),
        "canonical_request_json": canonical_json(request).decode("utf-8"),
        "canonical_source_json": canonical_artifact(request["artifact"]).decode("utf-8"),
        "evidence_record_sha256": record["integrity"]["canonical_sha256"],
        "request_sha256": compute_request_digest(request), "result": SUCCESS,
        "source_snapshot_sha256": compute_source_digest(request),
    }


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    path = repo_root / FIXTURE_PATH
    try:
        raw = read_bounded_file(path, label="artifact evidence golden fixture",
                                max_bytes=MAX_REQUEST_BYTES)
        value = decode_json(raw, root=())
    except (OSError, ContractError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    issues: list[str] = []
    if not _exact_fields(value, WRAPPER_FIELDS, "golden", issues):
        return issues
    if value["api_version"] != "forgeos.governance.artifact-evidence-adapter.fixture/v1":
        issues.append("golden.api_version: unsupported version")
    if not _exact_fields(value["expected"], EXPECTED_FIELDS, "golden.expected", issues):
        return issues
    request = value["request"]
    request_issues = validate_request(request)
    issues.extend(f"golden request: {issue}" for issue in request_issues)
    if issues:
        return issues
    try:
        computed = _computed_expected(request)
        record = adapt_request(request)
    except (ContractError, KeyError, TypeError) as error:
        return [f"golden adaptation: {error}"]
    for field, actual in computed.items():
        if value["expected"][field] != actual:
            issues.append(f"golden.expected.{field}: golden value mismatch")
    issues.extend(f"golden evidence: {issue}" for issue in validate_evidence_record(record))
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate golden values without leaking a memory-exhaustion traceback."""
    try:
        return _validate_golden_fixture(repo_root)
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
