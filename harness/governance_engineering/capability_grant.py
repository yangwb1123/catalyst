"""ADR-0056 CapabilityGrant contract-only governance integration."""

from __future__ import annotations

import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/capability-grant-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/capability-grant-v1.json"
CHECKER_RELATIVE = "harness/capability_grant_contract_check.py"
SKILL_RELATIVE = ".agent/skills/policy-authority.md"
DECISION_RELATIVE = "docs/adr/0056-capability-grant-v1-contract-only.md"
RESULT = (
    "ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, "
    "approval, revocation, usage, preflight, authorization, permission, "
    "persistence, execution, or effect attestation)"
)

CAPABILITY_GRANT = {
    "api_version": "forgeos.capability-grant/v1",
    "request_api_version": "forgeos.capability-grant-declared-assessment-request/v1",
    "assessment_api_version": "forgeos.capability-grant-declared-assessment/v1",
    "delivery": "strict_pure_contract_only",
    "mode": "authority_neutral_declared_envelope_only",
    "input": "exact_canonical_caller_supplied_grant_request_and_assessment",
    "canonicalization": "forgeos.canonical-json/v1",
    "dynamic_authority_status_fields": "forbidden",
    "declared_relations": {
        "binding": "compared_without_authentication",
        "budget": "compared_without_reservation_or_consumption",
        "capability": "compared_without_registry_resolution",
        "effect": "compared_without_execution",
        "scope": "deny_precedence_declared_relation_only",
        "subject": "compared_without_identity_authentication",
        "task": "compared_without_runtime_admission",
        "temporal": "explicit_caller_time_declared_window_only",
    },
    "unavailable_runtime": {
        "issuer_authentication": "unavailable",
        "principal_authentication": "unavailable",
        "policy_authentication": "unavailable",
        "authority_proof_verification": "unavailable",
        "policy_decision_point": "unavailable",
        "approval_validation": "unavailable",
        "revocation_evaluation": "unavailable",
        "usage_evaluation": "unavailable",
        "reservation_or_consumption": "unavailable",
        "preflight": "unavailable",
        "postflight": "unavailable",
        "audit_receipts": "unavailable",
        "context_package_integration": "unavailable",
        "persistence": "unavailable",
        "effect_execution": "unavailable",
    },
    "assessment_constants": {
        "authority_proof_state": "not_evaluated",
        "approval_state": "not_evaluated",
        "revocation_state": "not_evaluated",
        "usage_state": "not_evaluated",
        "authorization_decision": "none",
        "permission_attestation": False,
        "effect_attestation": False,
    },
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
    "production_effects": "forbidden",
}

CANONICAL_REFS = {
    "capability_grant_schema": SCHEMA_RELATIVE,
    "capability_grant_golden_fixture": FIXTURE_RELATIVE,
    "capability_grant_checker": CHECKER_RELATIVE,
    "capability_grant_skill": SKILL_RELATIVE,
    "capability_grant_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "capability_grant_python": {
        "ref": CHECKER_RELATIVE, "projection": "universal_scaffold"},
    "capability_grant_go": {
        "ref": "forge-core/internal/capabilitygrantcontract",
        "projection": "catalyst_repository_only"},
    "capability_grant_rust": {
        "ref": "forge-runtime/crates/domain/src/capability_grant_contract",
        "projection": "catalyst_repository_only"},
}

SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "effect_vocabulary_digest_domain": "forgeos.governance.effect-vocabulary.v1\0",
    "grant_digest_domain": "forgeos.capability-grant.v1\0",
    "requested_action_digest_domain": "forgeos.capability-requested-action.v1\0",
    "assessment_request_digest_domain":
        "forgeos.capability-grant-declared-assessment-request.v1\0",
    "assessment_digest_domain":
        "forgeos.capability-grant-declared-assessment.v1\0",
    "self_digest_rules": [
        "vocabulary_sha256 is empty while hashing the complete vocabulary",
        "grant_id, grant_sha256, and authority_proof.proof_base64url are empty while hashing the complete Grant",
        "request_sha256 is empty while hashing the complete assessment request",
        "assessment_sha256 is empty while hashing the complete declared assessment",
    ],
}

