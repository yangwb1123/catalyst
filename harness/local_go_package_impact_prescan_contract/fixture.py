"""Strict deterministic golden wrapper validation for ADR-0062."""

from __future__ import annotations

from pathlib import Path

from governance_contract import ContractError, read_bounded_file

from .codec import canonical_json, decode_canonical
from .constants import (FIXTURE_API, FIXTURE_EXPECTED_FIELDS, FIXTURE_FIELDS,
                        FIXTURE_INPUT_FIELDS, FIXTURE_PATH, HASH_RE,
                        MAX_ENVELOPE_BYTES, MAX_FIXTURE_BYTES)
from .derive import derive_envelope
from .graph import observation_digest, validate_graph_bytes
from .profiles import validate_changed_paths, validate_run_id
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


def _hash(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return value


def _derive_expected(value: object) -> tuple[bytes, dict[str, object]]:
    fixture_input = _exact_fields(value, FIXTURE_INPUT_FIELDS, "golden.input")
    graph_text = _text(fixture_input["canonical_graph_observation_json"],
                       "golden.input.canonical_graph_observation_json")
    graph_bytes = graph_text.encode("utf-8")
    graph = validate_graph_bytes(graph_bytes)
    graph_hash = _hash(fixture_input["graph_observation_sha256"],
                       "golden.input.graph_observation_sha256")
    if observation_digest(graph) != graph_hash:
        raise ContractError("golden.input.graph_observation_sha256: digest mismatch")
    paths = validate_changed_paths(fixture_input["changed_paths"])
    run_id = validate_run_id(fixture_input["run_id"], "golden.input.run_id")
    envelope = derive_envelope(graph_bytes, paths, run_id)
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
    digest_fields = {
        "envelope_sha256": envelope["envelope_sha256"],
        "report_sha256": envelope["report"]["report_sha256"],
        "request_sha256": envelope["request"]["request_sha256"],
    }
    for field, actual in digest_fields.items():
        if _hash(expected[field], f"golden.expected.{field}") != actual:
            raise ContractError(f"golden.expected.{field}: derived digest mismatch")


def _validate(raw: bytes) -> None:
    payload = raw[:-1] if raw.endswith(b"\n") else raw
    value = decode_canonical(payload, max_bytes=MAX_FIXTURE_BYTES,
                             label="ADR-0062 golden fixture")
    fixture = _exact_fields(value, FIXTURE_FIELDS, "golden")
    if fixture["api_version"] != FIXTURE_API:
        raise ContractError("golden.api_version: fixed fixture API drifted")
    derived, envelope = _derive_expected(fixture["input"])
    _validate_expected(fixture["expected"], derived, envelope)


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Validate exact wrapper inputs, canonical output, and all pinned digests."""
    fixture_path = repo_root / FIXTURE_PATH
    try:
        raw = read_bounded_file(
            fixture_path, label="ADR-0062 golden fixture",
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
