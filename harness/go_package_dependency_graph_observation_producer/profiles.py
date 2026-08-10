"""Closed ADR-0053 parameter, source, and graph member profiles."""

from __future__ import annotations

import posixpath

from local_command_observation_producer.profiles import (safe_repo_path,
                                                          validate_source)

from .codec import forbidden_scalar
from .constants import (CANONICALIZATION, DIAGNOSTIC_CODES,
                        DIAGNOSTIC_FIELDS, FILE_FIELDS,
                        FILE_SELECTION_PROFILE, HASH_RE,
                        IMPORT_RESOLUTION_PROFILE, MAX_DIAGNOSTICS,
                        MAX_GO_FILE_BYTES, MAX_GO_MOD_BYTES,
                        MAX_IMPORTS_PER_FILE,
                        MAX_I64, MAX_NESTED_MODULES, MAX_RUN_ID_BYTES,
                        MAX_TEXT_SCALARS, MODULE_FIELDS, MODULE_PROFILE,
                        NESTED_MODULE_FIELDS, GO_KEYWORDS, PARAMETERS_API,
                        PARAMETERS_FIELDS, PARSER_PROFILE, PRODUCER_FIELDS,
                        PRODUCER_ID, PRODUCER_VERSION, ROLE_RANK, RUN_ID_RE,
                        SOURCE_BINDING_FIELDS, SOURCE_PROFILE)


def exact_fields(value: object, fields: set[str], label: str,
                 issues: list[str]) -> bool:
    if not isinstance(value, dict):
        issues.append(f"{label}: expected object")
        return False
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown:
        issues.append(f"{label}: unknown fields {sorted(unknown)}")
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return not unknown and not missing


def bounded_text(value: object, label: str, issues: list[str],
                 max_bytes: int = 16_384) -> bool:
    if not isinstance(value, str) or not value:
        issues.append(f"{label}: expected nonempty text")
        return False
    try:
        encoded = value.encode("utf-8")
    except UnicodeError:
        issues.append(f"{label}: expected valid UTF-8")
        return False
    if (len(value) > MAX_TEXT_SCALARS or len(encoded) > max_bytes or
            any(forbidden_scalar(char) for char in value)):
        issues.append(f"{label}: bounded UTF-8 or Unicode profile violated")
        return False
    return True


def safe_directory(value: object) -> bool:
    return value == "." or safe_repo_path(value)


def path_within(path: str, directory: str) -> bool:
    return directory == "." or path == directory or path.startswith(directory + "/")


def join_directory(directory: str, name: str) -> str:
    return name if directory == "." else directory + "/" + name


def path_directory(path: str) -> str:
    directory = posixpath.dirname(path)
    return directory if directory else "."


def canonical_import_path(value: object) -> bool:
    if (not isinstance(value, str) or not value or
            len(value) > MAX_TEXT_SCALARS or not value.isascii() or
            value.startswith(("/", ".", "-"))):
        return False
    if value.endswith("/") or "//" in value:
        return False
    return all(_canonical_import_component(part) for part in value.split("/"))


def _canonical_import_component(value: str) -> bool:
    allowed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~+"
    if (not value or value.startswith(".") or value.endswith(".") or
            ".." in value or any(char not in allowed for char in value)):
        return False
    base = value.split(".", 1)[0]
    return not _windows_reserved_import_base(base) and not _short_name_import_base(base)


def _windows_reserved_import_base(value: str) -> bool:
    upper = value.upper()
    if upper in {"CON", "PRN", "AUX", "NUL"}:
        return True
    return (len(upper) == 4 and upper[:3] in {"COM", "LPT"} and
            "1" <= upper[3] <= "9")


def _short_name_import_base(value: str) -> bool:
    _, separator, suffix = value.rpartition("~")
    return bool(separator and suffix and all("0" <= char <= "9" for char in suffix))


