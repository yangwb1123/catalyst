"""Fail-closed byte validation for complete ADR-0062 envelopes."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import (canonical_json, decode_base64url, decode_canonical,
                    self_digest)
from .constants import (CANONICALIZATION, ENVELOPE_API, ENVELOPE_DOMAIN,
                        ENVELOPE_FIELDS, HASH_RE, MAX_ENVELOPE_BYTES,
                        MAX_REPORT_BYTES, MAX_REQUEST_BYTES, REPORT_FIELDS,
                        REQUEST_API, REQUEST_DOMAIN, REQUEST_FIELDS)
from .derive import derive_report
from .graph import observation_digest, validate_graph_bytes
from .profiles import validate_changed_paths, validate_run_id


def _exact_fields(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ContractError(f"{label}: expected object")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(
            f"{label}: unknown={sorted(unknown)} missing={sorted(missing)}")
    return value


def _hash(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return value


def _validate_request(value: object):
    request = _exact_fields(value, REQUEST_FIELDS, "request")
    canonical_json(request, max_bytes=MAX_REQUEST_BYTES)
    if (request["api_version"] != REQUEST_API or
            request["canonicalization"] != CANONICALIZATION):
        raise ContractError("request: fixed API or canonicalization drifted")
    validate_changed_paths(request["changed_paths"])
    run_id = validate_run_id(request["run_id"])
    graph_hash = _hash(
        request["graph_observation_sha256"], "request.graph_observation_sha256")
    request_hash = _hash(request["request_sha256"], "request.request_sha256")
    expected_hash = self_digest(
        REQUEST_DOMAIN, request, "request_sha256", max_bytes=MAX_REQUEST_BYTES)
    if request_hash != expected_hash:
        raise ContractError("request.request_sha256: self digest mismatch")
    graph_bytes = decode_base64url(request["graph_observation_base64url"])
    graph = validate_graph_bytes(graph_bytes)
    if observation_digest(graph) != graph_hash:
        raise ContractError("request.graph_observation_sha256: ADR-0053 digest mismatch")
    if graph["producer"]["run_id"] != run_id:
        raise ContractError("request.run_id: graph producer binding mismatch")
    return request, graph


def _validate(value: bytes) -> None:
    envelope_value = decode_canonical(
        value, max_bytes=MAX_ENVELOPE_BYTES, label="ADR-0062 envelope")
    envelope = _exact_fields(envelope_value, ENVELOPE_FIELDS, "envelope")
    if (envelope["api_version"] != ENVELOPE_API or
            envelope["canonicalization"] != CANONICALIZATION):
        raise ContractError("envelope: fixed API or canonicalization drifted")
    envelope_hash = _hash(envelope["envelope_sha256"], "envelope.envelope_sha256")
    request, graph = _validate_request(envelope["request"])
    report = _exact_fields(envelope["report"], REPORT_FIELDS, "report")
    canonical_json(report, max_bytes=MAX_REPORT_BYTES)
    expected_report = derive_report(request, graph)
    actual_report = canonical_json(report, max_bytes=MAX_REPORT_BYTES)
    expected_report_bytes = canonical_json(
        expected_report, max_bytes=MAX_REPORT_BYTES)
    if actual_report != expected_report_bytes:
        raise ContractError(
            "report: not the exact derived seed/closure/witness/status/digest value")
    expected_hash = self_digest(
        ENVELOPE_DOMAIN, envelope, "envelope_sha256", max_bytes=MAX_ENVELOPE_BYTES)
    if envelope_hash != expected_hash:
        raise ContractError("envelope.envelope_sha256: self digest mismatch")


def validate_envelope_bytes(raw: bytes) -> list[str]:
    """Return zero issues only for the unique canonical, fully derived envelope."""
    try:
        if not isinstance(raw, bytes):
            raise ContractError("ADR-0062 envelope input must be bytes")
        _validate(raw)
        return []
    except ContractError as error:
        return [str(error)]
    except MemoryError:
        return ["ADR-0062 validation exhausted bounded memory"]
    except (KeyError, TypeError, ValueError, AttributeError, IndexError,
            UnicodeError, RecursionError) as error:
        return [f"ADR-0062 fail-closed nested value: {error}"]
