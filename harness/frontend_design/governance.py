#!/usr/bin/env python3
"""Cross-record governance checks for a FrontendDesignPackage."""
import datetime

from engineering_check_support import repo_path_issue, unknown_field_issues
from .contract import (
    HIGH_RISK_EFFECTS,
    MATERIALITY_RANK,
    PROFILE_DEFAULTS,
    RISK_FLOORS,
    RISK_LEVELS,
    WORKFLOW_RANK,
)
from .evidence import subject_claim_issues

OVERRIDE_FIELDS = {
    "field", "default", "selected", "reason", "scope", "risk",
    "compensating_proof_claim_ids", "reviewer_id", "expires_at",
}
CONTROLLED_DECISIONS = {
    "shared_token", "shared_component", "public_interaction_contract", "auth_surface",
}


def _classification_value(package, field):
    classification = package.get("classification")
    record = classification.get(field) if isinstance(classification, dict) else None
    return record.get("value") if isinstance(record, dict) else None


def _non_empty_text(value, label, maximum=4096):
    return [] if isinstance(value, str) and value.strip() and len(value) <= maximum \
        else [f"{label}: must be non-empty string <= {maximum} characters"]


def profile_override_issues(package, claims):
    profile_id = _classification_value(package, "profile_id")
    defaults = PROFILE_DEFAULTS.get(profile_id) if isinstance(profile_id, str) else None
    records, issues, fields = package.get("profile_overrides"), [], set()
    if not isinstance(records, list) or len(records) > 2:
        return issues + ["FrontendDesignPackage.profile_overrides: expected list <= 2 items"]
    for index, record in enumerate(records):
        field = record.get("field") if isinstance(record, dict) else None
        if isinstance(field, str) and field in fields:
            issues.append(f"FrontendDesignPackage.profile_overrides: duplicate field {field!r}")
        elif isinstance(field, str):
            fields.add(field)
    deviations = {}
    if defaults is not None:
        for field, default in zip(("density", "motion_level"), defaults):
            selected = _classification_value(package, field)
            if selected != default:
                deviations[field] = (default, selected)
    if fields != set(deviations):
        issues.append("FrontendDesignPackage.profile_overrides: must exactly cover density/motion deviations")
    reviewer = package.get("review", {}).get("reviewer_id") \
        if isinstance(package.get("review"), dict) else None
    for index, record in enumerate(records):
        label = f"FrontendDesignPackage.profile_overrides[{index}]"
        issues.extend(_profile_override_record_issues(record, label, profile_id,
                                                       deviations, reviewer, claims))
    return issues


