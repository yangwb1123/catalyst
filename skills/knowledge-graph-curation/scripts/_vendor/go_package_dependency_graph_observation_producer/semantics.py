"""Pure structural semantics for ADR-0053 production packages.

This checker deliberately does not parse Go source. The live Go producer owns
package/import extraction; this module validates their source binding and
recomputes coverage, package grouping, lexical edge resolution, and digests.
"""

from __future__ import annotations

import hashlib

from governance_contract import ContractError

from .codec import canonical_json
from .constants import (CANONICALIZATION, COVERAGE_FIELDS, EDGE_FIELDS,
                        GRAPH_API, GRAPH_DOMAIN, GRAPH_FIELDS, GRAPH_PROFILE,
                        MAX_AGGREGATE_PARSER_BYTES, MAX_EDGES,
                        MAX_GO_FILE_BYTES, MAX_GO_FILES,
                        MAX_IMPORT_OCCURRENCES, MAX_PACKAGES,
                        PACKAGE_FIELDS, PARAMETERS_DOMAIN,
                        PRODUCTION_API, PRODUCTION_DOMAIN, PRODUCTION_FIELDS,
                        RESOLUTION_DETAILS, RESOLUTIONS, ROLE_RANK, SOURCE_DOMAIN)
from .profiles import (canonical_import_path, exact_fields, join_directory,
                       bounded_text, nonnegative_i64, path_directory, path_within,
                       safe_directory, source_index, validate_diagnostics, validate_file,
                       validate_module, validate_parameters, validate_producer,
                       validate_source_binding)


def domain_digest(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + canonical_json(value)).hexdigest()


def parameters_digest(value: object) -> str:
    return domain_digest(PARAMETERS_DOMAIN, value)


def graph_digest(value: object) -> str:
    return domain_digest(GRAPH_DOMAIN, value)


def source_digest(value: object) -> str:
    return domain_digest(SOURCE_DOMAIN, value)


def production_digest(value: object) -> str:
    return domain_digest(PRODUCTION_DOMAIN, value)


def _nested_directories(graph: dict[str, object]) -> list[str]:
    return [item["directory"] for item in graph["module"]["nested_modules"]]


def selected_source_entries(production: dict[str, object]):
    graph, parameters = production["graph_observation"], production["parameters_manifest"]
    directory = parameters["module_directory"]
    nested = set(_nested_directories(graph))
    universe, nested_entries, remaining = [], [], []
    for entry in production["source_manifest"]["entries"]:
        path = entry["path"]
        if path_within(path, directory) and path.endswith(".go"):
            universe.append(entry)
            if _within_nested(path_directory(path), nested):
                nested_entries.append(entry)
            else:
                remaining.append(entry)
    regular = [entry for entry in remaining if entry["kind"] == "regular"]
    nonregular = [entry for entry in remaining if entry["kind"] != "regular"]
    return universe, nested_entries, nonregular, regular


def expected_coverage(production: dict[str, object]) -> dict[str, int]:
    universe, nested, nonregular, regular = selected_source_entries(production)
    graph = production["graph_observation"]
    return {
        "go_entries_excluded_by_nested_module": len(nested),
        "go_entries_excluded_nonregular": len(nonregular),
        "go_entries_in_selected_subtree": len(universe),
        "regular_go_files_parsed": len(graph["files"]),
        "regular_go_files_selected": len(regular),
        "regular_go_files_with_diagnostics": len(graph["diagnostics"]),
    }


def expected_packages(graph: dict[str, object]) -> list[dict[str, object]]:
    grouped: dict[tuple[str, str], dict[str, list[str]]] = {}
    module = graph["module"]
    for item in graph["files"]:
        key = (path_directory(item["path"]), item["package_name"])
        record = grouped.setdefault(key, {"compile": [], "test": []})
        record[item["role"]].append(item["path"])
    result = []
    for (directory, name), roles in grouped.items():
        suffix = "" if directory == module["directory"] else _relative(directory, module["directory"])
        import_path = None
        if roles["compile"]:
            import_path = module["module_path"] + ("/" + suffix if suffix else "")
        result.append({"compile_files": sorted(roles["compile"]),
                       "directory": directory, "import_path": import_path,
                       "name": name, "test_files": sorted(roles["test"])})
    return sorted(result, key=lambda item: (item["directory"], item["name"]))


def _relative(path: str, root: str) -> str:
    if root == ".":
        return path
    return path[len(root) + 1:]


def _local_target(import_path: str, module: dict[str, object]) -> str | None:
    prefix = module["module_path"]
    if import_path == prefix:
        return module["directory"]
    if not import_path.startswith(prefix + "/"):
        return None
    suffix = import_path[len(prefix) + 1:]
    return join_directory(module["directory"], suffix)


