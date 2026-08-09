#!/usr/bin/env python3
"""Validate a FrontendDesignPackage without granting completion authority."""
import math

from engineering_check_support import unique_id_issues, unknown_field_issues
from .contract import (
    ASSUMPTION_BLOCK_THRESHOLD,
    DECISION_KINDS,
    DIMENSION_OWNERS,
    DIMENSION_PROOF_TYPES,
    FORBIDDEN_KEYS,
    MATERIALITY_RANK,
    POLICY_SHA256,
    PROFILE_SHA256,
    READINESS_DECISION_DEPENDENCIES,
    READINESS_DIMENSIONS,
    READINESS_PROOF_TYPES,
    SCHEMA_SHA256,
    TRIGGER_DIMENSIONS,
    TRIGGER_FLOORS,
    TRIGGERS,
    WORKFLOW_RANK,
)
from .evidence import (
    DIGEST,
    build_evidence_indexes,
    claim_refs_issues,
    subject_claim_issues,
)
from .governance import controlled_decision_issues, experience_issues, profile_override_issues
from .model import (
    classification_issues,
    flow_issues,
    state_model_issues,
    verification_case_issues,
)

PACKAGE_FIELDS = {
    "api_version", "task_id", "source_revision", "source_tree_sha256",
    "context_sha256", "requirements_sha256", "policy_sha256",
    "profile_catalog_sha256", "schema_sha256", "principals", "materiality",
    "workflow_profile", "change_kinds", "applicability", "review",
    "classification", "profile_overrides", "flows", "state_model", "decisions", "readiness",
    "verification_cases", "assumptions", "evidence_artifacts", "proof_claims",
    "residual_risks",
}
DECISION_FIELDS = {
    "id", "status", "facts", "decision", "alternatives", "rationale",
    "proof_claim_ids", "open_questions", "residual_risks", "reversibility",
    "decision_kinds", "migration_cost", "blast_radius", "adr_ref",
    "reviewer_id", "revisit_trigger",
}
ASSUMPTION_FIELDS = {
    "id", "statement", "confidence", "impact_if_wrong", "reversibility",
    "affected_dimensions", "verification_plan", "proof_claim_ids", "status",
}
RISK_FIELDS = {"id", "severity", "statement", "mitigation", "proof_claim_ids"}


def _shape(value, fields, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, fields, label)
    missing = fields - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _text(value, label, maximum=4096, *, required=False):
    if not isinstance(value, str) or len(value) > maximum or (required and not value.strip()):
        return [f"{label}: must be {'non-empty ' if required else ''}string <= {maximum} characters"]
    return []


def _strings(value, label, *, non_empty=False, maximum=128):
    if not isinstance(value, list) or (non_empty and not value) or len(value) > maximum:
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list <= {maximum} items"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    return [f"{label}: values must be unique"] if len(value) != len(set(value)) else []


def _forbidden_issues(node, label="FrontendDesignPackage"):
    issues, stack, visited = [], [(node, ())], 0
    while stack:
        value, ancestors = stack.pop()
        if not isinstance(value, (dict, list)):
            continue
        identity = id(value)
        if identity in ancestors:
            issues.append(f"{label}: cyclic container is not allowed")
            continue
        descendants = ancestors + (identity,)
        visited += 1
        if visited > 100000:
            return issues + [f"{label}: nested value count exceeds 100000"]
        if isinstance(value, dict):
            for key, child in value.items():
                if isinstance(key, str) and key in FORBIDDEN_KEYS:
                    issues.append(f"{label}: forbidden completion-authority field {key!r}")
                stack.append((child, descendants))
        else:
            stack.extend((child, descendants) for child in value)
    return issues


