"""ADR-0068 authority-neutral Capability Registry governance integration."""

from __future__ import annotations

import json

from capability_registry_contract import validate_golden_fixture
from governance_contract import ContractError, read_bounded_file
from engineering_check_support import load_yaml


SCHEMA_RELATIVE = "docs/contracts/capability-registry-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/capability-registry-v1.json"
CHECKER_RELATIVE = "harness/capability_registry_contract/check.py"
SKILL_RELATIVE = ".agent/skills/capability-registry.md"
DECISION_RELATIVE = "docs/adr/ADR-0068-authority-neutral-capability-registry-v1.md"
SCAFFOLD_RELATIVE = "harness/scaffold/capability-registry-copy-fragment.mjs"
REGISTRY_SHA256 = "23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4"
SCHEMA_SHA256 = "f5c5c5abc68e9c5f5d80dce66bb5b97e4e4dedc8cc69189bcc28612991f1ea81"
FIXTURE_SHA256 = "0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5"
RESULT = (
    "RESOLVED_DECLARED_CAPABILITY_REFERENCE_ONLY (no registry or owner "
    "authentication, rule or gate applicability, proof satisfaction, test "
    "pass, implementation availability, Grant activation, authorization, "
    "permission, invocation, runtime routing, persistence, transition, "
    "execution, or effect attestation)"
)

CAPABILITY_REGISTRY = {
    "api_version": "forgeos.capability-registry/v1",
    "delivery": "shipped_go_python_pure_validator_resolver_physical_checker_and_cli",
    "wire_status": "staged_profile_no_lifecycle_authority",
    "mode": "authority_neutral_read_only_contract_catalog",
    "coverage": {
        "coverage_mode": "explicit_entries_only_not_global_inventory",
        "entry_count": 1,
        "entry_key": "local-go-package-impact-prescan/1",
        "catalog_binding": None,
        "planning_catalog_input": "none",
        "planning_catalog_projection": False,
        "catalog_to_package_adapter_generation": False,
    },
    "identity": {
        "registry_sha256": REGISTRY_SHA256,
        "canonicalization": "forgeos.canonical-json/v1",
        "digest_chain": [
            "content_set", "contract", "entry", "registry", "request", "assessment",
        ],
        "exact_singleton_registry_bytes_required": True,
        "semver_range_alias_latest_or_fallback": False,
    },
    "resolution": {
        "input": "explicit_canonical_registry_and_request_bytes_only",
        "order": [
            "capability_id", "capability_version",
            "capability_contract_sha256", "optional_exact_contract_bytes",
        ],
        "outcomes": [
            "capability_contract_digest_mismatch", "capability_id_not_found",
            "capability_version_not_found", "resolved_exact",
        ],
        "expected_contract": "nullable_exact_reference_projection",
        "legacy_repository_reader": "ordinary_unregistered_id_not_found",
        "implementation_selection": False,
        "rule_gate_proof_or_permission_evaluation": False,
    },
    "physical_validation": {
        "explicit_declared_refs_only": True,
        "recursive_set_completeness": True,
        "regular_files_only": True,
        "symlinks_and_special_files": "reject",
        "pre_and_post_identity_stability": True,
        "max_walk_entries": 65536,
        "pure_resolver_dereferences_content": False,
    },
    "local_execution": {
        "repository_access_by_resolver": False,
        "catalog_access": False,
        "environment_access": False,
        "clock_access": False,
        "credential_access": False,
        "process_access": False,
        "provider_access": False,
        "network_access": False,
        "database_access": False,
        "implementation_or_test_execution": False,
        "persistence": "none",
    },
    "authority_semantics": {
        "registry_or_owner_authentication": "not_evaluated",
        "rule_gate_or_proof_applicability": "not_evaluated",
        "authorization_decision": "none",
        "capability_grant_activation": False,
        "capability_invocation": False,
        "permission_attestation": False,
        "implementation_availability_attestation": False,
        "test_pass_attestation": False,
        "runtime_routing_attestation": False,
        "transition_attestation": False,
        "effect_attestation": False,
        "plugin_loading_or_distribution": False,
        "positive_result": RESULT,
        "attestations": [],
    },
    "semantic_validation": {
        "schema_alone_sufficient": False,
        "strict_compact_canonical_json": True,
        "domain_separated_identity_chain_recomputed": True,
        "exact_singleton_semantics_reconstructed": True,
        "exact_registry_byte_pin_required": True,
        "complete_assessment_reassembly_required": True,
        "duplicate_unknown_unicode_depth_cardinality_and_byte_drift_fail_closed": True,
        "cross_language_golden_required": True,
    },
    "fixture": {
        "physical_bytes": 28758,
        "physical_sha256": FIXTURE_SHA256,
        "framing": "compact_canonical_json_then_exactly_one_lf",
        "cases": [
            "legacy_repository_reader_not_registered",
            "registered_key_digest_mismatch", "resolved_exact",
        ],
    },
    "limits": {
        "max_registry_bytes": 16777216,
        "max_entry_bytes": 1048576,
        "max_contract_bytes": 524288,
        "max_content_set_bytes": 4194304,
        "max_request_bytes": 262144,
        "max_assessment_bytes": 262144,
        "max_golden_bytes": 33554432,
        "max_json_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 256,
        "max_string_utf8_bytes": 16384,
        "max_identifier_utf8_bytes": 160,
        "max_repository_path_utf8_bytes": 4096,
        "max_content_files": 256,
        "max_physical_walk_entries": 65536,
        "max_entries": 1,
        "integer_domain": "signed_int64",
    },
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
}

