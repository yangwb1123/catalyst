#!/usr/bin/env python3
"""Validate one source-resolved BackendDecisionPackage without granting approval."""
from pathlib import Path

from backend_decision_contract import (
    DIMENSION_PROOF_TYPES,
    DIMENSION_OWNERS,
    FORBIDDEN_DECISION_KEYS,
    IRREVERSIBLE_KINDS,
    MATERIALITY_RANK,
    POLICY_SHA256,
    READINESS_DIMENSIONS,
    READINESS_DECISION_DEPENDENCIES,
    READINESS_PROOF_TYPES,
    SCHEMA_SHA256,
    TRIGGER_DIMENSIONS,
    TRIGGER_FLOORS,
    TRIGGER_IRREVERSIBLE_BINDINGS,
    TRIGGERS,
    WORKFLOW_RANK,
)
from backend_evidence_check import (
    DIGEST,
    GOOD_RESULTS,
    build_evidence_index,
    proof_refs_issues,
    subject_proof_issues,
)
from engineering_check_support import repo_path_issue, unique_id_issues, unknown_field_issues


PACKAGE_FIELDS = {
    "api_version", "task_id", "source_revision", "source_tree_sha256",
    "context_sha256", "requirements_sha256", "policy_sha256", "schema_sha256",
    "principals", "materiality", "workflow_profile", "change_kinds", "applicability", "review",
    "decisions", "readiness", "assumptions", "evidence", "residual_risks",
}
DECISION_FIELDS = {
    "id", "status", "facts", "decision", "alternatives", "rationale",
    "proof_refs", "open_questions", "residual_risks", "decision_kinds", "reversibility",
    "migration_cost", "blast_radius", "adr_ref", "reviewer_id", "revisit_trigger",
}


def _shape(value, required, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, required, label)
    missing = set(required) - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _strings(value, label, *, non_empty=False, unique=False, maximum=None):
    if not isinstance(value, list) or (non_empty and not value):
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list"]
    issues = []
    if not all(isinstance(item, str) and item.strip() for item in value):
        issues.append(f"{label}: values must be non-empty strings")
    if unique and all(isinstance(item, str) for item in value) and len(value) != len(set(value)):
        issues.append(f"{label}: values must be unique")
    if maximum is not None and len(value) > maximum:
        issues.append(f"{label}: exceeds maximum {maximum}")
    return issues


def _forbidden_key_issues(node, label):
    issues = []
    if isinstance(node, dict):
        for key, value in node.items():
            if key in FORBIDDEN_DECISION_KEYS:
                issues.append(f"{label}: forbidden completion-authority field {key!r}")
            issues.extend(_forbidden_key_issues(value, label))
    elif isinstance(node, list):
        for value in node:
            issues.extend(_forbidden_key_issues(value, label))
    return issues


def _bounded_text(record, fields, label, *, required=False):
    issues = []
    for field, maximum in fields.items():
        value = record.get(field)
        if not isinstance(value, str) or len(value) > maximum or (required and not value.strip()):
            issues.append(f"{label}.{field}: must be {'non-empty ' if required else ''}string <= {maximum} characters")
    return issues


def _claim_issues(claim, label, evidence, dimension):
    fields = {"claim_type", "statement", "evidence_id", "confidence"}
    issues = _shape(claim, fields, label)
    if not isinstance(claim, dict):
        return issues
    issues.extend(_bounded_text(claim, {"statement": 4096, "evidence_id": 128}, label, required=True))
    claim_type, confidence = claim.get("claim_type"), claim.get("confidence")
    if claim_type not in ("fact", "inference"):
        issues.append(f"{label}.claim_type: invalid")
    if not isinstance(confidence, (int, float)) or isinstance(confidence, bool) or not 0 <= confidence <= 1:
        issues.append(f"{label}.confidence: must be within 0..1")
    elif claim_type == "fact" and confidence != 1:
        issues.append(f"{label}: confirmed fact requires confidence 1.0")
    elif claim_type == "inference" and confidence in {0, 1}:
        issues.append(f"{label}: inference confidence must be strictly between 0 and 1")
    evidence_id = claim.get("evidence_id")
    if isinstance(evidence_id, str):
        issues.extend(subject_proof_issues([evidence_id], f"{label}.evidence_id", evidence,
                                           "decision", dimension, {"source_fact"}))
    else:
        issues.append(f"{label}.evidence_id: unknown evidence id")
    return issues


