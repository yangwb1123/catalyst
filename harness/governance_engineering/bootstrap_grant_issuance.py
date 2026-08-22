"""ADR-0057 narrow authenticated bootstrap Grant issuance governance."""

from __future__ import annotations

import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/bootstrap-grant-issuance-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/bootstrap-grant-issuance-v1.json"
CHECKER_RELATIVE = "harness/bootstrap_grant_issuance_contract/check.py"
SKILL_RELATIVE = ".agent/skills/policy-authority.md"
DECISION_RELATIVE = (
    "docs/adr/0057-authenticated-bootstrap-repo-read-grant-issuance.md"
)
RUNTIME_PROFILE = "authenticated_bootstrap_repo_read_grant_issuance_v1"
EXECUTION_PROFILE = "authenticated_bootstrap_repo_read_execution_v1"
CONTRACT_PROFILE = "bootstrap_planning_repo_read_only_v1"

BOOTSTRAP_GRANT_ISSUANCE = {
    "runtime_profile_id": RUNTIME_PROFILE,
    "contract_profile_id": CONTRACT_PROFILE,
    "delivery": "catalyst_go_kernel_only",
    "status": "shipped_narrow_runtime_profile",
    "api_versions": {
        "signature_profile": "forgeos.ed25519-domain-sha256-profile/v1",
        "trust_root": "forgeos.governance-trust-root/v1",
        "policy": "forgeos.bootstrap-grant-policy/v1",
        "request": "forgeos.bootstrap-grant-request/v1",
        "grant": "forgeos.capability-grant/v1",
        "receipt": "forgeos.grant-issuance-receipt/v1",
        "ledger": "forgeos.grant-issuance-ledger/v1",
        "result": "forgeos.bootstrap-grant-issuance-result/v1",
    },
    "authenticated_authority": {
        "issuer": "non_agent_forgeos_kernel",
        "deployment_owner": "external_operator",
        "trust_root": "externally_pinned_outside_repository",
        "trust_root_signature": (
            "unsigned_bootstrap_material_exact_digest_operator_pinned"
        ),
        "signed_inputs": ["policy", "request"],
        "signed_outputs": ["grant_when_issued", "ledger", "receipt"],
        "signature_algorithm": "ed25519_over_domain_separated_sha256",
        "signature_profile_id": "forgeos.ed25519-domain-sha256/v1",
        "key_usages": ["grant_issue", "policy_sign", "request_auth"],
        "key_principals_and_public_keys_pairwise_distinct": True,
        "grant_issue_and_policy_sign_principal_type": "service",
        "request_auth_principal_type": "agent",
        "agent_self_issuance": "forbidden",
    },
    "scope": {
        "issuance_phase": "bootstrap_planning",
        "capabilities": ["repository-reader/v1"],
        "effects": ["repo.read"],
        "environments": ["development", "local", "test"],
        "resources": "canonical_exact_repo_paths_only",
        "budgets": "policy_bounded_small_ceiling",
        "ttl": "policy_bounded_short_window",
        "transferable": False,
        "decisions": ["denied", "issued"],
        "authenticated_policy_denial": (
            "signed_receipt_with_null_grant_and_policy_denied"
        ),
        "malformed_or_untrusted": "error_without_receipt",
    },
    "persistence": {
        "artifact": "signed_complete_issuance_ledger_snapshot",
        "receipt": "durable_signed_grant_issuance_receipt_bound_into_ledger",
        "ledger_results": ["exact_replay", "stored"],
        "exact_replay_returns_original_receipt": True,
        "clock_high_water_scope": "relative_to_current_signed_snapshot_only",
        "external_monotonic_anchor": "unavailable",
        "tpm_or_remote_witness": "unavailable",
        "replaceable_local_authority_state_rollback_resistance": "unavailable",
        "receipt_attests_effect_execution": False,
    },
    "runtime_boundary": {
        "binary": "forge-kernel",
        "binary_separate_from_agent_host_cli": True,
        "supported_host_family": "unix",
        "non_unix_behavior": "fail_closed_before_authority_input_or_key",
        "root_key_and_state_outside_repository": "required",
        "authority_root_path": "absolute_canonical_all_ancestors_no_symlink",
        "repository_authority_overlap": "forbidden_both_directions_by_resolved_ancestor_file_identity",
        "repository_source_binding": "stable_absolute_source_resolution_and_opened_directory_identity_for_session",
        "directory_mode": "0700",
        "local_file_mode": "0600",
        "effective_uid_check": "required_on_unix",
        "state_and_leaf_paths": "closed_canonical_relative",
        "leaf_identity": "regular_single_link_no_special_mode_bits",
        "file_mode_or_uid_is_os_principal_or_hsm_isolation": False,
        "same_uid_test_attests_production_trust_boundary": False,
        "production_tcb_requires": (
            "distinct_os_principal_or_external_secret_hsm_service"
        ),
        "key_generation_or_provisioning": "forbidden",
        "known_public_fixture_authority": (
            "forbidden_exact_root_and_any_fixture_public_key"
        ),
    },
    "limits": {
        "max_signature_profile_bytes": 16_384,
        "max_trust_root_bytes": 262_144,
        "max_policy_bytes": 524_288,
        "max_request_bytes": 1_048_576,
        "max_grant_bytes": 1_048_576,
        "max_receipt_bytes": 262_144,
        "max_result_bytes": 2_097_152,
        "max_ledger_bytes": 16_777_216,
        "max_golden_bytes": 20_971_520,
        "max_json_depth": 16,
        "max_object_fields": 64,
        "max_array_items": 256,
        "max_string_bytes": 16_384,
        "max_source_revision_bytes": 160,
        "max_grant_ttl_ms": 3_600_000,
        "max_policy_validity_ms": 86_400_000,
        "max_request_validity_ms": 300_000,
        "max_timeout_ms": 300_000,
        "max_output_bytes": 1_048_576,
        "max_ledger_entries": 256,
        "integer_domain": "signed_int64",
    },
    "unavailable_runtime": {
        "plan_finalization": "unavailable",
        "other_twenty_effects": "unavailable",
        "other_capabilities": "unavailable",
        "staging_environment": "unavailable",
        "production_environment": "unavailable",
        "approval": "unavailable",
        "revocation": "unavailable",
        "usage": "unavailable",
        "reservation": "unavailable",
        "preflight": "unavailable",
        "postflight": "unavailable",
        "policy_enforcement_point": "unavailable",
        "effect_execution": "unavailable",
        "context_package_integration": "unavailable",
        "provider_integration": "unavailable",
        "transition_integration": "unavailable",
        "knowledge_integration": "unavailable",
        "key_provisioning": "unavailable",
        "key_rotation": "unavailable",
        "remote_operation": "unavailable",
        "high_availability": "unavailable",
        "multitenancy": "unavailable",
        "complete_governance_kernel_pdp": "planned",
    },
    "scaffold": {
        "inherits": [
            "contract", "fixture", "shadow_structural_checker", "governance_skill",
        ],
        "installs_forge_kernel": False,
        "installs_trust_root_keys_or_state": False,
        "unavailable_runtime_result": "not_executed",
    },
    "structural_checker": {
        "detector": "shadow_structural_non_load_bearing",
        "validates": "canonical_shape_digest_and_declared_relations",
        "authenticates_ed25519": False,
        "authenticates_external_root_pin": False,
        "attests_persistence_or_effect": False,
    },
    "completion_authority_is_issuer": False,
}

