"""Deterministic builder for the one staged physical Registry profile."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .codec import canonical_json
from .constants import (
    CANONICALIZATION, CONTRACT_API, EFFECT_VOCABULARY_SHA256, ENTRY_API,
    FIXTURE_API, FROZEN_SET_PINS, LEGACY_OPAQUE_SHA256, REGISTRY_API, REQUEST_API,
)
from .digests import seal
from .filesystem import (
    Snapshots, assert_snapshots, guard_root, read_regular, scan_regular, stable_root,
)
from .resolver import resolve_declared


def _safe_bytes(root: Path, relative: str, guard: Snapshots) -> bytes:
    return read_regular(root, relative, 16 * 1024 * 1024, guard)


def _ref(root: Path, relative: str, media_type: str,
         selector: str | None, guard: Snapshots) -> dict[str, object]:
    raw = _safe_bytes(root, relative, guard)
    return {
        "content_bytes": len(raw), "content_sha256": hashlib.sha256(raw).hexdigest(),
        "media_type": media_type, "path": relative, "selector": selector,
    }


def _recursive_set(root: Path, relative: str, suffix: str,
                   media_type: str, guard: Snapshots) -> dict[str, object]:
    paths, snapshots = scan_regular(root, relative, (suffix,), guard)
    if len(paths) > 256:
        raise ValueError("builder recursive content set exceeds 256 matching files")
    if len(paths) != FROZEN_SET_PINS[relative][0]:
        raise ValueError(f"builder recursive content count drifted: {relative}")
    files = []
    for path in paths:
        raw = read_regular(root, path, 16 * 1024 * 1024, snapshots)
        files.append({"content_bytes": len(raw),
                      "content_sha256": hashlib.sha256(raw).hexdigest(),
                      "media_type": media_type, "path": path, "selector": None})
    assert_snapshots(snapshots, relative)
    files.sort(key=canonical_json)
    value = {
        "files": files,
        "selection": {"mode": "all_regular_files_recursive_with_suffixes",
                      "root": relative, "suffixes": [suffix]},
        "set_sha256": "",
    }
    return seal("content_set", value)


def _explicit_set(root: Path, guard: Snapshots) -> dict[str, object]:
    paths = [
        "harness/local_go_package_impact_prescan_contract_check.py",
        "harness/test_local_go_package_impact_prescan_bounds.py",
        "harness/test_local_go_package_impact_prescan_contract_check.py",
    ]
    files = [_ref(root, path, "text/x-python", None, guard) for path in paths]
    files.sort(key=canonical_json)
    value = {
        "files": files,
        "selection": {"mode": "explicit_files", "root": None, "suffixes": []},
        "set_sha256": "",
    }
    return seal("content_set", value)


def _proofs(root: Path, guard: Snapshots) -> list[dict[str, object]]:
    cases = {
        "deterministic-shortest-witness": [
            "forge-core/internal/goimpactprescan/locality_test.go",
            "harness/test_local_go_package_impact_prescan_contract_check.py"],
        "exact-graph-digest-and-run-binding": [
            "forge-core/internal/goimpactprescan/golden_test.go",
            "harness/test_local_go_package_impact_prescan_contract_check.py"],
        "exact-local-reverse-fixed-point": [
            "forge-core/internal/goimpactprescan/build_test.go",
            "harness/test_local_go_package_impact_prescan_contract_check.py"],
        "system-impact-remains-unknown": [
            "forge-core/internal/goimpactprescan/observation_test.go",
            "harness/test_local_go_package_impact_prescan_bounds.py"],
    }
    result = []
    for obligation_id, paths in sorted(cases.items()):
        refs = [_ref(root, path, "text/x-go" if path.endswith(".go") else "text/x-python",
                     None, guard)
                for path in paths]
        refs.sort(key=canonical_json)
        result.append({"description": obligation_id.replace("-", " "),
                       "obligation_id": obligation_id, "verification_refs": refs})
    return result


def _contract(root: Path, guard: Snapshots) -> dict[str, object]:
    schema = "docs/contracts/local-go-package-impact-prescan-v1.schema.json"
    return seal("contract", {
        "api_version": CONTRACT_API, "canonicalization": CANONICALIZATION,
        "capability_contract_id": "", "capability_contract_sha256": "",
        "capability_id": "local-go-package-impact-prescan", "capability_version": "1",
        "domain": "reasoning", "effects": [],
        "failure_modes": [
            {"disposition": "fail_closed_no_output", "failure_id": "invalid-input",
             "result": "invalid or out-of-bounds caller bytes produce no prescan envelope"},
            {"disposition": "structured_negative_assessment", "failure_id": "unresolved-local-path",
             "result": "unknown or non-Go paths remain bounded unresolved evidence"}],
        "input_schemas": [_ref(root, schema, "application/schema+json",
                               "#/$defs/request", guard)],
        "kind": "CapabilityContract", "not_applicable": {"mode": "never", "predicates": []},
        "observability": [{"signal_id": "prescan-envelope", "signal_kind": "artifact"}],
        "output_schemas": [_ref(root, schema, "application/schema+json", "#", guard)],
        "permission_requirements": [],
        "postconditions": ["exact-local-reverse-dependency-closure", "system-impact-is-unknown"],
        "preconditions": ["caller-supplies-canonical-request-bytes",
                          "caller-supplies-explicit-graph-observation"],
        "proof_obligations": _proofs(root, guard),
        "quality_gates": [
            {"gate_id": "cross-language-exact-golden",
             "required_test_ids": ["go-contract-suite", "python-contract-suite"]},
            {"gate_id": "malformed-tamper-and-bounds",
             "required_test_ids": ["go-contract-suite", "python-contract-suite"]}],
        "risk_floor": "L1",
        "rollback_or_compensation": {"description": "pure read-only prescan has no effects",
                                     "mode": "not_required_no_effects"},
        "rules": [
            {"enforcement_mode": "hard_gate", "rule_id": "caller-bytes-only",
             "statement": "consume only explicit caller-supplied canonical bytes"},
            {"enforcement_mode": "hard_gate", "rule_id": "local-lexical-scope-only",
             "statement": "compute only the bounded local lexical package closure"},
            {"enforcement_mode": "hard_gate", "rule_id": "no-authority-or-effect",
             "statement": "do not grant authority, permission, transition, execution, or effect"},
            {"enforcement_mode": "hard_gate", "rule_id": "system-impact-unknown",
             "statement": "retain UNKNOWN system impact for every successful local prescan"}],
        "trigger": {"mode": "all", "predicates": [
            {"document": "input", "json_pointer": "/api_version", "operator": "equals",
             "value": "forgeos.governance.local-go-package-impact-prescan-request/v1"},
            {"document": "input", "json_pointer": "/canonicalization", "operator": "equals",
             "value": CANONICALIZATION}]},
    })


def _implementations(go_set: str, python_set: str) -> list[dict[str, object]]:
    return [
        {"adapters": [
            {"adapter_id": "go-command-line", "adapter_kind": "command_line",
             "entrypoint": "goimpactprescan.Command"},
            {"adapter_id": "go-library-api", "adapter_kind": "library_api",
             "entrypoint": "goimpactprescan.Build"}],
         "implementation_id": "go", "language": "go",
         "runtime_profile": "caller-bytes-only-local-pure", "source_set_sha256": go_set},
        {"adapters": [
            {"adapter_id": "python-command-line", "adapter_kind": "command_line",
             "entrypoint": "harness/local_go_package_impact_prescan_contract_check.py"},
            {"adapter_id": "python-library-api", "adapter_kind": "library_api",
             "entrypoint": "local_go_package_impact_prescan_contract.derive.derive_envelope"}],
         "implementation_id": "python", "language": "python",
         "runtime_profile": "caller-bytes-only-local-pure", "source_set_sha256": python_set},
    ]


def _tests(root: Path, go_set: str, python_test_set: str,
           guard: Snapshots) -> list[dict[str, object]]:
    golden = _ref(root, "docs/contracts/fixtures/local-go-package-impact-prescan-v1.json",
                  "application/json", None, guard)
    gates = ["cross-language-exact-golden", "malformed-tamper-and-bounds"]
    kinds = ["adversarial", "bounds", "cross_language_golden", "unit"]
    return [
        {"covers_gate_ids": gates, "entrypoint": "go test ./internal/goimpactprescan",
         "fixture_refs": [golden], "source_set_sha256": go_set,
         "test_id": "go-contract-suite", "test_kinds": kinds},
        {"covers_gate_ids": gates,
         "entrypoint": "python3 -B harness/test_local_go_package_impact_prescan_contract_check.py",
         "fixture_refs": [golden], "source_set_sha256": python_test_set,
         "test_id": "python-contract-suite", "test_kinds": kinds},
    ]


def build_registry(repo_root: Path, root_guard: Snapshots | None = None) -> dict[str, object]:
    root, root_identity = stable_root(repo_root)
    guard = guard_root(root, root_identity) if root_guard is None else root_guard
    assert_snapshots(guard, "repository root")
    go_set = _recursive_set(
        root, "forge-core/internal/goimpactprescan", ".go", "text/x-go", guard)
    python_set = _recursive_set(
        root, "harness/local_go_package_impact_prescan_contract", ".py", "text/x-python", guard)
    test_set = _explicit_set(root, guard)
    sets = [go_set, python_set, test_set]
    sets.sort(key=canonical_json)
    entry = seal("entry", {
        "api_version": ENTRY_API, "canonicalization": CANONICALIZATION,
        "catalog_binding": None, "content_sets": sets, "contract": _contract(root, guard),
        "entry_id": "", "entry_sha256": "", "implementations": _implementations(
            go_set["set_sha256"], python_set["set_sha256"]),
        "kind": "CapabilityRegistryEntry",
        "owner": {"module": "forge-core/internal/goimpactprescan", "team": "forgeos-core"},
        "tests": _tests(root, go_set["set_sha256"], test_set["set_sha256"], guard),
    })
    registry = seal("registry", {
        "api_version": REGISTRY_API, "canonicalization": CANONICALIZATION,
        "coverage_mode": "explicit_entries_only_not_global_inventory",
        "effect_vocabulary_sha256": EFFECT_VOCABULARY_SHA256, "entries": [entry],
        "kind": "CapabilityRegistry", "registry_id": "",
        "registry_mode": "authority_neutral_read_only_contract_catalog",
        "registry_sha256": "", "status": "staged",
    })
    assert_snapshots(guard, "repository root")
    return registry


def _request(registry: dict[str, object], expected_contract: object,
             capability_id: str, version: str, digest: str,
             origin: str) -> dict[str, object]:
    contract = registry["entries"][0]["contract"]
    return seal("request", {
        "api_version": REQUEST_API, "canonicalization": CANONICALIZATION,
        "expected_contract": expected_contract,
        "expected_reference": {"capability_contract_sha256": digest,
                               "capability_id": capability_id,
                               "capability_version": version,
                               "origin": origin},
        "kind": "CapabilityRegistryDeclaredResolutionRequest",
        "registry_sha256": registry["registry_sha256"], "request_sha256": "",
    })


def build_fixture(repo_root: Path) -> dict[str, object]:
    registry = build_registry(repo_root)
    contract = registry["entries"][0]["contract"]
    requests = {
        "legacy_repository_reader_not_registered": _request(
            registry, None, "repository-reader", "1", LEGACY_OPAQUE_SHA256,
            "external_legacy"),
        "registered_key_digest_mismatch": _request(
            registry, None, contract["capability_id"], contract["capability_version"],
            "0" * 64, "external_declared"),
        "resolved_exact": _request(
            registry, contract, contract["capability_id"], contract["capability_version"],
            contract["capability_contract_sha256"], "current_registry"),
    }
    assessments = {key: resolve_declared(registry, request)
                   for key, request in requests.items()}
    return {
        "api_version": FIXTURE_API, "assessments": assessments,
        "fixture_semantics": "exact_cross_language_declared_resolution_without_authority",
        "registry": registry, "requests": requests,
    }
