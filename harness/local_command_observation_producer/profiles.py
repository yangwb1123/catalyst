"""Closed profile semantics for the deterministic producer fixture."""

from __future__ import annotations

import hashlib
import posixpath
from pathlib import PurePosixPath

from .constants import (ENVIRONMENT_API, ENVIRONMENT_FIELDS, ENVIRONMENT_PROFILE,
                        ENV_NAME_RE, ENTRY_FIELDS, HASH_RE, HOP_FIELDS,
                        MAX_FILE_BYTES, MAX_SOURCE_BYTES, MAX_TEXT_BYTES,
                        MAX_TEXT_SCALARS, REVISION_RE, SECRET_FRAGMENTS,
                        SECRET_PREFIXES, SOURCE_API, SOURCE_FIELDS, SOURCE_PROFILE,
                        TOOL_API, TOOL_FIELDS, TOOL_PROFILE, VARIABLE_FIELDS)
from .codec import forbidden_scalar


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


def secret_name(name: str) -> bool:
    upper = name.upper()
    if upper in {"HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"}:
        return True
    return any(upper.startswith(prefix) for prefix in SECRET_PREFIXES) or any(
        fragment in upper for fragment in SECRET_FRAGMENTS)


def bounded_text(value: object, label: str, issues: list[str],
                 *, allow_empty: bool = False) -> bool:
    if not isinstance(value, str) or (not allow_empty and not value):
        issues.append(f"{label}: expected {'possibly empty ' if allow_empty else 'non-empty '}string")
        return False
    try:
        encoded = value.encode("utf-8")
    except UnicodeError:
        issues.append(f"{label}: expected valid UTF-8")
        return False
    if len(value) > MAX_TEXT_SCALARS or len(encoded) > MAX_TEXT_BYTES:
        issues.append(f"{label}: bounded text limit exceeded")
        return False
    if any(forbidden_scalar(character) for character in value):
        issues.append(f"{label}: forbidden Unicode control scalar")
        return False
    return True


def validate_environment(value: object) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, ENVIRONMENT_FIELDS, "environment", issues):
        return issues
    if (value["api_version"] != ENVIRONMENT_API or
            value["canonicalization"] != "forgeos.canonical-json/v1" or
            value["profile_id"] != ENVIRONMENT_PROFILE):
        issues.append("environment: fixed fields drifted")
    variables = value["variables"]
    if not isinstance(variables, list) or not 1 <= len(variables) <= 256:
        issues.append("environment.variables: expected list with 1..256 items")
        return issues
    names: list[str] = []
    for index, variable in enumerate(variables):
        if not exact_fields(variable, VARIABLE_FIELDS, f"environment.variables[{index}]", issues):
            continue
        name = variable["name"]
        name_valid = bounded_text(name, f"environment.variables[{index}].name", issues)
        if not name_valid or ENV_NAME_RE.fullmatch(name) is None or secret_name(name):
            issues.append(f"environment.variables[{index}].name: invalid or secret name")
        else:
            names.append(name)
        value_valid = bounded_text(
            variable["value"], f"environment.variables[{index}].value",
            issues, allow_empty=True)
        if name == "PATH" and (not value_valid or
                               not normalized_absolute_path_list(variable["value"])):
            issues.append(
                f"environment.variables[{index}].value: PATH components must be "
                "non-empty normalized absolute paths")
    if names != sorted(set(names)) or "PATH" not in names:
        issues.append("environment.variables: names must be sorted, unique, and contain PATH")
    return issues


def normalized_absolute(value: object) -> bool:
    if not isinstance(value, str) or not value.startswith("/") or value.startswith("//"):
        return False
    return posixpath.normpath(value) == value


def normalized_absolute_path_list(value: object) -> bool:
    """Validate every component of the Unix-family scrubbed PATH profile."""
    return isinstance(value, str) and all(
        normalized_absolute(component) for component in value.split(":"))


def validate_tool(value: object, requested: str) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, TOOL_FIELDS, "tool", issues):
        return issues
    if (value["api_version"] != TOOL_API or value["canonicalization"] != "forgeos.canonical-json/v1" or
            value["profile_id"] != TOOL_PROFILE or value["requested_path"] != requested):
        issues.append("tool: fixed fields drifted")
    bounded_text(value["requested_path"], "tool.requested_path", issues)
    if type(value["bytes"]) is not int or not 0 <= value["bytes"] <= MAX_FILE_BYTES:
        issues.append("tool.bytes: invalid file size")
    if type(value["mode"]) is not int or not 0 <= value["mode"] <= 0o777 or value["mode"] & 0o111 == 0:
        issues.append("tool.mode: expected executable permission mode")
    if not isinstance(value["sha256"], str) or HASH_RE.fullmatch(value["sha256"]) is None:
        issues.append("tool.sha256: expected lowercase SHA-256")
    resolved_valid = bounded_text(value["resolved_path"], "tool.resolved_path", issues)
    final_valid = bounded_text(value["final_path"], "tool.final_path", issues)
    if (not resolved_valid or not final_valid or not normalized_absolute(value["resolved_path"]) or
            not normalized_absolute(value["final_path"])):
        issues.append("tool paths: expected normalized absolute paths")
    hops = value["symlink_hops"]
    if not isinstance(hops, list) or len(hops) > 32:
        issues.append("tool.symlink_hops: expected list <= 32")
        return issues
    chain_valid = resolved_valid and final_valid
    for index, hop in enumerate(hops):
        if not exact_fields(hop, HOP_FIELDS, f"tool.symlink_hops[{index}]", issues):
            chain_valid = False
            continue
        path_valid = bounded_text(hop["path"], f"tool.symlink_hops[{index}].path", issues)
        target_valid = bounded_text(hop["target"], f"tool.symlink_hops[{index}].target", issues)
        if not path_valid or not target_valid or not normalized_absolute(hop["path"]):
            issues.append(f"tool.symlink_hops[{index}]: invalid path or target")
            chain_valid = False
    if chain_valid:
        validate_tool_chain(value, issues)
    return issues


