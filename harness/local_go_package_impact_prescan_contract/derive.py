"""Deterministic ADR-0062 request, closure, witness, and report derivation."""

from __future__ import annotations

import base64
import heapq
from collections import deque

from governance_contract import ContractError

from .codec import canonical_json, self_digest
from .constants import (CANONICALIZATION, CLOSURE_GAPS, ENVELOPE_API,
                        ENVELOPE_DOMAIN, MAX_AGGREGATE_WITNESS_HOPS,
                        MAX_ENVELOPE_BYTES, MAX_REPORT_BYTES, MAX_REQUEST_BYTES,
                        MAX_WITNESS_HOPS, REPORT_API, REPORT_DOMAIN, REQUEST_API,
                        REQUEST_DOMAIN, RESULT, SYSTEM_UNKNOWN_REASONS)
from .graph import (graph_edges, graph_nodes, observation_digest,
                    validate_graph_bytes)
from .profiles import (path_directory, path_is_within, utf8_key,
                       validate_changed_paths, validate_run_id)


def _resolve_seeds(graph: dict[str, object], changed_paths: list[str],
                   by_package: dict[tuple[str, str], dict[str, object]]):
    module = graph["module"]["directory"]
    nested = [item["directory"] for item in graph["module"]["nested_modules"]]
    files = {item["path"]: item for item in graph["files"]}
    diagnostics = {item["path"]: item["code"] for item in graph["diagnostics"]}
    grouped: dict[str, list[str]] = {}
    unresolved = []
    for path in changed_paths:
        reason, code = _unresolved_reason(path, module, nested, files, diagnostics)
        if reason is not None:
            unresolved.append({"changed_path": path, "diagnostic_code": code,
                               "reason": reason})
            continue
        file_value = files[path]
        key = (path_directory(path), file_value["package_name"])
        node = by_package.get(key)
        if node is None:
            raise ContractError("resolved graph file has no exact package node")
        grouped.setdefault(node["node_id"], []).append(path)
    resolved = [{"changed_paths": sorted(paths, key=utf8_key), "node_id": node_id}
                for node_id, paths in grouped.items()]
    resolved.sort(key=lambda item: item["node_id"])
    unresolved.sort(key=lambda item: utf8_key(item["changed_path"]))
    return resolved, unresolved


def _unresolved_reason(path: str, module: str, nested: list[str],
                       files: dict[str, dict[str, object]],
                       diagnostics: dict[str, str]):
    if not path_is_within(path, module):
        return "outside_selected_module", None
    if any(path_is_within(path, directory) for directory in nested):
        return "inside_nested_module_boundary", None
    if not path.endswith(".go"):
        return "not_a_go_file", None
    if path in diagnostics:
        return "go_file_diagnostic", diagnostics[path]
    if path not in files:
        return "not_in_observed_file_or_diagnostic", None
    return None, None


def _reverse_closure(seed_ids: set[str], edges: list[dict[str, object]]) -> set[str]:
    reverse: dict[str, list[str]] = {}
    for edge in edges:
        reverse.setdefault(edge["to_node_id"], []).append(edge["from_node_id"])
    reachable = set(seed_ids)
    pending = deque(sorted(seed_ids))
    while pending:
        dependency = pending.popleft()
        for importer in sorted(reverse.get(dependency, [])):
            if importer not in reachable:
                reachable.add(importer)
                pending.append(importer)
    return reachable


def _shortest_witnesses(seed_ids: set[str], reachable: set[str],
                        edges: list[dict[str, object]]) -> dict[str, dict[str, object]]:
    reverse: dict[str, list[tuple[str, str]]] = {}
    for edge in edges:
        reverse.setdefault(edge["to_node_id"], []).append(
            (edge["edge_id"], edge["from_node_id"]))
    for values in reverse.values():
        values.sort()
    pending = []
    for seed in sorted(seed_ids):
        heapq.heappush(pending, (0, seed, (), (seed,), seed))
    settled: dict[str, dict[str, object]] = {}
    while pending:
        hops, seed, edge_ids, node_ids, node = heapq.heappop(pending)
        if node in settled:
            continue
        if hops > MAX_WITNESS_HOPS:
            raise ContractError("ADR-0062 shortest witness exceeds per-node hop limit")
        settled[node] = {
            "edge_ids": list(edge_ids), "hop_count": hops,
            "node_ids": list(node_ids), "seed_node_id": seed,
        }
        for edge_id, importer in reverse.get(node, []):
            if importer not in node_ids:
                heapq.heappush(pending, (
                    hops + 1, seed, edge_ids + (edge_id,),
                    node_ids + (importer,), importer,
                ))
    if set(settled) != reachable:
        raise ContractError("ADR-0062 witness set does not cover reverse fixed point")
    if sum(item["hop_count"] for item in settled.values()) > MAX_AGGREGATE_WITNESS_HOPS:
        raise ContractError("ADR-0062 aggregate witness hops exceed limit")
    return settled


