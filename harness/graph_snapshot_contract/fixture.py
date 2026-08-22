"""Strict deterministic golden wrapper validation for ADR-0065."""

from __future__ import annotations

from pathlib import Path

from governance_contract import ContractError, read_bounded_file
from go_package_dependency_graph_observation_producer.graph_contract import (
    observation_digest, validate_graph_bytes,
)

from .codec import canonical_json, decode_canonical
from .constants import (
    FIXTURE_API, FIXTURE_EXPECTED_FIELDS, FIXTURE_FIELDS, FIXTURE_INPUT_FIELDS,
    FIXTURE_PATH, FIXTURE_SEMANTICS, MAX_ENVELOPE_BYTES, MAX_FIXTURE_BYTES,
)
from .derive import derive_envelope
from .profiles import validate_hash, validate_identifier
from .validation import validate_envelope_bytes


def _exact_fields(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label}: expected object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(
            f"{label}: unknown={sorted(unknown)} missing={sorted(missing)}")
    return value


def _text(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise ContractError(f"{label}: expected string")
    return value


def _derive(value: object):
    fixture_input = _exact_fields(value, FIXTURE_INPUT_FIELDS, "golden.input")
    graph_text = _text(fixture_input["canonical_graph_observation_json"],
                       "golden.input.canonical_graph_observation_json")
    graph_bytes = graph_text.encode("utf-8")
    graph = validate_graph_bytes(graph_bytes)
    graph_sha = validate_hash(
        fixture_input["graph_observation_sha256"],
        "golden.input.graph_observation_sha256")
    if observation_digest(graph) != graph_sha:
        raise ContractError("golden.input.graph_observation_sha256: digest mismatch")
    run_id = validate_identifier(fixture_input["run_id"], "golden.input.run_id")
    project_id = validate_identifier(
        fixture_input["project_id"], "golden.input.project_id")
    envelope = derive_envelope(graph_bytes, graph_sha, run_id, project_id)
    return canonical_json(envelope, max_bytes=MAX_ENVELOPE_BYTES), envelope


def _validate_expected(value: object, derived: bytes,
                       envelope: dict[str, object]) -> None:
    expected = _exact_fields(value, FIXTURE_EXPECTED_FIELDS, "golden.expected")
    stored = _text(expected["canonical_envelope_json"],
                   "golden.expected.canonical_envelope_json").encode("utf-8")
    issues = validate_envelope_bytes(stored)
    if issues:
        raise ContractError(f"golden.expected.canonical_envelope_json: {issues[0]}")
    if stored != derived:
        raise ContractError("golden.expected.canonical_envelope_json: derived bytes mismatch")
    snapshot = envelope["snapshot"]
    actual = {field: snapshot[field] for field in FIXTURE_EXPECTED_FIELDS
              if field not in {"canonical_envelope_json", "envelope_sha256"}}
    actual["envelope_sha256"] = envelope["envelope_sha256"]
    actual["request_sha256"] = envelope["request"]["request_sha256"]
    for field, digest in actual.items():
        stored_value = expected[field]
        if field == "snapshot_id":
            if stored_value != digest:
                raise ContractError(f"golden.expected.{field}: derived ID mismatch")
        elif validate_hash(stored_value, f"golden.expected.{field}") != digest:
            raise ContractError(f"golden.expected.{field}: derived digest mismatch")


def _validate(raw: bytes) -> None:
    payload = raw[:-1] if raw.endswith(b"\n") else raw
    fixture = _exact_fields(decode_canonical(
        payload, max_bytes=MAX_FIXTURE_BYTES, label="ADR-0065 golden fixture"),
        FIXTURE_FIELDS, "golden")
    if fixture["api_version"] != FIXTURE_API:
        raise ContractError("golden.api_version: fixed fixture API drifted")
    if fixture["fixture_semantics"] != FIXTURE_SEMANTICS:
        raise ContractError("golden.fixture_semantics: authority boundary drifted")
    derived, envelope = _derive(fixture["input"])
    _validate_expected(fixture["expected"], derived, envelope)


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate wrapper input, exact envelope bytes, all IDs, and all digests."""
    try:
        raw = read_bounded_file(
            repo_root / FIXTURE_PATH, label="ADR-0065 golden fixture",
            max_bytes=MAX_FIXTURE_BYTES)
        _validate(raw)
        return []
    except (OSError, ContractError) as error:
        return [f"{FIXTURE_PATH}: {error}"]
    except MemoryError:
        return [f"{FIXTURE_PATH}: golden fixture processing exhausted memory"]
    except (KeyError, TypeError, ValueError, AttributeError, IndexError,
            UnicodeError, RecursionError) as error:
        return [f"{FIXTURE_PATH}: fail-closed nested value: {error}"]
