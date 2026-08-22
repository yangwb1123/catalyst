"""Pure acyclic reconstruction of every Project Source Snapshot v1 seal."""

from __future__ import annotations

from .codec import canonical_json, domain_digest, path_digest
from .constants import (
    API_COVERAGE, API_ENVELOPE, API_MANIFEST, API_REQUEST, API_SNAPSHOT, CANONICAL,
    CONSISTENCY, COVERAGE_SPECS, DOMAINS, ENVELOPE_KIND, EXTRACTOR_ID,
    EXTRACTOR_VERSION, GIT_CONFIG, GIT_IDENTITY, GIT_NETWORK, PATH_POLICY_ID,
    POSITIVE_RESULT, PROFILE_ID, SNAPSHOT_KIND, MAX_ENVELOPE_BYTES,
    MAX_MANIFEST_BYTES,
)


def _seal(value: dict[str, object], field: str, domain: str,
          maximum: int = MAX_MANIFEST_BYTES) -> dict[str, object]:
    result = dict(value)
    result[field] = ""
    result[field] = domain_digest(domain, result, maximum)
    canonical_json(result, maximum)
    return result


def _set_digest(domain: str, values: list[str]) -> str:
    return domain_digest(domain, {"item_count": len(values), "items": values})


def build_request(project_id: str, run_id: str) -> dict[str, object]:
    value = {
        "api_version": API_REQUEST,
        "canonicalization": CANONICAL,
        "extractor_id": EXTRACTOR_ID,
        "extractor_version": EXTRACTOR_VERSION,
        "path_policy_id": PATH_POLICY_ID,
        "profile_id": PROFILE_ID,
        "project_id": project_id,
        "request_sha256": "",
        "run_id": run_id,
    }
    return _seal(value, "request_sha256", DOMAINS["request"])


def build_entry(facts: dict[str, object]) -> dict[str, object]:
    value = {
        "bytes": facts["bytes"],
        "content_sha256": facts["content_sha256"],
        "entry_sha256": "",
        "executable": facts["executable"],
        "index_mode": facts["index_mode"],
        "kind": facts["kind"],
        "path": facts["path"],
        "path_sha256": path_digest(facts["path"], DOMAINS["path"]),
        "tracking": facts["tracking"],
    }
    return _seal(value, "entry_sha256", DOMAINS["entry"])


def build_exclusion(facts: dict[str, object]) -> dict[str, object]:
    value = {
        "exclusion_sha256": "",
        "index_mode": facts["index_mode"],
        "leaf_filesystem_observed": facts["leaf_filesystem_observed"],
        "path_sha256": facts["path_sha256"],
        "reason": facts["reason"],
        "tracking": facts["tracking"],
    }
    return _seal(value, "exclusion_sha256", DOMAINS["exclusion"])


def build_git_observer(facts: dict[str, object]) -> dict[str, object]:
    return {
        "executable_bytes": facts["executable_bytes"],
        "executable_sha256": facts["executable_sha256"],
        "identity_attestation": GIT_IDENTITY,
        "local_config_isolation": GIT_CONFIG,
        "network_containment": GIT_NETWORK,
        "version": facts["version"],
    }


def derive_counts(entries: list[dict[str, object]], excluded: list[dict[str, object]],
                  ignored_path_count: int) -> dict[str, int]:
    counts = {
        "excluded_control_count": 0, "excluded_sensitive_count": 0,
        "excluded_symlink_count": 0, "ignored_path_count": ignored_path_count,
        "included_regular_count": 0, "tracked_absent_count": 0,
        "tracked_count": 0, "universe_count": len(entries) + len(excluded),
        "untracked_count": 0,
    }
    for entry in entries:
        counts[f"{entry['tracking']}_count"] += 1
        counts["included_regular_count" if entry["kind"] == "regular"
               else "tracked_absent_count"] += 1
    for item in excluded:
        counts[f"{item['tracking']}_count"] += 1
        counts[f"excluded_{item['reason'].removesuffix('_path').removesuffix('_leaf')}_count"] += 1
    return counts


