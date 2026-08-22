"""ADR-0059 ApprovalRecord contract-only governance integration."""

from __future__ import annotations

import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/approval-record-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/approval-record-v1.json"
CHECKER_RELATIVE = "harness/approval_record_contract_check.py"
SKILL_RELATIVE = ".agent/skills/policy-authority.md"
DECISION_RELATIVE = "docs/adr/0059-approval-record-v1-contract-only.md"
SCHEMA_SHA256 = "bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64"
FIXTURE_SHA256 = "501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978"
RESULT = (
    "ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority "
    "authentication, attestation or SoD proof verification, condition or "
    "RiskAcceptance validation, revocation evaluation, policy decision, "
    "effective approval, authorization, permission, persistence, transition, "
    "execution, or effect attestation)"
)

ASSESSMENT_CONSTANTS = {
    "approver_identity_state": "not_evaluated",
    "authority_proof_state": "not_evaluated",
    "condition_satisfaction_state": "not_evaluated",
    "effective_approval_state": "not_evaluated",
    "revocation_registry_state": "not_evaluated",
    "risk_acceptance_state": "not_evaluated",
    "separation_of_duty_proof_state": "not_evaluated",
    "policy_decision": "none",
    "authorization_decision": "none",
    "permission_attestation": False,
    "effect_attestation": False,
    "persistence_attestation": False,
    "transition_attestation": False,
}

APPROVAL_RECORD = {
    "api_version": "forgeos.approval-record/v1",
    "request_api_version": "forgeos.approval-record-declared-assessment-request/v1",
    "assessment_api_version": "forgeos.approval-record-declared-assessment/v1",
    "delivery": "strict_pure_contract_only",
    "mode": "authority_neutral_declared_approval_only",
    "input": "exact_canonical_caller_supplied_record_request_and_assessment",
    "canonicalization": "forgeos.canonical-json/v1",
    "marker_import": "forbidden",
    "declared_relations": {
        "approver": "compared_without_identity_authentication",
        "authority_binding": "compared_without_proof_verification",
        "binding": "compared_without_preimage_resolution",
        "conditions": "compared_without_validation",
        "decision": "compared_without_policy_decision",
        "risk_acceptance": "compared_without_validation",
        "scope": "compared_without_effect_authority",
        "separation_of_duty": "compared_without_proof_verification",
        "subject": "compared_without_identity_authentication",
        "temporal": "explicit_caller_time_declared_window_only",
        "declared_revocation": "explicit_caller_time_only_no_registry",
        "capability_grant_projection": (
            "approval_id_approval_sha256_authority_domain_relation_only"
        ),
    },
    "unavailable_runtime": {
        "approver_authentication": "unavailable",
        "authority_source_authentication": "unavailable",
        "authority_proof_verification": "unavailable",
        "separation_of_duty_proof_verification": "unavailable",
        "condition_validation": "unavailable",
        "risk_acceptance_validation": "unavailable",
        "revocation_registry_evaluation": "unavailable",
        "policy_decision_point": "unavailable",
        "effective_approval": "unavailable",
        "authorization": "unavailable",
        "permission": "unavailable",
        "persistence": "unavailable",
        "transition": "unavailable",
        "effect_execution": "unavailable",
    },
    "assessment_constants": ASSESSMENT_CONSTANTS,
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
    "production_effects": "forbidden",
}

CANONICAL_REFS = {
    "approval_record_schema": SCHEMA_RELATIVE,
    "approval_record_golden_fixture": FIXTURE_RELATIVE,
    "approval_record_checker": CHECKER_RELATIVE,
    "approval_record_skill": SKILL_RELATIVE,
    "approval_record_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "approval_record_python": {
        "ref": CHECKER_RELATIVE, "projection": "universal_scaffold"},
    "approval_record_go": {
        "ref": "forge-core/internal/approvalrecordcontract",
        "projection": "catalyst_repository_only"},
    "approval_record_rust": {
        "ref": "forge-runtime/crates/domain/src/approval_record_contract",
        "projection": "catalyst_repository_only"},
}

SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "approval_record_digest_domain": "forgeos.approval-record.v1\0",
    "declared_target_digest_domain": "forgeos.approval-declared-target.v1\0",
    "assessment_request_digest_domain":
        "forgeos.approval-record-declared-assessment-request.v1\0",
    "assessment_digest_domain":
        "forgeos.approval-record-declared-assessment.v1\0",
    "self_digest_rules": [
        "approval_id, approval_sha256, authority_proof.proof_base64url, and separation_of_duty.proof_base64url are empty while hashing the complete ApprovalRecord",
        "the complete declared target has no self-digest field",
        "request_sha256 is empty while hashing the complete assessment request, including both proof byte strings",
        "assessment_sha256 is empty while hashing the complete declared assessment",
    ],
}

