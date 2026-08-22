"""Strict pure decoder and exact semantic reconstruction."""

from __future__ import annotations

from .codec import ContractError, canonical_json, decode_canonical, path_digest
from .constants import (
    API_COVERAGE, API_ENVELOPE, API_MANIFEST, API_REQUEST, API_SNAPSHOT, CANONICAL,
    DOMAINS, ENVELOPE_KIND, EXTRACTOR_ID, EXTRACTOR_VERSION, GIT_CONFIG,
    GIT_IDENTITY, GIT_NETWORK, MAX_ENVELOPE_BYTES, MAX_MANIFEST_BYTES,
    PATH_POLICY_ID, PROFILE_ID,
    SNAPSHOT_KIND,
)
from .derive import build_production
from .shapes import (
    COUNT_FIELDS, COVERAGE_FIELDS, ENVELOPE_FIELDS, EXCLUSION_FIELDS,
    EXTRACTOR_FIELDS, GIT_FIELDS, MANIFEST_FIELDS, REQUEST_FIELDS, SNAPSHOT_FIELDS,
    SURFACE_FIELDS, digest, exact_array, exact_object, fixed, identifier, integer,
    validate_entry, validate_exclusion, validate_git_facts, validate_manifest_bounds,
    validate_revision, validate_total_bytes,
)


def _fixed_request(value: object) -> dict[str, object]:
    item = exact_object(value, REQUEST_FIELDS, "request")
    fixed(item["api_version"], API_REQUEST, "request.api_version")
    fixed(item["canonicalization"], CANONICAL, "request.canonicalization")
    fixed(item["extractor_id"], EXTRACTOR_ID, "request.extractor_id")
    fixed(item["extractor_version"], EXTRACTOR_VERSION, "request.extractor_version")
    fixed(item["path_policy_id"], PATH_POLICY_ID, "request.path_policy_id")
    fixed(item["profile_id"], PROFILE_ID, "request.profile_id")
    identifier(item["project_id"], "request.project_id")
    identifier(item["run_id"], "request.run_id")
    digest(item["request_sha256"], "request.request_sha256")
    return item


def _validate_manifest(value: object) -> dict[str, object]:
    item = exact_object(value, MANIFEST_FIELDS, "snapshot.source_manifest")
    canonical_json(item, MAX_MANIFEST_BYTES)
    fixed(item["api_version"], API_MANIFEST, "manifest.api_version")
    fixed(item["canonicalization"], CANONICAL, "manifest.canonicalization")
    fixed(item["path_policy_id"], PATH_POLICY_ID, "manifest.path_policy_id")
    fixed(item["profile_id"], PROFILE_ID, "manifest.profile_id")
    digest(item["entry_set_sha256"], "manifest.entry_set_sha256")
    digest(item["exclusion_set_sha256"], "manifest.exclusion_set_sha256")
    digest(item["source_manifest_sha256"], "manifest.source_manifest_sha256")
    validate_revision(item["source_revision"])
    validate_manifest_bounds(item)
    _validate_records(item)
    _validate_git(item["git_observer"])
    return item


def _validate_records(manifest: dict[str, object]) -> None:
    entries = [validate_entry(value, f"manifest.entries[{index}]")
               for index, value in enumerate(manifest["entries"])]
    excluded = [validate_exclusion(value, f"manifest.excluded[{index}]")
                for index, value in enumerate(manifest["excluded"])]
    validate_total_bytes(entries)
    entry_paths = [item["path_sha256"] for item in entries]
    excluded_paths = [item["path_sha256"] for item in excluded]
    if entry_paths != sorted(entry_paths) or len(entry_paths) != len(set(entry_paths)):
        raise ContractError("manifest.entries: not strict path-digest sorted unique")
    if excluded_paths != sorted(excluded_paths) or len(excluded_paths) != len(set(excluded_paths)):
        raise ContractError("manifest.excluded: not strict path-digest sorted unique")
    if set(entry_paths) & set(excluded_paths):
        raise ContractError("manifest: cross-array path digest duplicate")
    for index, entry in enumerate(entries):
        expected = path_digest(entry["path"], DOMAINS["path"])
        if entry["path_sha256"] != expected:
            raise ContractError(f"manifest.entries[{index}]: path digest mismatch")


def _validate_git(value: object) -> None:
    item = validate_git_facts(value)
    fixed(item["identity_attestation"], GIT_IDENTITY, "git.identity_attestation")
    fixed(item["local_config_isolation"], GIT_CONFIG, "git.local_config_isolation")
    fixed(item["network_containment"], GIT_NETWORK, "git.network_containment")


