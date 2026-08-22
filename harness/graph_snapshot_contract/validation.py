"""Fail-closed exact reconstruction for complete ADR-0065 envelopes."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import (
    canonical_json, decode_base64url, decode_canonical,
    decode_profile_discriminators,
)
from .constants import (
    CANONICALIZATION, ENVELOPE_API, ENVELOPE_FIELDS, MAX_AGGREGATE_LOCATORS,
    MAX_CROSSWALKS, MAX_EDGE_UNION, MAX_ENVELOPE_BYTES, MAX_LOCATORS_PER_RECORD,
    MAX_NODES, MAX_REQUEST_BYTES, MAX_SNAPSHOT_BYTES, MAX_UNRESOLVED_EDGES,
    MAX_UNRESOLVED_NODES, PROFILE_ID, REQUEST_API, REQUEST_FIELDS,
    SNAPSHOT_API, SNAPSHOT_FIELDS,
)
from .derive import derive_envelope


def _exact_fields(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label}: expected object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(
            f"{label}: unknown={sorted(unknown)} missing={sorted(missing)}")
    return value


def _bounded_list(value: object, limit: int, label: str,
                  *, exact: int | None = None) -> list[object]:
    if not isinstance(value, list):
        raise ContractError(f"{label}: expected array")
    if exact is not None and len(value) != exact:
        raise ContractError(f"{label}: expected exactly {exact} items")
    if len(value) > limit:
        raise ContractError(f"{label}: exceeds {limit} items")
    return value


def _locator_limits(snapshot: dict[str, object]) -> None:
    total = 0
    for collection_name in ("nodes", "edges", "unresolved_nodes", "unresolved_edges"):
        collection = snapshot[collection_name]
        for index, record in enumerate(collection):
            if not isinstance(record, dict):
                raise ContractError(f"snapshot.{collection_name}[{index}]: expected object")
            locators = _bounded_list(
                record.get("source_locators"), MAX_LOCATORS_PER_RECORD,
                f"snapshot.{collection_name}[{index}].source_locators")
            total += len(locators)
    if total > MAX_AGGREGATE_LOCATORS:
        raise ContractError("snapshot: aggregate source locator limit exceeded")


def _snapshot_limits(value: object) -> dict[str, object]:
    snapshot = _exact_fields(value, SNAPSHOT_FIELDS, "snapshot")
    canonical_json(snapshot, max_bytes=MAX_SNAPSHOT_BYTES)
    _bounded_list(snapshot["sources"], 1, "snapshot.sources", exact=1)
    _bounded_list(snapshot["extractors"], 1, "snapshot.extractors", exact=1)
    _bounded_list(snapshot["nodes"], MAX_NODES, "snapshot.nodes")
    _bounded_list(snapshot["edges"], MAX_EDGE_UNION, "snapshot.edges")
    _bounded_list(snapshot["unresolved_nodes"], MAX_UNRESOLVED_NODES,
                  "snapshot.unresolved_nodes")
    _bounded_list(snapshot["unresolved_edges"], MAX_UNRESOLVED_EDGES,
                  "snapshot.unresolved_edges")
    _bounded_list(snapshot["adr_0062_node_crosswalk"], MAX_CROSSWALKS,
                  "snapshot.adr_0062_node_crosswalk")
    coverage = snapshot["coverage"]
    if not isinstance(coverage, dict):
        raise ContractError("snapshot.coverage: expected object")
    _bounded_list(coverage.get("surfaces"), 11, "snapshot.coverage.surfaces", exact=11)
    _locator_limits(snapshot)
    return snapshot


def _unsupported_profile(value: object) -> bool:
    if not isinstance(value, dict):
        return False
    request, snapshot = value.get("request"), value.get("snapshot")
    return (
        isinstance(value.get("api_version"), str) and
        value["api_version"] != ENVELOPE_API or
        isinstance(request, dict) and (
            isinstance(request.get("api_version"), str) and
            request["api_version"] != REQUEST_API or
            isinstance(request.get("projector_profile_id"), str) and
            request["projector_profile_id"] != PROFILE_ID) or
        isinstance(snapshot, dict) and (
            isinstance(snapshot.get("api_version"), str) and
            snapshot["api_version"] != SNAPSHOT_API or
            isinstance(snapshot.get("profile_id"), str) and
            snapshot["profile_id"] != PROFILE_ID))


def _validate(raw: bytes) -> None:
    discriminator_value = decode_profile_discriminators(
        raw, max_bytes=MAX_ENVELOPE_BYTES, label="ADR-0065 envelope")
    if _unsupported_profile(discriminator_value):
        raise ContractError(
            "unsupported_profile: unsupported graph snapshot version or profile")
    value = decode_canonical(raw, max_bytes=MAX_ENVELOPE_BYTES,
                             label="ADR-0065 envelope")
    envelope = _exact_fields(value, ENVELOPE_FIELDS, "envelope")
    if (envelope["api_version"] != ENVELOPE_API or
            envelope["canonicalization"] != CANONICALIZATION):
        raise ContractError("envelope: fixed API or canonicalization drifted")
    request = _exact_fields(envelope["request"], REQUEST_FIELDS, "request")
    canonical_json(request, max_bytes=MAX_REQUEST_BYTES)
    _snapshot_limits(envelope["snapshot"])
    graph_bytes = decode_base64url(request["graph_observation_base64url"])
    expected = derive_envelope(
        graph_bytes, request["graph_observation_sha256"], request["run_id"],
        request["project_id"])
    if canonical_json(expected, max_bytes=MAX_ENVELOPE_BYTES) != raw:
        raise ContractError(
            "envelope: not the unique exact reconstructed request/snapshot/digest value")


def validate_envelope_bytes(raw: bytes) -> list[str]:
    """Return zero issues only for the unique fully reconstructed envelope."""
    try:
        _validate(raw)
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["ADR-0065 validation exhausted bounded memory"]
    except (KeyError, TypeError, ValueError, AttributeError, IndexError,
            UnicodeError, RecursionError) as error:
        return [f"ADR-0065 fail-closed nested value: {error}"]