def _decision_issues(record, label, evidence):
    issues = _shape(record, DECISION_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    status = record.get("status")
    if status not in ("addressed", "not_applicable", "blocked"):
        issues.append(f"{label}.status: invalid")
    for field, maximum in (("alternatives", 32), ("open_questions", 64), ("residual_risks", 64)):
        issues.extend(_strings(record.get(field), f"{label}.{field}", maximum=maximum))
    dimension = record.get("id")
    required = DIMENSION_PROOF_TYPES.get(dimension, set()) \
        if status == "addressed" and isinstance(dimension, str) else set()
    if status == "not_applicable":
        required = {"applicability_assessment"}
    if status in ("addressed", "not_applicable"):
        issues.extend(subject_proof_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence,
                                           "decision", dimension, required))
    else:
        issues.extend(proof_refs_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence))
    facts = record.get("facts")
    if not isinstance(facts, list) or len(facts) > 128:
        issues.append(f"{label}.facts: expected list with at most 128 items")
    else:
        for index, claim in enumerate(facts):
            issues.extend(_claim_issues(claim, f"{label}.facts[{index}]", evidence, dimension))
    issues.extend(_bounded_text(record, {"decision": 8192, "rationale": 8192,
                                        "migration_cost": 4096, "blast_radius": 4096,
                                        "adr_ref": 512, "reviewer_id": 128,
                                        "revisit_trigger": 4096}, label))
    if status == "addressed" and (not facts or not str(record.get("decision", "")).strip()
                                   or not str(record.get("rationale", "")).strip()
                                   or not record.get("alternatives")):
        issues.append(f"{label}: addressed requires facts, decision, alternative, rationale and positive proof")
    if status == "not_applicable" and (str(record.get("decision", "")).strip()
                                        or record.get("alternatives")
                                        or not str(record.get("rationale", "")).strip()
                                        or record.get("reversibility") != "not_applicable"):
        issues.append(f"{label}: not_applicable requires rationale/no decision and not_applicable reversibility")
    if status == "blocked" and (str(record.get("decision", "")).strip() or not record.get("open_questions")):
        issues.append(f"{label}: blocked requires open_questions and cannot claim a decision")
    if record.get("reversibility") not in ("high", "medium", "low", "not_applicable"):
        issues.append(f"{label}.reversibility: invalid")
    kinds = record.get("decision_kinds")
    issues.extend(_strings(kinds, f"{label}.decision_kinds", non_empty=True, unique=True, maximum=8))
    if isinstance(kinds, list) and all(isinstance(item, str) for item in kinds):
        if set(kinds) - (IRREVERSIBLE_KINDS | {"other"}):
            issues.append(f"{label}.decision_kinds: contains unknown kind")
    return issues


