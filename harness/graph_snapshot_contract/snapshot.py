"""Structured-set seals and final GraphSnapshot identity/record construction."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import domain_digest, self_digest
from .constants import (
    CANONICALIZATION, COVERAGE_DOMAIN, CROSSWALK_SET_DOMAIN, EDGE_SET_DOMAIN,
    EXTRACTOR_SET_DOMAIN, IDENTITY_FIELDS, MAX_EDGE_UNION, MAX_SNAPSHOT_BYTES,
    NODE_SET_DOMAIN, PROFILE_ID, RESULT, SNAPSHOT_API, SNAPSHOT_DOMAIN,
    SNAPSHOT_IDENTITY_DOMAIN, SOURCE_SET_DOMAIN, SYSTEM_UNKNOWN_REASONS,
    UNRESOLVED_EDGE_SET_DOMAIN, UNRESOLVED_NODE_SET_DOMAIN,
)
from .coverage import build_coverage, build_freshness
from .records import ensure_global_identity_uniqueness, structured_set_digest


def _set_digests(sources, extractors, nodes, edges, unresolved_nodes,
                 unresolved_edges, crosswalk):
    return {
        "source_set_sha256": structured_set_digest(SOURCE_SET_DOMAIN, sources),
        "extractor_set_sha256": structured_set_digest(EXTRACTOR_SET_DOMAIN, extractors),
        "node_set_sha256": structured_set_digest(NODE_SET_DOMAIN, nodes),
        "edge_set_sha256": structured_set_digest(EDGE_SET_DOMAIN, edges),
        "unresolved_node_set_sha256": structured_set_digest(
            UNRESOLVED_NODE_SET_DOMAIN, unresolved_nodes),
        "unresolved_edge_set_sha256": structured_set_digest(
            UNRESOLVED_EDGE_SET_DOMAIN, unresolved_edges),
        "crosswalk_set_sha256": structured_set_digest(
            CROSSWALK_SET_DOMAIN, crosswalk),
    }


def build_snapshot(graph: dict[str, object], project_id: str, request_sha256: str,
                   sources, extractors, nodes, edges, unresolved_nodes,
                   unresolved_edges, crosswalk) -> dict[str, object]:
    if len(edges) > MAX_EDGE_UNION:
        raise ContractError("ADR-0065 resolved edge union limit exceeded")
    coverage = build_coverage(graph, len(nodes), len(edges))
    digests = _set_digests(sources, extractors, nodes, edges, unresolved_nodes,
                           unresolved_edges, crosswalk)
    coverage_sha = domain_digest(
        COVERAGE_DOMAIN, coverage, max_bytes=MAX_SNAPSHOT_BYTES)
    snapshot = {
        "adr_0062_node_crosswalk": crosswalk, "api_version": SNAPSHOT_API,
        "canonicalization": CANONICALIZATION, "coverage": coverage,
        "coverage_sha256": coverage_sha, "edges": edges,
        "extractors": extractors, "freshness": build_freshness(graph),
        "nodes": nodes, "profile_id": PROFILE_ID, "project_id": project_id,
        "request_sha256": request_sha256, "result": RESULT,
        "snapshot_id": "", "snapshot_identity_sha256": "",
        "snapshot_sha256": "", "sources": sources,
        "system_knowledge_status": "unknown",
        "system_unknown_reason_codes": list(SYSTEM_UNKNOWN_REASONS),
        "unresolved_edges": unresolved_edges, "unresolved_nodes": unresolved_nodes,
        **digests,
    }
    identity = {field: snapshot[field] for field in IDENTITY_FIELDS["snapshot"]}
    identity_sha = domain_digest(
        SNAPSHOT_IDENTITY_DOMAIN, identity, max_bytes=MAX_SNAPSHOT_BYTES)
    snapshot["snapshot_identity_sha256"] = identity_sha
    snapshot["snapshot_id"] = "graph-snapshot-" + identity_sha
    ensure_global_identity_uniqueness([
        (sources, "source_identity_sha256"),
        (extractors, "extractor_identity_sha256"),
        (nodes, "node_identity_sha256"),
        (edges, "edge_identity_sha256"),
        (unresolved_nodes, "unresolved_node_identity_sha256"),
        (unresolved_edges, "unresolved_edge_identity_sha256"),
        (crosswalk, "adr_0062_node_sha256"),
    ], identity_sha)
    snapshot["snapshot_sha256"] = self_digest(
        SNAPSHOT_DOMAIN, snapshot, "snapshot_sha256", max_bytes=MAX_SNAPSHOT_BYTES)
    return snapshot
