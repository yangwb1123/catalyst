"""Caller- and policy-bound expected raw repository file manifest."""

from __future__ import annotations

from typing import Any

from .canonical import ContractError, bounded_canonical_json, self_digest
from .constants import (CANONICALIZATION, MANIFEST_API, MANIFEST_DOMAIN,
                        MAX_MANIFEST_BYTES, MAX_OUTPUT_BYTES, MAX_PATHS)
from .shape import integer, require_keys, sha256, validate_path

FIELDS = {"api_version", "canonicalization", "entries", "kind", "manifest_sha256"}
ENTRY_FIELDS = {"content_bytes", "content_sha256", "kind", "path"}


def manifest_sha256(value: dict[str, Any]) -> str:
    return self_digest(MANIFEST_DOMAIN, value, "manifest_sha256", MAX_MANIFEST_BYTES,
                       "RepoReadExpectedManifest")


def validate_manifest(value: Any) -> dict[str, Any]:
    node = require_keys(value, "RepoReadExpectedManifest", FIELDS)
    bounded_canonical_json(node, MAX_MANIFEST_BYTES, "RepoReadExpectedManifest")
    expected = (MANIFEST_API, CANONICALIZATION, "RepoReadExpectedManifest")
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError("RepoReadExpectedManifest envelope drifted from v1")
    entries = node["entries"]
    if not isinstance(entries, list) or not 1 <= len(entries) <= MAX_PATHS:
        raise ContractError("manifest entries must contain 1..16 exact paths")
    total = _validate_entries(entries)
    if total > MAX_OUTPUT_BYTES:
        raise ContractError("manifest aggregate raw content exceeds 1048576 bytes")
    sha256(node["manifest_sha256"], "manifest.manifest_sha256")
    if node["manifest_sha256"] != manifest_sha256(node):
        raise ContractError("RepoReadExpectedManifest self digest does not match")
    return node


def _validate_entries(entries: list[Any]) -> int:
    paths: list[str] = []
    total = 0
    for index, value in enumerate(entries):
        label = f"manifest.entries[{index}]"
        entry = require_keys(value, label, ENTRY_FIELDS)
        if entry["kind"] != "regular":
            raise ContractError(f"{label}.kind must be 'regular'")
        paths.append(validate_path(entry["path"], f"{label}.path"))
        total += integer(entry["content_bytes"], f"{label}.content_bytes",
                         0, MAX_OUTPUT_BYTES)
        sha256(entry["content_sha256"], f"{label}.content_sha256")
    encoded = [path.encode("utf-8") for path in paths]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError("manifest paths must be UTF-8-byte sorted and unique")
    return total


def manifest_paths(value: dict[str, Any]) -> list[str]:
    return [entry["path"] for entry in value["entries"]]


def manifest_content_bytes(value: dict[str, Any]) -> int:
    return sum(entry["content_bytes"] for entry in value["entries"])