def _applicability_review_issues(package, evidence):
    issues = []
    principals = package.get("principals")
    principal_fields = {"package_author_id", "implementer_id"}
    issues.extend(_shape(principals, principal_fields, "BackendDecisionPackage.principals"))
    if isinstance(principals, dict):
        issues.extend(_bounded_text(principals, {"package_author_id": 128, "implementer_id": 128},
                                    "BackendDecisionPackage.principals", required=True))
    applicability = package.get("applicability")
    app_fields = {"status", "reason", "source_evidence_ids", "reviewer_id", "reviewer_independent"}
    issues.extend(_shape(applicability, app_fields, "BackendDecisionPackage.applicability"))
    if isinstance(applicability, dict):
        status = applicability.get("status")
        if status not in ("required", "skip_requested"):
            issues.append("BackendDecisionPackage.applicability.status: invalid")
        refs = applicability.get("source_evidence_ids")
        if status == "skip_requested":
            issues.extend(subject_proof_issues(refs,
                                               "BackendDecisionPackage.applicability.source_evidence_ids",
                                               evidence, "applicability", package.get("task_id"),
                                               {"applicability_assessment"}))
        else:
            issues.extend(proof_refs_issues(refs,
                                            "BackendDecisionPackage.applicability.source_evidence_ids",
                                            evidence))
        if status == "skip_requested" and (not str(applicability.get("reason", "")).strip()
                                             or not str(applicability.get("reviewer_id", "")).strip()
                                             or applicability.get("reviewer_independent") is not True):
            issues.append("BackendDecisionPackage.applicability: skip requires reason, evidence and independent reviewer")
    review = package.get("review")
    issues.extend(_shape(review, {"reviewer_id", "independent", "evidence_ids"}, "BackendDecisionPackage.review"))
    if isinstance(review, dict):
        reviewer_id = review.get("reviewer_id")
        if not isinstance(reviewer_id, str) or not reviewer_id.strip() or review.get("independent") is not True:
            issues.append("BackendDecisionPackage.review: requires a named independent reviewer")
        else:
            if isinstance(principals, dict) and reviewer_id in principals.values():
                issues.append("BackendDecisionPackage.review: reviewer must differ from author and implementer")
            issues.extend(subject_proof_issues(review.get("evidence_ids"),
                                               "BackendDecisionPackage.review.evidence_ids",
                                               evidence, "review", reviewer_id,
                                               {"independent_review"}))
        if isinstance(applicability, dict) and applicability.get("status") == "skip_requested":
            if applicability.get("reviewer_id") != reviewer_id:
                issues.append("BackendDecisionPackage.applicability: skip reviewer must match package review")
    return issues


def _readiness_issues(records, evidence, decisions):
    id_issues, ids = unique_id_issues(records, "BackendDecisionPackage", "readiness")
    issues = list(id_issues)
    if ids != set(READINESS_DIMENSIONS):
        issues.append("BackendDecisionPackage: readiness must cover every canonical dimension exactly once")
    fields = {"id", "result", "rationale", "proof_refs"}
    for index, record in enumerate(records if isinstance(records, list) else []):
        label = f"BackendDecisionPackage.readiness[{index}]"
        issues.extend(_shape(record, fields, label))
        if not isinstance(record, dict):
            continue
        result = record.get("result")
        if result not in ("ready", "not_ready", "not_applicable"):
            issues.append(f"{label}.result: invalid")
        if not isinstance(record.get("rationale"), str) or not record["rationale"].strip():
            issues.append(f"{label}.rationale: must be non-empty")
        if result in ("ready", "not_applicable"):
            readiness_id = record.get("id")
            required = READINESS_PROOF_TYPES.get(readiness_id, set()) \
                if result == "ready" and isinstance(readiness_id, str) else {
                "applicability_assessment"
            }
            issues.extend(subject_proof_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence,
                                               "readiness", record.get("id"), required))
            if result == "ready":
                dependencies = READINESS_DECISION_DEPENDENCIES.get(readiness_id, set()) \
                    if isinstance(readiness_id, str) else set()
                unresolved = sorted(item for item in dependencies
                                    if decisions.get(item, {}).get("status") != "addressed")
                if unresolved:
                    issues.append(f"{label}: ready requires addressed decisions {unresolved}")
        else:
            issues.extend(proof_refs_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence))
    return issues