def _principal_review_issues(package, claims):
    issues, principals = [], package.get("principals")
    issues.extend(_shape(principals, {"package_author_id", "implementer_id"},
                         "FrontendDesignPackage.principals"))
    if isinstance(principals, dict):
        for field in ("package_author_id", "implementer_id"):
            issues.extend(_text(principals.get(field), f"FrontendDesignPackage.principals.{field}", 128,
                                required=True))
    review = package.get("review")
    issues.extend(_shape(review, {"reviewer_id", "independent", "proof_claim_ids"},
                         "FrontendDesignPackage.review"))
    if not isinstance(review, dict):
        return issues
    reviewer = review.get("reviewer_id")
    issues.extend(_text(reviewer, "FrontendDesignPackage.review.reviewer_id", 128, required=True))
    if review.get("independent") is not True:
        issues.append("FrontendDesignPackage.review.independent: must be true")
    if isinstance(reviewer, str) and isinstance(principals, dict) \
            and any(reviewer == value for value in principals.values() if isinstance(value, str)):
        issues.append("FrontendDesignPackage.review: reviewer must differ from author and implementer")
    issues.extend(subject_claim_issues(review.get("proof_claim_ids"),
                                       "FrontendDesignPackage.review.proof_claim_ids", claims,
                                       "review", reviewer, {"independent_review"}))
    applicability = package.get("applicability")
    if isinstance(applicability, dict) and applicability.get("status") == "skip_requested" \
            and applicability.get("reviewer_id") != reviewer:
        issues.append("FrontendDesignPackage.applicability: skip reviewer must match package review")
    return issues


def _applicability_issues(package, claims):
    record, issues = package.get("applicability"), []
    fields = {"status", "reason", "source_claim_ids", "reviewer_id", "reviewer_independent"}
    issues.extend(_shape(record, fields, "FrontendDesignPackage.applicability"))
    if not isinstance(record, dict):
        return issues
    status = record.get("status")
    if not isinstance(status, str) or status not in {"required", "skip_requested"}:
        issues.append("FrontendDesignPackage.applicability.status: invalid")
    refs = record.get("source_claim_ids")
    if status == "skip_requested":
        issues.extend(subject_claim_issues(refs, "FrontendDesignPackage.applicability.source_claim_ids",
                                           claims, "applicability", package.get("task_id"),
                                           {"applicability_assessment"}))
        required = all((isinstance(record.get("reason"), str) and record["reason"].strip(),
                        isinstance(record.get("reviewer_id"), str) and record["reviewer_id"].strip(),
                        record.get("reviewer_independent") is True))
        if not required:
            issues.append("FrontendDesignPackage.applicability: skip requires reason and independent review")
    else:
        issues.extend(claim_refs_issues(refs, "FrontendDesignPackage.applicability.source_claim_ids", claims))
    return issues


def _claim_fact_issues(record, label, claims, dimension):
    fields, issues = {"claim_type", "statement", "proof_claim_id", "confidence"}, []
    issues.extend(_shape(record, fields, label))
    if not isinstance(record, dict):
        return issues
    issues.extend(_text(record.get("statement"), f"{label}.statement", 4096, required=True))
    kind, confidence = record.get("claim_type"), record.get("confidence")
    if kind != "fact":
        issues.append(f"{label}.claim_type: decision premises must be facts; use AssumptionRecord for inference")
    if not isinstance(confidence, (int, float)) or isinstance(confidence, bool) \
            or not math.isfinite(confidence) or not 0 <= confidence <= 1:
        issues.append(f"{label}.confidence: must be within 0..1")
    elif kind == "fact" and confidence != 1:
        issues.append(f"{label}: fact requires confidence 1.0")
    if kind == "fact":
        issues.extend(subject_claim_issues([record.get("proof_claim_id")], f"{label}.proof_claim_id", claims,
                                           "decision", dimension, {"source_fact"}))
    return issues