def _within_nested(target: str, nested: set[str]) -> bool:
    current = target
    while True:
        if current in nested:
            return True
        if current == ".":
            return False
        current = path_directory(current)


def _resolver_indexes(graph: dict[str, object], packages: list[dict[str, object]]):
    nested = {item["directory"] for item in graph["module"]["nested_modules"]}
    compile_packages: dict[str, list[str]] = {}
    for item in packages:
        if item["compile_files"]:
            compile_packages.setdefault(item["directory"], []).append(item["name"])
    return nested, compile_packages


def _resolve(import_path: str, graph: dict[str, object], nested: set[str],
             compile_packages: dict[str, list[str]]):
    if import_path == "C":
        return "cgo_pseudo", None, None, None
    if not canonical_import_path(import_path):
        return "unsupported", "noncanonical_import_path", None, None
    target = _local_target(import_path, graph["module"])
    if target is None:
        head = import_path.split("/", 1)[0]
        resolution = "stdlib_candidate" if "." not in head else "external_candidate"
        return resolution, None, None, None
    if _within_nested(target, nested):
        return "nested_module_boundary", "nested_module_boundary", target, None
    candidates = compile_packages.get(target, [])
    if not candidates:
        return "unresolved_local", "no_compile_package", target, None
    if len(candidates) > 1:
        return "ambiguous_local", "multiple_compile_packages", target, None
    return "local", None, target, candidates[0]


def expected_dependencies(graph: dict[str, object]) -> list[dict[str, object]]:
    packages = expected_packages(graph)
    nested, compile_packages = _resolver_indexes(graph, packages)
    grouped: dict[tuple[str, str, str, str], list[str]] = {}
    for item in graph["files"]:
        directory = path_directory(item["path"])
        for import_path in item["imports"]:
            key = (directory, item["package_name"], item["role"], import_path)
            grouped.setdefault(key, []).append(item["path"])
    result = []
    for key, paths in grouped.items():
        directory, package, role, import_path = key
        resolution, detail, target_dir, target_name = _resolve(
            import_path, graph, nested, compile_packages)
        result.append({
            "from_directory": directory, "from_package_name": package,
            "import_path": import_path, "relation": "depends_on",
            "resolution": resolution, "resolution_detail": detail, "role": role,
            "source_paths": sorted(set(paths)), "target_directory": target_dir,
            "target_package_name": target_name,
        })
    return sorted(result, key=lambda item: (
        item["from_directory"], item["from_package_name"],
        ROLE_RANK[item["role"]], item["import_path"],
    ))


def _validate_files(graph: dict[str, object], source: dict[str, object]) -> list[str]:
    values, issues = graph["files"], []
    if not isinstance(values, list) or len(values) > MAX_GO_FILES:
        return ["graph.files: invalid list or limit"]
    index = {entry["path"]: entry for entry in source["entries"]}
    for number, item in enumerate(values):
        issues.extend(validate_file(item, number, index))
    paths = [item.get("path") for item in values if isinstance(item, dict)]
    if paths != sorted(set(paths)):
        issues.append("graph.files: paths must be sorted and unique")
    total_bytes = sum(item.get("bytes", 0) for item in values if isinstance(item, dict)
                      and type(item.get("bytes")) is int)
    imports = sum(len(item.get("imports", [])) for item in values if isinstance(item, dict)
                  and isinstance(item.get("imports"), list))
    if total_bytes > MAX_AGGREGATE_PARSER_BYTES:
        issues.append("graph.files: aggregate parser input exceeds limit")
    if imports > MAX_IMPORT_OCCURRENCES:
        issues.append("graph.files: aggregate import occurrences exceed limit")
    return issues


def _validate_packages(graph: dict[str, object], issues: list[str]) -> None:
    values = graph["packages"]
    if not isinstance(values, list) or len(values) > MAX_PACKAGES:
        issues.append("graph.packages: invalid list or limit")
        return
    for index, item in enumerate(values):
        if not exact_fields(item, PACKAGE_FIELDS, f"graph.packages[{index}]", issues):
            continue
        if item["import_path"] is not None:
            label = f"graph.packages[{index}].import_path"
            if not bounded_text(item["import_path"], label, issues):
                continue
            if not canonical_import_path(item["import_path"]):
                issues.append(f"{label}: invalid path")
    if values != expected_packages(graph):
        issues.append("graph.packages: file-derived package grouping drifted")