def _validate_coverage(value: object) -> dict[str, object]:
    item = exact_object(value, COVERAGE_FIELDS, "snapshot.coverage")
    fixed(item["api_version"], API_COVERAGE, "coverage.api_version")
    fixed(item["canonicalization"], CANONICAL, "coverage.canonicalization")
    digest(item["coverage_sha256"], "coverage.coverage_sha256")
    digest(item["source_manifest_sha256"], "coverage.source_manifest_sha256")
    counts = exact_object(item["counts"], COUNT_FIELDS, "coverage.counts")
    for field, count in counts.items():
        integer(count, 0, 262_144, f"coverage.counts.{field}")
    surfaces = exact_array(item["surfaces"], 12, "coverage.surfaces")
    if len(surfaces) != 12:
        raise ContractError("coverage.surfaces: expected exactly 12 records")
    for index, surface in enumerate(surfaces):
        record = exact_object(surface, SURFACE_FIELDS, f"coverage.surfaces[{index}]")
        integer(record["observed_item_count"], 0, 262_144,
                f"coverage.surfaces[{index}].observed_item_count")
        reasons = exact_array(record["reason_codes"], 8,
                              f"coverage.surfaces[{index}].reason_codes")
        if not reasons or any(not isinstance(reason, str) or not reason for reason in reasons):
            raise ContractError("coverage surface reason codes must be nonempty strings")
    return item


def _validate_snapshot(value: object) -> dict[str, object]:
    item = exact_object(value, SNAPSHOT_FIELDS, "snapshot")
    fixed(item["api_version"], API_SNAPSHOT, "snapshot.api_version")
    fixed(item["canonicalization"], CANONICAL, "snapshot.canonicalization")
    fixed(item["kind"], SNAPSHOT_KIND, "snapshot.kind")
    for field in ("coverage_sha256", "request_sha256", "snapshot_identity_sha256",
                  "snapshot_sha256", "source_manifest_sha256"):
        digest(item[field], f"snapshot.{field}")
    identifier(item["project_id"], "snapshot.project_id")
    identifier(item["run_id"], "snapshot.run_id")
    exact_object(item["extractor"], EXTRACTOR_FIELDS, "snapshot.extractor")
    _validate_manifest(item["source_manifest"])
    _validate_coverage(item["coverage"])
    return item


def _facts(value: dict[str, object]) -> tuple[object, ...]:
    request, snapshot = value["request"], value["snapshot"]
    manifest = snapshot["source_manifest"]
    entries = [{key: item[key] for key in ENTRY_FACT_FIELDS}
               for item in manifest["entries"]]
    excluded = [{key: item[key] for key in EXCLUSION_FACT_FIELDS}
                for item in manifest["excluded"]]
    git = {key: manifest["git_observer"][key] for key in GIT_FACT_FIELDS}
    return (request["project_id"], request["run_id"], entries, excluded, git,
            manifest["ignored_path_count"], manifest["source_revision"])


ENTRY_FACT_FIELDS = {"bytes", "content_sha256", "executable", "index_mode", "kind", "path", "tracking"}
EXCLUSION_FACT_FIELDS = {"index_mode", "leaf_filesystem_observed", "path_sha256", "reason", "tracking"}
GIT_FACT_FIELDS = {"executable_bytes", "executable_sha256", "version"}


def validate_production(value: object) -> dict[str, object]:
    envelope = exact_object(value, ENVELOPE_FIELDS, "envelope")
    fixed(envelope["api_version"], API_ENVELOPE, "envelope.api_version")
    fixed(envelope["canonicalization"], CANONICAL, "envelope.canonicalization")
    fixed(envelope["kind"], ENVELOPE_KIND, "envelope.kind")
    digest(envelope["envelope_sha256"], "envelope.envelope_sha256")
    _fixed_request(envelope["request"])
    _validate_snapshot(envelope["snapshot"])
    expected = build_production(*_facts(envelope))
    if canonical_json(envelope, MAX_ENVELOPE_BYTES) != canonical_json(expected, MAX_ENVELOPE_BYTES):
        raise ContractError("production differs from exact semantic reconstruction")
    return envelope


def decode_production(raw: bytes) -> dict[str, object]:
    return validate_production(decode_canonical(raw, MAX_ENVELOPE_BYTES,
                                                "project source snapshot production"))
