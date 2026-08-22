"""Pure exact ADR-0053 bytes to ADR-0066 test-source GraphSnapshot."""

from __future__ import annotations

import base64

from governance_contract import ContractError
from go_package_dependency_graph_observation_producer.graph_contract import (
    observation_digest, validate_graph_bytes,
)

from .codec import canonical_json, decode_profile_discriminators, self_digest
from .profiles import validate_hash, validate_identifier
from .provenance import source_record
from .lexical_test_source_constants import (
    CANONICALIZATION, ENVELOPE_API, ENVELOPE_DOMAIN, GRAPH_API, GRAPH_PROFILE,
    MAX_ENVELOPE_BYTES, MAX_GRAPH_BYTES, MAX_REQUEST_BYTES, MAX_SNAPSHOT_BYTES,
    REQUEST_API, REQUEST_DOMAIN, TEST_SOURCE_PROFILE_ID,
)
from .lexical_test_source_provenance import extractor_record
from .lexical_test_source_snapshot import build_snapshot
from .lexical_test_source_topology import (
    build_edges, build_nodes, enforce_locator_aggregate,
)
from .topology import build_crosswalk
from .unresolved import build_unresolved_edges, build_unresolved_nodes


def _reject_unsupported_graph_profile(graph_bytes: bytes) -> None:
    value = decode_profile_discriminators(
        graph_bytes, max_bytes=MAX_GRAPH_BYTES, label="ADR-0053 graph observation")
    if isinstance(value, dict) and (
            isinstance(value.get("api_version"), str) and
            value["api_version"] != GRAPH_API or
            isinstance(value.get("profile_id"), str) and
            value["profile_id"] != GRAPH_PROFILE):
        raise ContractError(
            "unsupported_profile: unsupported ADR-0053 graph version or profile")


def _request(graph_bytes: bytes, graph_sha256: object, run_id: object,
             project_id: object):
    if not isinstance(graph_bytes, bytes) or len(graph_bytes) > MAX_GRAPH_BYTES:
        raise ContractError("ADR-0066 graph observation must be bounded bytes")
    _reject_unsupported_graph_profile(graph_bytes)
    graph = validate_graph_bytes(graph_bytes)
    graph_sha = validate_hash(
        graph_sha256, "request.graph_observation_sha256")
    if observation_digest(graph) != graph_sha:
        raise ContractError("request.graph_observation_sha256: ADR-0053 digest mismatch")
    checked_run = validate_identifier(run_id, "request.run_id")
    checked_project = validate_identifier(project_id, "request.project_id")
    if graph["producer"]["run_id"] != checked_run:
        raise ContractError("request.run_id: graph producer binding mismatch")
    request = {
        "api_version": REQUEST_API, "canonicalization": CANONICALIZATION,
        "graph_observation_base64url": base64.urlsafe_b64encode(
            graph_bytes).rstrip(b"=").decode("ascii"),
        "graph_observation_sha256": graph_sha, "project_id": checked_project,
        "projector_profile_id": TEST_SOURCE_PROFILE_ID,
        "request_sha256": "", "run_id": checked_run,
    }
    request["request_sha256"] = self_digest(
        REQUEST_DOMAIN, request, "request_sha256", max_bytes=MAX_REQUEST_BYTES)
    canonical_json(request, max_bytes=MAX_REQUEST_BYTES)
    return request, graph


def _projection(graph, graph_sha256: str, project_id: str,
                request_sha256: str):
    source = source_record(graph, graph_sha256)
    extractor = extractor_record(source)
    result = build_nodes(graph, project_id, source, extractor)
    (nodes, module, by_package, package_locators, by_test_package,
     test_locators, files) = result
    edges = build_edges(
        graph, module, by_package, package_locators, by_test_package,
        test_locators, files, nodes, source, extractor)
    unresolved_nodes = build_unresolved_nodes(
        graph, project_id, source, extractor)
    unresolved_edges = build_unresolved_edges(
        graph, project_id, by_package, files, source, extractor)
    enforce_locator_aggregate(nodes, edges, unresolved_nodes, unresolved_edges)
    crosswalk = build_crosswalk(graph, by_package)
    return build_snapshot(
        graph, project_id, request_sha256, [source], [extractor], nodes, edges,
        unresolved_nodes, unresolved_edges, crosswalk)


def derive_test_source_envelope(
        graph_json: bytes, graph_sha256: object, run_id: object,
        project_id: object) -> dict[str, object]:
    """Build the unique ADR-0066 envelope from caller-supplied bounded bytes."""
    request, graph = _request(graph_json, graph_sha256, run_id, project_id)
    snapshot = _projection(
        graph, request["graph_observation_sha256"], request["project_id"],
        request["request_sha256"])
    canonical_json(snapshot, max_bytes=MAX_SNAPSHOT_BYTES)
    envelope = {
        "api_version": ENVELOPE_API, "canonicalization": CANONICALIZATION,
        "envelope_sha256": "", "request": request, "snapshot": snapshot,
    }
    envelope["envelope_sha256"] = self_digest(
        ENVELOPE_DOMAIN, envelope, "envelope_sha256",
        max_bytes=MAX_ENVELOPE_BYTES)
    canonical_json(envelope, max_bytes=MAX_ENVELOPE_BYTES)
    return envelope