def _validate_dependencies(graph: dict[str, object], issues: list[str]) -> None:
    values = graph["dependencies"]
    if not isinstance(values, list) or len(values) > MAX_EDGES:
        issues.append("graph.dependencies: invalid list or limit")
        return
    for index, item in enumerate(values):
        if not exact_fields(item, EDGE_FIELDS, f"graph.dependencies[{index}]", issues):
            continue
        resolution = item["resolution"]
        if resolution not in RESOLUTIONS or item["resolution_detail"] not in RESOLUTION_DETAILS.get(resolution, set()):
            issues.append(f"graph.dependencies[{index}]: invalid resolution/detail")
        if item["relation"] != "depends_on" or item["role"] not in ROLE_RANK:
            issues.append(f"graph.dependencies[{index}]: invalid relation or role")
        target = item["target_directory"]
        if target is not None and not safe_directory(target):
            issues.append(f"graph.dependencies[{index}].target_directory: unsafe path")
    if values != expected_dependencies(graph):
        issues.append("graph.dependencies: lexical edge derivation drifted")


def _validate_accounting(production: dict[str, object], issues: list[str]) -> None:
    graph, coverage = production["graph_observation"], production["graph_observation"]["coverage"]
    if not exact_fields(coverage, COVERAGE_FIELDS, "graph.coverage", issues):
        return
    if any(not nonnegative_i64(value) for value in coverage.values()):
        issues.append("graph.coverage: counts must be nonnegative signed-int64")
    if coverage != expected_coverage(production):
        issues.append("graph.coverage: source selection accounting drifted")
    _, _, _, selected = selected_source_entries(production)
    if len(selected) > MAX_GO_FILES:
        issues.append("graph: selected regular Go file count exceeds limit")
    parser_bytes = sum(item["bytes"] for item in selected
                       if item["bytes"] <= MAX_GO_FILE_BYTES)
    if parser_bytes > MAX_AGGREGATE_PARSER_BYTES:
        issues.append("graph: aggregate selected parser input exceeds limit")
    file_paths = {item["path"] for item in graph["files"] if isinstance(item, dict)}
    diagnostic_paths = {item["path"] for item in graph["diagnostics"] if isinstance(item, dict)}
    if file_paths & diagnostic_paths or file_paths | diagnostic_paths != {item["path"] for item in selected}:
        issues.append("graph: parsed files and diagnostics do not partition selected regular Go files")
    selected_index = {item["path"]: item for item in selected}
    for diagnostic in graph["diagnostics"]:
        entry = selected_index.get(diagnostic["path"])
        if entry is None:
            continue
        oversized = entry["bytes"] > MAX_GO_FILE_BYTES
        code_says_oversized = diagnostic["code"] == "go_file_exceeds_parser_limit"
        if oversized != code_says_oversized:
            issues.append(
                f"graph.diagnostics: size-derived diagnostic drifted for {diagnostic['path']}")


def _validate_graph(production: dict[str, object], parameters_sha: str,
                    source_sha: str) -> list[str]:
    graph, source, parameters, issues = (production["graph_observation"],
                                          production["source_manifest"],
                                          production["parameters_manifest"], [])
    if not exact_fields(graph, GRAPH_FIELDS, "graph", issues):
        return issues
    if (graph["api_version"] != GRAPH_API or graph["canonicalization"] != CANONICALIZATION or
            graph["profile_id"] != GRAPH_PROFILE):
        issues.append("graph: API, canonicalization, or profile drifted")
    issues.extend(validate_module(graph["module"], parameters, source))
    issues.extend(_validate_files(graph, source))
    issues.extend(validate_diagnostics(graph["diagnostics"]))
    issues.extend(validate_producer(graph["producer"], parameters_sha))
    issues.extend(validate_source_binding(graph["source"], source, source_sha))
    if not nonnegative_i64(graph["observed_at_unix_ms"]):
        issues.append("graph.observed_at_unix_ms: expected nonnegative signed-int64")
    if not issues:
        _validate_accounting(production, issues)
        _validate_packages(graph, issues)
        _validate_dependencies(graph, issues)
    return issues


def _validate_production(value: object) -> list[str]:
    issues: list[str] = []
    canonical_json(value)
    if not exact_fields(value, PRODUCTION_FIELDS, "production", issues):
        return issues
    if value["api_version"] != PRODUCTION_API or value["canonicalization"] != CANONICALIZATION:
        issues.append("production: API or canonicalization drifted")
    parameter_issues = validate_parameters(value["parameters_manifest"])
    _, source_issues = source_index(value["source_manifest"])
    issues.extend(parameter_issues)
    issues.extend(source_issues)
    if not parameter_issues and not source_issues:
        issues.extend(_validate_graph(
            value, parameters_digest(value["parameters_manifest"]),
            source_digest(value["source_manifest"]),
        ))
    return issues


def validate_production(value: object) -> list[str]:
    try:
        return _validate_production(value)
    except (ContractError, KeyError, TypeError, AttributeError,
            UnicodeError, IndexError) as error:
        return [f"production: invalid nested value: {error}"]
    except MemoryError:
        return ["production: validation exhausted memory"]
