"""ADR-0060 accepted TransitionReceipt contract-only governance integration."""

from __future__ import annotations

import hashlib
import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/transition-receipt-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/transition-receipt-v1.json"
CHECKER_RELATIVE = "harness/transition_receipt_contract_check.py"
SKILL_RELATIVE = ".agent/skills/policy-authority.md"
DECISION_RELATIVE = "docs/adr/0060-transition-receipt-v1-contract-only.md"
SCHEMA_SHA256 = "94962069c93f55129506b9d4b45f1f9db6d9425ecbdbaef9c06fcbe155e43cbf"
FIXTURE_SHA256 = "dac0b6d8921aaecaf138c5b62924c8a3b9ac8f9c531a67f2be358d47c1c30da9"
RESULT = (
    "ASSESSED_TRANSITION_DECLARATIONS_ONLY (no controller, actor, Grant, "
    "Approval, evidence, waiver, precondition or state authentication; no "
    "policy decision, authorization, persistence, transition, ledger, "
    "execution, effect or completion attestation)"
)

ASSESSMENT_CONSTANTS = {
    "controller_authentication_state": "not_evaluated",
    "grant_state": "not_evaluated",
    "approval_state": "not_evaluated",
    "evidence_state": "not_evaluated",
    "waiver_state": "not_evaluated",
    "precondition_truth_state": "not_evaluated",
    "ledger_state": "not_evaluated",
    "policy_decision": "none",
    "authorization_decision": "none",
    "permission_attestation": False,
    "persistence_attestation": False,
    "transition_attestation": False,
    "execution_attestation": False,
    "effect_attestation": False,
    "completion_attestation": False,
}

TRANSITION_RECEIPT = {
    "api_version": "forgeos.transition-receipt/v1",
    "vocabulary_api_version": "forgeos.transition-state-vocabulary/v1",
    "request_api_version":
        "forgeos.transition-receipt-declared-assessment-request/v1",
    "assessment_api_version":
        "forgeos.transition-receipt-declared-assessment/v1",
    "delivery": "strict_pure_contract_only",
    "mode": "authority_neutral_declared_transition_only",
    "input": (
        "exact_canonical_caller_supplied_receipt_target_previous_request_"
        "and_assessment"
    ),
    "canonicalization": "forgeos.canonical-json/v1",
    "workflow_or_marker_import": "forbidden",
    "declared_relations": {
        "target": "compared_without_state_authentication",
        "edge": "compared_against_frozen_declared_graph_only",
        "chain": "compared_against_explicit_caller_predecessor_only",
        "continuity": "compared_without_authoritative_current_state",
        "preconditions": "compared_without_truth_evaluation",
        "applicability": "compared_without_stage_authority",
        "recovery": "compared_without_rework_or_resume_execution",
        "temporal": "explicit_caller_time_only",
        "capability_grant": (
            "compared_without_permission_or_transition_authority"
        ),
        "approval_records": (
            "compared_without_effective_approval_or_transition_authority"
        ),
    },
    "unavailable_runtime": {
        "controller_authentication": "unavailable",
        "actor_authentication": "unavailable",
        "authoritative_current_state": "unavailable",
        "policy_decision_point": "unavailable",
        "grant_or_approval_evaluation": "unavailable",
        "evidence_or_waiver_evaluation": "unavailable",
        "append_only_transition_ledger": "unavailable",
        "cas_idempotency_or_replay": "unavailable",
        "transition_execution": "unavailable",
        "persistence": "unavailable",
        "effect_execution": "unavailable",
        "completion_attestation": "unavailable",
    },
    "assessment_constants": ASSESSMENT_CONSTANTS,
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
    "production_effects": "forbidden",
}

CANONICAL_REFS = {
    "transition_receipt_schema": SCHEMA_RELATIVE,
    "transition_receipt_golden_fixture": FIXTURE_RELATIVE,
    "transition_receipt_checker": CHECKER_RELATIVE,
    "transition_receipt_skill": SKILL_RELATIVE,
    "transition_receipt_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "transition_receipt_python": {
        "ref": CHECKER_RELATIVE, "projection": "universal_scaffold"},
    "transition_receipt_go": {
        "ref": "forge-core/internal/transitionreceiptcontract",
        "projection": "catalyst_repository_only"},
    "transition_receipt_rust": {
        "ref": "forge-runtime/crates/domain/src/transition_receipt_contract",
        "projection": "catalyst_repository_only"},
}

SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "transition_vocabulary_digest_domain":
        "forgeos.governance.transition-state-vocabulary.v1\0",
    "transition_receipt_digest_domain": "forgeos.transition-receipt.v1\0",
    "declared_target_digest_domain": "forgeos.transition-declared-target.v1\0",
    "assessment_request_digest_domain":
        "forgeos.transition-receipt-declared-assessment-request.v1\0",
    "assessment_digest_domain":
        "forgeos.transition-receipt-declared-assessment.v1\0",
    "self_digest_rules": [
        "vocabulary_sha256 is empty while hashing the complete vocabulary",
        "receipt_id and receipt_sha256 are empty while hashing the complete TransitionReceipt",
        "the complete declared target has no self-digest field",
        "request_sha256 is empty while hashing the complete request",
        "assessment_sha256 is empty while hashing the complete assessment",
    ],
}

SCHEMA_LIMITS = {
    "max_vocabulary_bytes": 262144, "max_receipt_bytes": 1048576,
    "max_previous_receipt_bytes": 1048576,
    "max_declared_target_bytes": 1048576,
    "max_assessment_request_bytes": 4194304,
    "max_assessment_bytes": 262144, "max_golden_bytes": 8388608,
    "max_json_depth": 16, "max_object_fields": 64, "max_array_items": 256,
    "max_string_bytes": 16384, "max_short_text_bytes": 160,
    "max_reference_text_bytes": 4096, "max_preconditions": 64,
    "max_evidence_refs_per_precondition": 32,
    "max_total_evidence_refs": 256,
    "max_reason_codes_per_nested_declaration": 16,
    "max_total_reason_codes": 256, "integer_domain": "signed_int64",
    "runtime_string_length_unit": "utf8_bytes",
    "schema_max_length_unit": "unicode_code_points",
    "schema_length_keywords_are_non_authoritative_approximations": True,
}

SCHEMA_AUTHORITY = {
    "delivery": "strict_pure_contract_only",
    "assessment_mode": "authority_neutral_declared_transition_only",
    "ambient_workflow_or_marker_import": "forbidden",
    **ASSESSMENT_CONSTANTS,
    "positive_result": RESULT,
    "attestations": [],
}

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root",
             "transition_receipt_assessment_request",
             "transition_receipt_assessment"],
    "positive": "test_frozen_golden_schema_and_all_five_hashes",
    "negative": "test_authority_escalation_and_assessment_drift_fail_closed",
}

SKILL_MARKERS = [
    "forgeos.transition-receipt/v1",
    "authority_neutral_declared_transition_only",
    "ASSESSED_TRANSITION_DECLARATIONS_ONLY",
    "controller_authentication_state=not_evaluated",
    "ledger_state=not_evaluated",
    "transition_attestation=false",
    "lifecycle.transition",
    "Accepted contract-only slice",
]

PROMOTION_MARKERS = {
    DECISION_RELATIVE: "- Status: Accepted",
    ".agent/DECISIONS.md": "D32 TransitionReceipt v1 contract-only（2026-08-12）",
    ".agent/ROADMAP.md": "DONE — Wave 0F-B–3b-5 TransitionReceipt v1 contract-only",
    ".agent/CURRENT_SPRINT.md": (
        "Sprint 108（✅ DONE；TransitionReceipt v1 contract-only）— ADR 0060"
    ),
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": (
        "| DONE | TransitionReceipt v1 contract-only wire (ADR 0060) |"
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
    if data.get("transition_receipt") != TRANSITION_RECEIPT:
        issues.append(f"{path}: TransitionReceipt contract-only boundary drifted")
    scope = _mapping(data.get("scope"))
    expected = [
        "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
        "TransitionReceipt",
    ]
    if scope.get("shipped_contract_only_kinds") != expected:
        issues.append(f"{path}: contract-only kinds must remain {expected!r}")
    if "TransitionReceipt" in (scope.get("shipped_kinds") or []):
        issues.append(f"{path}: TransitionReceipt cannot be a shipped runtime kind")
    if scope.get("planned_kinds") != []:
        issues.append(f"{path}: planned kinds must be empty after ADR-0061 wire freeze")
    for field, expected_value in CANONICAL_REFS.items():
        if _mapping(data.get("canonical_refs")).get(field) != expected_value:
            issues.append(f"{path}: canonical_refs.{field} drifted")
    for field, expected_value in REFERENCE_IMPLEMENTATIONS.items():
        if _mapping(data.get("reference_implementations")).get(field) != expected_value:
            issues.append(f"{path}: reference_implementations.{field} drifted")
    return issues


def _load_json(repo_root, relative, max_bytes):
    raw = read_bounded_file(repo_root / relative, label=relative, max_bytes=max_bytes)
    return raw, json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs)