def _decision_issues(record, label, claims, repo_root):
    issues = _shape(record, DECISION_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    dimension, status = record.get("id"), record.get("status")
    if not isinstance(status, str) or status not in {"addressed", "not_applicable", "blocked"}:
        issues.append(f"{label}.status: invalid")
    for field, maximum in (("alternatives", 32), ("open_questions", 64),
                           ("residual_risks", 64), ("decision_kinds", 8)):
        issues.extend(_strings(record.get(field), f"{label}.{field}",
                               non_empty=field == "decision_kinds", maximum=maximum))
    for field, maximum in (("decision", 8192), ("rationale", 8192),
                           ("migration_cost", 4096), ("blast_radius", 4096),
                           ("adr_ref", 512), ("reviewer_id", 128),
                           ("revisit_trigger", 4096)):
        issues.extend(_text(record.get(field), f"{label}.{field}", maximum))
    kinds = record.get("decision_kinds")
    if isinstance(kinds, list) and all(isinstance(item, str) for item in kinds) and set(kinds) - DECISION_KINDS:
        issues.append(f"{label}.decision_kinds: contains unknown kind")
    issues.extend(_decision_status_issues(record, label, claims, dimension))
    return issues


def _decision_status_issues(record, label, claims, dimension):
    status, issues = record.get("status"), []
    facts = record.get("facts")
    if not isinstance(facts, list) or len(facts) > 128:
        issues.append(f"{label}.facts: expected list <= 128 items")
    else:
        for index, fact in enumerate(facts):
            issues.extend(_claim_fact_issues(fact, f"{label}.facts[{index}]", claims, dimension))
    required = DIMENSION_PROOF_TYPES.get(dimension, set()) \
        if status == "addressed" and isinstance(dimension, str) else set()
    if status == "not_applicable":
        required = {"applicability_assessment"}
    if isinstance(status, str) and status in {"addressed", "not_applicable"}:
        issues.extend(subject_claim_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids",
                                           claims, "decision", dimension, required))
    else:
        issues.extend(claim_refs_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids", claims))
    if status == "addressed" and (not facts or not str(record.get("decision", "")).strip()
                                   or not record.get("alternatives") or not str(record.get("rationale", "")).strip()):
        issues.append(f"{label}: addressed requires fact, decision, alternative and rationale")
    if status == "not_applicable" and (facts or str(record.get("decision", "")).strip()
                                        or record.get("alternatives") or not str(record.get("rationale", "")).strip()
                                        or record.get("reversibility") != "not_applicable"):
        issues.append(f"{label}: not_applicable requires rationale and no fact or decision")
    if status == "blocked" and (str(record.get("decision", "")).strip() or not record.get("open_questions")):
        issues.append(f"{label}: blocked requires open_questions and cannot claim a decision")
    reversibility = record.get("reversibility")
    if not isinstance(reversibility, str) or reversibility not in {"high", "medium", "low", "not_applicable"}:
        issues.append(f"{label}.reversibility: invalid")
    return issues


def _decisions_issues(package, claims, repo_root):
    records, issues = package.get("decisions"), []
    id_issues, ids = unique_id_issues(records, "FrontendDesignPackage.decisions", "decision")
    issues.extend(id_issues)
    if ids != set(DIMENSION_OWNERS):
        issues.append("FrontendDesignPackage.decisions: must cover every canonical dimension exactly once")
    index = {}
    for position, record in enumerate(records if isinstance(records, list) else []):
        issues.extend(_decision_issues(record, f"FrontendDesignPackage.decisions[{position}]", claims, repo_root))
        if isinstance(record, dict) and isinstance(record.get("id"), str):
            index[record["id"]] = record
    for trigger in package.get("change_kinds", []) if isinstance(package.get("change_kinds"), list) else []:
        if not isinstance(trigger, str):
            continue
        for dimension in TRIGGER_DIMENSIONS.get(trigger, set()):
            if index.get(dimension, {}).get("status") == "not_applicable":
                issues.append(f"FrontendDesignPackage: triggered dimension {dimension!r} cannot be not_applicable")
    return issues, index


def _assumption_issues(records, claims):
    issues, risky = [], set()
    id_issues, ids = unique_id_issues(records, "FrontendDesignPackage.assumptions", "assumption")
    issues.extend(id_issues)
    if not isinstance(records, list) or len(records) > 128:
        return issues + ["FrontendDesignPackage.assumptions: expected list <= 128 items"], {}, set()
    assumption_index = {}
    for position, record in enumerate(records):
        label = f"FrontendDesignPackage.assumptions[{position}]"
        issues.extend(_shape(record, ASSUMPTION_FIELDS, label))
        if not isinstance(record, dict):
            continue
        issues.extend(_assumption_record_issues(record, label, claims))
        if isinstance(record.get("id"), str):
            assumption_index[record["id"]] = record
        values = (record.get("confidence"), record.get("impact_if_wrong"), record.get("reversibility"))
        if all(isinstance(value, (int, float)) and not isinstance(value, bool)
               and math.isfinite(value) for value in values):
            score = (1 - values[0]) * values[1] * (1 - values[2])
            if score >= ASSUMPTION_BLOCK_THRESHOLD and record.get("status") != "verified":
                dimensions = record.get("affected_dimensions")
                if isinstance(dimensions, list):
                    risky.update(item for item in dimensions if isinstance(item, str))
    return issues, assumption_index, risky


def _assumption_record_issues(record, label, claims):
    issues = []
    for field in ("statement", "verification_plan"):
        issues.extend(_text(record.get(field), f"{label}.{field}", 4096, required=True))
    for field in ("confidence", "impact_if_wrong", "reversibility"):
        value = record.get(field)
        if not isinstance(value, (int, float)) or isinstance(value, bool) \
                or not math.isfinite(value) or not 0 <= value <= 1:
            issues.append(f"{label}.{field}: must be within 0..1")
    dimensions = record.get("affected_dimensions")
    issues.extend(_strings(dimensions, f"{label}.affected_dimensions", non_empty=True))
    if isinstance(dimensions, list) and all(isinstance(item, str) for item in dimensions) \
            and set(dimensions) - set(DIMENSION_OWNERS):
        issues.append(f"{label}.affected_dimensions: contains unknown dimension")
    status = record.get("status")
    if not isinstance(status, str) or status not in {"unverified", "accepted", "verified", "rejected"}:
        issues.append(f"{label}.status: invalid")
    if status == "verified":
        issues.extend(subject_claim_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids",
                                           claims, "assumption", record.get("id"),
                                           {"assumption_verification"}))
    else:
        issues.extend(claim_refs_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids", claims))
    return issues


def _readiness_issues(records, claims, decisions):
    issues, index = [], {}
    id_issues, ids = unique_id_issues(records, "FrontendDesignPackage.readiness", "readiness")
    issues.extend(id_issues)
    if ids != set(READINESS_DIMENSIONS):
        issues.append("FrontendDesignPackage.readiness: must cover every canonical dimension exactly once")
    for position, record in enumerate(records if isinstance(records, list) else []):
        label = f"FrontendDesignPackage.readiness[{position}]"
        issues.extend(_readiness_record_issues(record, label, claims, decisions))
        if isinstance(record, dict) and isinstance(record.get("id"), str):
            index[record["id"]] = record
    return issues, index


def _readiness_record_issues(record, label, claims, decisions):
    issues = _shape(record, {"id", "result", "rationale", "proof_claim_ids"}, label)
    if not isinstance(record, dict):
        return issues
    dimension, result = record.get("id"), record.get("result")
    if not isinstance(result, str) or result not in {"ready", "not_ready", "not_applicable"}:
        issues.append(f"{label}.result: invalid")
    issues.extend(_text(record.get("rationale"), f"{label}.rationale", 4096, required=True))
    required = READINESS_PROOF_TYPES.get(dimension, set()) \
        if result == "ready" and isinstance(dimension, str) else set()
    if result == "not_applicable":
        required = {"applicability_assessment"}
    if isinstance(result, str) and result in {"ready", "not_applicable"}:
        issues.extend(subject_claim_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids",
                                           claims, "readiness", dimension, required))
    else:
        issues.extend(claim_refs_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids", claims))
    if result == "ready":
        dependencies = READINESS_DECISION_DEPENDENCIES.get(dimension, set()) \
            if isinstance(dimension, str) else set()
        blocked = [item for item in dependencies
                   if decisions.get(item, {}).get("status") != "addressed"]
        if blocked:
            issues.append(f"{label}: ready has unresolved decision dependencies {sorted(blocked)}")
    return issues


def _risk_issues(records, claims):
    issues = []
    id_issues, _ = unique_id_issues(records, "FrontendDesignPackage.residual_risks", "risk")
    issues.extend(id_issues)
    if not isinstance(records, list) or len(records) > 128:
        return issues + ["FrontendDesignPackage.residual_risks: expected list <= 128 items"]
    for index, record in enumerate(records):
        label = f"FrontendDesignPackage.residual_risks[{index}]"
        issues.extend(_shape(record, RISK_FIELDS, label))
        if not isinstance(record, dict):
            continue
        for field in ("statement", "mitigation"):
            issues.extend(_text(record.get(field), f"{label}.{field}", 4096, required=True))
        if not isinstance(record.get("severity"), str) \
                or record.get("severity") not in {"low", "medium", "high", "critical"}:
            issues.append(f"{label}.severity: invalid")
        issues.extend(claim_refs_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids", claims))
    return issues


def _header_and_floor_issues(package):
    issues = []
    if package.get("api_version") != "forgeos.frontend-design/v1":
        issues.append("FrontendDesignPackage.api_version: unsupported")
    for field, maximum in (("task_id", 128), ("source_revision", 256)):
        issues.extend(_text(package.get(field), f"FrontendDesignPackage.{field}", maximum, required=True))
    for field, expected in (("policy_sha256", POLICY_SHA256),
                            ("profile_catalog_sha256", PROFILE_SHA256),
                            ("schema_sha256", SCHEMA_SHA256)):
        if package.get(field) != "sha256:" + expected:
            issues.append(f"FrontendDesignPackage.{field}: does not match canonical pin")
    for field in ("source_tree_sha256", "context_sha256", "requirements_sha256"):
        if not isinstance(package.get(field), str) or not DIGEST.fullmatch(package[field]):
            issues.append(f"FrontendDesignPackage.{field}: invalid digest")
    triggers = package.get("change_kinds")
    issues.extend(_strings(triggers, "FrontendDesignPackage.change_kinds", non_empty=True))
    if isinstance(triggers, list) and all(isinstance(item, str) for item in triggers):
        if set(triggers) - TRIGGERS:
            issues.append("FrontendDesignPackage.change_kinds: contains unknown trigger")
        minimum_materiality = max((MATERIALITY_RANK[TRIGGER_FLOORS[item][0]] for item in triggers if item in TRIGGERS), default=1)
        minimum_workflow = max((WORKFLOW_RANK[TRIGGER_FLOORS[item][1]] for item in triggers if item in TRIGGERS), default=1)
        materiality = package.get("materiality")
        workflow = package.get("workflow_profile")
        materiality_rank = MATERIALITY_RANK.get(materiality, 0) if isinstance(materiality, str) else 0
        workflow_rank = WORKFLOW_RANK.get(workflow, 0) if isinstance(workflow, str) else 0
        if materiality_rank < minimum_materiality:
            issues.append("FrontendDesignPackage.materiality: below activated trigger floor")
        if workflow_rank < minimum_workflow:
            issues.append("FrontendDesignPackage.workflow_profile: below activated trigger floor")
    return issues


def _claim_artifact_reuse_issues(claims, artifacts):
    issues, owners = [], {}
    for claim_id, claim in claims.items():
        subject = (claim.get("subject_type"), claim.get("subject_id"))
        for artifact_id in claim.get("artifact_ids", []) if isinstance(claim.get("artifact_ids"), list) else []:
            if not isinstance(artifact_id, str) or artifacts.get(artifact_id, {}).get("kind") == "source":
                continue
            prior = owners.setdefault(artifact_id, subject)
            if prior != subject:
                issues.append(f"FrontendDesignPackage.proof_claims: non-source artifact {artifact_id!r} reused across subjects")
    return issues


def _verification_readiness_issues(cases, readiness):
    kinds = {item.get("kind") for item in cases if isinstance(item, dict)} \
        if isinstance(cases, list) else set()
    issues = []
    if readiness.get("interaction_evidence", {}).get("result") == "ready" \
            and "interaction" not in kinds:
        issues.append("FrontendDesignPackage.verification_cases: interaction readiness requires an interaction case")
    if readiness.get("visual_evidence", {}).get("result") == "ready" and "capture" not in kinds:
        issues.append("FrontendDesignPackage.verification_cases: visual readiness requires a capture case")
    return issues


def _validate_frontend_package(package, repo_root):
    issues = []
    if not isinstance(package, dict):
        return ["FrontendDesignPackage: expected mapping"]
    issues.extend(unknown_field_issues(package, PACKAGE_FIELDS, "FrontendDesignPackage"))
    missing = PACKAGE_FIELDS - set(package)
    if missing:
        issues.append(f"FrontendDesignPackage: missing fields {sorted(missing)}")
    issues.extend(_forbidden_issues(package))
    issues.extend(_header_and_floor_issues(package))
    evidence_issues, artifacts, claims = build_evidence_indexes(package, repo_root)
    issues.extend(evidence_issues)
    assumption_issues, assumptions, risky_dimensions = _assumption_issues(package.get("assumptions"), claims)
    issues.extend(assumption_issues)
    issues.extend(classification_issues(package.get("classification"), claims, assumptions))
    issues.extend(profile_override_issues(package, claims))
    state_issues, state_ids, actions, high_risk = state_model_issues(package.get("state_model"), claims)
    issues.extend(state_issues)
    issues.extend(flow_issues(package.get("flows"), claims, actions, state_ids))
    raw_flows = package.get("flows")
    flow_ids = {item.get("id") for item in raw_flows if isinstance(item, dict)
                and isinstance(item.get("id"), str)} if isinstance(raw_flows, list) else set()
    issues.extend(verification_case_issues(package.get("verification_cases"), artifacts, claims,
                                           flow_ids, state_ids, package.get("source_tree_sha256"),
                                           repo_root))
    decision_issues, decisions = _decisions_issues(package, claims, repo_root)
    issues.extend(decision_issues)
    issues.extend(controlled_decision_issues(package, decisions, artifacts, claims, repo_root))
    for dimension in risky_dimensions:
        if decisions.get(dimension, {}).get("status") != "blocked":
            issues.append(f"FrontendDesignPackage: load-bearing unverified assumption must block {dimension!r}")
    readiness_issues, readiness = _readiness_issues(package.get("readiness"), claims, decisions)
    issues.extend(readiness_issues)
    issues.extend(_verification_readiness_issues(package.get("verification_cases"), readiness))
    issues.extend(_principal_review_issues(package, claims))
    issues.extend(_applicability_issues(package, claims))
    issues.extend(_risk_issues(package.get("residual_risks"), claims))
    issues.extend(experience_issues(package, high_risk))
    issues.extend(_claim_artifact_reuse_issues(claims, artifacts))
    return issues


def validate_frontend_package(package, repo_root):
    """Fail closed on malformed in-memory values as well as parsed YAML."""
    try:
        return _validate_frontend_package(package, repo_root)
    except (RecursionError, TypeError, ValueError, OverflowError) as exc:
        return [f"FrontendDesignPackage: malformed nested value ({type(exc).__name__})"]


def classify_frontend_package(package, issues):
    """Classify structure/readiness without using completion-language verdicts."""
    if issues or not isinstance(package, dict) or not PACKAGE_FIELDS <= set(package):
        return "INVALID"
    mappings = {"principals", "applicability", "review", "classification", "state_model"}
    lists = {"change_kinds", "profile_overrides", "flows", "decisions", "readiness",
             "verification_cases", "assumptions", "evidence_artifacts", "proof_claims", "residual_risks"}
    scalars = PACKAGE_FIELDS - mappings - lists
    if any(not isinstance(package.get(key), dict) for key in mappings) \
            or any(not isinstance(package.get(key), list) for key in lists) \
            or any(not isinstance(package.get(key), str) for key in scalars):
        return "INVALID"
    applicability = package.get("applicability")
    if isinstance(applicability, dict) and applicability.get("status") == "skip_requested":
        return "SKIP_REVIEW_REQUIRED"
    decisions = package.get("decisions") if isinstance(package.get("decisions"), list) else []
    if any(item.get("status") == "blocked" for item in decisions if isinstance(item, dict)):
        return "VALID_BLOCKED"
    risks = package.get("residual_risks") if isinstance(package.get("residual_risks"), list) else []
    readiness = package.get("readiness") if isinstance(package.get("readiness"), list) else []
    critical = any(item.get("severity") == "critical" for item in risks if isinstance(item, dict))
    not_ready = any(item.get("result") != "ready" for item in readiness if isinstance(item, dict))
    return "VALID_NOT_READY" if critical or not_ready else "STRUCTURALLY_VALID"
