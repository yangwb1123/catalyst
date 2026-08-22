"""ADR-0061 accepted KnowledgeUpdateProposal governance integration."""

from __future__ import annotations

import hashlib
import json

from governance_contract import ContractError, read_bounded_file


SCHEMA_RELATIVE = "docs/contracts/knowledge-update-proposal-v1.schema.json"
FIXTURE_RELATIVE = "docs/contracts/fixtures/knowledge-update-proposal-v1.json"
CHECKER_RELATIVE = "harness/knowledge_update_proposal_contract_check.py"
SKILL_RELATIVE = ".agent/skills/evidence-claim-management.md"
DECISION_RELATIVE = "docs/adr/0061-knowledge-update-proposal-v1-contract-only.md"
SCHEMA_SHA256 = "5825658017a9debf197cd82a0df4d553bf101ed20b1a35f6ff3e9d07064e4c4b"
FIXTURE_SHA256 = "2808e44b27df5f7b183ae7da3847d5780a3f66887d6b49e5fb4544a069a7ad5f"
GOLDEN_HASHES = {
    "record_set": "c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f",
    "proposal": "a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1",
    "target": "34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85",
    "request": "d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413",
    "assessment": "e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037",
}
RESULT = (
    "ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no proposer, Grant, Context, "
    "evidence, current-knowledge, conflict, freshness, policy or authority "
    "evaluation; no truth, adoption, authorization, permission, persistence, "
    "apply, receipt, execution or effect attestation)"
)

ASSESSMENT_CONSTANTS = {
    "proposer_authentication_state": "not_evaluated",
    "grant_state": "not_evaluated",
    "context_state": "not_evaluated",
    "evidence_state": "not_evaluated",
    "current_knowledge_state": "not_evaluated",
    "conflict_state": "not_evaluated",
    "freshness_state": "not_evaluated",
    "policy_decision": "none",
    "authorization_decision": "none",
    "truth_attestation": False,
    "knowledge_adoption_attestation": False,
    "permission_attestation": False,
    "persistence_attestation": False,
    "execution_attestation": False,
    "effect_attestation": False,
}

KNOWLEDGE_UPDATE_PROPOSAL = {
    "api_version": "forgeos.knowledge-update-proposal/v1",
    "request_api_version":
        "forgeos.knowledge-update-proposal-declared-assessment-request/v1",
    "assessment_api_version":
        "forgeos.knowledge-update-proposal-declared-assessment/v1",
    "delivery": "strict_pure_contract_only",
    "mode": "authority_neutral_declared_knowledge_update_only",
    "input": "exact_canonical_caller_supplied_proposal_target_and_assessment",
    "canonicalization": "forgeos.canonical-json/v1",
    "proposal_semantics": {
        "records": "exact_reachable_after_claim_closure",
        "mutations": "sorted_unique_create_or_supersede_declarations",
        "create": "sequence_one_null_before_and_no_supersedes",
        "supersede": (
            "exact_immediate_predecessor_stable_semantic_identity_and_shadow_"
            "lifecycle_only"
        ),
        "knowledge_scope_hash": "opaque_declared_binding_not_recomputed",
        "current_head_or_conflict_lookup": "none",
        "apply_or_persistence": "none",
    },
    "declared_relations": {
        "binding": "compared_without_source_or_context_authentication",
        "grant_ref": "compared_without_grant_authority",
        "mutations": "compared_without_current_knowledge_state",
        "proposer": "compared_without_identity_authentication",
        "record_set": "compared_without_evidence_truth",
        "scope": "compared_without_scope_authority",
        "task_binding": "compared_without_task_authority",
        "temporal": "explicit_caller_time_only",
    },
    "cross_contract_relations": {
        "capability_grant": "declared_compatibility_without_permission_or_apply_authority",
        "context_package": "caller_reassembled_comparison_without_source_or_instruction_authority",
        "artifact_resources": "declared_projection_without_artifact_authentication",
    },
    "unavailable_runtime": {
        "proposer_authentication": "unavailable",
        "grant_or_context_authentication": "unavailable",
        "evidence_truth_or_freshness": "unavailable",
        "authoritative_current_knowledge": "unavailable",
        "conflict_arbitration": "unavailable",
        "policy_decision_point": "unavailable",
        "knowledge_update_receipt": "unavailable",
        "persistence_or_apply": "unavailable",
        "execution_or_effect": "unavailable",
    },
    "assessment_constants": ASSESSMENT_CONSTANTS,
    "positive_result": RESULT,
    "attests": [],
    "persistence": "none",
    "production_effects": "forbidden",
}