def _assumption_issues(record, label, evidence, decisions, readiness):
    fields = {"id", "statement", "confidence", "impact_if_wrong", "reversibility",
              "affected_dimensions", "verification_plan", "proof_refs", "status"}
    issues = _shape(record, fields, label)
    if not isinstance(record, dict):
        return issues
    issues.extend(_bounded_text(record, {"id": 128, "statement": 4096,
                                        "verification_plan": 4096}, label, required=True))
    for field in ("confidence", "impact_if_wrong", "reversibility"):
        value = record.get(field)
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not 0 <= value <= 1:
            issues.append(f"{label}.{field}: must be within 0..1")
    affected = record.get("affected_dimensions")
    issues.extend(_strings(affected, f"{label}.affected_dimensions", non_empty=True, unique=True))
    if isinstance(affected, list) and all(isinstance(item, str) for item in affected) and set(affected) - set(DIMENSION_OWNERS):
        issues.append(f"{label}.affected_dimensions: contains unknown dimension")
    status = record.get("status")
    if status not in ("unverified", "accepted", "verified", "rejected"):
        issues.append(f"{label}.status: invalid")
    if status in ("verified", "rejected"):
        issues.extend(subject_proof_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence,
                                           "assumption", record.get("id"), {"assumption_verification"}))
    else:
        issues.extend(proof_refs_issues(record.get("proof_refs"), f"{label}.proof_refs", evidence))
    values = [record.get(field) for field in ("confidence", "impact_if_wrong", "reversibility")]
    risk = (1 - values[0]) * values[1] * (1 - values[2]) if all(isinstance(v, (int, float)) for v in values) else 1
    if status == "rejected" or (status in ("unverified", "accepted") and risk >= 0.25):
        for dimension in affected if isinstance(affected, list) else []:
            if not isinstance(dimension, str):
                continue
            if decisions.get(dimension, {}).get("status") != "blocked" and readiness.get(dimension) != "not_ready":
                issues.append(f"{label}: risky assumption requires {dimension!r} blocked or not_ready")
    return issues


def _risk_issues(record, label, evidence):
    fields = {"id", "severity", "statement", "mitigation", "proof_refs"}
    issues = _shape(record, fields, label)
    if not isinstance(record, dict):
        return issues
    issues.extend(_bounded_text(record, {"id": 128, "statement": 4096, "mitigation": 4096}, label, required=True))
    if record.get("severity") not in ("low", "medium", "high", "critical"):
        issues.append(f"{label}.severity: invalid")
    refs = record.get("proof_refs")
    issues.extend(proof_refs_issues(refs, f"{label}.proof_refs", evidence))
    if isinstance(refs, list) and refs and all(isinstance(ref, str) for ref in refs):
        issues.extend(subject_proof_issues(refs, f"{label}.proof_refs", evidence,
                                           "risk", record.get("id"), set()))
    return issues


def _floor_issues(package, kinds):
    issues = []
    required_materiality = max((MATERIALITY_RANK[TRIGGER_FLOORS[k][0]] for k in kinds), default=1)
    required_workflow = max((WORKFLOW_RANK[TRIGGER_FLOORS[k][1]] for k in kinds), default=1)
    materiality = package.get("materiality") if isinstance(package.get("materiality"), str) else ""
    workflow = package.get("workflow_profile") if isinstance(package.get("workflow_profile"), str) else ""
    if MATERIALITY_RANK.get(materiality, 0) < required_materiality:
        issues.append("BackendDecisionPackage.materiality: below activated trigger floor")
    if WORKFLOW_RANK.get(workflow, 0) < required_workflow:
        issues.append("BackendDecisionPackage.workflow_profile: below activated trigger floor")
    return issues


def _package_header_issues(package, label):
    issues = _forbidden_key_issues(package, label)
    if package.get("api_version") != "forgeos.backend-decision/v1":
        issues.append(f"{label}.api_version: unsupported")
    issues.extend(_bounded_text(package, {"task_id": 128, "source_revision": 256}, label, required=True))
    for field in ("source_tree_sha256", "context_sha256", "requirements_sha256"):
        value = package.get(field)
        if not isinstance(value, str) or not DIGEST.fullmatch(value):
            issues.append(f"{label}.{field}: requires sha256:<64 lowercase hex>")
    if package.get("policy_sha256") != "sha256:" + POLICY_SHA256:
        issues.append(f"{label}.policy_sha256: does not bind the canonical policy")
    if package.get("schema_sha256") != "sha256:" + SCHEMA_SHA256:
        issues.append(f"{label}.schema_sha256: does not bind the canonical schema")
    kinds = package.get("change_kinds")
    issues.extend(_strings(kinds, f"{label}.change_kinds", non_empty=True, unique=True))
    known_kinds = [kind for kind in kinds if isinstance(kind, str) and kind in TRIGGERS] \
        if isinstance(kinds, list) else []
    if isinstance(kinds, list) and any(not isinstance(kind, str) or kind not in TRIGGERS for kind in kinds):
        issues.append(f"{label}.change_kinds: contains unknown trigger")
    issues.extend(_floor_issues(package, known_kinds))
    return issues, known_kinds