SCHEMA_LIMITS = {
    "max_vocabulary_bytes": 131_072,
    "max_grant_bytes": 1_048_576,
    "max_assessment_request_bytes": 2_097_152,
    "max_assessment_bytes": 262_144,
    "max_json_depth": 16,
    "max_object_fields": 64,
    "max_array_items": 256,
    "max_string_bytes": 16_384,
    "max_grant_ttl_ms": 86_400_000,
    "integer_domain": "signed_int64",
}

SCHEMA_AUTHORITY_SEMANTICS = {
    "delivery": "strict_pure_contract_only",
    "assessment_mode": "authority_neutral_declared_envelope_only",
    "frozen_effect_count": 21,
    "frozen_effect_vocabulary_sha256":
        "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f",
    "single_effect_per_grant": True,
    "scope_relation": "deny_precedence_declared_relation_only",
    "resource_order": "scope_kind_utf8_then_canonical_resource_utf8",
    "allow_semantics": "clauses_are_or_and_resources_do_not_cross_clause_boundaries",
    "deny_semantics": "flat_deny_matches_before_any_allow_clause",
    "migration_generate_environment_qualifier": (
        "allow_clause_and_requested_action_presence_must_match_and_value_is_exact"
    ),
    "process_exec_requested_timeout_relation": (
        "requested_action.resources[0].timeout_ms == "
        "requested_action.usage.timeout_ms"
    ),
    "canonical_ip_rejects_ipv4_mapped_ipv6": True,
    "dns_rejects_canonical_ipv4_literal": True,
    "ipv6_zone_ids_rejected": True,
    "secret_version_ref_ascii_pattern": (
        "^[A-Za-z0-9][A-Za-z0-9._:/@+\\-]{0,4095}(?![\\s\\S])"
    ),
    "secret_version_ref_moving_aliases_case_insensitive": [
        "active", "current", "latest",
    ],
    "effect_mismatch_scope_relation": "outside_declared_scope",
    "issuer_authentication": "not_evaluated",
    "policy_decision": "none",
    "approval_revocation_usage": "not_evaluated",
    "permission_attestation": False,
    "effect_attestation": False,
    "persistence": "none",
    "production_effects": "forbidden",
    "positive_result": RESULT,
    "attestations": [],
}

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root",
             "capability_grant_assessment_request", "capability_grant_assessment"],
    "positive": "test_golden_fixture_is_assessed_declarations_only",
    "negative": "test_authority_escalation_is_rejected",
}

EFFECT_VOCABULARY = (
    "approval.decide", "approval.request", "knowledge.apply", "knowledge.propose",
    "migration.apply", "migration.generate", "network.read", "network.write",
    "placement.plan", "policy.propose", "policy.write", "process.exec",
    "release.execute", "release.plan", "repo.read", "repo.write", "secrets.read",
    "target.execute", "target.inventory", "target.probe", "target.reserve",
)

SKILL_MARKERS = [
    "forgeos.capability-grant/v1",
    "authority_neutral_declared_envelope_only",
    "authorization_decision=none",
    "permission_attestation=false",
    "requested_action",
    "bootstrap_planning|plan_finalization",
    "authority_proof.issuer.authority_class",
    "external_operator",
    "scope.allow",
    "scope.deny",
    "资源不可跨 clause 拼接",
    "`migration.generate` 的 optional environment 是 clause qualifier",
    "command `timeout_ms` 必须与 proposed usage `timeout_ms` 相等",
    "IPv4-mapped IPv6",
    "IPv6 zone ID",
    "DNS-tagged canonical IPv4",
    "ASCII visible immutable identifier",
    "完整 canonical 文档本身也必须满足对应 byte ceiling",
    "scope=outside_declared_scope",
    "canonical-resource UTF-8",
    "AuthorityGrant",
    "production_effects: forbidden",
    "forge accept",
] + list(EFFECT_VOCABULARY)


def _pairs(pairs):
    value = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key {key!r}")
        value[key] = child
    return value