CANONICAL_REFS = {
    "bootstrap_grant_issuance_schema": SCHEMA_RELATIVE,
    "bootstrap_grant_issuance_golden_fixture": FIXTURE_RELATIVE,
    "bootstrap_grant_issuance_checker": CHECKER_RELATIVE,
    "bootstrap_grant_issuance_skill": SKILL_RELATIVE,
    "bootstrap_grant_issuance_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "bootstrap_grant_issuance_python_checker": {
        "ref": CHECKER_RELATIVE,
        "projection": "universal_scaffold_structural_only",
    },
    "bootstrap_grant_issuance_go": {
        "ref": "forge-core/internal/bootstrapgrantauthority",
        "projection": "catalyst_repository_only",
    },
    "bootstrap_grant_issuance_runtime_go": {
        "ref": "forge-core/internal/bootstrapgrantissuance",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
    "bootstrap_grant_issuance_kernel_cli": {
        "ref": "forge-core/cmd/forge-kernel",
        "projection": "catalyst_repository_only_not_scaffolded",
    },
    "bootstrap_grant_issuance_state_go": {
        "ref": "forge-core/internal/grantstate",
        "projection": "catalyst_repository_only_wire_neutral_not_scaffolded",
    },
}

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "--golden", "repo_root"],
    "positive": "test_current_profile_is_narrow_and_structural_detector_is_shadow",
    "negative": "test_runtime_profile_scope_escalation_is_rejected",
}

