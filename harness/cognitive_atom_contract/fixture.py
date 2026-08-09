"""Golden fixture validation for CognitiveAtom v1 cross-language parity."""

from __future__ import annotations

from pathlib import Path

from governance_contract import (ContractError, canonical_json, read_bounded_file,
                                 validate_record_set)
from governance_contract.codec import decode_json
from governance_contract.shape import identifier

from .constants import (ATOM_DOMAIN, ATOM_ID_DOMAIN, ATOM_SET_DOMAIN,
                        CANONICALIZATION, SOURCE_SET_DOMAIN)
from .projection import (canonical_atom_payload, compute_atom_digest,
                         compute_atom_set_digest, project_atom_set, source_closure)

FIXTURE_PATH = Path("docs/contracts/fixtures/cognitive-atom-projection-v1.json")
WRAPPER_FIELDS = {
    "api_version", "canonicalization", "digest_domains", "expected", "source_records",
    "task_id",
}
DOMAIN_FIELDS = {"atom", "atom_id", "atom_set", "source_closure"}
EXPECTED_FIELDS = {
    "atom_id", "atom_set_sha256", "canonical_atom_json", "canonical_atom_payload_json",
    "canonical_atom_set_json", "canonical_atom_sha256", "canonical_source_closure_json",
    "source_closure_sha256",
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


def _validate_domains(value: object, issues: list[str]) -> None:
    if not _exact_fields(value, DOMAIN_FIELDS, "golden.digest_domains", issues):
        return
    expected = {
        "atom": ATOM_DOMAIN[:-1].decode("ascii"),
        "atom_id": ATOM_ID_DOMAIN[:-1].decode("ascii"),
        "atom_set": ATOM_SET_DOMAIN[:-1].decode("ascii"),
        "source_closure": SOURCE_SET_DOMAIN[:-1].decode("ascii"),
    }
    for field, domain in expected.items():
        if value[field] != domain:
            issues.append(f"golden.digest_domains.{field}: unexpected domain")


def _computed_expected(task_id: str, source_records: list[dict[str, object]]) -> dict[str, str]:
    atoms = project_atom_set(task_id, source_records)
    if len(atoms) != 1:
        raise ContractError("golden source_records must project exactly one CognitiveAtom")
    atom = atoms[0]
    claim_id = atom["source"]["claim_record_id"]
    claim = next(record for record in source_records
                 if record["metadata"]["record_id"] == claim_id)
    _, closure_bytes, closure_digest = source_closure(claim, source_records)
    return {
        "atom_id": atom["metadata"]["atom_id"],
        "atom_set_sha256": compute_atom_set_digest(atoms),
        "canonical_atom_json": canonical_json(atom).decode("utf-8"),
        "canonical_atom_payload_json": canonical_atom_payload(atom).decode("utf-8"),
        "canonical_atom_set_json": canonical_json(atoms).decode("utf-8"),
        "canonical_atom_sha256": compute_atom_digest(atom),
        "canonical_source_closure_json": closure_bytes.decode("utf-8"),
        "source_closure_sha256": closure_digest,
    }


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    try:
        raw = read_bounded_file(repo_root / FIXTURE_PATH, label="cognitive atom golden fixture")
        value = decode_json(raw)
    except (OSError, ContractError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    issues: list[str] = []
    if not _exact_fields(value, WRAPPER_FIELDS, "golden", issues):
        return issues
    if value["api_version"] != "forgeos.aadm.cognitive-atom-golden/v1":
        issues.append("golden.api_version: unsupported version")
    if value["canonicalization"] != CANONICALIZATION:
        issues.append("golden.canonicalization: unsupported format")
    _validate_domains(value["digest_domains"], issues)
    task_issues: list[str] = []
    identifier(value["task_id"], "golden.task_id", task_issues)
    issues.extend(task_issues)
    expected = value["expected"]
    if not _exact_fields(expected, EXPECTED_FIELDS, "golden.expected", issues):
        return issues
    records = value["source_records"]
    if not isinstance(records, list):
        issues.append("golden.source_records: expected list")
        return issues
    source_issues = validate_record_set(records)
    issues.extend(f"golden source: {issue}" for issue in source_issues)
    if issues:
        return issues
    try:
        computed = _computed_expected(value["task_id"], records)
    except (ContractError, KeyError, StopIteration, TypeError) as error:
        return [f"golden projection: {error}"]
    for field, actual in computed.items():
        if expected[field] != actual:
            issues.append(f"golden.expected.{field}: golden value mismatch")
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate the golden fixture with controlled memory exhaustion."""
    try:
        return _validate_golden_fixture(repo_root)
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
