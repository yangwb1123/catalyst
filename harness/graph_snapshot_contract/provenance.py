"""Distinct upstream source and projector provenance records."""

from __future__ import annotations

from .constants import GRAPH_API, GRAPH_PROFILE, PROFILE_ID
from .records import seal_record


def source_record(graph: dict[str, object], graph_sha256: str) -> dict[str, object]:
    body = {
        "graph_api_version": GRAPH_API,
        "graph_observation_sha256": graph_sha256,
        "graph_profile_id": GRAPH_PROFILE,
        "observed_at_unix_ms": graph["observed_at_unix_ms"],
        "observer_parameters_sha256": graph["producer"]["parameters_sha256"],
        "observer_producer_id": graph["producer"]["producer_id"],
        "observer_producer_type": graph["producer"]["producer_type"],
        "observer_producer_version": graph["producer"]["producer_version"],
        "observer_run_id": graph["producer"]["run_id"],
        "source_revision": graph["source"]["source_revision"],
        "source_tree_sha256": graph["source"]["source_tree_sha256"],
        "source_type": "adr_0053_graph_observation",
    }
    return seal_record("source", body)


def extractor_record(source: dict[str, object]) -> dict[str, object]:
    body = {
        "extractor_type": "graph_snapshot_projector",
        "extractor_version": "v1",
        "input_graph_api_version": GRAPH_API,
        "input_graph_profile_id": GRAPH_PROFILE,
        "input_source_id": source["source_id"],
        "producer_id": "forgeos.local-go-graph-snapshot-projector",
        "producer_type": "tool",
        "producer_version": "v1",
        "projector_profile_id": PROFILE_ID,
    }
    return seal_record("extractor", body)