def _evidence_index(package, repo_root, label):
    issues, evidence = build_evidence_index(package, repo_root, label)
    issues.extend(_applicability_review_issues(package, evidence))
    return issues, evidence


def _decision_index(package, evidence, known_kinds, label):
    issues = []
    decisions = package.get("decisions")
    id_issues, ids = unique_id_issues(decisions, label, "decision")
    issues.extend(id_issues)
    if ids != set(DIMENSION_OWNERS):
        issues.append(f"{label}: decisions must cover every canonical dimension exactly once")
    by_id = {item.get("id"): item for item in decisions
             if isinstance(item, dict) and isinstance(item.get("id"), str)} if isinstance(decisions, list) else {}
    for index, record in enumerate(decisions if isinstance(decisions, list) else []):
        issues.extend(_decision_issues(record, f"{label}.decisions[{index}]", evidence))
    required_dimensions = set().union(*(TRIGGER_DIMENSIONS.get(kind, set()) for kind in known_kinds))
    applicability = package.get("applicability")
    skip_requested = isinstance(applicability, dict) and applicability.get("status") == "skip_requested"
    for dimension in required_dimensions:
        status = by_id.get(dimension, {}).get("status")
        if status == "not_applicable":
            issues.append(f"{label}: triggered dimension {dimension!r} cannot be not_applicable")
        if skip_requested and status != "blocked":
            issues.append(f"{label}: skip request keeps triggered dimension {dimension!r} blocked")
    return issues, by_id


def _irreversible_issues(package, decisions, evidence, repo_root, label):
    issues = []
    kinds = package.get("change_kinds") if isinstance(package.get("change_kinds"), list) else []
    for trigger, (dimension, decision_kind) in TRIGGER_IRREVERSIBLE_BINDINGS.items():
        record = decisions.get(dimension, {})
        if trigger in kinds and decision_kind not in (record.get("decision_kinds") or []):
            issues.append(f"{label}: trigger {trigger!r} requires decision kind {decision_kind!r} in {dimension!r}")
    reviewer = package.get("review", {}).get("reviewer_id") if isinstance(package.get("review"), dict) else None
    for dimension, record in decisions.items():
        decision_kinds = record.get("decision_kinds") if isinstance(record.get("decision_kinds"), list) else []
        known_kinds = {item for item in decision_kinds if isinstance(item, str)}
        controlled = record.get("reversibility") == "low" or bool(known_kinds & IRREVERSIBLE_KINDS)
        if not controlled:
            continue
        controls = ("migration_cost", "blast_radius", "adr_ref", "reviewer_id", "revisit_trigger")
        if len(record.get("alternatives") or []) < 2 or any(not str(record.get(field, "")).strip() for field in controls):
            issues.append(f"{label}.decisions[{dimension}]: irreversible control fields are incomplete")
            continue
        if record.get("reviewer_id") != reviewer:
            issues.append(f"{label}.decisions[{dimension}]: reviewer must match package independent reviewer")
        adr_ref = record.get("adr_ref")
        path_issue = repo_path_issue(repo_root, adr_ref, f"{label}.decisions[{dimension}].adr_ref")
        if path_issue or not (repo_root / adr_ref).is_file():
            issues.append(path_issue or f"{label}.decisions[{dimension}].adr_ref: must be a regular file")
            continue
        refs = record.get("proof_refs") if isinstance(record.get("proof_refs"), list) else []
        adr_bound = any(evidence.get(ref, {}).get("locator") == adr_ref
                        and "adr" in evidence.get(ref, {}).get("proof_types", []) for ref in refs if isinstance(ref, str))
        if not adr_bound:
            issues.append(f"{label}.decisions[{dimension}]: ADR must be digest-bound by an adr proof_ref")
    return issues


