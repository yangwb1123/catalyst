"""ADR-0066 set seals and final profile-bound GraphSnapshot record."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import domain_digest, self_digest
from .coverage import build_freshness
from .records import ensure_global_identity_uniqueness, structured_set_digest
from .lexical_test_source_constants import (
    CANONICALIZATION, COVERAGE_DOMAIN, CROSSWALK_SET_DOMAIN, EDGE_SET_DOMAIN,
    EXTRACTOR_SET_DOMAIN, IDENTITY_FIELDS, MAX_EDGE_UNION, MAX_NODES,
    MAX_SNAPSHOT_BYTES, NODE_SET_DOMAIN, RESULT, SNAPSHOT_API, SNAPSHOT_DOMAIN,
    SNAPSHOT_IDENTITY_DOMAIN, SOURCE_SET_DOMAIN, SYSTEM_UNKNOWN_REASONS,
    TEST_SOURCE_PROFILE_ID, UNRESOLVED_EDGE_SET_DOMAIN,
    UNRESOLVED_NODE_SET_DOMAIN,
)
from .lexical_test_source_coverage import build_coverage


def _set_digests(sources, extractors, nodes, edges, unresolved_nodes,
                 unresolved_edges, crosswalk):
    return {
        "source_set_sha256": structured_set_digest(SOURCE_SET_DOMAIN, sources),
        "extractor_set_sha256": structured_set_digest(EXTRACTOR_SET_DOMAIN, extractors),
        "node_set_sha256": structured_set_digest(NODE_SET_DOMAIN, nodes),
        "edge_set_sha256": structured_set_digest(
            EDGE_SET_DOMAIN, edges, max_items=MAX_EDGE_UNION),
        "unresolved_node_set_sha256": structured_set_digest(
            UNRESOLVED_NODE_SET_DOMAIN, unresolved_nodes),
        "unresolved_edge_set_sha256": structured_set_digest(
            UNRESOLVED_EDGE_SET_DOMAIN, unresolved_edges),
        "crosswalk_set_sha256": structured_set_digest(
            CROSSWALK_SET_DOMAIN, crosswalk),
    }


def _seal_snapshot(snapshot, sources, extractors, nodes, edges,
                   unresolved_nodes, unresolved_edges, crosswalk):
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


def build_snapshot(graph, project_id: str, request_sha256: str, sources,
                   extractors, nodes, edges, unresolved_nodes,
                   unresolved_edges, crosswalk):
    if len(nodes) > MAX_NODES or len(edges) > MAX_EDGE_UNION:
        raise ContractError("ADR-0066 resolved node or edge union limit exceeded")
    coverage = build_coverage(graph, nodes, edges)
    digests = _set_digests(
        sources, extractors, nodes, edges, unresolved_nodes, unresolved_edges,
        crosswalk)
    snapshot = {
        "adr_0062_node_crosswalk": crosswalk, "api_version": SNAPSHOT_API,
        "canonicalization": CANONICALIZATION, "coverage": coverage,
        "coverage_sha256": domain_digest(
            COVERAGE_DOMAIN, coverage, max_bytes=MAX_SNAPSHOT_BYTES),
        "edges": edges, "extractors": extractors,
        "freshness": build_freshness(graph), "nodes": nodes,
        "profile_id": TEST_SOURCE_PROFILE_ID, "project_id": project_id,
        "request_sha256": request_sha256, "result": RESULT,
        "snapshot_id": "", "snapshot_identity_sha256": "",
        "snapshot_sha256": "", "sources": sources,
        "system_knowledge_status": "unknown",
        "system_unknown_reason_codes": list(SYSTEM_UNKNOWN_REASONS),
        "unresolved_edges": unresolved_edges, "unresolved_nodes": unresolved_nodes,
        **digests,
    }
    return _seal_snapshot(
        snapshot, sources, extractors, nodes, edges, unresolved_nodes,
        unresolved_edges, crosswalk)