SCHEMA_LIMITS = {
    "max_record_bytes": 1_048_576,
    "max_declared_target_bytes": 1_048_576,
    "max_assessment_request_bytes": 2_097_152,
    "max_assessment_bytes": 262_144,
    "max_golden_bytes": 4_194_304,
    "max_json_depth": 16,
    "max_object_fields": 64,
    "max_array_items": 256,
    "max_string_bytes": 16_384,
    "max_short_text_bytes": 160,
    "max_proof_text_bytes": 16_384,
    "max_validity_ms": 86_400_000,
    "integer_domain": "signed_int64",
}

SCHEMA_AUTHORITY_SEMANTICS = {
    "delivery": "strict_pure_contract_only",
    "assessment_mode": "authority_neutral_declared_approval_only",
    "marker_import": "forbidden",
    "local_approved_marker_is_approval_record": False,
    "actor_hint_is_authenticated_identity": False,
    **ASSESSMENT_CONSTANTS,
    "approver_authentication": "not_evaluated",
    "condition_satisfaction": "not_evaluated",
    "risk_acceptance_validation": "not_evaluated",
    "separation_of_duty_proof_verification": "not_evaluated",
    "revocation_registry_evaluation": "not_evaluated",
    "effective_approval": "not_evaluated",
    "production_effects": "forbidden",
    "positive_result": RESULT,
    "attestations": [],
}
# Schema uses shorter semantic field names than the assessment wire.
for _field in tuple(ASSESSMENT_CONSTANTS):
    SCHEMA_AUTHORITY_SEMANTICS.pop(_field, None)
SCHEMA_AUTHORITY_SEMANTICS.update({
    "authority_proof_verification": "not_evaluated",
    "policy_decision": "none",
    "authorization_decision": "none",
    "permission_attestation": False,
    "effect_attestation": False,
    "persistence_attestation": False,
    "transition_attestation": False,
})

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root",
             "approval_record_assessment_request", "approval_record_assessment"],
    "positive": "test_golden_digests_and_result_are_frozen",
    "negative": "test_authority_escalation_is_rejected",
}

SKILL_MARKERS = [
    "forgeos.approval-record/v1",
    "authority_neutral_declared_approval_only",
    "ASSESSED_APPROVAL_DECLARATIONS_ONLY",
    "effective_approval_state=not_evaluated",
    "authorization_decision=none",
    "permission_attestation=false",
    ".forge/<stage>.approved",
    "--approved",
    "actor_hint",
    "RiskAcceptance",
    "approval_id",
    "approval_sha256",
    "authority_domain",
    "forge accept",
]

