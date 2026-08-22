"""Closed shapes and source-fact validation for the v1 wire."""

from __future__ import annotations

import re

from .codec import ContractError, forbidden_scalar, short_text
from .constants import (
    CONTROL_COMPONENTS, MAX_EXCLUDED, MAX_FILE_BYTES, MAX_GIT_BYTES, MAX_IGNORED,
    MAX_PATH_BYTES, MAX_PATH_COMPONENTS, MAX_PATH_SCALARS, MAX_TOTAL_BYTES,
    MAX_UNIVERSE, SENSITIVE_BASENAMES, SENSITIVE_COMPONENTS, SENSITIVE_PREFIXES,
    SENSITIVE_SUFFIXES,
)

DIGEST_RE = re.compile(r"[0-9a-f]{64}")
IDENTIFIER_RE = re.compile(r"[a-z0-9][a-z0-9._:/-]*")
REVISION_RE = re.compile(r"git-sha(?:1:[0-9a-f]{40}|256:[0-9a-f]{64})")
MODES = {"100644", "100755", "120000"}

REQUEST_FIELDS = {"api_version", "canonicalization", "extractor_id",
                  "extractor_version", "path_policy_id", "profile_id",
                  "project_id", "request_sha256", "run_id"}
ENTRY_FIELDS = {"bytes", "content_sha256", "entry_sha256", "executable",
                "index_mode", "kind", "path", "path_sha256", "tracking"}
EXCLUSION_FIELDS = {"exclusion_sha256", "index_mode", "leaf_filesystem_observed",
                    "path_sha256", "reason", "tracking"}
GIT_FIELDS = {"executable_bytes", "executable_sha256", "identity_attestation",
              "local_config_isolation", "network_containment", "version"}
MANIFEST_FIELDS = {"api_version", "canonicalization", "entries", "entry_set_sha256",
                   "excluded", "exclusion_set_sha256", "git_observer",
                   "ignored_path_count", "path_policy_id", "profile_id",
                   "source_manifest_sha256", "source_revision", "universe_count"}
COUNT_FIELDS = {"excluded_control_count", "excluded_sensitive_count",
                "excluded_symlink_count", "ignored_path_count",
                "included_regular_count", "tracked_absent_count", "tracked_count",
                "universe_count", "untracked_count"}
SURFACE_FIELDS = {"observed_item_count", "reason_codes", "status", "surface"}
COVERAGE_FIELDS = {"api_version", "canonicalization", "counts", "coverage_sha256",
                   "source_manifest_sha256", "surfaces"}
EXTRACTOR_FIELDS = {"extractor_id", "extractor_version"}
SNAPSHOT_FIELDS = {"api_version", "atomic", "authority_attested", "canonicalization",
                   "consistency", "coverage", "coverage_sha256", "currentness",
                   "effect_attested", "extractor", "freshness", "kind",
                   "permission_attested", "persistence_attested", "positive_result",
                   "profile_id", "project_id", "request_sha256", "run_id", "snapshot_id",
                   "snapshot_identity_sha256", "snapshot_sha256", "source_manifest",
                   "source_manifest_sha256", "system_completeness", "truth_attested"}
ENVELOPE_FIELDS = {"api_version", "canonicalization", "envelope_sha256", "kind",
                   "request", "snapshot"}


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ContractError(f"{label}: unexpected, missing, or non-object fields")
    return value


def exact_array(value: object, maximum: int, label: str) -> list[object]:
    if not isinstance(value, list) or len(value) > maximum:
        raise ContractError(f"{label}: expected array of at most {maximum} items")
    return value