CANONICAL_REFS = {
    "knowledge_update_proposal_schema": SCHEMA_RELATIVE,
    "knowledge_update_proposal_golden_fixture": FIXTURE_RELATIVE,
    "knowledge_update_proposal_checker": CHECKER_RELATIVE,
    "knowledge_update_proposal_skill": SKILL_RELATIVE,
    "knowledge_update_proposal_decision": DECISION_RELATIVE,
}

REFERENCE_IMPLEMENTATIONS = {
    "knowledge_update_proposal_python": {
        "ref": CHECKER_RELATIVE, "projection": "universal_scaffold"},
    "knowledge_update_proposal_go": {
        "ref": "forge-core/internal/knowledgeupdateproposalcontract",
        "projection": "catalyst_repository_only"},
    "knowledge_update_proposal_rust": {
        "ref": "forge-runtime/crates/domain/src/knowledge_update_proposal_contract",
        "projection": "catalyst_repository_only"},
}

SCHEMA_CANONICALIZATION = {
    "format": "forgeos.canonical-json/v1",
    "record_set_digest_domain": "forgeos.governance.record-set.v1\0",
    "proposal_digest_domain": "forgeos.knowledge-update-proposal.v1\0",
    "declared_target_digest_domain": "forgeos.knowledge-update-declared-target.v1\0",
    "assessment_request_digest_domain":
        "forgeos.knowledge-update-proposal-declared-assessment-request.v1\0",
    "assessment_digest_domain":
        "forgeos.knowledge-update-proposal-declared-assessment.v1\0",
    "self_digest_rules": [
        "record_set_sha256 hashes the exact canonical records array",
        "proposal_id and proposal_sha256 are empty while hashing the complete proposal",
        "the complete seven-field declared target has no self-digest field",
        "request_sha256 is empty while hashing the complete request",
        "assessment_sha256 is empty while hashing the complete assessment",
    ],
}

SCHEMA_LIMITS = {
    "max_proposal_bytes": 2097152, "max_declared_target_bytes": 1048576,
    "max_assessment_request_bytes": 4194304, "max_assessment_bytes": 262144,
    "max_golden_bytes": 8388608, "max_record_bytes": 131072,
    "max_record_set_bytes": 1048576, "max_records": 256,
    "max_mutations": 64, "max_artifacts": 32,
    "max_mutation_reason_codes": 16, "max_json_depth": 16,
    "max_object_fields": 64, "max_array_items": 256,
    "max_string_bytes": 16384, "max_short_text_bytes": 160,
    "max_reference_text_bytes": 4096, "integer_domain": "signed_int64",
    "runtime_string_length_unit": "utf8_bytes",
    "schema_max_length_unit": "unicode_code_points",
    "schema_length_keywords_are_non_authoritative_approximations": True,
}

SCHEMA_AUTHORITY = {
    "delivery": "strict_pure_contract_only",
    "assessment_mode": "authority_neutral_declared_knowledge_update_only",
    **ASSESSMENT_CONSTANTS,
    "positive_result": RESULT,
    "attestations": [],
}

SCHEMA_PROPOSAL = {
    "records": "exact reachable closure from every mutation after Claim",
    "mutations": "target_aggregate_id UTF-8 sorted unique create-or-supersede declarations",
    "create": "sequence one, null before Claim, and no supersedes",
    "supersede": (
        "exact immediate predecessor with stable ADR-0054 semantic identity and "
        "shadow lifecycle only"
    ),
    "knowledge_scope_hash": "opaque_declared_binding_not_recomputed",
    "current_head_or_conflict_lookup": "none",
    "apply_or_persistence": "none",
}

DETECTOR = {
    "argv": ["python3", CHECKER_RELATIVE, "repo_root",
             "knowledge_update_proposal_assessment_request",
             "knowledge_update_proposal_assessment"],
    "positive": "test_all_five_domain_separated_hashes_are_frozen",
    "negative": "test_authority_escalation_and_reassembly_drift_fail_closed",
}