def validate_tool_chain(value: dict[str, object], issues: list[str]) -> None:
    resolved, requested = value["resolved_path"], value["requested_path"]
    if not isinstance(requested, str) or posixpath.basename(resolved) != requested:
        issues.append("tool.resolved_path: basename does not match requested_path")
        return
    candidate, seen = resolved, set()
    for hop in value["symlink_hops"]:
        path, target = hop["path"], hop["target"]
        if path in seen:
            issues.append("tool.symlink_hops: cycle or duplicate path")
            return
        seen.add(path)
        if path != candidate and not candidate.startswith(path + "/"):
            issues.append("tool.symlink_hops: hop is not on resolution path")
            return
        remainder = candidate[len(path):].lstrip("/")
        if not target.startswith("/"):
            target = posixpath.join(posixpath.dirname(path), target)
        candidate = posixpath.normpath(posixpath.join(target, remainder))
    if candidate != value["final_path"]:
        issues.append("tool.symlink_hops: chain does not resolve to final_path")


def safe_repo_path(value: object) -> bool:
    if (not isinstance(value, str) or not value or value == "." or
            value.startswith("/") or "\\" in value):
        return False
    if len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":":
        return False
    path = PurePosixPath(value)
    parts = path.parts
    if str(path) != value or any(part in {".", ".."} for part in parts):
        return False
    return not parts or parts[0].lower() not in {".git", ".forge"}


def validate_source(value: object) -> list[str]:
    issues: list[str] = []
    if not exact_fields(value, SOURCE_FIELDS, "source", issues):
        return issues
    if (value["api_version"] != SOURCE_API or value["canonicalization"] != "forgeos.canonical-json/v1" or
            value["profile_id"] != SOURCE_PROFILE or not isinstance(value["source_revision"], str) or
            REVISION_RE.fullmatch(value["source_revision"]) is None):
        issues.append("source: fixed fields or revision drifted")
    bounded_text(value["source_revision"], "source.source_revision", issues)
    entries = value["entries"]
    if not isinstance(entries, list) or len(entries) > 65_536:
        issues.append("source.entries: expected list <= 65536")
        return issues
    paths, total = [], 0
    for index, entry in enumerate(entries):
        issues.extend(validate_source_entry(entry, index))
        if isinstance(entry, dict) and isinstance(entry.get("path"), str):
            paths.append(entry["path"])
        if isinstance(entry, dict) and type(entry.get("bytes")) is int and entry["bytes"] >= 0:
            total += entry["bytes"]
    if paths != sorted(set(paths)):
        issues.append("source.entries: paths must be sorted and unique")
    if total > MAX_SOURCE_BYTES:
        issues.append("source.entries: cumulative bytes exceed limit")
    return issues


def validate_source_entry(value: object, index: int) -> list[str]:
    issues: list[str] = []
    label = f"source.entries[{index}]"
    if not exact_fields(value, ENTRY_FIELDS, label, issues):
        return issues
    if not bounded_text(value["path"], f"{label}.path", issues) or not safe_repo_path(value["path"]):
        issues.append(f"{label}.path: unsafe repository path")
    tracking, mode = value["tracking"], value["index_mode"]
    if tracking not in {"tracked", "untracked"}:
        issues.append(f"{label}.tracking: invalid value")
    if tracking == "tracked" and mode not in {"100644", "100755", "120000"}:
        issues.append(f"{label}.index_mode: tracked entry requires index mode")
    if tracking == "untracked" and mode is not None:
        issues.append(f"{label}.index_mode: untracked entry requires null")
    kind = value["kind"]
    if kind == "regular":
        validate_regular(value, label, issues)
        if tracking == "tracked" and mode == "120000":
            issues.append(f"{label}: regular kind conflicts with symlink index mode")
    elif kind == "symlink":
        validate_symlink(value, label, issues)
        if tracking == "tracked" and mode != "120000":
            issues.append(f"{label}: symlink kind conflicts with regular index mode")
    elif kind == "deleted":
        if (tracking != "tracked" or type(value["bytes"]) is not int or
                value["bytes"] != 0 or any(value[field] is not None for field in (
                    "content_sha256", "executable", "symlink_target"))):
            issues.append(f"{label}: invalid deleted facts")
    else:
        issues.append(f"{label}.kind: invalid value")
    return issues


def validate_regular(value: dict[str, object], label: str, issues: list[str]) -> None:
    digest = value["content_sha256"]
    if (type(value["bytes"]) is not int or not 0 <= value["bytes"] <= MAX_FILE_BYTES or
            not isinstance(digest, str) or HASH_RE.fullmatch(digest) is None or
            type(value["executable"]) is not bool or value["symlink_target"] is not None):
        issues.append(f"{label}: invalid regular facts")


def validate_symlink(value: dict[str, object], label: str, issues: list[str]) -> None:
    target, digest = value["symlink_target"], value["content_sha256"]
    if not bounded_text(target, f"{label}.symlink_target", issues) or value["executable"] is not False:
        issues.append(f"{label}: invalid symlink target or executable")
        return
    expected = hashlib.sha256(target.encode("utf-8")).hexdigest()
    if (type(value["bytes"]) is not int or
            value["bytes"] != len(target.encode("utf-8")) or digest != expected):
        issues.append(f"{label}: inconsistent symlink bytes or digest")
