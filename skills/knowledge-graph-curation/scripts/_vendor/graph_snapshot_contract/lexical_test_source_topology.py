"""ADR-0066 lexical test-source nodes and disjoint structural edges."""

from __future__ import annotations

from governance_contract import ContractError

from .profiles import locator_key, relative_name, sorted_unique, validate_repo_path
from .records import ensure_unique_ids, knowledge_defaults, seal_record
from .lexical_test_source_constants import (
    MAX_AGGREGATE_LOCATORS, MAX_EDGE_UNION, MAX_LOCATORS_PER_RECORD,
    MAX_NODES, MAX_TEST_CONTAINS_EDGES, MAX_TEST_NODES,
)
from .topology import build_edges as build_legacy_edges
from .topology import build_nodes as build_legacy_nodes


def _test_locators(package: dict[str, object], files, source_id: str):
    locators = []
    for path in package["test_files"]:
        item = files.get(path)
        if item is None or item["role"] != "test":
            raise ContractError("ADR-0066 test locator lacks exact upstream file")
        locators.append({
            "content_sha256": item["content_sha256"],
            "path": validate_repo_path(path, "test source locator"),
            "role": "test", "source_id": source_id,
        })
    if not 1 <= len(locators) <= MAX_LOCATORS_PER_RECORD:
        raise ContractError("ADR-0066 test locator set is empty or exceeds limit")
    return sorted_unique(locators, locator_key, "test source locators")


def _test_node(graph, package, project_id: str, locators,
               source_id: str, extractor_sha256: str):
    relative = relative_name(
        package["directory"], graph["module"]["directory"],
        "test-source package directory")
    body = {
        "identity_namespace": "go",
        "identity_profile_id": (
            "go-test-source-set-module-relative-directory-package-name-v1"),
        "node_type": "test", "project_id": project_id,
        "qualified_name_components": [
            graph["module"]["module_path"], relative, package["name"],
        ],
    }
    body.update(knowledge_defaults())
    body.update({
        "extractor_sha256s": [extractor_sha256], "source_ids": [source_id],
        "source_locators": locators,
    })
    return seal_record("node", body)


def _build_test_nodes(graph, project_id: str, files, source, extractor):
    nodes, by_package, locators_by_package = [], {}, {}
    for package in graph["packages"]:
        if not package["test_files"]:
            continue
        key = (package["directory"], package["name"])
        if key in by_package:
            raise ContractError("ADR-0066 upstream package maps to multiple test nodes")
        locators = _test_locators(package, files, source["source_id"])
        node = _test_node(
            graph, package, project_id, locators, source["source_id"],
            extractor["extractor_sha256"])
        nodes.append(node)
        by_package[key], locators_by_package[key] = node, locators
    if len(nodes) > MAX_TEST_NODES:
        raise ContractError("ADR-0066 test node limit exceeded")
    return nodes, by_package, locators_by_package


def build_nodes(graph, project_id: str, source, extractor):
    legacy, module, by_package, package_locators, files = build_legacy_nodes(
        graph, project_id, source, extractor)
    tests, by_test_package, test_locators = _build_test_nodes(
        graph, project_id, files, source, extractor)
    nodes = sorted(legacy + tests, key=lambda item: item["node_id"])
    if len(nodes) > MAX_NODES:
        raise ContractError("ADR-0066 node union limit exceeded")
    ensure_unique_ids(nodes, "node_id", "ADR-0066 node union")
    return (nodes, module, by_package, package_locators, by_test_package,
            test_locators, files)


def _test_contains_edge(module, node, locators, source, extractor):
    body = {
        "category_axes": ["structural"], "from_node_id": module["node_id"],
        "identity_profile_id": "graph-edge-semantic-endpoints-v1",
        "import_discriminator": None, "parallel_discriminator": "contains",
        "relation": "contains", "source_role": None,
        "to_node_id": node["node_id"],
    }
    body.update(knowledge_defaults())
    body.update({
        "extractor_sha256s": [extractor["extractor_sha256"]],
        "source_ids": [source["source_id"]], "source_locators": locators,
    })
    return seal_record("edge", body)


def build_edges(graph, module, by_package, package_locators, by_test_package,
                test_locators, files, nodes, source, extractor):
    edges = build_legacy_edges(
        graph, module, by_package, package_locators, files, source, extractor)
    test_edges = [_test_contains_edge(
        module, node, test_locators[key], source, extractor)
        for key, node in by_test_package.items()]
    if len(test_edges) > MAX_TEST_CONTAINS_EDGES:
        raise ContractError("ADR-0066 test contains edge limit exceeded")
    edges = sorted(edges + test_edges, key=lambda item: item["edge_id"])
    if len(edges) > MAX_EDGE_UNION:
        raise ContractError("ADR-0066 resolved edge union limit exceeded")
    ensure_unique_ids(edges, "edge_id", "ADR-0066 resolved edge union")
    endpoints = {item["node_id"] for item in nodes}
    if any(item["from_node_id"] not in endpoints or
           item["to_node_id"] not in endpoints for item in edges):
        raise ContractError("ADR-0066 resolved edge endpoint is dangling")
    return edges


def enforce_locator_aggregate(*groups: list[dict[str, object]]) -> None:
    total = sum(len(record["source_locators"])
                for records in groups for record in records)
    if total > MAX_AGGREGATE_LOCATORS:
        raise ContractError("ADR-0066 aggregate source locator limit exceeded")