PROMOTION_MARKERS = {
    DECISION_RELATIVE: "- Status: Accepted",
    ".agent/DECISIONS.md": "D31 ApprovalRecord v1 contract-only（2026-08-11）",
    ".agent/ROADMAP.md": "DONE — Wave 0F-B–3b-4 ApprovalRecord v1 contract-only",
    ".agent/CURRENT_SPRINT.md": (
        "Sprint 107（✅ DONE；ApprovalRecord v1 contract-only）— ADR 0059"
    ),
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": (
        "| DONE | ApprovalRecord v1 contract-only wire (ADR 0059) |"
    ),
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
    if data.get("approval_record") != APPROVAL_RECORD:
        issues.append(f"{path}: ApprovalRecord contract-only boundary drifted")
    scope = _mapping(data.get("scope"))
    expected_kinds = [
        "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
        "TransitionReceipt",
    ]
    if scope.get("shipped_contract_only_kinds") != expected_kinds:
        issues.append(f"{path}: contract-only kinds must remain {expected_kinds!r}")
    if "ApprovalRecord" in (scope.get("shipped_kinds") or []):
        issues.append(f"{path}: ApprovalRecord cannot be a shipped runtime kind")
    if "ApprovalRecord" in (scope.get("planned_kinds") or []):
        issues.append(f"{path}: ApprovalRecord cannot remain planned after ADR-0059")
    refs = _mapping(data.get("canonical_refs"))
    for field, expected in CANONICAL_REFS.items():
        if refs.get(field) != expected:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    implementations = _mapping(data.get("reference_implementations"))
    for field, expected in REFERENCE_IMPLEMENTATIONS.items():
        if implementations.get(field) != expected:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw = read_bounded_file(path, label=SCHEMA_RELATIVE, max_bytes=2_097_152)
        schema = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate ApprovalRecord Schema: {error}"]
    issues = []
    expected = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://forgeos.dev/contracts/approval-record-v1.schema.json",
        "x-forgeos-canonicalization": SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": SCHEMA_LIMITS,
        "x-forgeos-authority-semantics": SCHEMA_AUTHORITY_SEMANTICS,
    }
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if schema.get(field) != value)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        issues.append(f"{path}: ApprovalRecord golden envelope must be closed")
    definitions = _mapping(schema.get("$defs"))
    if set(_mapping(definitions.get("approval_ref")).get("required") or []) != {
            "approval_id", "approval_sha256", "authority_domain"}:
        issues.append(f"{path}: CapabilityGrant ApprovalRef projection drifted")
    return issues


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        raw = read_bounded_file(path, label=FIXTURE_RELATIVE, max_bytes=4_194_304)
        fixture = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate ApprovalRecord fixture: {error}"]
    expected_fields = {
        "approval_record", "assessment_request", "expected_approval_ref",
        "expected_assessment",
    }
    if not isinstance(fixture, dict) or set(fixture) != expected_fields:
        return [f"{path}: ApprovalRecord fixture root fields drifted"]
    assessment = _mapping(fixture.get("expected_assessment"))
    expected = {**ASSESSMENT_CONSTANTS, "assessment_mode": APPROVAL_RECORD["mode"],
                "result": RESULT}
    return [f"{path}: expected_assessment.{field} drifted"
            for field, value in expected.items() if assessment.get(field) != value]


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.approval_record_contract")
    if not isinstance(detector, dict):
        return ["ApprovalRecord contract-only detector is missing"]
    issues = []
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("ApprovalRecord detector must remain shadow and fail closed")
    expected_invocation = {
        "owner": "operator", "adapter": "standalone.approvalRecordContract",
        "acceptance_criterion": None, "load_bearing": False,
    }
    if detector.get("invocation") != expected_invocation:
        issues.append("ApprovalRecord detector cannot become load-bearing authority")
    implementation = _mapping(detector.get("implementation"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("ApprovalRecord detector requires exact request/assessment arguments")
    tests = _mapping(detector.get("tests"))
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"ApprovalRecord detector {polarity} test drifted")
    return issues


def route_issues(repo_root):
    path = repo_root / ".agent/engineering/context-routes.yml"
    try:
        import yaml
        data = yaml.safe_load(read_bounded_file(path, label=str(path),
                                               max_bytes=524_288))
    except (ImportError, OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: cannot validate ApprovalRecord route: {error}"]
    routes = data.get("routes") if isinstance(data, dict) else []
    route = next((item for item in routes or [] if item.get("id") == "governance"), {})
    includes = {item.get("ref"): item for item in route.get("include", [])}
    expected = {
        ".agent/engineering/governance-contracts.yml": ("instruction", 131_072),
        SKILL_RELATIVE: ("instruction", 65_536),
        SCHEMA_RELATIVE: ("trusted_context", 131_072),
    }
    return [f"{path}: ApprovalRecord route {ref!r} must be required {lane}/{limit}"
            for ref, (lane, limit) in expected.items()
            if includes.get(ref) != {"ref": ref, "lane": lane, "required": True,
                                     "max_bytes": limit}]


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate ApprovalRecord Skill: {error}"]
    return [f"{path}: missing ApprovalRecord marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in text]


def _promotion_fact_present(repo_root):
    for relative, marker in PROMOTION_MARKERS.items():
        if relative == DECISION_RELATIVE:
            continue
        path = repo_root / relative
        if not path.exists():
            continue
        try:
            text = read_bounded_file(path, label=relative,
                                     max_bytes=2_097_152).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError):
            return True
        if marker in text:
            return True
    return False


def promotion_issues(repo_root, *, optional=False):
    if optional and not _promotion_fact_present(repo_root):
        return []
    issues = []
    for relative, marker in PROMOTION_MARKERS.items():
        path = repo_root / relative
        try:
            text = read_bounded_file(path, label=relative,
                                     max_bytes=2_097_152).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR-0059 promotion: {error}")
            continue
        if marker not in text:
            issues.append(f"{path}: missing accepted ADR-0059 marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    from approval_record_contract_check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(route_issues(repo_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(promotion_issues(repo_root, optional=True))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
