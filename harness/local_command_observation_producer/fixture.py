"""Deterministic golden fixture validation for local observation production."""

from __future__ import annotations

import hashlib
from pathlib import Path

from command_observation_evidence_adapter import canonical_observation
from governance_contract import ContractError, read_bounded_file

from .codec import canonical_json, decode_json
from .constants import (ENVIRONMENT_DOMAIN, EXPECTED_FIELDS, FILE_PREIMAGE_FIELDS,
                        FIXTURE_API, FIXTURE_PATH, FIXTURE_SEMANTICS,
                        MAX_FIXTURE_BYTES, PREIMAGES_FIELDS, RESULT, SOURCE_DOMAIN,
                        TOOL_DOMAIN, TOOL_PREIMAGE_FIELDS, WRAPPER_FIELDS)
from .profiles import exact_fields
from .semantics import (domain_digest, production_digest, validate_production)


def computed_expected(production: dict[str, object]) -> dict[str, object]:
    environment = production["environment_manifest"]
    observation = production["observation"]
    source = production["source_manifest"]
    tool = production["tool_manifest"]
    return {
        "canonical_environment_manifest_json": canonical_json(environment).decode("utf-8"),
        "canonical_observation_json": canonical_observation(observation).decode("utf-8"),
        "canonical_production_json": canonical_json(production).decode("utf-8"),
        "canonical_source_manifest_json": canonical_json(source).decode("utf-8"),
        "canonical_tool_manifest_json": canonical_json(tool).decode("utf-8"),
        "environment_sha256": domain_digest(ENVIRONMENT_DOMAIN, environment),
        "production_sha256": production_digest(production),
        "result": RESULT,
        "source_tree_sha256": domain_digest(SOURCE_DOMAIN, source),
        "tool_snapshot_sha256": domain_digest(TOOL_DOMAIN, tool),
    }


def _validate_golden_fixture(repo_root: Path) -> list[str]:
    relative = Path(FIXTURE_PATH)
    try:
        raw = read_bounded_file(repo_root / relative, label="local producer golden fixture",
                                max_bytes=MAX_FIXTURE_BYTES)
        # Wrapper preimages are source/tool bytes and may legitimately contain
        # newlines. Production strings are validated separately with the
        # closed forbidden-Unicode profile below.
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
        computed = computed_expected(production)
    except (ContractError, KeyError, TypeError) as error:
        return [f"golden production: {error}"]
    for field, actual in computed.items():
        if value["expected"][field] != actual:
            issues.append(f"golden.expected.{field}: golden value mismatch")
    try:
        canonical_wire = value["expected"]["canonical_production_json"].encode("utf-8")
        if canonical_json(decode_json(canonical_wire)) != canonical_wire:
            issues.append("golden.expected.canonical_production_json: not exact canonical JSON")
    except (AttributeError, ContractError) as error:
        issues.append(f"golden.expected.canonical_production_json: {error}")
    return issues


def validate_preimages(preimages: object, production: dict[str, object]) -> list[str]:
    issues: list[str] = []
    if not exact_fields(preimages, PREIMAGES_FIELDS, "golden.preimages", issues):
        return issues
    tool = preimages["tool"]
    if exact_fields(tool, TOOL_PREIMAGE_FIELDS, "golden.preimages.tool", issues):
        payload = tool["utf8"].encode("utf-8") if isinstance(tool["utf8"], str) else None
        manifest = production["tool_manifest"]
        if (payload is None or tool["final_path"] != manifest["final_path"] or
                len(payload) != manifest["bytes"] or hashlib.sha256(payload).hexdigest() != manifest["sha256"]):
            issues.append("golden.preimages.tool: bytes or SHA-256 mismatch")
    regular = {entry["path"]: entry for entry in production["source_manifest"]["entries"]
               if entry["kind"] == "regular"}
    files = preimages["source_regular_files"]
    if not isinstance(files, list) or len(files) != len(regular):
        issues.append("golden.preimages.source_regular_files: incomplete closed set")
        return issues
    seen = set()
    for index, preimage in enumerate(files):
        if not exact_fields(preimage, FILE_PREIMAGE_FIELDS,
                            f"golden.preimages.source_regular_files[{index}]", issues):
            continue
        path, text = preimage["path"], preimage["utf8"]
        payload = text.encode("utf-8") if isinstance(text, str) else None
        entry = regular.get(path)
        if (entry is None or payload is None or path in seen or len(payload) != entry["bytes"] or
                hashlib.sha256(payload).hexdigest() != entry["content_sha256"]):
            issues.append(f"golden.preimages.source_regular_files[{index}]: bytes or SHA-256 mismatch")
        seen.add(path)
    if seen != set(regular):
        issues.append("golden.preimages.source_regular_files: path coverage mismatch")
    return issues


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate deterministic bytes without claiming a live process capture."""
    try:
        return _validate_golden_fixture(repo_root)
    except (ContractError, KeyError, TypeError, AttributeError, UnicodeError) as error:
        return [f"{FIXTURE_PATH}: invalid nested value: {error}"]
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