CANONICAL_REFS = {
    "capability_registry_schema": SCHEMA_RELATIVE,
    "capability_registry_golden_fixture": FIXTURE_RELATIVE,
    "capability_registry_checker": CHECKER_RELATIVE,
    "capability_registry_skill": SKILL_RELATIVE,
    "capability_registry_decision": DECISION_RELATIVE,
}
ACTIVATION_REFS = {
    "capability_registry_schema": SCHEMA_RELATIVE,
    "capability_registry_fixture": FIXTURE_RELATIVE,
    "capability_registry_checker": CHECKER_RELATIVE,
    "capability_registry_skill": SKILL_RELATIVE,
    "capability_registry_decision": DECISION_RELATIVE,
}
REFERENCE_IMPLEMENTATIONS = {
    "capability_registry_go": {
        "ref": "forge-core/internal/capabilityregistry",
        "projection": "catalyst_repository_only_pure_validator_resolver_and_cli",
    },
    "capability_registry_python": {
        "ref": "harness/capability_registry_contract",
        "projection": "universal_scaffold_pure_validator_resolver_and_physical_checker",
    },
}
NON_CAPABILITY = (
    "Capability Registry v1 validates and resolves exactly one physically bound "
    "local-go-package-impact-prescan/1 declaration; it does not import or project "
    "the planning-only 140-item catalog, generate catalog-to-package adapters, "
    "authenticate the Registry or owner, activate Grant/PDP, construct a "
    "CapabilityInvocation, evaluate/select/execute an implementation, load a "
    "plugin, route runtime work, persist state, authorize permission, advance a "
    "transition, dispatch an effect, or attest availability test pass or authority"
)
DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "--golden", "repo_root"],
    "positive": "test_registry_classifies_exact_resolver_without_authority",
    "negative": "test_scope_authority_and_catalog_drift_fail_closed",
}
SKILL_MARKERS = [
    "ADR-0068", "forgeos.capability-registry/v1", REGISTRY_SHA256,
    "forge capability-registry validate", "forge capability-registry resolve",
    "local-go-package-impact-prescan/1", "repository-reader/1", "planning_only",
    "catalog_binding:null", "CapabilityInvocation", "Grant/PDP", "runtime routing",
    "forge accept",
]
DOC_MARKERS = {
    ".agent/AGENTS.md": "ADR-0068",
    ".agent/ARCHITECTURE.md": "Capability Registry v1",
    ".agent/ROADMAP.md": "Authority-neutral Capability Registry v1",
    ".agent/CURRENT_SPRINT.md": "Capability Registry v1",
    ".agent/DECISIONS.md": "Authority-neutral Capability Registry v1",
    ".agent/engineering/README.md": "ADR-0068",
    "docs/design/ai-engineering-os/README.md": "ADR-0068",
    "docs/design/ai-engineering-os/governance-contracts.md": "ADR-0068",
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": "Authority-neutral Capability Registry v1",
}
SCAFFOLD_MARKERS = {
    SCAFFOLD_RELATIVE: [
        "ADR-0068-authority-neutral-capability-registry-v1.md",
        "capability-registry-v1.schema.json", "capability-registry-v1.json",
        "capability_registry_contract", "test_capability_registry_contract.py",
    ],
    "harness/scaffold/capability-registry-upgrade-verification.mjs": [
        FIXTURE_SHA256, "CAPABILITY_REGISTRY_EXPECTED_FILES",
        "assertCapabilityRegistryScaffold", "existsSync(join(target, 'forge-core'))",
    ],
    "harness/scaffold/copy-manifest.mjs": [
        "CAPABILITY_REGISTRY_COPIED_FILES",
        "...CAPABILITY_REGISTRY_COPIED_FILES",
    ],
    "harness/scaffold/forge-init-test-assets.mjs": [
        "CAPABILITY_REGISTRY_EXPECTED_FILES",
        "...CAPABILITY_REGISTRY_EXPECTED_FILES",
    ],
    "harness/scaffold/test_forge-init.mjs": ["assertCapabilityRegistryScaffold"],
    "harness/scaffold/engineering-upgrade-fixture.mjs": [
        "assertCapabilityRegistryScaffold", "CAPABILITY_REGISTRY_LEGACY_FILES",
    ],
}


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("capability_registry") != CAPABILITY_REGISTRY:
        issues.append(f"{path}: Capability Registry evaluator contract drifted")
    scope = _mapping(data.get("scope"))
    evaluators = scope.get("shipped_evaluators") or []
    if evaluators != [
            "local_go_package_impact_prescan", "graph_snapshot",
            "graph_snapshot_test_source", "architecture_decision_record_v2",
            "capability_registry", "planning_capability_ownership",
            "project_source_snapshot"]:
        issues.append(f"{path}: shipped pure evaluator scope drifted")
    forbidden = sum((scope.get(name) or [] for name in (
        "shipped_kinds", "shipped_contract_only_kinds", "shipped_producers",
        "shipped_projectors", "shipped_runtime_profiles")), [])
    if "capability_registry" in forbidden or "CapabilityRegistry" in forbidden:
        issues.append(f"{path}: Capability Registry cannot be runtime authority")
    if NON_CAPABILITY not in (data.get("non_capabilities") or []):
        issues.append(f"{path}: Capability Registry non-capability boundary drifted")
    return issues + _registry_ref_issues(data, path)


