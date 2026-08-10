#!/usr/bin/env python3
"""Shared strict parsing and reference helpers for engineering contracts."""
import os
from pathlib import Path

import yaml


MAX_SPEC_BYTES = 512 * 1024


class _UniqueKeyLoader(yaml.SafeLoader):
    """Safe YAML loader that rejects duplicate keys instead of overwriting."""


def _construct_unique_mapping(loader, node, deep=False):
    mapping = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        try:
            duplicate = key in mapping
        except TypeError as exc:
            raise yaml.constructor.ConstructorError(
                None, None, "unhashable mapping key", key_node.start_mark,
            ) from exc
        if duplicate:
            raise yaml.constructor.ConstructorError(
                None, None, f"duplicate key {key!r}", key_node.start_mark,
            )
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


_UniqueKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    _construct_unique_mapping,
)


def read_bounded_spec(path):
    try:
        with path.open("rb") as stream:
            if os.fstat(stream.fileno()).st_size > MAX_SPEC_BYTES:
                raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
            raw = stream.read(MAX_SPEC_BYTES + 1)
    except MemoryError as error:
        raise ValueError("bounded spec read exhausted memory") from error
    if len(raw) > MAX_SPEC_BYTES:
        raise ValueError(f"file exceeds {MAX_SPEC_BYTES} bytes")
    return raw


def load_yaml(path):
    try:
        raw = read_bounded_spec(path)
        text = raw.decode("utf-8")
        token_types = (yaml.tokens.AnchorToken, yaml.tokens.AliasToken)
        if any(isinstance(token, token_types) for token in yaml.scan(text)):
            raise ValueError("YAML anchors and aliases are not allowed")
        return yaml.load(text, Loader=_UniqueKeyLoader), None
    except (OSError, UnicodeDecodeError, ValueError, RecursionError, yaml.YAMLError) as exc:
        return None, str(exc).replace("\n", " ")


def mapping_issues(data, path, kind):
    if isinstance(data, dict):
        return []
    return [f"{path}: {kind} must be a YAML mapping"]


def unknown_field_issues(data, allowed, label):
    if not isinstance(data, dict):
        return []
    unknown = set(data) - set(allowed)
    return [f"{label}: unknown fields {sorted(unknown, key=lambda item: str(item))}"] if unknown else []


def header_issues(data, path, kind):
    issues = []
    if data.get("api_version") != "forgeos.agent-engineering/v1":
        issues.append(f"{path}: unsupported api_version")
    if data.get("kind") != kind or data.get("status") != "active_contract":
        issues.append(f"{path}: expected kind {kind!r} with active_contract status")
    return issues


def unique_id_issues(items, path, kind):
    issues, seen = [], set()
    for index, item in enumerate(items if isinstance(items, list) else []):
        item_id = item.get("id") if isinstance(item, dict) else None
        if not isinstance(item_id, str) or not item_id.strip():
            issues.append(f"{path}: {kind}[{index}] requires a non-empty id")
        elif item_id in seen:
            issues.append(f"{path}: duplicate {kind} id {item_id!r}")
        else:
            seen.add(item_id)
    return issues, seen


def repo_path_issue(repo_root, raw, label, require_exists=True):
    if not isinstance(raw, str) or not raw.strip():
        return f"{label}: expected a non-empty repository-relative path"
    rel = Path(raw)
    if rel.is_absolute() or ".." in rel.parts:
        return f"{label}: unsafe repository path {raw!r}"
    target = repo_root / rel
    if require_exists and not target.exists():
        return f"{label}: referenced path does not exist: {raw!r}"
    if require_exists:
        try:
            target.resolve(strict=True).relative_to(repo_root.resolve(strict=True))
        except (OSError, ValueError):
            return f"{label}: path escapes repository through a symlink: {raw!r}"
    return None
