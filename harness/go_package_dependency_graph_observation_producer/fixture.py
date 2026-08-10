"""Deterministic ADR-0053 golden fixture validation.

The fixture carries source byte preimages so digests can be independently
checked. It intentionally does not parse Go; package/import facts remain the
live producer's bounded lexical observation and are checked structurally.
"""

from __future__ import annotations

import hashlib
from pathlib import Path

from governance_contract import ContractError, read_bounded_file

from .codec import canonical_json, decode_json
from .constants import (EXPECTED_FIELDS, FIXTURE_API, FIXTURE_PATH,
                        FIXTURE_SEMANTICS, MAX_FIXTURE_BYTES,
                        PREIMAGE_FILE_FIELDS, PREIMAGES_FIELDS, RESULT,
                        WRAPPER_FIELDS)
from .profiles import exact_fields
from .semantics import (graph_digest, parameters_digest, production_digest,
                        source_digest, validate_production)


def computed_expected(production: dict[str, object]) -> dict[str, object]:
    graph = production["graph_observation"]
    parameters = production["parameters_manifest"]
    source = production["source_manifest"]
    return {
        "canonical_graph_observation_json": canonical_json(graph).decode("utf-8"),
        "canonical_parameters_manifest_json": canonical_json(parameters).decode("utf-8"),
        "canonical_production_json": canonical_json(production).decode("utf-8"),
        "canonical_source_manifest_json": canonical_json(source).decode("utf-8"),
        "graph_sha256": graph_digest(graph),
        "parameters_sha256": parameters_digest(parameters),
        "production_sha256": production_digest(production),
        "result": RESULT,
        "source_tree_sha256": source_digest(source),
    }


def validate_preimages(preimages: object,
                       production: dict[str, object]) -> list[str]:
    issues: list[str] = []
    if not exact_fields(preimages, PREIMAGES_FIELDS, "golden.preimages", issues):
        return issues
    regular = {
        entry["path"]: entry for entry in production["source_manifest"]["entries"]
        if entry["kind"] == "regular"
    }
    values = preimages["source_regular_files"]
    if not isinstance(values, list) or len(values) != len(regular):
        return ["golden.preimages.source_regular_files: incomplete closed set"]
    seen: set[str] = set()
    for index, item in enumerate(values):
        label = f"golden.preimages.source_regular_files[{index}]"
        if not exact_fields(item, PREIMAGE_FILE_FIELDS, label, issues):
            continue
        path, text = item["path"], item["utf8"]
        payload = text.encode("utf-8") if isinstance(text, str) else None
        entry = regular.get(path)
        if (entry is None or payload is None or path in seen or
                len(payload) != entry["bytes"] or
                hashlib.sha256(payload).hexdigest() != entry["content_sha256"]):
            issues.append(f"{label}: bytes or SHA-256 mismatch")
        if isinstance(path, str):
            seen.add(path)
    if seen != set(regular):
        issues.append("golden.preimages.source_regular_files: path coverage mismatch")
    return issues


def _read_fixture(repo_root: Path) -> object:
    return decode_json(
        read_bounded_file(repo_root / FIXTURE_PATH, label="ADR-0053 golden fixture",
                          max_bytes=MAX_FIXTURE_BYTES),
        max_bytes=MAX_FIXTURE_BYTES, enforce_text=False,
    )


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    try:
        value = _read_fixture(repo_root)
    except (OSError, ContractError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    issues: list[str] = []
    if not exact_fields(value, WRAPPER_FIELDS, "golden", issues):
        return issues
    if (value["api_version"] != FIXTURE_API or
            value["fixture_semantics"] != FIXTURE_SEMANTICS):
        issues.append("golden: API or pure-contract semantics drifted")
    if not exact_fields(value["expected"], EXPECTED_FIELDS,
                        "golden.expected", issues):
        return issues
    production = value["production"]
    production_issues = validate_production(production)
    issues.extend(f"golden production: {item}" for item in production_issues)
    if issues or not isinstance(production, dict):
        return issues
    issues.extend(validate_preimages(value["preimages"], production))
    if issues:
        return issues
    expected = computed_expected(production)
    for field, actual in expected.items():
        if value["expected"][field] != actual:
            issues.append(f"golden.expected.{field}: golden value mismatch")
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate deterministic bytes without claiming live Go parsing."""
    try:
        return _validate_golden_fixture(repo_root)
    except (ContractError, KeyError, TypeError, AttributeError,
            UnicodeError) as error:
        return [f"{FIXTURE_PATH}: invalid nested value: {error}"]
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
