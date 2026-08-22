"""Strict validation for self-contained ADR-0053 graph observations."""

from __future__ import annotations

import re

from governance_contract import ContractError
from local_command_observation_producer.profiles import safe_repo_path

from .codec import decode_production
from .constants import (
    CANONICALIZATION, COVERAGE_FIELDS, EDGE_FIELDS, FILE_FIELDS, GRAPH_API,
    GRAPH_FIELDS, GRAPH_PROFILE, HASH_RE, MAX_AGGREGATE_PARSER_BYTES,
    MAX_EDGES, MAX_GO_FILE_BYTES, MAX_GO_FILES, MAX_GO_MOD_BYTES,
    MAX_IMPORT_OCCURRENCES, MAX_IMPORTS_PER_FILE, MAX_NESTED_MODULES,
    MAX_PACKAGES, MAX_SOURCE_ENTRIES, MODULE_FIELDS, NESTED_MODULE_FIELDS,
    PACKAGE_FIELDS, PRODUCER_FIELDS, PRODUCER_ID, PRODUCER_VERSION,
    RESOLUTION_DETAILS, RESOLUTIONS, ROLE_RANK, RUN_ID_RE,
    SOURCE_BINDING_FIELDS,
)
from .profiles import (
    bounded_text, canonical_import_path, exact_fields, go_identifier,
    join_directory, nonnegative_i64, path_within, safe_directory,
    validate_diagnostics,
)
from .semantics import expected_dependencies, expected_packages, graph_digest

_REVISION_RE = re.compile(
    r"^(?:git-sha1:[a-f0-9]{40}|git-sha256:[a-f0-9]{64})$")


def _fail(issues: list[str]) -> None:
    if issues:
        raise ContractError("invalid ADR-0053 graph observation: " + "; ".join(issues))


def _hash(value: object) -> bool:
    return isinstance(value, str) and HASH_RE.fullmatch(value) is not None


def _bounded_repo_path(value: object) -> bool:
    if not safe_repo_path(value):
        return False
    return len(value) <= 4_096 and len(value.encode("utf-8")) <= 16_384


def _inside_nested(path: str, nested: list[str]) -> bool:
    return any(path == directory or path.startswith(directory + "/")
               for directory in nested)


def _validate_module(graph: dict[str, object], issues: list[str]) -> list[str]:
    value = graph.get("module")
    if not exact_fields(value, MODULE_FIELDS, "graph.module", issues):
        return []
    assert isinstance(value, dict)
    directory = value["directory"]
    if not safe_directory(directory) or not bounded_text(
            directory, "graph.module.directory", issues):
        issues.append("graph.module.directory: invalid repository directory")
        return []
    if (type(value["go_mod_bytes"]) is not int or
            not 0 < value["go_mod_bytes"] <= MAX_GO_MOD_BYTES or
            not _hash(value["go_mod_content_sha256"])):
        issues.append("graph.module: invalid go.mod size or digest")
    if (not _bounded_repo_path(value["go_mod_path"]) or
            value["go_mod_path"] != join_directory(directory, "go.mod")):
        issues.append("graph.module.go_mod_path: does not match selected directory")
    if (not bounded_text(value["module_path"], "graph.module.module_path", issues) or
            not canonical_import_path(value["module_path"])):
        issues.append("graph.module.module_path: invalid canonical Go import path")
    nested = value["nested_modules"]
    if not isinstance(nested, list) or len(nested) > MAX_NESTED_MODULES:
        issues.append("graph.module.nested_modules: invalid list or limit")
        return []
    keys: list[tuple[str, str]] = []
    directories: list[str] = []
    for index, item in enumerate(nested):
        label = f"graph.module.nested_modules[{index}]"
        if not exact_fields(item, NESTED_MODULE_FIELDS, label, issues):
            continue
        nested_directory = item["directory"]
        if (not safe_directory(nested_directory) or nested_directory == directory or
                not path_within(nested_directory, directory)):
            issues.append(f"{label}.directory: invalid selected-module child")
        if (not _bounded_repo_path(item["go_mod_path"]) or
                item["go_mod_path"] != join_directory(nested_directory, "go.mod")):
            issues.append(f"{label}.go_mod_path: does not match directory")
        if item["kind"] not in {"regular", "symlink"}:
            issues.append(f"{label}.kind: unsupported boundary kind")
        if isinstance(nested_directory, str) and isinstance(item["go_mod_path"], str):
            keys.append((nested_directory, item["go_mod_path"]))
            directories.append(nested_directory)
    if keys != sorted(set(keys)) or len(directories) != len(set(directories)):
        issues.append("graph.module.nested_modules: must be sorted and unique")
    return directories