SKILL_MARKERS = [
    "forgeos.knowledge-update-proposal/v1",
    "authority_neutral_declared_knowledge_update_only",
    "ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY",
    "knowledge.propose",
    "current_knowledge_state=not_evaluated",
    "knowledge_adoption_attestation=false",
    "Accepted contract-only slice",
]

PROMOTION_MARKERS = {
    DECISION_RELATIVE: "- Status: Accepted",
    ".agent/DECISIONS.md": "D33 KnowledgeUpdateProposal v1 contract-only（2026-08-12）",
    ".agent/ROADMAP.md": (
        "DONE — Wave 0F-B–3b-6 KnowledgeUpdateProposal v1 contract-only"
    ),
    ".agent/CURRENT_SPRINT.md": (
        "Sprint 109（✅ DONE；KnowledgeUpdateProposal v1 contract-only）— ADR 0061"
    ),
    "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md": (
        "| DONE | KnowledgeUpdateProposal v1 contract-only wire (ADR 0061) |"
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
    if data.get("knowledge_update_proposal") != KNOWLEDGE_UPDATE_PROPOSAL:
        issues.append(f"{path}: KnowledgeUpdateProposal contract-only boundary drifted")
    scope = _mapping(data.get("scope"))
    expected = ["ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
                "TransitionReceipt"]
    if scope.get("shipped_contract_only_kinds") != expected:
        issues.append(f"{path}: contract-only kinds must remain {expected!r}")
    if "KnowledgeUpdateProposal" in (scope.get("shipped_kinds") or []):
        issues.append(f"{path}: KnowledgeUpdateProposal cannot be a shipped runtime kind")
    if scope.get("planned_kinds") != []:
        issues.append(f"{path}: planned kinds must be empty after the v1 wire freeze")
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
        return [f"{path}: cannot validate KnowledgeUpdateProposal Schema: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != SCHEMA_SHA256:
        issues.append(f"{path}: KnowledgeUpdateProposal Schema pin drifted")
    expected = {
        "$id": "https://forgeos.dev/contracts/knowledge-update-proposal-v1.schema.json",
        "x-forgeos-canonicalization": SCHEMA_CANONICALIZATION,
        "x-forgeos-limits": SCHEMA_LIMITS,
        "x-forgeos-authority-semantics": SCHEMA_AUTHORITY,
        "x-forgeos-proposal-semantics": SCHEMA_PROPOSAL,
    }
    issues.extend(f"{path}: {field} drifted" for field, value in expected.items()
                  if schema.get(field) != value)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        issues.append(f"{path}: KnowledgeUpdateProposal golden envelope must be closed")
    return issues


def fixture_issues(repo_root):
    path = repo_root / FIXTURE_RELATIVE
    try:
        raw, fixture = _load_json(repo_root, FIXTURE_RELATIVE, 8388608)
    except (OSError, ContractError, UnicodeDecodeError, ValueError,
            json.JSONDecodeError) as error:
        return [f"{path}: cannot validate KnowledgeUpdateProposal fixture: {error}"]
    issues = []
    if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA256:
        issues.append(f"{path}: KnowledgeUpdateProposal fixture pin drifted")
    fields = {"assessment_request", "expected_artifact_resources",
              "expected_assessment", "expected_capability_grant_ref",
              "knowledge_update_proposal"}
    if not isinstance(fixture, dict) or set(fixture) != fields:
        return issues + [f"{path}: KnowledgeUpdateProposal fixture root fields drifted"]
    proposal = _mapping(fixture.get("knowledge_update_proposal"))
    request = _mapping(fixture.get("assessment_request"))
    assessment = _mapping(fixture.get("expected_assessment"))
    observed = {
        "record_set": proposal.get("record_set_sha256"),
        "proposal": proposal.get("proposal_sha256"),
        "target": request.get("expected_target_sha256"),
        "request": request.get("request_sha256"),
        "assessment": assessment.get("assessment_sha256"),
    }
    issues.extend(f"{path}: {field} golden hash drifted"
                  for field, value in GOLDEN_HASHES.items()
                  if observed.get(field) != value)
    expected = {**ASSESSMENT_CONSTANTS,
                "assessment_mode": KNOWLEDGE_UPDATE_PROPOSAL["mode"],
                "result": RESULT}
    issues.extend(f"{path}: expected_assessment.{field} drifted"
                  for field, value in expected.items()
                  if assessment.get(field) != value)
    return issues


def detector_issues(agent_root):
    from engineering_detector_check import detector_index
    detector = detector_index(agent_root, "engineering/detectors.yml").get(
        "governance.knowledge_update_proposal_contract")
    if not isinstance(detector, dict):
        return ["KnowledgeUpdateProposal contract-only detector is missing"]
    issues = []
    if detector.get("state") != "shadow" or detector.get("fail_closed") is not True:
        issues.append("KnowledgeUpdateProposal detector must remain shadow and fail closed")
    invocation = {"owner": "operator", "adapter":
                  "standalone.knowledgeUpdateProposalContract",
                  "acceptance_criterion": None, "load_bearing": False}
    if detector.get("invocation") != invocation:
        issues.append("KnowledgeUpdateProposal detector cannot become load-bearing authority")
    implementation = _mapping(detector.get("implementation"))
    if implementation.get("argv") != DETECTOR["argv"]:
        issues.append("KnowledgeUpdateProposal detector requires exact request/assessment arguments")
    tests = _mapping(detector.get("tests"))
    for polarity in ("positive", "negative"):
        if _mapping(tests.get(polarity)).get("contains") != DETECTOR[polarity]:
            issues.append(f"KnowledgeUpdateProposal detector {polarity} test drifted")
    return issues


def route_issues(repo_root):
    path = repo_root / ".agent/engineering/context-routes.yml"
    try:
        import yaml
        data = yaml.safe_load(read_bounded_file(path, label=str(path), max_bytes=524288))
    except (ImportError, OSError, ContractError, UnicodeDecodeError, ValueError) as error:
        return [f"{path}: cannot validate KnowledgeUpdateProposal route: {error}"]
    routes = data.get("routes") if isinstance(data, dict) else []
    route = next((item for item in routes or [] if item.get("id") == "governance"), {})
    includes = {item.get("ref"): item for item in route.get("include", [])}
    expected = {"ref": SCHEMA_RELATIVE, "lane": "trusted_context",
                "required": True, "max_bytes": 131072}
    return [] if includes.get(SCHEMA_RELATIVE) == expected else [
        f"{path}: KnowledgeUpdateProposal schema route must be required trusted_context/131072"]


def skill_issues(repo_root):
    path = repo_root / SKILL_RELATIVE
    try:
        content = read_bounded_file(path, label=SKILL_RELATIVE).decode("utf-8")
    except (OSError, ContractError, UnicodeDecodeError) as error:
        return [f"{path}: cannot validate KnowledgeUpdateProposal Skill: {error}"]
    return [f"{path}: missing KnowledgeUpdateProposal marker {marker!r}"
            for marker in SKILL_MARKERS if marker not in content]


def _promotion_fact_present(repo_root):
    for relative, marker in PROMOTION_MARKERS.items():
        if relative == DECISION_RELATIVE:
            continue
        path = repo_root / relative
        if not path.exists():
            continue
        try:
            content = read_bounded_file(path, label=relative,
                                        max_bytes=2097152).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError):
            return True
        if marker in content:
            return True
    return False


def promotion_issues(repo_root, *, optional=False):
    if optional and not _promotion_fact_present(repo_root):
        return []
    issues = []
    for relative, marker in PROMOTION_MARKERS.items():
        path = repo_root / relative
        try:
            content = read_bounded_file(path, label=relative,
                                        max_bytes=2097152).decode("utf-8")
        except (OSError, ContractError, UnicodeDecodeError) as error:
            issues.append(f"{path}: cannot validate ADR-0061 promotion: {error}")
            continue
        if marker not in content:
            issues.append(f"{path}: missing accepted ADR-0061 marker {marker!r}")
    return issues


def integration_issues(data, path, repo_root, agent_root):
    issues = registry_issues(data, path)
    issues.extend(schema_issues(repo_root))
    issues.extend(fixture_issues(repo_root))
    issues.extend(detector_issues(agent_root))
    issues.extend(route_issues(repo_root))
    issues.extend(skill_issues(repo_root))
    issues.extend(promotion_issues(repo_root, optional=True))
    return issues