SKILL_MARKERS = [
    RUNTIME_PROFILE,
    CONTRACT_PROFILE,
    "forgeos.ed25519-domain-sha256-profile/v1",
    "capability_id=repository-reader",
    "capability_version=1",
    "repo.read",
    "stored|exact_replay",
    "0600",
    "effective-UID",
    "OS principal",
    "HSM",
    "external monotonic anchor",
    "TPM",
    "remote witness",
    "旧但签名合法的 ledger",
    "not_executed",
    "forge accept",
    "不是 issuer",
    "plan_finalization",
    "known public fixture",
    "Unix-only",
    "Scaffold/upgrade",
]

FIXTURE_FIELDS = {
    "signature_profile", "trust_root", "policy", "request", "grant", "receipt",
    "result", "ledger",
}


def _pairs(pairs):
    value = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key {key!r}")
        value[key] = child
    return value


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("bootstrap_grant_issuance") != BOOTSTRAP_GRANT_ISSUANCE:
        issues.append(f"{path}: bootstrap Grant issuance profile drifted")
    scope = _mapping(data.get("scope"))
    if scope.get("shipped_runtime_profiles") != [RUNTIME_PROFILE, EXECUTION_PROFILE]:
        issues.append(f"{path}: ADR-0057/0058 shipped runtime profiles drifted")
    if scope.get("candidate_runtime_profiles") != []:
        issues.append(f"{path}: accepted ADR-0058 cannot remain a candidate profile")
    if scope.get("shipped_contract_only_kinds") != [
            "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
            "TransitionReceipt"]:
        issues.append(
            f"{path}: ApprovalRecord, CapabilityGrant, KnowledgeUpdateProposal, and TransitionReceipt must remain contract-only")
    if scope.get("authority_attestation") != "available_only_in_shipped_runtime_profiles":
        issues.append(f"{path}: authenticated authority must remain profile-scoped")
    for field, expected in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def _load_json(path, relative):
    raw = read_bounded_file(path, label=relative, max_bytes=2_097_152)
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)