def build_manifest(entries: list[dict[str, object]], excluded: list[dict[str, object]],
                   git_observer: dict[str, object], ignored_path_count: int,
                   source_revision: str) -> dict[str, object]:
    ordered_entries = sorted(entries, key=lambda item: item["path_sha256"])
    ordered_excluded = sorted(excluded, key=lambda item: item["path_sha256"])
    value = {
        "api_version": API_MANIFEST,
        "canonicalization": CANONICAL,
        "entries": ordered_entries,
        "entry_set_sha256": _set_digest(
            DOMAINS["entry_set"], [item["entry_sha256"] for item in ordered_entries]),
        "excluded": ordered_excluded,
        "exclusion_set_sha256": _set_digest(
            DOMAINS["exclusion_set"],
            [item["exclusion_sha256"] for item in ordered_excluded]),
        "git_observer": git_observer,
        "ignored_path_count": ignored_path_count,
        "path_policy_id": PATH_POLICY_ID,
        "profile_id": PROFILE_ID,
        "source_manifest_sha256": "",
        "source_revision": source_revision,
        "universe_count": len(entries) + len(excluded),
    }
    return _seal(value, "source_manifest_sha256", DOMAINS["manifest"])


def build_coverage(manifest: dict[str, object], counts: dict[str, int]) -> dict[str, object]:
    surfaces = []
    for surface, status, observed, reasons in COVERAGE_SPECS:
        count = counts[observed] if isinstance(observed, str) else observed
        surfaces.append({"observed_item_count": count, "reason_codes": list(reasons),
                         "status": status, "surface": surface})
    value = {
        "api_version": API_COVERAGE,
        "canonicalization": CANONICAL,
        "counts": counts,
        "coverage_sha256": "",
        "source_manifest_sha256": manifest["source_manifest_sha256"],
        "surfaces": surfaces,
    }
    return _seal(value, "coverage_sha256", DOMAINS["coverage"])


def _snapshot_identity(request: dict[str, object], manifest: dict[str, object],
                       coverage: dict[str, object]) -> dict[str, object]:
    return {
        "coverage_sha256": coverage["coverage_sha256"],
        "extractor_id": EXTRACTOR_ID,
        "extractor_version": EXTRACTOR_VERSION,
        "profile_id": PROFILE_ID,
        "project_id": request["project_id"],
        "request_sha256": request["request_sha256"],
        "run_id": request["run_id"],
        "source_manifest_sha256": manifest["source_manifest_sha256"],
    }


def build_snapshot(request: dict[str, object], manifest: dict[str, object],
                   coverage: dict[str, object]) -> dict[str, object]:
    identity = _snapshot_identity(request, manifest, coverage)
    identity_digest = domain_digest(DOMAINS["snapshot_identity"], identity)
    value = {
        "api_version": API_SNAPSHOT, "atomic": False, "authority_attested": False,
        "canonicalization": CANONICAL, "consistency": CONSISTENCY,
        "coverage": coverage, "coverage_sha256": coverage["coverage_sha256"],
        "currentness": "unknown", "effect_attested": False,
        "extractor": {"extractor_id": EXTRACTOR_ID,
                      "extractor_version": EXTRACTOR_VERSION},
        "freshness": "unknown", "kind": SNAPSHOT_KIND,
        "permission_attested": False, "persistence_attested": False,
        "positive_result": POSITIVE_RESULT, "profile_id": PROFILE_ID,
        "project_id": request["project_id"], "request_sha256": request["request_sha256"],
        "run_id": request["run_id"], "snapshot_id": f"project-snapshot-{identity_digest}",
        "snapshot_identity_sha256": identity_digest, "snapshot_sha256": "",
        "source_manifest": manifest,
        "source_manifest_sha256": manifest["source_manifest_sha256"],
        "system_completeness": "unknown", "truth_attested": False,
    }
    return _seal(value, "snapshot_sha256", DOMAINS["snapshot"], MAX_ENVELOPE_BYTES)


def build_production(project_id: str, run_id: str, entry_facts: list[dict[str, object]],
                     exclusion_facts: list[dict[str, object]], git_facts: dict[str, object],
                     ignored_path_count: int, source_revision: str) -> dict[str, object]:
    request = build_request(project_id, run_id)
    entries = [build_entry(item) for item in entry_facts]
    excluded = [build_exclusion(item) for item in exclusion_facts]
    git_observer = build_git_observer(git_facts)
    manifest = build_manifest(entries, excluded, git_observer, ignored_path_count,
                              source_revision)
    coverage = build_coverage(manifest, derive_counts(entries, excluded,
                                                      ignored_path_count))
    snapshot = build_snapshot(request, manifest, coverage)
    envelope = {"api_version": API_ENVELOPE, "canonicalization": CANONICAL,
                "envelope_sha256": "", "kind": ENVELOPE_KIND,
                "request": request, "snapshot": snapshot}
    return _seal(envelope, "envelope_sha256", DOMAINS["envelope"], MAX_ENVELOPE_BYTES)