def _validate_files(graph: dict[str, object], nested: list[str],
                    issues: list[str]) -> None:
    values = graph.get("files")
    if not isinstance(values, list) or len(values) > MAX_GO_FILES:
        issues.append("graph.files: invalid list or limit")
        return
    module_directory = graph["module"]["directory"]
    paths: list[str] = []
    parser_bytes = 0
    import_count = 0
    for index, item in enumerate(values):
        label = f"graph.files[{index}]"
        if not exact_fields(item, FILE_FIELDS, label, issues):
            continue
        path = item["path"]
        if (not _bounded_repo_path(path) or not path.endswith(".go") or
                not path_within(path, module_directory) or _inside_nested(path, nested)):
            issues.append(f"{label}.path: outside selected regular Go file domain")
        size = item["bytes"]
        if type(size) is not int or not 0 <= size <= MAX_GO_FILE_BYTES:
            issues.append(f"{label}: invalid file size")
        else:
            parser_bytes += size
        if not _hash(item["content_sha256"]):
            issues.append(f"{label}.content_sha256: invalid lowercase SHA-256")
        expected_role = "test" if str(path).endswith("_test.go") else "compile"
        if item["role"] != expected_role:
            issues.append(f"{label}.role: filename-derived role drifted")
        if (not bounded_text(item["package_name"], f"{label}.package_name", issues) or
                not go_identifier(item["package_name"])):
            issues.append(f"{label}.package_name: invalid Go identifier")
        imports = item["imports"]
        if not isinstance(imports, list) or len(imports) > MAX_IMPORTS_PER_FILE:
            issues.append(f"{label}.imports: invalid list or limit")
        else:
            import_count += len(imports)
            if imports != sorted(set(imports), key=lambda value: value.encode("utf-8")):
                issues.append(f"{label}.imports: must be UTF-8-byte sorted and unique")
            for import_path in imports:
                bounded_text(import_path, f"{label}.imports", issues)
        if isinstance(path, str):
            paths.append(path)
    if paths != sorted(set(paths), key=lambda value: value.encode("utf-8")):
        issues.append("graph.files: paths must be UTF-8-byte sorted and unique")
    if parser_bytes > MAX_AGGREGATE_PARSER_BYTES:
        issues.append("graph.files: aggregate parser bytes exceed limit")
    if import_count > MAX_IMPORT_OCCURRENCES:
        issues.append("graph.files: aggregate import occurrences exceed limit")


def _validate_diagnostics(graph: dict[str, object], nested: list[str],
                          issues: list[str]) -> None:
    values = graph.get("diagnostics")
    diagnostic_issues = validate_diagnostics(values)
    issues.extend(diagnostic_issues)
    if diagnostic_issues or not isinstance(values, list):
        return
    module_directory = graph["module"]["directory"]
    for index, item in enumerate(values):
        path = item["path"]
        if (not _bounded_repo_path(path) or not path.endswith(".go") or
                not path_within(path, module_directory) or _inside_nested(path, nested)):
            issues.append(f"graph.diagnostics[{index}].path: outside selected Go domain")


def _validate_fixed_members(graph: dict[str, object], issues: list[str]) -> None:
    if (graph["api_version"] != GRAPH_API or
            graph["canonicalization"] != CANONICALIZATION or
            graph["profile_id"] != GRAPH_PROFILE):
        issues.append("graph: fixed API, canonicalization, or profile drifted")
    if not nonnegative_i64(graph["observed_at_unix_ms"]):
        issues.append("graph.observed_at_unix_ms: expected nonnegative signed-int64")
    producer = graph["producer"]
    if exact_fields(producer, PRODUCER_FIELDS, "graph.producer", issues):
        if (producer["producer_id"] != PRODUCER_ID or producer["producer_type"] != "tool" or
                producer["producer_version"] != PRODUCER_VERSION or
                not _hash(producer["parameters_sha256"])):
            issues.append("graph.producer: fixed identity or parameter digest drifted")
        run_id = producer["run_id"]
        if (not isinstance(run_id, str) or len(run_id.encode("utf-8")) > 160 or
                RUN_ID_RE.fullmatch(run_id) is None):
            issues.append("graph.producer.run_id: invalid bounded identifier")
    source = graph["source"]
    if exact_fields(source, SOURCE_BINDING_FIELDS, "graph.source", issues):
        if (not isinstance(source["source_revision"], str) or
                _REVISION_RE.fullmatch(source["source_revision"]) is None or
                not _hash(source["source_tree_sha256"])):
            issues.append("graph.source: invalid source revision or digest")


