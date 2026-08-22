"""Exact partial coverage, zero-duration freshness, and system UNKNOWN."""

from __future__ import annotations

from .constants import (
    FRESHNESS_REASONS, GO_COVERAGE_BASE_REASONS, RESOLUTION_COVERAGE_REASONS,
    SURFACES,
)
from .profiles import utf8_key


def go_reason_codes(graph: dict[str, object]) -> list[str]:
    reasons = set(GO_COVERAGE_BASE_REASONS)
    if graph["diagnostics"]:
        reasons.add("go_file_diagnostic_present")
    if graph["module"]["nested_modules"]:
        reasons.add("nested_module_boundary_present")
    if graph["coverage"]["go_entries_excluded_nonregular"]:
        reasons.add("nonregular_go_entries_not_located")
    for dependency in graph["dependencies"]:
        reason = RESOLUTION_COVERAGE_REASONS.get(dependency["resolution"])
        if reason is not None:
            reasons.add(reason)
    return sorted(reasons, key=utf8_key)


def build_coverage(graph: dict[str, object], node_count: int,
                   edge_count: int) -> dict[str, object]:
    surfaces = []
    for surface in SURFACES:
        if surface == "go_module_package_lexical":
            surfaces.append({
                "edge_count": edge_count, "node_count": node_count,
                "reason_codes": go_reason_codes(graph), "status": "partial",
                "surface": surface,
            })
        else:
            surfaces.append({
                "edge_count": 0, "node_count": 0,
                "reason_codes": [surface + "_surface_not_observed"],
                "status": "not_observed", "surface": surface,
            })
    return {"status": "partial", "surface_count": 11, "surfaces": surfaces}


def build_freshness(graph: dict[str, object]) -> dict[str, object]:
    observed = graph["observed_at_unix_ms"]
    return {
        "expires_at_unix_ms": observed, "observed_at_unix_ms": observed,
        "reason_codes": list(FRESHNESS_REASONS), "status": "unknown",
    }