def schema_issues(repo_root):
    path = repo_root / SCHEMA_RELATIVE
    try:
        raw, schema = _load_json(repo_root, SCHEMA_RELATIVE, 1048576)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate TransitionReceipt Schema: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != SCHEMA_SHA256:
        issues.append(f"{path}: TransitionReceipt Schema pin drifted")
    expected = {
        "$id": "https://forgeos.dev/contracts/transition-receipt-v1.schema.json",
        "x-forgeos-canonicalization": SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": SCHEMA_LIMITS,
        "x-forgeos-authority-semantics": SCHEMA_AUTHORITY,
    }
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if schema.get(field) != value)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        issues.append(f"{path}: TransitionReceipt golden envelope must be closed")
    return issues


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        raw, fixture = _load_json(repo_root, FIXTURE_RELATIVE, 8388608)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate TransitionReceipt fixture: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA256:
        issues.append(f"{path}: TransitionReceipt fixture pin drifted")
    fields = {"assessment_request", "expected_approval_refs", "expected_assessment",
              "expected_capability_grant_ref", "transition_receipt",
              "transition_vocabulary"}
    if not isinstance(fixture, dict) or set(fixture) != fields:
        return issues + [f"{path}: TransitionReceipt fixture root fields drifted"]
    assessment = _mapping(fixture.get("expected_assessment"))
    expected = {**ASSESSMENT_CONSTANTS, "assessment_mode": TRANSITION_RECEIPT["mode"],
                "result": RESULT}
    issues.extend(f"{path}: expected_assessment.{field} drifted"
                  for field, value in expected.items()
                  if assessment.get(field) != value)
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.transition_receipt_contract")
    if not isinstance(detector, dict):
        return ["TransitionReceipt contract-only detector is missing"]
    issues = []
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("TransitionReceipt detector must remain shadow and fail closed")
    invocation = {"owner": "operator", "adapter":
                  "standalone.transitionReceiptContract",
                  "acceptance_criterion": None, "load_bearing": False}
    if detector.get("invocation") != invocation:
        issues.append("TransitionReceipt detector cannot become load-bearing authority")
    implementation = _mapping(detector.get("implementation"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("TransitionReceipt detector requires exact request/assessment arguments")
    tests = _mapping(detector.get("tests"))
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"TransitionReceipt detector {polarity} test drifted")
    return issues


def route_issues(repo_root):
    path = repo_root / ".agent/engineering/context-routes.yml"
    try:
        import yaml
        data = yaml.safe_load(read_bounded_file(path, label=str(path), max_bytes=524288))
    except (ImportError, OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: cannot validate TransitionReceipt route: {error}"]
    routes = data.get("routes") if isinstance(data, dict) else []
    route = next((item for item in routes or [] if item.get("id") == "governance"), {})
    includes = {item.get("ref"): item for item in route.get("include", [])}
    expected = {"ref": SCHEMA_RELATIVE, "lane": "trusted_context",
                "required": True, "max_bytes": 131072}
    return [] if includes.get(SCHEMA_RELATIVE) == expected else [
        f"{path}: TransitionReceipt schema route must be required trusted_context/131072"]


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        text = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate TransitionReceipt Skill: {error}"]
    return [f"{path}: missing TransitionReceipt marker {marker!r}"
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
                                     max_bytes=2097152).decode("utf-8")
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
                                     max_bytes=2097152).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR-0060 promotion: {error}")
            continue
        if marker not in text:
            issues.append(f"{path}: missing accepted ADR-0060 marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    from transition_receipt_contract_check import validate_golden_fixture
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(route_issues(repo_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(promotion_issues(repo_root, optional=True))
    issues.extend(validate_golden_fixture(repo_root))
    return issues