def _set(value):
    return set(value) if isinstance(value, list) else set()


def _mapping(value):
    return value if isinstance(value, dict) else {}


def registry_issues(data, path):
    issues = []
    if data.get("capability_grant") != CAPABILITY_GRANT:
        issues.append(f"{path}: capability_grant contract-only boundary drifted")
    scope = data.get("scope") if isinstance(data.get("scope"), dict) else {}
    kind_lists = {
        "shipped_kinds": scope.get("shipped_kinds"),
        "shipped_contract_only_kinds": scope.get("shipped_contract_only_kinds"),
        "planned_kinds": scope.get("planned_kinds"),
    }
    for field, values in kind_lists.items():
        if (not isinstance(values, list)
                or not all(isinstance(value, str) and value for value in values)):
            issues.append(f"{path}: scope.{field} must be a string list")
        elif len(values) != len(set(values)):
            issues.append(f"{path}: scope.{field} contains duplicate kinds")
    shipped = _set(kind_lists["shipped_kinds"])
    contract_only = _set(kind_lists["shipped_contract_only_kinds"])
    planned = _set(kind_lists["planned_kinds"])
    if contract_only != {
            "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
            "TransitionReceipt"}:
        issues.append(
            f"{path}: ApprovalRecord, CapabilityGrant, KnowledgeUpdateProposal, and TransitionReceipt must be shipped contract-only kinds"
        )
    if "CapabilityGrant" in shipped or "CapabilityGrant" in planned:
        issues.append(f"{path}: CapabilityGrant cannot be shipped-runtime or planned after ADR-0056")
    if shipped & contract_only or shipped & planned or contract_only & planned:
        issues.append(f"{path}: shipped, contract-only and planned kind sets must be disjoint")
    refs = data.get("canonical_refs") if isinstance(data.get("canonical_refs"), dict) else {}
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} must remain {expected!r}")
    implementations = data.get("reference_implementations")
    implementations = implementations if isinstance(implementations, dict) else {}
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def _schema_contract_shape_issues(definitions, path):
    issues = []
    effect_id = _mapping(definitions.get("effect_id"))
    if effect_id.get("enum") != list(EFFECT_VOCABULARY):
        issues.append(f"{path}: 21-effect Schema vocabulary drifted")
    grant_properties = _mapping(_mapping(definitions.get("grant")).get("properties"))
    if grant_properties.get("issuance_phase") != {
            "enum": ["bootstrap_planning", "plan_finalization"]}:
        issues.append(f"{path}: issuance_phase vocabulary drifted")
    scope_properties = _mapping(_mapping(definitions.get("scope")).get("properties"))
    allow = _mapping(scope_properties.get("allow"))
    deny = _mapping(scope_properties.get("deny"))
    if (allow.get("items") != {"$ref": "#/$defs/scope_clause"}
            or deny.get("items") != {"$ref": "#/$defs/resource"}):
        issues.append(f"{path}: grouped allow / flat deny scope shape drifted")
    issuer_properties = _mapping(_mapping(definitions.get("issuer")).get("properties"))
    principal = _mapping(definitions.get("principal"))
    principal_properties = _mapping(principal.get("properties"))
    if (_mapping(issuer_properties.get("authority_class")).get("enum")
            != ["external_operator", "forgeos_kernel"]
            or principal.get("additionalProperties") is not False
            or "authority_class" in principal_properties):
        issues.append(f"{path}: external_operator must remain issuer authority_class only")
    assessment_properties = _mapping(
        _mapping(definitions.get("assessment")).get("properties"))
    constants = dict(CAPABILITY_GRANT["assessment_constants"])
    constants.update({"assessment_mode": CAPABILITY_GRANT["mode"], "result": RESULT})
    for field, expected in constants.items():
        if assessment_properties.get(field) != {"const": expected}:
            issues.append(f"{path}: assessment.{field} constant drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=2_097_152)
        schema = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate CapabilityGrant Schema: {error}"]
    issues = []
    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        issues.append(f"{path}: CapabilityGrant Schema draft drifted")
    if schema.get("$id") != "https://forgeos.dev/contracts/capability-grant-v1.schema.json":
        issues.append(f"{path}: CapabilityGrant Schema identity drifted")
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        issues.append(f"{path}: CapabilityGrant golden envelope must be a closed object")
    metadata = {
        "x-forgeos-canonicalization": SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": SCHEMA_LIMITS,
        "x-forgeos-authority-semantics": SCHEMA_AUTHORITY_SEMANTICS,
    }
    issues.extend(f"{path}: {field} drifted" for field, expected in metadata.items()
                  if schema.get(field) != expected)
    issues.extend(_schema_contract_shape_issues(_mapping(schema.get("$defs")), path))
    return issues


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        raw = read_bounded_file(path, label=FIXTURE_RELATIVE, max_bytes=2_097_152)
        fixture = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate CapabilityGrant fixture: {error}"]
    issues = []
    expected_root = {"assessment_request", "effect_vocabulary",
                     "expected_assessment", "grant"}
    if not isinstance(fixture, dict) or set(fixture) != expected_root:
        return [f"{path}: CapabilityGrant fixture root fields drifted"]
    vocabulary = fixture.get("effect_vocabulary")
    vocabulary = vocabulary if isinstance(vocabulary, dict) else {}
    effects = vocabulary.get("effects")
    effect_ids = [item.get("effect_id") for item in effects
                  if isinstance(item, dict)] if isinstance(effects, list) else []
    if effect_ids != list(EFFECT_VOCABULARY):
        issues.append(f"{path}: 21-effect canonical vocabulary drifted")
    expected_headers = (
        (vocabulary, "api_version", "forgeos.governance.effect-vocabulary/v1"),
        (vocabulary, "kind", "EffectVocabulary"),
        (fixture.get("grant"), "api_version", CAPABILITY_GRANT["api_version"]),
        (fixture.get("grant"), "kind", "CapabilityGrant"),
        (fixture.get("assessment_request"), "api_version",
         CAPABILITY_GRANT["request_api_version"]),
        (fixture.get("expected_assessment"), "api_version",
         CAPABILITY_GRANT["assessment_api_version"]),
    )
    for owner, field, expected in expected_headers:
        if not isinstance(owner, dict) or owner.get(field) != expected:
            issues.append(f"{path}: fixture {field} must remain {expected!r}")
    assessment = fixture.get("expected_assessment")
    assessment = assessment if isinstance(assessment, dict) else {}
    constants = dict(CAPABILITY_GRANT["assessment_constants"])
    constants.update({"assessment_mode": CAPABILITY_GRANT["mode"], "result": RESULT})
    for field, expected in constants.items():
        if assessment.get(field) != expected:
            issues.append(f"{path}: expected_assessment.{field} drifted")
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.capability_grant_contract")
    if not isinstance(detector, dict):
        return ["CapabilityGrant contract-only detector is missing"]
    issues = []
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("CapabilityGrant detector must remain shadow and fail closed")
    invocation = detector.get("invocation")
    expected_invocation = {
        "owner": "operator", "adapter": "standalone.capabilityGrantContract",
        "acceptance_criterion": None, "load_bearing": False,
    }
    if invocation != expected_invocation:
        issues.append("CapabilityGrant detector cannot become load-bearing authority")
    implementation = detector.get("implementation")
    if not isinstance(implementation, dict) or implementation.get("argv") != DETECTOR["argv"]:
        issues.append("CapabilityGrant detector requires exact request/assessment arguments")
    tests = detector.get("tests")
    for polarity in ("positive", "negative"):
        test = tests.get(polarity) if isinstance(tests, dict) else None
        if not isinstance(test, dict) or test.get("contains") != DETECTOR[polarity]:
            issues.append(f"CapabilityGrant detector {polarity} test drifted")
    return issues


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate CapabilityGrant Skill: {error}"]
    return [f"{path}: missing CapabilityGrant contract-only marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def integration_issues(data, path, repo_root, agent_root):
    from capability_grant_contract_check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