def _profile_override_record_issues(record, label, profile_id, deviations, reviewer, claims):
    if not isinstance(record, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(record, OVERRIDE_FIELDS, label)
    missing = OVERRIDE_FIELDS - set(record)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    field = record.get("field")
    if not isinstance(field, str) or field not in {"density", "motion_level"}:
        issues.append(f"{label}.field: invalid profile heuristic")
        return issues
    expected = deviations.get(field)
    if expected is None or (record.get("default"), record.get("selected")) != expected:
        issues.append(f"{label}: default/selected values do not match the pinned profile deviation")
    for name in ("reason", "scope"):
        issues.extend(_non_empty_text(record.get(name), f"{label}.{name}"))
    if not isinstance(record.get("risk"), str) or record.get("risk") not in RISK_LEVELS:
        issues.append(f"{label}.risk: invalid")
    if record.get("reviewer_id") != reviewer:
        issues.append(f"{label}.reviewer_id: must match package independent reviewer")
    try:
        expires_at = datetime.date.fromisoformat(record.get("expires_at"))
        if expires_at < datetime.date.today():
            issues.append(f"{label}.expires_at: profile override has expired")
    except (TypeError, ValueError):
        issues.append(f"{label}.expires_at: expected ISO-8601 calendar date")
    subject = f"{profile_id}:{field}"
    issues.extend(subject_claim_issues(record.get("compensating_proof_claim_ids"),
                                       f"{label}.compensating_proof_claim_ids", claims,
                                       "profile_override", subject, {"profile_override_review"}))
    return issues


def experience_issues(package, high_risk_actions):
    raw_flows, state_model = package.get("flows"), package.get("state_model")
    flows = [item for item in raw_flows if isinstance(item, dict)] \
        if isinstance(raw_flows, list) else []
    states = state_model.get("states", []) if isinstance(state_model, dict) else []
    states = states if isinstance(states, list) else []
    actions = [action for state in states if isinstance(state, dict)
               for action in (state.get("actions") if isinstance(state.get("actions"), list) else [])
               if isinstance(action, dict)]
    flow_actions = {flow.get("id"): {step.get("action_id") for step in flow.get("steps", [])
                                     if isinstance(step, dict) and isinstance(step.get("action_id"), str)}
                    for flow in flows if isinstance(flow.get("id"), str)}
    used_actions = set().union(*flow_actions.values()) if flow_actions else set()
    mutating = {item.get("id") for item in actions if item.get("effect_class") != "read_only"
                and isinstance(item.get("id"), str)}
    kinds = {item.get("kind") for item in flows if isinstance(item.get("kind"), str)}
    issues = []
    if mutating and not {"error", "recovery"} <= kinds:
        issues.append("FrontendDesignPackage.flows: mutating UI requires error and recovery flows")
    missing = mutating - used_actions
    if missing:
        issues.append(f"FrontendDesignPackage.flows: mutating actions lack flow coverage {sorted(missing)}")
    if _classification_value(package, "page_pattern") == "form" and "cancel" not in kinds:
        issues.append("FrontendDesignPackage.flows: form requires a cancel flow")
    issues.extend(_high_risk_coverage_issues(package, flows, flow_actions, high_risk_actions, kinds))
    issues.extend(_declared_risk_issues(package, flows))
    return issues


def _high_risk_coverage_issues(package, flows, flow_actions, high_risk_actions, kinds):
    if not high_risk_actions:
        return []
    issues = []
    triggers = {item for item in package.get("change_kinds", []) if isinstance(item, str)} \
        if isinstance(package.get("change_kinds"), list) else set()
    if "high_risk_action" not in triggers:
        issues.append("FrontendDesignPackage.change_kinds: high-risk action trigger is required")
    if not {"cancel", "error", "recovery"} <= kinds:
        issues.append("FrontendDesignPackage.flows: high-risk action requires cancel, error and recovery flows")
    interaction_flows = {item.get("subject_id") for item in package.get("verification_cases", [])
                         if isinstance(item, dict) and item.get("kind") == "interaction"
                         and isinstance(item.get("subject_id"), str)} \
        if isinstance(package.get("verification_cases"), list) else set()
    for action_id in sorted(high_risk_actions):
        covering = {flow_id for flow_id, actions in flow_actions.items() if action_id in actions}
        if covering and not covering & interaction_flows:
            issues.append(f"FrontendDesignPackage.verification_cases: high-risk action {action_id!r} lacks interaction evidence")
        risky_flows = [flow for flow in flows if flow.get("id") in covering]
        if any(not isinstance(flow.get("risk_level"), str)
               or flow.get("risk_level") not in {"high", "critical"} for flow in risky_flows):
            issues.append(f"FrontendDesignPackage.flows: high-risk action {action_id!r} requires high/critical flow risk")
    state_model = package.get("state_model")
    raw_states = state_model.get("states") if isinstance(state_model, dict) else []
    raw_states = raw_states if isinstance(raw_states, list) else []
    effects = {item.get("effect_class") for state in raw_states if isinstance(state, dict)
               for item in (state.get("actions") if isinstance(state.get("actions"), list) else [])
               if isinstance(item, dict) and isinstance(item.get("effect_class"), str)}
    if effects & {"legal_commitment", "financial_commitment"} and "regulated_commitment" not in triggers:
        issues.append("FrontendDesignPackage.change_kinds: regulated commitment trigger is required")
    return issues


def _declared_risk_issues(package, flows):
    declared = _classification_value(package, "risk_level")
    flow_risks = [flow.get("risk_level") for flow in flows
                  if isinstance(flow.get("risk_level"), str) and flow.get("risk_level") in RISK_FLOORS]
    raw_overrides = package.get("profile_overrides")
    override_risks = [record.get("risk") for record in raw_overrides
                      if isinstance(record, dict) and record.get("risk") in RISK_FLOORS] \
        if isinstance(raw_overrides, list) else []
    ranks = {name: index for index, name in enumerate(("low", "medium", "high", "critical"))}
    issues = []
    declared_known = isinstance(declared, str) and declared in ranks
    if declared_known and flow_risks and ranks[declared] < max(ranks[item] for item in flow_risks):
        issues.append("FrontendDesignPackage.classification.risk_level: below a declared flow risk")
    if declared_known and override_risks \
            and ranks[declared] < max(ranks[item] for item in override_risks):
        issues.append("FrontendDesignPackage.classification.risk_level: below a declared profile override risk")
    effective = max(([declared] if declared_known else []) + flow_risks + override_risks,
                    key=lambda item: ranks[item], default=None)
    if effective is None:
        return issues
    materiality, workflow = RISK_FLOORS[effective]
    package_materiality = package.get("materiality")
    package_workflow = package.get("workflow_profile")
    materiality_rank = MATERIALITY_RANK.get(package_materiality, 0) \
        if isinstance(package_materiality, str) else 0
    workflow_rank = WORKFLOW_RANK.get(package_workflow, 0) \
        if isinstance(package_workflow, str) else 0
    if materiality_rank < MATERIALITY_RANK[materiality]:
        issues.append("FrontendDesignPackage.materiality: below declared risk floor")
    if workflow_rank < WORKFLOW_RANK[workflow]:
        issues.append("FrontendDesignPackage.workflow_profile: below declared risk floor")
    return issues


def controlled_decision_issues(package, decisions, artifacts, claims, repo_root):
    issues = []
    review = package.get("review")
    reviewer = review.get("reviewer_id") if isinstance(review, dict) else None
    for claim_id, claim in claims.items():
        if isinstance(claim, dict) and claim.get("claim_class") == "review_observation" \
                and claim.get("claimant_id") != reviewer:
            issues.append(f"FrontendDesignPackage.proof_claims[{claim_id!r}]: review claimant must match package reviewer")
    for dimension, record in decisions.items():
        kinds = {item for item in record.get("decision_kinds", []) if isinstance(item, str)} \
            if isinstance(record, dict) and isinstance(record.get("decision_kinds"), list) else set()
        controlled = isinstance(record, dict) and (record.get("reversibility") == "low"
                                                    or bool(kinds & CONTROLLED_DECISIONS))
        if not controlled:
            continue
        label = f"FrontendDesignPackage.decisions[{dimension}]"
        alternatives = record.get("alternatives")
        controls = ("migration_cost", "blast_radius", "adr_ref", "reviewer_id", "revisit_trigger")
        if not isinstance(alternatives, list) or len(alternatives) < 2 \
                or any(not isinstance(record.get(field), str) or not record[field].strip() for field in controls):
            issues.append(f"{label}: low-reversibility control fields are incomplete")
            continue
        if record.get("reviewer_id") != reviewer:
            issues.append(f"{label}: reviewer must match package independent reviewer")
        adr_ref = record.get("adr_ref")
        path_issue = repo_path_issue(repo_root, adr_ref, f"{label}.adr_ref")
        if path_issue or not (repo_root / adr_ref).is_file():
            issues.append(path_issue or f"{label}.adr_ref: must be a regular file")
            continue
        if not _adr_bound(record, dimension, adr_ref, artifacts, claims):
            issues.append(f"{label}: ADR must be digest-bound by an architecture_decision_record claim")
    return issues


def _adr_bound(record, dimension, adr_ref, artifacts, claims):
    refs = record.get("proof_claim_ids") if isinstance(record.get("proof_claim_ids"), list) else []
    for ref in refs:
        claim = claims.get(ref) if isinstance(ref, str) else None
        proof_types = claim.get("proof_types") if isinstance(claim, dict) else None
        if not isinstance(claim, dict) or claim.get("subject_type") != "decision" \
                or claim.get("subject_id") != dimension or not isinstance(proof_types, list) \
                or "architecture_decision_record" not in proof_types:
            continue
        artifact_ids = claim.get("artifact_ids")
        for artifact_id in artifact_ids if isinstance(artifact_ids, list) else []:
            artifact = artifacts.get(artifact_id) if isinstance(artifact_id, str) else None
            if isinstance(artifact, dict) and artifact.get("kind") == "source" \
                    and artifact.get("locator") == adr_ref:
                return True
    return False