def go_identifier(value: object) -> bool:
    if (not isinstance(value, str) or not value or value in GO_KEYWORDS or
            not value.isascii()):
        return False
    first, *remaining = value
    if first != "_" and not ("A" <= first <= "Z" or "a" <= first <= "z"):
        return False
    return all(char == "_" or "A" <= char <= "Z" or "a" <= char <= "z" or
               "0" <= char <= "9"
               for char in remaining)


def validate_parameters(value: object) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, PARAMETERS_FIELDS, "parameters", issues):
        return issues
    fixed = {
        "api_version": PARAMETERS_API,
        "canonicalization": CANONICALIZATION,
        "file_selection_profile_id": FILE_SELECTION_PROFILE,
        "import_resolution_profile_id": IMPORT_RESOLUTION_PROFILE,
        "module_profile_id": MODULE_PROFILE,
        "parser_profile_id": PARSER_PROFILE,
        "source_profile_id": SOURCE_PROFILE,
    }
    if any(value[key] != expected for key, expected in fixed.items()):
        issues.append("parameters: fixed profile fields drifted")
    if not safe_directory(value["module_directory"]):
        issues.append("parameters.module_directory: expected canonical repository directory")
    bounded_text(value["module_directory"], "parameters.module_directory", issues)
    return issues


def source_index(value: object) -> tuple[dict[str, dict[str, object]], list[str]]:
    issues = validate_source(value)
    if issues or not isinstance(value, dict):
        return {}, issues
    return {entry["path"]: entry for entry in value["entries"]}, issues


def expected_nested_modules(source: dict[str, object], module_directory: str):
    own = join_directory(module_directory, "go.mod")
    result = []
    for entry in source["entries"]:
        path = entry["path"]
        if (path != own and posixpath.basename(path) == "go.mod" and
                path_within(path, module_directory) and
                entry["kind"] in {"regular", "symlink"}):
            result.append({"directory": path_directory(path),
                           "go_mod_path": path, "kind": entry["kind"]})
    return sorted(result, key=lambda item: (item["directory"], item["go_mod_path"]))


def _validate_nested(value: object, index: int, issues: list[str]) -> None:
    label = f"module.nested_modules[{index}]"
    if not exact_fields(value, NESTED_MODULE_FIELDS, label, issues):
        return
    if not safe_directory(value["directory"]):
        issues.append(f"{label}.directory: unsafe directory")
    if not safe_repo_path(value["go_mod_path"]):
        issues.append(f"{label}.go_mod_path: unsafe path")
    if value["go_mod_path"] != join_directory(value["directory"], "go.mod"):
        issues.append(f"{label}: go_mod_path does not match directory")
    if value["kind"] not in {"regular", "symlink"}:
        issues.append(f"{label}.kind: expected current regular or symlink")


def validate_module(value: object, parameters: dict[str, object],
                    source: dict[str, object]) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, MODULE_FIELDS, "module", issues):
        return issues
    directory = parameters["module_directory"]
    expected_path = join_directory(directory, "go.mod")
    entry = {item["path"]: item for item in source["entries"]}.get(expected_path)
    if value["directory"] != directory or value["go_mod_path"] != expected_path:
        issues.append("module: selected directory or go_mod_path drifted")
    if (not isinstance(entry, dict) or entry.get("kind") != "regular" or
            value["go_mod_bytes"] != entry.get("bytes") or
            value["go_mod_content_sha256"] != entry.get("content_sha256")):
        issues.append("module: selected go.mod is not the matching regular source entry")
    if (type(value["go_mod_bytes"]) is not int or
            not 0 < value["go_mod_bytes"] <= MAX_GO_MOD_BYTES or
            not isinstance(value["go_mod_content_sha256"], str) or
            HASH_RE.fullmatch(value["go_mod_content_sha256"]) is None):
        issues.append("module: invalid selected go.mod size or digest")
    if not bounded_text(value["module_path"], "module.module_path", issues):
        return issues
    if not canonical_import_path(value["module_path"]):
        issues.append("module.module_path: unsupported canonical import path")
    nested = value["nested_modules"]
    if not isinstance(nested, list) or len(nested) > MAX_NESTED_MODULES:
        issues.append("module.nested_modules: invalid list or limit")
        return issues
    for index, item in enumerate(nested):
        _validate_nested(item, index, issues)
    if nested != expected_nested_modules(source, directory):
        issues.append("module.nested_modules: source-derived boundary set drifted")
    return issues


