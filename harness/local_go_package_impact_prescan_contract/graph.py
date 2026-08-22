"""ADR-0053 graph compatibility exports and ADR-0062 graph identities."""

from __future__ import annotations

from governance_contract import ContractError
from go_package_dependency_graph_observation_producer.graph_contract import (
    observation_digest, validate_graph_bytes,
)

from .codec import domain_digest
from .constants import (
    EDGE_DOMAIN, EDGE_IDENTITY_FIELDS, MAX_EDGES, MAX_GRAPH_BYTES, MAX_NODES,
    MAX_SOURCE_PATHS, NODE_DOMAIN, NODE_IDENTITY_FIELDS,
)


def graph_nodes(graph: dict[str, object]) -> tuple[list[dict[str, object]], dict[tuple[str, str], dict[str, object]]]:
    nodes: list[dict[str, object]] = []
    by_package: dict[tuple[str, str], dict[str, object]] = {}
    seen: set[str] = set()
    for package in graph["packages"]:
        identity = {
            "directory": package["directory"], "import_path": package["import_path"],
            "module_path": graph["module"]["module_path"], "package_name": package["name"],
        }
        assert set(identity) == NODE_IDENTITY_FIELDS
        digest = domain_digest(NODE_DOMAIN, identity, max_bytes=MAX_GRAPH_BYTES)
        node = dict(identity, node_id="go-package-node-" + digest, node_sha256=digest)
        if node["node_id"] in seen:
            raise ContractError("ADR-0062 node identity digest collision")
        seen.add(node["node_id"])
        nodes.append(node)
        by_package[(package["directory"], package["name"])] = node
    if len(nodes) > MAX_NODES or len(by_package) != len(nodes):
        raise ContractError("ADR-0062 package node set is not unique or bounded")
    return sorted(nodes, key=lambda item: item["node_id"]), by_package


def graph_edges(graph: dict[str, object], by_package: dict[tuple[str, str], dict[str, object]]) -> list[dict[str, object]]:
    edges: list[dict[str, object]] = []
    seen: set[str] = set()
    for dependency in graph["dependencies"]:
        if dependency["resolution"] != "local":
            continue
        if dependency["resolution_detail"] is not None:
            raise ContractError("ADR-0062 local dependency has non-null resolution detail")
        source = by_package.get((dependency["from_directory"], dependency["from_package_name"]))
        target = by_package.get((dependency["target_directory"], dependency["target_package_name"]))
        if source is None or target is None:
            raise ContractError("ADR-0062 local dependency lacks exact package endpoint")
        projection = {
            "from_node_id": source["node_id"], "import_path": dependency["import_path"],
            "relation": dependency["relation"], "role": dependency["role"],
            "source_paths": dependency["source_paths"], "to_node_id": target["node_id"],
        }
        assert set(projection) == EDGE_IDENTITY_FIELDS
        if len(projection["source_paths"]) > MAX_SOURCE_PATHS:
            raise ContractError("ADR-0062 local edge source path limit exceeded")
        digest = domain_digest(EDGE_DOMAIN, projection, max_bytes=MAX_GRAPH_BYTES)
        edge = dict(projection, edge_id="go-package-edge-" + digest, edge_sha256=digest)
        if edge["edge_id"] in seen:
            raise ContractError("ADR-0062 edge identity digest collision")
        seen.add(edge["edge_id"])
        edges.append(edge)
    if len(edges) > MAX_EDGES:
        raise ContractError("ADR-0062 local edge set exceeds limit")
    return sorted(edges, key=lambda item: item["edge_id"])


__all__ = [
    "graph_edges", "graph_nodes", "observation_digest", "validate_graph_bytes",
]