def integer(value: object, low: int, high: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{label}: expected integer in {low}..{high}")
    return value


def digest(value: object, label: str) -> str:
    if not isinstance(value, str) or DIGEST_RE.fullmatch(value) is None:
        raise ContractError(f"{label}: expected lowercase SHA-256")
    return value


def identifier(value: object, label: str) -> str:
    if (not isinstance(value, str) or len(value) > 160 or
            IDENTIFIER_RE.fullmatch(value) is None):
        raise ContractError(f"{label}: invalid identifier")
    return value


def fixed(value: object, expected: object, label: str) -> None:
    if type(value) is not type(expected) or value != expected:
        raise ContractError(f"{label}: expected fixed value {expected!r}")


def ascii_fold(value: str) -> str:
    return "".join(chr(ord(char) + 32) if "A" <= char <= "Z" else char for char in value)


def validate_path(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ContractError(f"{label}: expected nonempty path")
    encoded = value.encode("utf-8")
    components = value.split("/")
    drive = len(value) >= 2 and value[0].isascii() and value[0].isalpha() and value[1] == ":"
    if (len(encoded) > MAX_PATH_BYTES or len(value) > MAX_PATH_SCALARS or
            len(components) > MAX_PATH_COMPONENTS or value.startswith("/") or
            value.endswith("/") or "\\" in value or drive or
            any(part in {"", ".", ".."} for part in components) or
            any(forbidden_scalar(char) for char in value)):
        raise ContractError(f"{label}: invalid canonical repository-relative path")
    return value


def protected_reason(path: str) -> str | None:
    components = [ascii_fold(component) for component in path.split("/")]
    if any(component in CONTROL_COMPONENTS for component in components):
        return "control_path"
    if any(component in SENSITIVE_COMPONENTS for component in components):
        return "sensitive_path"
    basename = components[-1]
    if (basename in SENSITIVE_BASENAMES or
            any(basename.startswith(prefix) for prefix in SENSITIVE_PREFIXES) or
            any(basename.endswith(suffix) for suffix in SENSITIVE_SUFFIXES)):
        return "sensitive_path"
    return None


def validate_entry(value: object, label: str) -> dict[str, object]:
    entry = exact_object(value, ENTRY_FIELDS, label)
    integer(entry["bytes"], 0, MAX_FILE_BYTES, f"{label}.bytes")
    digest(entry["entry_sha256"], f"{label}.entry_sha256")
    path = validate_path(entry["path"], f"{label}.path")
    digest(entry["path_sha256"], f"{label}.path_sha256")
    if protected_reason(path) is not None:
        raise ContractError(f"{label}.path: protected path disclosed as entry")
    if entry["tracking"] not in {"tracked", "untracked"}:
        raise ContractError(f"{label}.tracking: invalid value")
    if entry["tracking"] == "tracked" and entry["index_mode"] not in MODES:
        raise ContractError(f"{label}.index_mode: tracked entry requires a mode")
    if entry["tracking"] == "untracked" and entry["index_mode"] is not None:
        raise ContractError(f"{label}.index_mode: untracked entry must be null")
    _validate_entry_kind(entry, label)
    return entry


def _validate_entry_kind(entry: dict[str, object], label: str) -> None:
    if entry["kind"] == "regular":
        digest(entry["content_sha256"], f"{label}.content_sha256")
        if type(entry["executable"]) is not bool:
            raise ContractError(f"{label}.executable: regular entry requires boolean")
        if entry["tracking"] == "tracked" and entry["index_mode"] == "120000":
            raise ContractError(f"{label}: regular entry cannot retain symlink index mode")
        return
    if (entry["kind"] != "tracked_absent" or entry["tracking"] != "tracked" or
            entry["bytes"] != 0 or entry["content_sha256"] is not None or
            entry["executable"] is not None):
        raise ContractError(f"{label}: invalid tracked_absent facts")


def validate_exclusion(value: object, label: str) -> dict[str, object]:
    item = exact_object(value, EXCLUSION_FIELDS, label)
    digest(item["exclusion_sha256"], f"{label}.exclusion_sha256")
    digest(item["path_sha256"], f"{label}.path_sha256")
    if item["tracking"] not in {"tracked", "untracked"}:
        raise ContractError(f"{label}.tracking: invalid value")
    if item["tracking"] == "tracked" and item["index_mode"] not in MODES:
        raise ContractError(f"{label}.index_mode: tracked exclusion requires a mode")
    if item["tracking"] == "untracked" and item["index_mode"] is not None:
        raise ContractError(f"{label}.index_mode: untracked exclusion must be null")
    if item["reason"] not in {"control_path", "sensitive_path", "symlink_leaf"}:
        raise ContractError(f"{label}.reason: invalid value")
    expected = item["reason"] == "symlink_leaf"
    if type(item["leaf_filesystem_observed"]) is not bool or item["leaf_filesystem_observed"] != expected:
        raise ContractError(f"{label}.leaf_filesystem_observed: reason mismatch")
    return item


def validate_manifest_bounds(manifest: dict[str, object]) -> None:
    entries = exact_array(manifest["entries"], MAX_UNIVERSE, "manifest.entries")
    excluded = exact_array(manifest["excluded"], MAX_EXCLUDED, "manifest.excluded")
    if len(entries) + len(excluded) > MAX_UNIVERSE:
        raise ContractError("manifest universe exceeds bound")
    if manifest["universe_count"] != len(entries) + len(excluded):
        raise ContractError("manifest universe_count conservation failed")
    integer(manifest["ignored_path_count"], 0, MAX_IGNORED, "manifest.ignored_path_count")


def validate_total_bytes(entries: list[dict[str, object]]) -> None:
    if sum(entry["bytes"] for entry in entries) > MAX_TOTAL_BYTES:
        raise ContractError("manifest aggregate included bytes exceeds bound")


def validate_revision(value: object) -> str:
    if not isinstance(value, str) or REVISION_RE.fullmatch(value) is None:
        raise ContractError("manifest.source_revision: invalid Git revision hint")
    return value


def validate_git_facts(value: object) -> dict[str, object]:
    git = exact_object(value, GIT_FIELDS, "manifest.git_observer")
    integer(git["executable_bytes"], 1, MAX_GIT_BYTES, "git.executable_bytes")
    digest(git["executable_sha256"], "git.executable_sha256")
    version = short_text(git["version"], "git.version")
    if not version.startswith("git version ") or len(version) <= len("git version "):
        raise ContractError("git.version: expected exact nonempty version output")
    return git