def _schema_profile_issues(schema, path):
    definitions = _mapping(schema.get("$defs"))
    capability = _mapping(_mapping(definitions.get("capability")).get("properties"))
    task = _mapping(_mapping(definitions.get("task_binding")).get("properties"))
    scope = _mapping(_mapping(definitions.get("scope")).get("properties"))
    result = _mapping(_mapping(definitions.get("result")).get("properties"))
    request_bindings = _mapping(
        _mapping(definitions.get("request_bindings")).get("properties")
    )
    expected = {
        "profile": (_mapping(definitions.get("profile_id")).get("const"), CONTRACT_PROFILE),
        "capability_id": (_mapping(capability.get("capability_id")).get("const"),
                          "repository-reader"),
        "capability_version": (
            _mapping(capability.get("capability_version")).get("const"), "1"),
        "effect": (_mapping(scope.get("effect_id")).get("const"), "repo.read"),
        "environments": (_mapping(task.get("environment_class")).get("enum"),
                         ["development", "local", "test"]),
        "results": (_mapping(result.get("delivery_disposition")).get("enum"),
                    ["exact_replay", "stored"]),
        "source_revision": (
            _mapping(request_bindings.get("source_revision")).get("$ref"),
            "#/$defs/short_text",
        ),
    }
    return [f"{path}: bootstrap issuance {field} drifted"
            for field, values in expected.items() if values[0] != values[1]]


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        schema = _load_json(path, SCHEMA_RELATIVE)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate bootstrap issuance Schema: {error}"]
    issues = []
    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        issues.append(f"{path}: bootstrap issuance Schema draft drifted")
    expected_id = "https://forgeos.dev/contracts/bootstrap-grant-issuance-v1.schema.json"
    if schema.get("$id") != expected_id:
        issues.append(f"{path}: bootstrap issuance Schema identity drifted")
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        issues.append(f"{path}: bootstrap issuance envelope must be closed")
    if set(schema.get("required", [])) != FIXTURE_FIELDS:
        issues.append(f"{path}: bootstrap issuance root fields drifted")
    semantics = _mapping(schema.get("x-forgeos-authority-semantics"))
    expected_semantics = {
        "runtime_profile": RUNTIME_PROFILE,
        "contract_profile_id": CONTRACT_PROFILE,
        "root_source": "operator_pinned_external_to_request",
        "ed25519_verifier": "forge-kernel_go_runtime_not_universal_python_checker",
        "same_euid_isolation": False,
        "production_effects": False,
    }
    for field, expected in expected_semantics.items():
        if semantics.get(field) != expected:
            issues.append(f"{path}: authority semantics {field} drifted")
    limits = _mapping(schema.get("x-forgeos-limits"))
    if limits.get("max_result_bytes") != 2_097_152:
        issues.append(f"{path}: result byte ceiling drifted")
    if limits.get("max_source_revision_bytes") != 160:
        issues.append(f"{path}: source revision byte ceiling drifted")
    issues.extend(_schema_profile_issues(schema, path))
    return issues


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        fixture = _load_json(path, FIXTURE_RELATIVE)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate bootstrap issuance fixture: {error}"]
    if not isinstance(fixture, dict) or set(fixture) != FIXTURE_FIELDS:
        return [f"{path}: bootstrap issuance fixture root fields drifted"]
    encoded = json.dumps(fixture, sort_keys=True, separators=(",", ":"))
    required_values = [CONTRACT_PROFILE, "repository-reader", "repo.read"]
    issues = [f"{path}: fixture is missing frozen value {value!r}"
              for value in required_values if value not in encoded]
    result = _mapping(fixture.get("result"))
    if result.get("delivery_disposition") not in {"stored", "exact_replay"}:
        issues.append(f"{path}: fixture result must be stored or exact_replay")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.bootstrap_grant_issuance_contract"
    )
    if not isinstance(detector, dict):
        return ["bootstrap Grant issuance structural detector is missing"]
    issues = []
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("bootstrap issuance detector must remain shadow and fail closed")
    invocation = detector.get("invocation")
    expected = {
        "owner": "operator",
        "adapter": "standalone.bootstrapGrantIssuanceContract",
        "acceptance_criterion": None,
        "load_bearing": False,
    }
    if invocation != expected:
        issues.append("bootstrap issuance detector cannot become authority or load-bearing")
    implementation = detector.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != DETECTOR["argv"]:
        issues.append("bootstrap issuance detector must validate only the structural golden")
    tests = detector.get("tests")
    for polarity in ("positive", "negative"):
        test = tests.get(polarity) if isinstance(tests, dict) else None
        if not isinstance(test, dict) or test.get("contains") != DETECTOR[polarity]:
            issues.append(f"bootstrap issuance detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate Policy/Authority Skill: {error}"]
    return [f"{path}: missing ADR-0057 boundary marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def integration_issues(data, path, repo_root, agent_root):
    from bootstrap_grant_issuance_contract.check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
