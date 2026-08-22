"""Exact diagnostic/boundary unresolved nodes and nonlocal dependency edges."""

from __future__ import annotations

from governance_contract import ContractError

from .constants import (
    MAX_UNRESOLVED_EDGES, MAX_UNRESOLVED_NODES, RESOLUTION_REASONS,
)
from .profiles import locator_key, relative_name, sorted_unique, validate_repo_path
from .records import ensure_unique_ids, seal_record
from .topology import dependency_locators


def _locator(path: str, role: str, source_id: str) -> dict[str, object]:
    return {
        "content_sha256": None, "path": validate_repo_path(path, "unresolved locator"),
        "role": role, "source_id": source_id,
    }


def _diagnostic_node(graph: dict[str, object], diagnostic: dict[str, object],
                     project_id: str, source: dict[str, object],
                     extractor: dict[str, object]) -> dict[str, object]:
    path = diagnostic["path"]
    relative = relative_name(path, graph["module"]["directory"], "diagnostic path")
    body = {
        "candidate_identity_namespace": "go_source_path",
        "candidate_identity_profile_id": "go-source-path-v1",
        "candidate_qualified_name_components": [
            graph["module"]["module_path"], relative,
        ],
        "diagnostic_code": diagnostic["code"],
        "extractor_sha256s": [extractor["extractor_sha256"]],
        "kind": "go_file_diagnostic", "project_id": project_id,
        "reason_code": "go_file_diagnostic", "source_ids": [source["source_id"]],
        "source_locators": [_locator(
            path, "test" if path.endswith("_test.go") else "compile",
            source["source_id"])],
    }
    return seal_record("unresolved_node", body)


def _boundary_node(graph: dict[str, object], boundary: dict[str, object],
                   project_id: str, source: dict[str, object],
                   extractor: dict[str, object]) -> dict[str, object]:
    relative = relative_name(
        boundary["directory"], graph["module"]["directory"], "nested boundary")
    body = {
        "candidate_identity_namespace": "go_module_boundary",
        "candidate_identity_profile_id": "go-module-boundary-v1",
        "candidate_qualified_name_components": [
            graph["module"]["module_path"], relative,
        ],
        "diagnostic_code": None,
        "extractor_sha256s": [extractor["extractor_sha256"]],
        "kind": "nested_module_boundary", "project_id": project_id,
        "reason_code": "nested_module_boundary", "source_ids": [source["source_id"]],
        "source_locators": [_locator(
            boundary["go_mod_path"], "go_mod", source["source_id"])],
    }
    return seal_record("unresolved_node", body)


def build_unresolved_nodes(graph: dict[str, object], project_id: str,
                           source: dict[str, object], extractor: dict[str, object]):
    nodes = [_diagnostic_node(graph, item, project_id, source, extractor)
             for item in graph["diagnostics"]]
    nodes.extend(_boundary_node(graph, item, project_id, source, extractor)
                 for item in graph["module"]["nested_modules"])
    nodes.sort(key=lambda item: item["unresolved_node_id"])
    if len(nodes) > MAX_UNRESOLVED_NODES:
        raise ContractError("ADR-0065 unresolved node limit exceeded")
    ensure_unique_ids(nodes, "unresolved_node_id", "unresolved node union")
    return nodes


def _local_target(graph: dict[str, object], dependency: dict[str, object],
                  by_package: dict[tuple[str, str], dict[str, object]]):
    directory = dependency["target_directory"]
    if not isinstance(directory, str):
        raise ContractError("ADR-0065 local-candidate dependency lacks directory")
    relative = relative_name(directory, graph["module"]["directory"],
                             "dependency target directory")
    target_ids = []
    if dependency["resolution"] == "ambiguous_local":
        packages = {(item["directory"], item["name"]): item for item in graph["packages"]}
        target_ids = [node["node_id"] for key, node in by_package.items()
                      if key[0] == directory and packages[key]["compile_files"]]
        target_ids.sort()
        if len(target_ids) < 2:
            raise ContractError("ADR-0065 ambiguous target lacks multiple package nodes")
    return {
        "identity_namespace": "go",
        "identity_profile_id": "go-package-directory-candidate-v1",
        "qualified_name_components": [graph["module"]["module_path"], relative],
        "target_node_ids": target_ids,
    }


def _import_target(dependency: dict[str, object]) -> dict[str, object]:
    return {
        "identity_namespace": "go_import_candidate",
        "identity_profile_id": "go-import-candidate-v1",
        "qualified_name_components": [dependency["import_path"]],
        "target_node_ids": [],
    }


def _target(graph: dict[str, object], dependency: dict[str, object], by_package):
    if dependency["resolution"] in {
            "ambiguous_local", "unresolved_local", "nested_module_boundary"}:
        return _local_target(graph, dependency, by_package)
    return _import_target(dependency)


def _unresolved_edge(graph: dict[str, object], dependency: dict[str, object],
                     project_id: str, by_package, files,
                     source: dict[str, object], extractor: dict[str, object]):
    from_node = by_package.get(
        (dependency["from_directory"], dependency["from_package_name"]))
    if from_node is None:
        raise ContractError("ADR-0065 unresolved dependency has dangling source")
    resolution = dependency["resolution"]
    reason = RESOLUTION_REASONS.get(resolution)
    if reason is None:
        raise ContractError("ADR-0065 unsupported unresolved dependency resolution")
    body = {
        "category_axes": ["static_source"], "epistemic_status": "derived",
        "extractor_sha256s": [extractor["extractor_sha256"]],
        "from_node_id": from_node["node_id"],
        "identity_profile_id": "go-unresolved-import-edge-v1",
        "import_discriminator": dependency["import_path"],
        "parallel_discriminator": dependency["role"] + ":" + dependency["import_path"],
        "project_id": project_id, "reason_code": reason, "relation": "depends_on",
        "resolution": resolution, "resolution_detail": dependency["resolution_detail"],
        "source_ids": [source["source_id"]],
        "source_locators": dependency_locators(
            dependency, files, source["source_id"]),
        "source_role": dependency["role"],
        "target_candidate": _target(graph, dependency, by_package),
    }
    return seal_record("unresolved_edge", body)


def build_unresolved_edges(graph: dict[str, object], project_id: str, by_package,
                           files, source: dict[str, object],
                           extractor: dict[str, object]):
    edges = [_unresolved_edge(graph, item, project_id, by_package, files,
                             source, extractor)
             for item in graph["dependencies"] if item["resolution"] != "local"]
    edges.sort(key=lambda item: item["unresolved_edge_id"])
    if len(edges) > MAX_UNRESOLVED_EDGES:
        raise ContractError("ADR-0065 unresolved edge limit exceeded")
    ensure_unique_ids(edges, "unresolved_edge_id", "unresolved edge union")
    return edges
