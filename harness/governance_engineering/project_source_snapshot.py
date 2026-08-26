"""ADR-0070 bounded Project Source Snapshot governance freeze."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

from architecture_decision_record_v2.document import validate_document_file
from governance_contract import ContractError, read_bounded_file
from project_source_snapshot_contract.constants import (
    FIXTURE_SHA256, POSITIVE_RESULT,
)
from project_source_snapshot_contract.fixture import load_golden


SCHEMA_RELATIVE = "docs/contracts/project-source-snapshot-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/project-source-snapshot-v1.json"
DECISION_RELATIVE = "docs/adr/ADR-0070-local-project-source-snapshot-v1.md"
ADAPTER_RELATIVE = ".agent/skills/project-snapshot.md"
PORTABLE_SKILL_RELATIVE = "skills/project-snapshot/SKILL.md"
PACKAGE_MANIFEST_RELATIVE = (
    "skills/project-snapshot/references/package-manifest.json"
)
SCHEMA_SHA256 = "7e281850579329356090dd19b9f66a8544f7af04c37ac01bca87bad1824a23b1"
PACKAGE_MANIFEST_SHA256 = "39eb2218a52f7dd78af35e7765a3b942587a9542b726d1ecee4f59381df7cf35"

CANONICAL_REFS = {
    "project_source_snapshot_schema": SCHEMA_RELATIVE,
    "project_source_snapshot_golden_fixture": FIXTURE_RELATIVE,
    "project_source_snapshot_checker": (
        "harness/project_source_snapshot_contract/check.py"
    ),
    "project_source_snapshot_skill": ADAPTER_RELATIVE,
    "project_source_snapshot_portable_skill": PORTABLE_SKILL_RELATIVE,
    "project_source_snapshot_package_manifest": PACKAGE_MANIFEST_RELATIVE,
    "project_source_snapshot_decision": DECISION_RELATIVE,
}
REFERENCE_IMPLEMENTATIONS = {
    "project_source_snapshot_go": {
        "ref": "forge-core/internal/projectsnapshot",
        "projection": (
            "catalyst_repository_only_strict_pure_decoder_and_linux_live_"
            "producer_not_scaffolded"
        ),
    },
    "project_source_snapshot_python": {
        "ref": "harness/project_source_snapshot_contract",
        "projection": "source_portable_strict_pure_decoder_and_golden_checker",
    },
    "project_source_snapshot_portable_skill": {
        "ref": "skills/project-snapshot",
        "projection": "source_distributed_closed_package_adapter_without_runtime",
    },
}

SURFACES = [
    {"surface": "atomicity", "status": "unknown"},
    {"surface": "configuration_semantics", "status": "not_performed"},
    {"surface": "content_secret_scan", "status": "not_performed"},
    {"surface": "currentness", "status": "unknown"},
    {"surface": "deployment_semantics", "status": "not_performed"},
    {"surface": "freshness", "status": "unknown"},
    {"surface": "git_control_metadata", "status": "not_observed"},
    {"surface": "graph_topology", "status": "not_performed"},
    {"surface": "ignored_paths", "status": "partial"},
    {"surface": "nested_repositories_and_submodules", "status": "not_observed"},
    {"surface": "nonignored_untracked", "status": "partial"},
    {"surface": "tracked_worktree", "status": "partial"},
]
LIMITS = {
    "max_canonical_envelope_bytes": 33554432,
    "max_final_sealed_manifest_bytes": 16777216,
    "max_individual_regular_file_bytes": 67108864,
    "max_git_executable_bytes": 67108864,
    "max_git_command_stdout_bytes": 33554432,
    "max_git_command_seconds": 30,
    "max_product_command_seconds": 120,
    "max_aggregate_regular_file_bytes": 1073741824,
    "max_universe_records": 16384,
    "max_excluded_records": 4096,
    "max_ignored_path_count": 262144,
    "max_path_utf8_bytes": 16384,
    "max_path_unicode_scalars": 4096,
    "max_path_components": 256,
    "max_json_depth": 16,
    "max_object_fields": 64,
    "integer_domain": "signed_int64",
}
PROJECT_SOURCE_SNAPSHOT = {
    "api_version": (
        "forgeos.governance.local-project-source-snapshot-production/v1"
    ),
    "request_api_version": (
        "forgeos.governance.local-project-source-snapshot-request/v1"
    ),
    "snapshot_api_version": "forgeos.project-source-snapshot/v1",
    "delivery": (
        "shipped_linux_live_producer_portable_strict_checker_and_closed_skill"
    ),
    "adr_status": "proposed",
    "mode": "explicit_opt_in_bounded_local_git_worktree_observation",
    "profile_id": "local-git-worktree-bounded-sensitive-path-exclusion-v1",
    "path_policy_id": "forgeos.project-source-sensitive-path-policy/v1",
    "capture": {
        "public_root_input": "clean_canonical_absolute_git_worktree_root",
        "universe": "tracked_stage_zero_plus_nonignored_untracked",
        "consistency": "bounded_interval_two_endpoint_exact_match",
        "atomic": False,
        "fixed_path_policy_only": True,
        "protected_leaf_classification_before_collector_leaf_access": True,
        "allowed_regular_files_require_single_link": True,
        "symlink_target_read_or_disclosure": False,
        "git_identity": "unauthenticated_local_path_binary",
        "git_network_containment": "not_provided",
    },
    "runtime_platform": {
        "live_producer_host": "linux_only",
        "capture_adapter_host": "linux_only",
        "unsupported_host_adapter_result": (
            "exit_3_not_executed_before_runtime_access"
        ),
        "python_decoder_portability": "source_portable_no_live_capture",
        "python_entrypoint_startup": (
            "isolated_required_before_non_builtin_import"
        ),
    },
    "coverage": {
        "surface_count": 12,
        "surfaces": SURFACES,
        "system_completeness": "unknown",
    },
    "limits": LIMITS,
    "portable_package": {
        "source_distributed": True,
        "closed_manifest_required": True,
        "copies_catalyst_go_runtime": False,
        "installs_host_skill": False,
        "grants_filesystem_or_process_permission": False,
        "absent_named_runtime_result": "exit_3_not_executed",
        "existing_incompatible_or_execution_failure_result": "exit_1_failure",
        "package_integrity_nofollow_unavailable_result": "exit_1_fail_closed",
        "vendored_contract_loading": (
            "adapter_anchored_exact_file_location_without_sys_path_mutation"
        ),
        "package_check_capture_consistency": "separate_non_atomic_operations",
        "package_check_authenticates_publisher": False,
        "output_target": "outside_captured_root",
        "fallback_reader": "forbidden",
    },
    "authority_semantics": {
        "source_or_git_authentication": False,
        "atomic_current_complete_or_secret_free_attestation": False,
        "graph_snapshot_attestation": False,
        "registry_mutation": False,
        "grant_pdp_or_capability_invocation": False,
        "permission_attestation": False,
        "truth_attestation": False,
        "persistence_attestation": False,
        "runtime_routing_attestation": False,
        "effect_attestation": False,
        "positive_result": POSITIVE_RESULT,
        "attestations": [],
    },
    "positive_result": POSITIVE_RESULT,
    "attests": [],
    "persistence": "none",
}
NON_CAPABILITY = (
    "Project Source Snapshot v1 is only a Linux-produced two-endpoint bounded "
    "local Git worktree observation with fixed pre-read path exclusions and "
    "partial/unknown coverage; it does not authenticate Git, HEAD, repository, "
    "caller or host, scan allowed content for secrets, provide atomic/current/"
    "complete project or graph state, mutate the ADR-0068 Registry, activate "
    "Grant/PDP/CapabilityInvocation, install a host Skill, grant permission, "
    "persist current state, route runtime work, execute effects or attest truth"
)

DETECTOR = {
    "argv": [
        "python3", "harness/project_source_snapshot_contract/check.py",
        "--golden", "repo_root",
    ],
    "positive": "test_registry_classifies_evaluator_and_linux_producer_without_authority",
    "negative": "test_authority_platform_and_scaffold_drift_fail_closed",
}
SKILL_MARKERS = {
    ADAPTER_RELATIVE: ["exit 3/`not_executed`", "固定 exit 1",
                       "python3 -I -B", "两个非原子操作",
                       "exact vendored package location",
                       "descriptor-relative no-follow primitives",
                       "/absolute/path/outside/worktree/",
                       "path policy 不是 content DLP",
                       "不复制 Catalyst `forge-core` runtime"],
    PORTABLE_SKILL_RELATIVE: [
        "python3 -I -B", "Package validation is an observation, not a lock",
        "exact package location anchored to the adapter",
        "exit `3`: unsupported host or named runtime absent",
        "status is `not_executed`", "existing but malformed",
        "exit `1` execution failure",
        "any nonzero `git ls-files --debug` flag", "intent-to-add",
        "descriptor-relative no-follow filesystem primitives",
        "output locator outside the captured worktree",
        "path-policy exclusions are not content DLP"],
}
DELIVERY_DOCUMENT_MARKERS = {
    ".agent/AGENTS.md": ["Project snapshot 保持 bounded observation"],
    ".agent/ARCHITECTURE.md": ["Local Project Source Snapshot v1 边界"],
    ".agent/ROADMAP.md": ["Wave 4–B1 `project-snapshot` narrow package slice"],
    ".agent/CURRENT_SPRINT.md": ["Sprint 117（✅ DONE；`project-snapshot`"],
    ".agent/DECISIONS.md": ["D42 Local Project Source Snapshot v1"],
    ".agent/engineering/README.md": ["ADR-0070 adds"],
    "docs/design/ai-engineering-os/README.md": [
        "ADR-0070 Local Project Source Snapshot v1"],
    "docs/design/ai-engineering-os/governance-contracts.md": [
        "ADR-0070 Local Project Source Snapshot v1"],
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": [
        "`project-snapshot` narrow package slice"],
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("project_source_snapshot") != PROJECT_SOURCE_SNAPSHOT:
        issues.append(f"{path}: Project Source Snapshot contract drifted")
    scope = _mapping(data.get("scope"))
    if scope.get("shipped_producers") != [
            "local_gate_command_observation_producer",
            "local_evolve_repo_locator_observation_producer",
            "local_go_package_dependency_graph_observation_producer",
            "local_project_source_snapshot_producer"]:
        issues.append(f"{path}: shipped producer scope drifted")
    if scope.get("shipped_evaluators") != [
            "local_go_package_impact_prescan", "graph_snapshot",
            "graph_snapshot_test_source", "architecture_decision_record_v2",
            "capability_registry", "planning_capability_ownership",
            "project_source_snapshot"]:
        issues.append(f"{path}: shipped evaluator scope drifted")
    forbidden = sum((scope.get(name) or [] for name in (
        "shipped_kinds", "shipped_contract_only_kinds", "shipped_projectors",
        "shipped_runtime_profiles", "planned_kinds")), [])
    if any(item in forbidden for item in (
            "project_source_snapshot", "local_project_source_snapshot_producer",
            "ProjectSourceSnapshot")):
        issues.append(f"{path}: Project Snapshot cannot be authority or projection kind")
    refs = _mapping(data.get("canonical_refs"))
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: Project Snapshot non-capability boundary drifted")
    return issues


def schema_and_fixture_issues(repo_root):
    issues = []
    schema_path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(schema_path, label=SCHEMA_RELATIVE, max_bytes=1048576)
        schema = json.loads(raw)
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{schema_path}: cannot validate Project Snapshot Schema: {error}"]
    if hashlib.sha256(raw).hexdigest() != SCHEMA_SHA256:
        issues.append(f"{schema_path}: Project Snapshot Schema pin drifted")
    semantic = _mapping(schema.get("x-forge-semantic-contract"))
    bounds = _mapping(schema.get("x-forge-resource-bounds"))
    delivery = str(semantic.get("delivery_boundary"))
    for marker in ("source-portable", "Linux-only", "exits 3",
                   "exit 1", "descriptor-relative no-follow"):
        if marker not in delivery:
            issues.append(
                f"{schema_path}: delivery boundary marker drifted: {marker}"
            )
    if "not content DLP" not in schema.get("description", "") and (
            "secret absence" not in schema.get("description", "")):
        issues.append(f"{schema_path}: content-secret nonclaim drifted")
    if bounds.get("canonical_envelope_bytes") != LIMITS[
            "max_canonical_envelope_bytes"]:
        issues.append(f"{schema_path}: envelope bound drifted")
    fixture_path = repo_root / FIXTURE_RELATIVE
    try:
        physical = read_bounded_file(fixture_path, label=FIXTURE_RELATIVE)
        load_golden(repo_root)
    except (OSError, ContractError, ValueError) as error:
        issues.append(f"{fixture_path}: Project Snapshot golden rejected: {error}")
    else:
        if hashlib.sha256(physical).hexdigest() != FIXTURE_SHA256:
            issues.append(f"{fixture_path}: Project Snapshot golden pin drifted")
    return issues


def package_issues(repo_root):
    manifest_path = repo_root / PACKAGE_MANIFEST_RELATIVE
    try:
        raw = read_bounded_file(manifest_path, label=PACKAGE_MANIFEST_RELATIVE)
    except (OSError, ContractError) as error:
        return [f"{manifest_path}: cannot validate portable package pin: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != PACKAGE_MANIFEST_SHA256:
        issues.append(f"{manifest_path}: portable package manifest pin drifted")
    checker_path = repo_root / "skills/project-snapshot/scripts/check_package.py"
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(checker_path),
             str(repo_root / "skills/project-snapshot")],
            cwd=repo_root, capture_output=True, check=False, timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        issues.append(f"{checker_path}: portable package rejected: {error}")
    else:
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", "replace").strip()
            issues.append(f"{checker_path}: portable package rejected: {detail}")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.project_source_snapshot")
    if not isinstance(detector, dict):
        return ["Project Source Snapshot shadow detector is missing"]
    issues = []
    if _mapping(detector.get("implementation")).get("argv") != DETECTOR["argv"]:
        issues.append("Project Snapshot detector argv drifted")
    invocation = _mapping(detector.get("invocation"))
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("Project Snapshot detector must remain shadow/non-load-bearing")
    tests = _mapping(detector.get("tests"))
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"Project Snapshot detector {polarity} test drifted")
    return issues


def wiring_issues(repo_root, agent_root):
    from engineering_check_support import load_yaml
    activation, error = load_yaml(agent_root / "engineering/activation.yml")
    extension = _mapping(activation.get("canonical_extension_refs")) if not error else {}
    expected = {
        "project_source_snapshot_schema": SCHEMA_RELATIVE,
        "project_source_snapshot_fixture": FIXTURE_RELATIVE,
        "project_source_snapshot_checker": CANONICAL_REFS[
            "project_source_snapshot_checker"],
        "project_source_snapshot_skill": ADAPTER_RELATIVE,
        "project_source_snapshot_portable_skill": PORTABLE_SKILL_RELATIVE,
        "project_source_snapshot_package_manifest": PACKAGE_MANIFEST_RELATIVE,
        "project_source_snapshot_decision": DECISION_RELATIVE,
    }
    issues = [f"activation canonical_extension_refs.{field} drifted"
              for field, value in expected.items() if extension.get(field) != value]
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if route_error or discipline_error:
        return issues + ["Project Snapshot route/discipline registry unreadable"]
    required = {ADAPTER_RELATIVE, SCHEMA_RELATIVE}
    for route_id in ("governance", "architecture-boundary"):
        route = next((item for item in routes["routes"]
                      if item.get("id") == route_id), {})
        refs = {item.get("ref") for item in route.get("include") or []}
        if not required.issubset(refs):
            issues.append(f"Project Snapshot {route_id} route is incomplete")
    contract = next((item for item in disciplines["disciplines"]
                     if item.get("id") == "contract"), {})
    if not required.issubset(set(contract.get("assets") or [])):
        issues.append("Project Snapshot contract discipline assets incomplete")
    return issues


def skill_and_documentation_issues(repo_root):
    issues = []
    markers = dict(SKILL_MARKERS)
    source_delivery = (
        repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file()
    if source_delivery:
        markers.update(DELIVERY_DOCUMENT_MARKERS)
    for relative, expected in markers.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate Project Snapshot marker: {error}")
            continue
        issues.extend(f"{path}: missing Project Snapshot marker {marker!r}"
                      for marker in expected if marker not in text)
    if not source_delivery:
        return issues
    roadmap = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(roadmap, label=str(roadmap)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return issues + [f"{roadmap}: cannot validate Project Snapshot roadmap: {error}"]
    if "- [ ] 按 `implementation_wave` 逐 package 实现 Skill" not in text:
        issues.append(f"{roadmap}: 38-package parent must remain unchecked")
    if "  - [x] `project-snapshot` 窄切片" not in text:
        issues.append(f"{roadmap}: project-snapshot nested item must be checked")
    return issues


def adr_issues(repo_root):
    path = repo_root / DECISION_RELATIVE
    try:
        metadata = validate_document_file(path)
        text = read_bounded_file(path, label=DECISION_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: ADR-0070 v2 validation failed: {error}"]
    issues = []
    if metadata.get("status") != "proposed":
        issues.append(f"{path}: ADR-0070 must remain proposed")
    normalized = " ".join(text.split())
    for marker in ("Linux-only", "content DLP", "not_executed",
                   "parent 38-package item", "No source or coverage object",
                   "same supplied facts", "distinct valid live observation",
                   "representative generic and semantic resource N/N+1",
                   "output target must be chosen outside",
                   "descriptor-relative no-follow primitives",
                   "nonzero index debug", "intent-to-add"):
        if marker not in normalized:
            issues.append(f"{path}: missing delivered-boundary marker {marker!r}")
    if "reject all resealed mutations" in normalized:
        issues.append(f"{path}: ADR cannot reject a distinct valid live observation")
    if "ls-files -v" in normalized:
        issues.append(f"{path}: stale ls-files -v index-flag claim returned")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_and_fixture_issues(repo_root))
    issues.extend(package_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(wiring_issues(repo_root, agent_root))
    issues.extend(skill_and_documentation_issues(repo_root))
    issues.extend(adr_issues(repo_root))
    return issues