def validate_file(value: object, index: int,
                  source: dict[str, dict[str, object]]) -> list[str]:
    issues: list[str] = []
    label = f"graph.files[{index}]"
    if not exact_fields(value, FILE_FIELDS, label, issues):
        return issues
    entry = source.get(value["path"]) if isinstance(value.get("path"), str) else None
    if (not safe_repo_path(value["path"]) or not isinstance(entry, dict) or
            entry.get("kind") != "regular" or value["bytes"] != entry.get("bytes") or
            value["content_sha256"] != entry.get("content_sha256")):
        issues.append(f"{label}: file facts do not match a regular source entry")
    if (type(value["bytes"]) is not int or not 0 <= value["bytes"] <= MAX_GO_FILE_BYTES or
            not isinstance(value["content_sha256"], str) or
            HASH_RE.fullmatch(value["content_sha256"]) is None):
        issues.append(f"{label}: invalid size or digest")
    expected_role = "test" if str(value["path"]).endswith("_test.go") else "compile"
    if value["role"] != expected_role:
        issues.append(f"{label}.role: filename-derived role drifted")
    package_name_valid = bounded_text(
        value["package_name"], f"{label}.package_name", issues)
    if package_name_valid and not go_identifier(value["package_name"]):
        issues.append(f"{label}.package_name: invalid Go package identifier")
    imports = value["imports"]
    if not isinstance(imports, list) or len(imports) > MAX_IMPORTS_PER_FILE:
        issues.append(f"{label}.imports: invalid list or limit")
    elif imports != sorted(set(imports)):
        issues.append(f"{label}.imports: must be byte-sorted and unique")
    else:
        for item in imports:
            bounded_text(item, f"{label}.imports", issues)
    return issues


def validate_diagnostics(value: object) -> list[str]:
    issues: list[str] = []
    if not isinstance(value, list) or len(value) > MAX_DIAGNOSTICS:
        return ["graph.diagnostics: invalid list or limit"]
    keys, paths = [], []
    for index, item in enumerate(value):
        label = f"graph.diagnostics[{index}]"
        if not exact_fields(item, DIAGNOSTIC_FIELDS, label, issues):
            continue
        if item["code"] not in DIAGNOSTIC_CODES:
            issues.append(f"{label}.code: unsupported stable diagnostic")
        if not safe_repo_path(item["path"]):
            issues.append(f"{label}.path: unsafe path")
        keys.append((item["path"], item["code"]))
        paths.append(item["path"])
    if keys != sorted(set(keys)):
        issues.append("graph.diagnostics: must be sorted and unique")
    if len(paths) != len(set(paths)):
        issues.append("graph.diagnostics: each failed path must occur exactly once")
    return issues


def validate_producer(value: object, parameters_sha256: str) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, PRODUCER_FIELDS, "graph.producer", issues):
        return issues
    if (value["parameters_sha256"] != parameters_sha256 or
            value["producer_id"] != PRODUCER_ID or value["producer_type"] != "tool" or
            value["producer_version"] != PRODUCER_VERSION):
        issues.append("graph.producer: fixed producer binding drifted")
    run_id = value["run_id"]
    if (not isinstance(run_id, str) or len(run_id.encode("utf-8")) > MAX_RUN_ID_BYTES or
            RUN_ID_RE.fullmatch(run_id) is None):
        issues.append("graph.producer.run_id: invalid bounded identifier")
    return issues


def validate_source_binding(value: object, source: dict[str, object],
                            source_sha256: str) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, SOURCE_BINDING_FIELDS, "graph.source", issues):
        return issues
    if (value["source_revision"] != source["source_revision"] or
            value["source_tree_sha256"] != source_sha256):
        issues.append("graph.source: source revision or tree digest drifted")
    return issues


def nonnegative_i64(value: object) -> bool:
    return type(value) is int and 0 <= value <= MAX_I64
