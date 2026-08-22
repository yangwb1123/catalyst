"""ADR-0066 disjoint Go/test coverage and conservative UNKNOWN semantics."""

from __future__ import annotations

from governance_contract import ContractError

from .profiles import utf8_key
from .lexical_test_source_constants import (
    GO_BASE_REASONS, RESOLUTION_COVERAGE_REASONS, SURFACES, TEST_BASE_REASONS,
)


def _conditional_reasons(graph: dict[str, object], role: str) -> set[str]:
    reasons: set[str] = set()
    for diagnostic in graph["diagnostics"]:
        diagnostic_role = "test" if diagnostic["path"].endswith("_test.go") else "compile"
        if diagnostic_role == role:
            reasons.add("go_file_diagnostic_present")
    for dependency in graph["dependencies"]:
        if dependency["role"] != role:
            continue
        reason = RESOLUTION_COVERAGE_REASONS.get(dependency["resolution"])
        if reason is not None:
            reasons.add(reason)
    if graph["module"]["nested_modules"]:
        reasons.add("nested_module_boundary_present")
    if graph["coverage"]["go_entries_excluded_nonregular"]:
        reasons.add("nonregular_go_entries_not_located")
    return reasons


def reason_codes(graph: dict[str, object], role: str) -> list[str]:
    base = GO_BASE_REASONS if role == "compile" else TEST_BASE_REASONS
    return sorted(set(base) | _conditional_reasons(graph, role), key=utf8_key)


def _partition_counts(nodes, edges) -> tuple[int, int, int, int]:
    types = {item["node_id"]: item["node_type"] for item in nodes}
    go_nodes = sum(item["node_type"] != "test" for item in nodes)
    test_nodes = len(nodes) - go_nodes
    go_edges = test_edges = 0
    for edge in edges:
        if edge["relation"] == "contains":
            is_test = types.get(edge["to_node_id"]) == "test"
        elif edge["relation"] == "depends_on":
            is_test = edge["source_role"] == "test"
        else:
            raise ContractError("ADR-0066 coverage saw unsupported relation")
        test_edges += int(is_test)
        go_edges += int(not is_test)
    if go_nodes + test_nodes != len(nodes) or go_edges + test_edges != len(edges):
        raise ContractError("ADR-0066 coverage partition is not disjoint and complete")
    return go_nodes, test_nodes, go_edges, test_edges


def _not_observed(surface: str) -> dict[str, object]:
    return {
        "edge_count": 0, "node_count": 0,
        "reason_codes": [surface + "_surface_not_observed"],
        "status": "not_observed", "surface": surface,
    }


def build_coverage(graph: dict[str, object], nodes, edges) -> dict[str, object]:
    go_nodes, test_nodes, go_edges, test_edges = _partition_counts(nodes, edges)
    surfaces = []
    for surface in SURFACES:
        if surface == "go_module_package_lexical":
            item = {"edge_count": go_edges, "node_count": go_nodes,
                    "reason_codes": reason_codes(graph, "compile"),
                    "status": "partial", "surface": surface}
        elif surface == "test_verification":
            item = {"edge_count": test_edges, "node_count": test_nodes,
                    "reason_codes": reason_codes(graph, "test"),
                    "status": "partial", "surface": surface}
        else:
            item = _not_observed(surface)
        surfaces.append(item)
    return {"status": "partial", "surface_count": 11, "surfaces": surfaces}