def _registry_ref_issues(data, path):
    issues = []
    refs = _mapping(data.get("canonical_refs"))
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=1_048_576)
        schema = json.loads(raw.decode("utf-8"))
    except (OSError, ContractError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"{path}: cannot validate Capability Registry Schema: {error}"]
    issues = []
    if _mapping(schema.get("properties")).get("status") != {"const": "staged"}:
        issues.append(f"{path}: staged wire profile drifted")
    golden = _mapping(schema.get("x-forgeos-golden"))
    if golden.get("physical_bytes") != 28758 or golden.get("physical_sha256") != FIXTURE_SHA256:
        issues.append(f"{path}: physical golden metadata drifted")
    authority = _mapping(schema.get("x-forgeos-authority-semantics"))
    if authority.get("positive_result") != RESULT or authority.get("attestations") != []:
        issues.append(f"{path}: authority-neutral result drifted")
    for field in ("capability_grant_activation", "capability_invocation", "effect_dispatch",
                  "transition_or_controller", "runtime_routing"):
        if authority.get(field) is not False:
            issues.append(f"{path}: authority field {field} must remain false")
    if schema.get("x-forgeos-limits") != CAPABILITY_REGISTRY["limits"]:
        issues.append(f"{path}: Capability Registry resource limits drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.capability_registry_contract")
    if not isinstance(detector, dict):
        return ["Capability Registry shadow detector is missing"]
    implementation = _mapping(detector.get("implementation"))
    invocation = _mapping(detector.get("invocation"))
    tests = _mapping(detector.get("tests"))
    issues = []
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("Capability Registry detector requires exact golden arguments")
    if detector.get("state") != "shadow" or invocation.get("load_bearing") is not False:
        issues.append("Capability Registry detector must remain shadow and non-load-bearing")
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"Capability Registry detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate Capability Registry Skill: {error}"]
    return [f"{path}: missing Capability Registry marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def wiring_issues(repo_root, agent_root):
    issues = []
    activation, error = load_yaml(agent_root / "engineering/activation.yml")
    extension = _mapping(activation.get("canonical_extension_refs")) if not error else {}
    for field, expected in ACTIVATION_REFS.items():
        if extension.get(field) != expected:
            issues.append(f"activation canonical_extension_refs.{field} drifted")
    issues.extend(_route_and_discipline_issues(agent_root))
    issues.extend(_scaffold_issues(repo_root))
    return issues


def _route_and_discipline_issues(agent_root):
    routes, route_error = load_yaml(agent_root / "engineering/context-routes.yml")
    disciplines, discipline_error = load_yaml(agent_root / "engineering/disciplines.yml")
    if route_error or discipline_error:
        return ["Capability Registry route or discipline registry is unreadable"]
    expected = {SKILL_RELATIVE, SCHEMA_RELATIVE}
    route_refs = {}
    for route_id in ("governance", "architecture-boundary"):
        route = next((item for item in routes["routes"]
                      if item.get("id") == route_id), {})
        route_refs[route_id] = {item.get("ref")
                                for item in route.get("include") or []}
    contract = next((item for item in disciplines["disciplines"]
                     if item.get("id") == "contract"), {})
    issues = []
    for route_id, refs in route_refs.items():
        if not expected.issubset(refs):
            issues.append(f"Capability Registry {route_id} route is incomplete")
    if not expected.issubset(set(contract.get("assets") or [])):
        issues.append("Capability Registry contract discipline assets are incomplete")
    return issues


def _scaffold_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, markers in SCAFFOLD_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate Registry scaffold: {error}")
            continue
        issues.extend(f"{path}: missing scaffold marker {marker!r}"
                      for marker in markers if marker not in text)
    return issues


def documentation_issues(repo_root):
    if not (repo_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
        return []
    issues = []
    for relative, marker in DOC_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR-0068 promotion: {error}")
            continue
        if marker not in text:
            issues.append(f"{path}: missing ADR-0068 promotion marker {marker!r}")
    issues.extend(_roadmap_issues(repo_root))
    return issues


def _roadmap_issues(repo_root):
    path = repo_root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    try:
        text = read_bounded_file(path, label=str(path)).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate Registry roadmap state: {error}"]
    required = [
        "- [x] 实现最小 Capability Registry",
        "- [x] 建 capability registry schema",
        "- [x] 校验 catalog fine capability → package primary owner 全覆盖且唯一",
        "- [ ] 定义 CapabilityPluginManifest",
    ]
    return [f"{path}: Capability Registry roadmap boundary {marker!r} missing"
            for marker in required if marker not in text]


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(wiring_issues(repo_root, agent_root))
    issues.extend(documentation_issues(repo_root))
    return issues
