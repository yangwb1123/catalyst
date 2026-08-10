"""Deterministic golden fixture checking for ADR-0052."""

from __future__ import annotations

import hashlib
from pathlib import Path

from evolve_repo_locator_evidence_adapter import canonical_observation
from governance_contract import ContractError, read_bounded_file

from .codec import canonical_json, decode_json
from .constants import (EXPECTED_FIELDS, FILE_PREIMAGE_FIELDS, FIXTURE_API,
                        FIXTURE_PATH, FIXTURE_SEMANTICS, MAX_FIXTURE_BYTES,
                        PREIMAGES_FIELDS, RESULT, WRAPPER_FIELDS)
from .profiles import exact_fields
from .semantics import (expected_observations, parameters_digest,
                        production_digest, source_digest, validate_production)


def computed_expected(production: dict[str, object]) -> dict[str, object]:
    observations = expected_observations(production)
    parameters = production["parameters_manifest"]
    report = production["report_manifest"]
    source = production["source_manifest"]
    return {
        "canonical_observation_jsons": [
            canonical_observation(value).decode("utf-8") for value in observations
        ],
        "canonical_parameters_manifest_json": canonical_json(parameters).decode("utf-8"),
        "canonical_production_json": canonical_json(production).decode("utf-8"),
        "canonical_report_manifest_json": canonical_json(report).decode("utf-8"),
        "canonical_source_manifest_json": canonical_json(source).decode("utf-8"),
        "parameters_sha256": parameters_digest(parameters),
        "production_sha256": production_digest(production),
        "report_sha256": report["sha256"],
        "result": RESULT,
        "source_tree_sha256": source_digest(source),
    }


def _regular_entries(production: dict[str, object]) -> dict[str, dict[str, object]]:
    return {entry["path"]: entry for entry in production["source_manifest"]["entries"]
            if entry["kind"] == "regular"}


def validate_preimages(value: object, production: dict[str, object]) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, PREIMAGES_FIELDS, "golden.preimages", issues):
        return issues
    files, regular = value["source_regular_files"], _regular_entries(production)
    if not isinstance(files, list) or len(files) != len(regular):
        issues.append("golden.preimages.source_regular_files: incomplete closed set")
        return issues
    seen = set()
    for index, preimage in enumerate(files):
        label = f"golden.preimages.source_regular_files[{index}]"
        if not exact_fields(preimage, FILE_PREIMAGE_FIELDS, label, issues):
            continue
        path, text = preimage["path"], preimage["utf8"]
        entry = regular.get(path)
        payload = text.encode("utf-8") if isinstance(text, str) else None
        if (entry is None or payload is None or path in seen or
                len(payload) != entry["bytes"] or
                hashlib.sha256(payload).hexdigest() != entry["content_sha256"]):
            issues.append(f"{label}: bytes or SHA-256 mismatch")
        seen.add(path)
    if seen != set(regular):
        issues.append("golden.preimages.source_regular_files: path coverage mismatch")
    return issues


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    relative = Path(FIXTURE_PATH)
    try:
        raw = read_bounded_file(repo_root / relative, label="ADR-0052 golden fixture",
                                max_bytes=MAX_FIXTURE_BYTES)
        value = decode_json(raw, max_bytes=MAX_FIXTURE_BYTES, enforce_text=False)
    except (OSError, ContractError) as error:
        return [f"{relative}: {error}"]
    issues: list[str] = []
    if not exact_fields(value, WRAPPER_FIELDS, "golden", issues):
        return issues
    if value["api_version"] != FIXTURE_API or value["fixture_semantics"] != FIXTURE_SEMANTICS:
        issues.append("golden: API or pure-contract semantics drifted")
    if not exact_fields(value["expected"], EXPECTED_FIELDS, "golden.expected", issues):
        return issues
    production = value["production"]
    issues.extend(f"golden production: {issue}" for issue in validate_production(production))
    if issues or not isinstance(production, dict):
        return issues
    issues.extend(validate_preimages(value["preimages"], production))
    if issues:
        return issues
    try:
        expected = computed_expected(production)
    except (ContractError, KeyError, TypeError, IndexError) as error:
        return [f"golden production: {error}"]
    for field, actual in expected.items():
        if value["expected"][field] != actual:
            issues.append(f"golden.expected.{field}: golden value mismatch")
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    try:
        return _validate_golden_fixture(repo_root)
    except (ContractError, KeyError, TypeError, AttributeError,
            UnicodeError, IndexError) as error:
        return [f"{FIXTURE_PATH}: invalid nested value: {error}"]
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
