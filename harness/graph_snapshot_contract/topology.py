"""Stable module/package nodes, resolved edges, locators, and crosswalk."""

from __future__ import annotations

from governance_contract import ContractError

from .codec import domain_digest
from .constants import (
    ADR_0062_NODE_DOMAIN, MAX_AGGREGATE_LOCATORS, MAX_CONTAINS_EDGES,
    MAX_CROSSWALKS, MAX_DEPENDENCY_CANDIDATES, MAX_GRAPH_BYTES,
    MAX_LOCATORS_PER_RECORD, MAX_NODES, MAX_PACKAGE_NODES,
)
from .profiles import locator_key, relative_name, sorted_unique, validate_repo_path
from .records import ensure_unique_ids, knowledge_defaults, seal_record


def _locator(path: str, role: str, digest: str | None,
             source_id: str) -> dict[str, object]:
    return {
        "content_sha256": digest, "path": validate_repo_path(path, "source locator"),
        "role": role, "source_id": source_id,
    }


def file_index(graph: dict[str, object]) -> dict[str, dict[str, object]]:
    return {item["path"]: item for item in graph["files"]}


def package_locators(package: dict[str, object], files: dict[str, dict[str, object]],
                     source_id: str) -> list[dict[str, object]]:
    locators = []
    for role, field in (("compile", "compile_files"), ("test", "test_files")):
        for path in package[field]:
            item = files.get(path)
            if item is None or item["role"] != role:
                raise ContractError("ADR-0065 package locator lacks exact upstream file")
            locators.append(_locator(path, role, item["content_sha256"], source_id))
    if not 1 <= len(locators) <= MAX_LOCATORS_PER_RECORD:
        raise ContractError("ADR-0065 package locator set is empty or exceeds limit")
    return sorted_unique(locators, locator_key, "package locators")


def dependency_locators(dependency: dict[str, object],
                        files: dict[str, dict[str, object]],
                        source_id: str) -> list[dict[str, object]]:
    locators = []
    for path in dependency["source_paths"]:
        item = files.get(path)
        if item is None or item["role"] != dependency["role"]:
            raise ContractError("ADR-0065 dependency locator lacks exact upstream file")
        locators.append(_locator(
            path, dependency["role"], item["content_sha256"], source_id))
    if not 1 <= len(locators) <= MAX_LOCATORS_PER_RECORD:
        raise ContractError("ADR-0065 dependency locator set is empty or exceeds limit")
    return sorted_unique(locators, locator_key, "dependency locators")


def _node(body: dict[str, object], locators: list[dict[str, object]],
          source_id: str, extractor_sha256: str) -> dict[str, object]:
    body.update(knowledge_defaults())
    body.update({
        "extractor_sha256s": [extractor_sha256], "source_ids": [source_id],
        "source_locators": locators,
    })
    return seal_record("node", body)


def _module_node(graph: dict[str, object], project_id: str, source_id: str,
                 extractor_sha256: str) -> dict[str, object]:
    module = graph["module"]
    locators = [_locator(module["go_mod_path"], "go_mod",
                         module["go_mod_content_sha256"], source_id)]
    return _node({
        "identity_namespace": "go", "identity_profile_id": "go-module-path-v1",
        "node_type": "module", "project_id": project_id,
        "qualified_name_components": [module["module_path"]],
    }, locators, source_id, extractor_sha256)


def _package_node(graph: dict[str, object], package: dict[str, object],
                  project_id: str, locators: list[dict[str, object]],
                  source_id: str, extractor_sha256: str) -> dict[str, object]:
    relative = relative_name(package["directory"], graph["module"]["directory"],
                             "package directory")
    return _node({
        "identity_namespace": "go",
        "identity_profile_id": "go-package-module-relative-directory-name-v1",
        "node_type": "package", "project_id": project_id,
        "qualified_name_components": [
            graph["module"]["module_path"], relative, package["name"],
        ],
    }, locators, source_id, extractor_sha256)


def build_nodes(graph: dict[str, object], project_id: str, source: dict[str, object],
                extractor: dict[str, object]):
    packages = graph["packages"]
    if len(packages) > MAX_PACKAGE_NODES:
        raise ContractError("ADR-0065 package node limit exceeded")
    files = file_index(graph)
    module = _module_node(graph, project_id, source["source_id"],
                          extractor["extractor_sha256"])
    package_nodes, by_package, locators_by_package = [], {}, {}
    for package in packages:
        key = (package["directory"], package["name"])
        locators = package_locators(package, files, source["source_id"])
        node = _package_node(graph, package, project_id, locators,
                             source["source_id"], extractor["extractor_sha256"])
        if key in by_package:
            raise ContractError("ADR-0065 upstream package maps more than once")
        package_nodes.append(node)
        by_package[key], locators_by_package[key] = node, locators
    nodes = sorted([module] + package_nodes, key=lambda item: item["node_id"])
    if len(nodes) > MAX_NODES:
        raise ContractError("ADR-0065 node union limit exceeded")
    ensure_unique_ids(nodes, "node_id", "node union")
    return nodes, module, by_package, locators_by_package, files