def _validate_coverage(graph: dict[str, object], issues: list[str]) -> None:
    coverage = graph["coverage"]
    if not exact_fields(coverage, COVERAGE_FIELDS, "graph.coverage", issues):
        return
    if any(not nonnegative_i64(item) for item in coverage.values()):
        issues.append("graph.coverage: counts must be nonnegative signed-int64")
        return
    parsed, diagnostics = len(graph["files"]), len(graph["diagnostics"])
    selected = parsed + diagnostics
    if (coverage["regular_go_files_parsed"] != parsed or
            coverage["regular_go_files_with_diagnostics"] != diagnostics or
            coverage["regular_go_files_selected"] != selected or
            coverage["go_entries_in_selected_subtree"] != selected +
            coverage["go_entries_excluded_by_nested_module"] +
            coverage["go_entries_excluded_nonregular"]):
        issues.append("graph.coverage: graph-contained selection accounting drifted")
    if selected > MAX_GO_FILES:
        issues.append("graph.coverage: selected regular Go file count exceeds limit")
    if coverage["go_entries_in_selected_subtree"] > MAX_SOURCE_ENTRIES:
        issues.append("graph.coverage: selected subtree entry count exceeds limit")


def _validate_derived_members(graph: dict[str, object], issues: list[str]) -> None:
    packages = graph["packages"]
    if not isinstance(packages, list) or len(packages) > MAX_PACKAGES:
        issues.append("graph.packages: invalid list or limit")
    else:
        for index, item in enumerate(packages):
            if not exact_fields(item, PACKAGE_FIELDS, f"graph.packages[{index}]", issues):
                continue
            if item["import_path"] is not None and not canonical_import_path(item["import_path"]):
                issues.append(f"graph.packages[{index}].import_path: invalid canonical path")
        if not issues and packages != expected_packages(graph):
            issues.append("graph.packages: file-derived grouping or ordering drifted")
    dependencies = graph["dependencies"]
    if not isinstance(dependencies, list) or len(dependencies) > MAX_EDGES:
        issues.append("graph.dependencies: invalid list or limit")
        return
    for index, item in enumerate(dependencies):
        if not exact_fields(item, EDGE_FIELDS, f"graph.dependencies[{index}]", issues):
            continue
        resolution = item["resolution"]
        if (resolution not in RESOLUTIONS or
                item["resolution_detail"] not in RESOLUTION_DETAILS.get(resolution, set()) or
                item["relation"] != "depends_on" or item["role"] not in ROLE_RANK):
            issues.append(f"graph.dependencies[{index}]: invalid closed resolution tuple")
    if not issues and dependencies != expected_dependencies(graph):
        issues.append("graph.dependencies: lexical derivation or ordering drifted")


def validate_graph_bytes(raw: bytes) -> dict[str, object]:
    """Validate every fact available inside exact ADR-0053 graph bytes."""
    graph = decode_production(raw)
    issues: list[str] = []
    if not exact_fields(graph, GRAPH_FIELDS, "graph", issues):
        _fail(issues)
    _validate_fixed_members(graph, issues)
    nested = _validate_module(graph, issues)
    if issues:
        _fail(issues)
    _validate_files(graph, nested, issues)
    _validate_diagnostics(graph, nested, issues)
    if not issues:
        file_paths = {item["path"] for item in graph["files"]}
        diagnostic_paths = {item["path"] for item in graph["diagnostics"]}
        if file_paths & diagnostic_paths:
            issues.append("graph: files and diagnostics must be disjoint")
        _validate_coverage(graph, issues)
        _validate_derived_members(graph, issues)
    _fail(issues)
    return graph


def observation_digest(graph: dict[str, object]) -> str:
    """Return ADR-0053's frozen domain-separated graph digest."""
    return graph_digest(graph)


__all__ = ["observation_digest", "validate_graph_bytes"]
