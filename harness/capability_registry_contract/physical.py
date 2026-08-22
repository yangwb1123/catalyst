"""Fail-closed physical repository bindings for the staged singleton entry."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .codec import ContractError, canonical_json
from .constants import FORBIDDEN_ENTRY_PATHS
from .filesystem import (
    assert_snapshots, guard_root, read_regular, scan_regular, stable_root,
)
from .validation import validate_registry


def read_physical_ref(root: Path, ref: dict[str, object]) -> bytes:
    relative = ref["path"]
    raw = read_regular(root, relative, int(ref["content_bytes"]))
    if len(raw) != ref["content_bytes"]:
        raise ContractError(f"content ref byte length mismatch: {relative}")
    if hashlib.sha256(raw).hexdigest() != ref["content_sha256"]:
        raise ContractError(f"content ref digest mismatch: {relative}")
    return raw


def _forbidden(relative: str) -> bool:
    return (relative in FORBIDDEN_ENTRY_PATHS or
            relative.startswith("harness/capability_registry_contract/") or
            relative.startswith("harness/governance_engineering/capability_registry"))


def _verify_set(root: Path, content_set: dict[str, object], root_guard,
                all_paths: set[str]) -> None:
    declared = [item["path"] for item in content_set["files"]]
    if any(path in all_paths for path in declared):
        raise ContractError("physical content path appears in multiple sets")
    all_paths.update(declared)
    if any(_forbidden(path) for path in declared):
        raise ContractError("entry content set contains registry/governance self-reference")
    selection, inventory = content_set["selection"], None
    if selection["mode"] == "all_regular_files_recursive_with_suffixes":
        actual, inventory = scan_regular(
            root, selection["root"], tuple(selection["suffixes"]), root_guard)
        if set(declared) != set(actual):
            raise ContractError("recursive content-set selection differs from declared files")
    for ref in content_set["files"]:
        raw = read_regular(root, ref["path"], int(ref["content_bytes"]),
                           inventory or root_guard)
        if (len(raw) != ref["content_bytes"] or
                hashlib.sha256(raw).hexdigest() != ref["content_sha256"]):
            raise ContractError(f"content ref physical bytes drifted: {ref['path']}")
    if inventory is not None:
        assert_snapshots(inventory, selection["root"])


def _verify_refs(root: Path, entry: dict[str, object], root_guard) -> None:
    contract = entry["contract"]
    refs = contract["input_schemas"] + contract["output_schemas"]
    refs += [ref for proof in contract["proof_obligations"]
             for ref in proof["verification_refs"]]
    refs += [ref for test in entry["tests"] for ref in test["fixture_refs"]]
    for ref in refs:
        if _forbidden(ref["path"]):
            raise ContractError("entry contract/test contains registry self-reference")
        raw = read_regular(root, ref["path"], int(ref["content_bytes"]), root_guard)
        if (len(raw) != ref["content_bytes"] or
                hashlib.sha256(raw).hexdigest() != ref["content_sha256"]):
            raise ContractError(f"content ref physical bytes drifted: {ref['path']}")


def validate_physical_registry(repo_root: Path,
                               registry: object) -> list[str]:
    try:
        checked = validate_registry(registry)
        root, root_identity = stable_root(repo_root)
        root_guard = guard_root(root, root_identity)
        from .builder import build_registry
        rebuilt = build_registry(root, root_guard)
        if canonical_json(checked) != canonical_json(rebuilt):
            raise ContractError("registry differs from exact reconstructed singleton profile")
        entry, all_paths = checked["entries"][0], set()
        for content_set in entry["content_sets"]:
            _verify_set(root, content_set, root_guard, all_paths)
        _verify_refs(root, entry, root_guard)
        assert_snapshots(root_guard, "repository root")
        return []
    except (ContractError, OSError, ValueError) as error:
        return [f"Capability Registry v1 physical binding: {error}"]
    except (MemoryError, RecursionError) as error:
        return [f"Capability Registry v1 physical binding exhausted bound: {error}"]