def _assumptions_issues(package, evidence, decisions, readiness, label):
    issues = []
    assumptions = package.get("assumptions")
    if not isinstance(assumptions, list) or len(assumptions) > 128:
        return [f"{label}.assumptions: expected list with at most 128 items"]
    assumption_ids = set()
    for index, record in enumerate(assumptions):
        issues.extend(_assumption_issues(record, f"{label}.assumptions[{index}]",
                                         evidence, decisions, readiness))
        assumption_id = record.get("id") if isinstance(record, dict) else None
        if isinstance(assumption_id, str) and assumption_id in assumption_ids:
            issues.append(f"{label}: duplicate assumption id {assumption_id!r}")
        if isinstance(assumption_id, str):
            assumption_ids.add(assumption_id)
    open_assumptions = {str(item.get("statement", "")).strip().casefold() for item in assumptions
                        if isinstance(item, dict) and item.get("status") in ("unverified", "accepted")}
    for record in decisions.values():
        facts = record.get("facts", []) if isinstance(record, dict) else []
        facts = facts if isinstance(facts, list) else []
        for claim in facts:
            statement = str(claim.get("statement", "")).strip().casefold() if isinstance(claim, dict) else ""
            if isinstance(claim, dict) and claim.get("claim_type") == "fact" and statement in open_assumptions:
                issues.append(f"{label}: an open assumption cannot also be a confirmed fact")
    return issues


def _residual_risk_issues(package, evidence, label):
    issues = []
    risks = package.get("residual_risks")
    if not isinstance(risks, list) or len(risks) > 128:
        return [f"{label}.residual_risks: expected list with at most 128 items"]
    risk_ids = set()
    for index, record in enumerate(risks):
        issues.extend(_risk_issues(record, f"{label}.residual_risks[{index}]", evidence))
        risk_id = record.get("id") if isinstance(record, dict) else None
        if isinstance(risk_id, str) and risk_id in risk_ids:
            issues.append(f"{label}: duplicate risk id {risk_id!r}")
        if isinstance(risk_id, str):
            risk_ids.add(risk_id)
    return issues


def validate_backend_package(package, repo_root=None, label="BackendDecisionPackage"):
    issues = _shape(package, PACKAGE_FIELDS, label)
    if not isinstance(package, dict):
        return issues
    repo_root = Path(repo_root or ".").resolve()
    header_issues, known_kinds = _package_header_issues(package, label)
    evidence_issues, evidence = _evidence_index(package, repo_root, label)
    decision_issues, by_id = _decision_index(package, evidence, known_kinds, label)
    issues.extend(header_issues + evidence_issues + decision_issues)
    issues.extend(_irreversible_issues(package, by_id, evidence, repo_root, label))
    readiness_records = package.get("readiness")
    issues.extend(_readiness_issues(readiness_records, evidence, by_id))
    readiness = {item.get("id"): item.get("result") for item in readiness_records
                 if isinstance(item, dict) and isinstance(item.get("id"), str)} \
        if isinstance(readiness_records, list) else {}
    issues.extend(_assumptions_issues(package, evidence, by_id, readiness, label))
    issues.extend(_residual_risk_issues(package, evidence, label))
    return issues


def classify_backend_package(package):
    """Classify a valid package without producing approval/completion authority."""
    if not isinstance(package, dict):
        return "INVALID"
    applicability = package.get("applicability")
    if not isinstance(applicability, dict):
        return "INVALID"
    if isinstance(applicability, dict) and applicability.get("status") == "skip_requested":
        return "SKIP_REVIEW_REQUIRED"
    if not all(isinstance(package.get(field), list)
               for field in ("decisions", "readiness", "residual_risks")):
        return "INVALID"
    decisions, readiness, risks = (package["decisions"], package["readiness"], package["residual_risks"])
    if any(isinstance(item, dict) and item.get("status") == "blocked" for item in decisions):
        return "VALID_BLOCKED"
    if any(isinstance(item, dict) and item.get("result") == "not_ready" for item in readiness):
        return "VALID_NOT_READY"
    if any(isinstance(item, dict) and item.get("severity") == "critical" for item in risks):
        return "VALID_NOT_READY"
    return "STRUCTURALLY_VALID"