def _edge(body: dict[str, object], locators: list[dict[str, object]],
          source: dict[str, object], extractor: dict[str, object]):
    body.update(knowledge_defaults())
    body.update({
        "extractor_sha256s": [extractor["extractor_sha256"]],
        "source_ids": [source["source_id"]], "source_locators": locators,
    })
    return seal_record("edge", body)


def _contains_edges(module: dict[str, object], by_package, locators_by_package,
                    source: dict[str, object], extractor: dict[str, object]):
    edges = []
    for key, node in by_package.items():
        body = {
            "category_axes": ["structural"], "from_node_id": module["node_id"],
            "identity_profile_id": "graph-edge-semantic-endpoints-v1",
            "import_discriminator": None, "parallel_discriminator": "contains",
            "relation": "contains", "source_role": None,
            "to_node_id": node["node_id"],
        }
        edges.append(_edge(body, locators_by_package[key], source, extractor))
    if len(edges) > MAX_CONTAINS_EDGES:
        raise ContractError("ADR-0065 contains edge limit exceeded")
    return edges


def _dependency_edges(graph: dict[str, object], by_package, files,
                      source: dict[str, object], extractor: dict[str, object]):
    edges = []
    for dependency in graph["dependencies"]:
        if dependency["resolution"] != "local":
            continue
        source_node = by_package.get(
            (dependency["from_directory"], dependency["from_package_name"]))
        target_node = by_package.get(
            (dependency["target_directory"], dependency["target_package_name"]))
        if source_node is None or target_node is None:
            raise ContractError("ADR-0065 local dependency has dangling endpoint")
        body = {
            "category_axes": ["static_source"], "from_node_id": source_node["node_id"],
            "identity_profile_id": "graph-edge-semantic-endpoints-v1",
            "import_discriminator": dependency["import_path"],
            "parallel_discriminator": dependency["role"] + ":" + dependency["import_path"],
            "relation": "depends_on", "source_role": dependency["role"],
            "to_node_id": target_node["node_id"],
        }
        edges.append(_edge(body, dependency_locators(
            dependency, files, source["source_id"]), source, extractor))
    return edges


def build_edges(graph: dict[str, object], module: dict[str, object], by_package,
                locators_by_package, files, source: dict[str, object],
                extractor: dict[str, object]) -> list[dict[str, object]]:
    if len(graph["dependencies"]) > MAX_DEPENDENCY_CANDIDATES:
        raise ContractError("ADR-0065 dependency candidate limit exceeded")
    edges = _contains_edges(module, by_package, locators_by_package, source, extractor)
    edges.extend(_dependency_edges(graph, by_package, files, source, extractor))
    edges.sort(key=lambda item: item["edge_id"])
    ensure_unique_ids(edges, "edge_id", "resolved edge union")
    endpoint_ids = {module["node_id"]} | {item["node_id"] for item in by_package.values()}
    if any(edge["from_node_id"] not in endpoint_ids or
           edge["to_node_id"] not in endpoint_ids for edge in edges):
        raise ContractError("ADR-0065 resolved edge endpoint is dangling")
    return edges


def build_crosswalk(graph: dict[str, object], by_package) -> list[dict[str, object]]:
    crosswalk = []
    for package in graph["packages"]:
        identity = {
            "directory": package["directory"], "import_path": package["import_path"],
            "module_path": graph["module"]["module_path"],
            "package_name": package["name"],
        }
        digest = domain_digest(ADR_0062_NODE_DOMAIN, identity, max_bytes=MAX_GRAPH_BYTES)
        node = by_package[(package["directory"], package["name"])]
        crosswalk.append({
            "adr_0062_node_id": "go-package-node-" + digest,
            "adr_0062_node_sha256": digest, "graph_node_id": node["node_id"],
        })
    crosswalk.sort(key=lambda item: item["graph_node_id"])
    if len(crosswalk) > MAX_CROSSWALKS:
        raise ContractError("ADR-0065 crosswalk limit exceeded")
    for field in ("adr_0062_node_id", "adr_0062_node_sha256", "graph_node_id"):
        values = [item[field] for item in crosswalk]
        if len(values) != len(set(values)):
            raise ContractError(f"ADR-0065 crosswalk {field} identity collision")
    return crosswalk


def enforce_locator_aggregate(*groups: list[dict[str, object]]) -> None:
    total = 0
    for records in groups:
        total += sum(len(record["source_locators"]) for record in records)
    if total > MAX_AGGREGATE_LOCATORS:
        raise ContractError("ADR-0065 aggregate source locator limit exceeded")
