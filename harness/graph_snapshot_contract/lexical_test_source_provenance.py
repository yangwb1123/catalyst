"""ADR-0066 lexical test-source profile-bound projector provenance."""

from __future__ import annotations

from .constants import GRAPH_API, GRAPH_PROFILE, TEST_SOURCE_PROFILE_ID
from .records import seal_record


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
        "projector_profile_id": TEST_SOURCE_PROFILE_ID,
    }
    return seal_record("extractor", body)