def _reachable_outputs(nodes: list[dict[str, object]], edges: list[dict[str, object]],
                       seed_ids: set[str]):
    reachable = _reverse_closure(seed_ids, edges)
    witnesses = _shortest_witnesses(seed_ids, reachable, edges)
    output_nodes = [dict(node, witness=witnesses[node["node_id"]])
                    for node in nodes if node["node_id"] in reachable]
    output_edges = [edge for edge in edges
                    if edge["from_node_id"] in reachable and
                    edge["to_node_id"] in reachable]
    return output_nodes, output_edges


def _closure(graph: dict[str, object], unresolved: list[dict[str, object]]):
    reasons = set()
    if unresolved:
        reasons.add("changed_path_unresolved")
    if graph["diagnostics"]:
        reasons.add("go_file_diagnostic_present")
    for dependency in graph["dependencies"]:
        reason = CLOSURE_GAPS.get(dependency["resolution"])
        if reason is not None:
            reasons.add(reason)
    ordered = sorted(reasons, key=utf8_key)
    status = "unknown" if ordered else "complete_within_observation"
    return status, ordered


def derive_report(request: dict[str, object], graph: dict[str, object]) -> dict[str, object]:
    """Derive the only valid report for a validated request and graph."""
    changed_paths = validate_changed_paths(request["changed_paths"])
    nodes, by_package = graph_nodes(graph)
    edges = graph_edges(graph, by_package)
    resolved, unresolved = _resolve_seeds(graph, changed_paths, by_package)
    seed_ids = {item["node_id"] for item in resolved}
    reachable_nodes, reachable_edges = _reachable_outputs(nodes, edges, seed_ids)
    status, reasons = _closure(graph, unresolved)
    report = {
        "api_version": REPORT_API, "canonicalization": CANONICALIZATION,
        "closure_reason_codes": reasons,
        "graph_observation_sha256": request["graph_observation_sha256"],
        "package_lexical_closure_status": status,
        "reachable_edges": reachable_edges, "reachable_nodes": reachable_nodes,
        "report_sha256": "", "request_sha256": request["request_sha256"],
        "resolved_seeds": resolved, "result": RESULT, "run_id": request["run_id"],
        "system_impact_status": "unknown",
        "system_unknown_reason_codes": list(SYSTEM_UNKNOWN_REASONS),
        "unresolved_seeds": unresolved,
    }
    report["report_sha256"] = self_digest(
        REPORT_DOMAIN, report, "report_sha256", max_bytes=MAX_REPORT_BYTES)
    canonical_json(report, max_bytes=MAX_REPORT_BYTES)
    return report


def _request(graph_bytes: bytes, changed_paths: object, run_id: object):
    graph = validate_graph_bytes(graph_bytes)
    paths = validate_changed_paths(changed_paths)
    checked_run_id = validate_run_id(run_id)
    if graph["producer"]["run_id"] != checked_run_id:
        raise ContractError("request.run_id does not match graph producer.run_id")
    request = {
        "api_version": REQUEST_API, "canonicalization": CANONICALIZATION,
        "changed_paths": paths,
        "graph_observation_base64url": base64.urlsafe_b64encode(
            graph_bytes).rstrip(b"=").decode("ascii"),
        "graph_observation_sha256": observation_digest(graph),
        "request_sha256": "", "run_id": checked_run_id,
    }
    request["request_sha256"] = self_digest(
        REQUEST_DOMAIN, request, "request_sha256", max_bytes=MAX_REQUEST_BYTES)
    canonical_json(request, max_bytes=MAX_REQUEST_BYTES)
    return request, graph


def derive_envelope(graph_bytes: bytes, changed_paths: object,
                    run_id: object) -> dict[str, object]:
    """Build the unique canonical envelope without repository or process access."""
    request, graph = _request(graph_bytes, changed_paths, run_id)
    report = derive_report(request, graph)
    envelope = {
        "api_version": ENVELOPE_API, "canonicalization": CANONICALIZATION,
        "envelope_sha256": "", "report": report, "request": request,
    }
    envelope["envelope_sha256"] = self_digest(
        ENVELOPE_DOMAIN, envelope, "envelope_sha256", max_bytes=MAX_ENVELOPE_BYTES)
    canonical_json(envelope, max_bytes=MAX_ENVELOPE_BYTES)
    return envelope
