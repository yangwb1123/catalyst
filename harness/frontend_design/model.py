#!/usr/bin/env python3
"""Validate classification, flows, states, and verification-case integrity."""
import math
import re
from engineering_check_support import unique_id_issues, unknown_field_issues
from .contract import (
    CLASSIFICATION_FIELDS,
    DENSITIES,
    EFFECT_CLASSES,
    FREQUENCIES,
    HIGH_RISK_EFFECTS,
    MOTION_LEVELS,
    PAGE_PATTERN_IDS,
    PLATFORMS,
    PROFILE_IDS,
    RISK_LEVELS,
    VERIFICATION_KINDS,
)
from .evidence import DIGEST, claim_refs_issues, png_dimensions, subject_claim_issues

CLASSIFIED_FIELDS = {"value", "claim_type", "confidence", "proof_claim_id", "assumption_id"}
FLOW_FIELDS = {
    "id", "kind", "actor", "goal", "entry", "trigger", "preconditions", "steps",
    "terminal_outcome", "context_preservation", "permissions", "risk_level", "proof_claim_ids",
}
FLOW_STEP_FIELDS = {"action_id", "expected_feedback", "expected_state", "context_preserved"}
STATE_MODEL_FIELDS = {"id", "initial_state", "states", "proof_claim_ids"}
STATE_FIELDS = {"id", "label", "terminal", "actions"}
ACTION_FIELDS = {
    "id", "label", "effect_class", "permissions", "data_guards", "system_guards",
    "next_states", "feedback", "recovery",
}
CASE_FIELDS = {
    "id", "kind", "subject_id", "source_tree_sha256", "build_sha256", "fixture_id",
    "environment", "artifact_ids", "proof_claim_ids",
}
ENV_FIELDS = {
    "platform", "runtime", "viewport", "dpr", "theme", "locale", "timezone",
    "color_scheme", "reduced_motion", "text_scale", "fonts_digest",
}
FLOW_KINDS = {"primary", "alternative", "error", "cancel", "recovery"}


def _shape(value, fields, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, fields, label)
    missing = fields - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def _text(value, label, maximum=4096):
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        return [f"{label}: must be non-empty string <= {maximum} characters"]
    return []


def _strings(value, label, *, non_empty=False, maximum=128):
    if not isinstance(value, list) or (non_empty and not value) or len(value) > maximum:
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list <= {maximum} items"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    return [f"{label}: values must be unique"] if len(value) != len(set(value)) else []


def _classified_value_issues(field, record, claims, assumptions):
    label = f"FrontendDesignPackage.classification.{field}"
    issues = _shape(record, CLASSIFIED_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    value, claim_type, confidence = record.get("value"), record.get("claim_type"), record.get("confidence")
    if field == "motion_level":
        if not isinstance(value, int) or isinstance(value, bool) or value not in MOTION_LEVELS:
            issues.append(f"{label}.value: invalid motion level")
    elif not isinstance(value, str) or not value.strip() or len(value) > 256:
        issues.append(f"{label}.value: must be non-empty string <= 256 characters")
    if not isinstance(confidence, (int, float)) or isinstance(confidence, bool) \
            or not math.isfinite(confidence) or not 0 <= confidence <= 1:
        issues.append(f"{label}.confidence: must be within 0..1")
    elif claim_type == "fact" and confidence != 1:
        issues.append(f"{label}: fact requires confidence 1.0")
    elif claim_type == "inference" and confidence in {0, 1}:
        issues.append(f"{label}: inference confidence must be strictly between 0 and 1")
    issues.extend(_classification_basis_issues(field, record, claims, assumptions, label))
    return issues


def _classification_basis_issues(field, record, claims, assumptions, label):
    claim_type, proof_id, assumption_id = record.get("claim_type"), record.get("proof_claim_id"), record.get("assumption_id")
    if claim_type == "fact":
        if assumption_id != "":
            return [f"{label}: fact cannot cite an assumption"]
        return subject_claim_issues([proof_id], f"{label}.proof_claim_id", claims,
                                    "classification", field, {"classification_fact"})
    if claim_type == "inference":
        issues = []
        if proof_id != "":
            issues.append(f"{label}: inference must not disguise a proof claim as fact")
        assumption = assumptions.get(assumption_id) if isinstance(assumption_id, str) else None
        if not isinstance(assumption, dict):
            issues.append(f"{label}.assumption_id: must resolve to an assumption")
        elif assumption.get("status") == "rejected":
            issues.append(f"{label}.assumption_id: rejected assumption cannot support an inference")
        return issues
    return [f"{label}.claim_type: invalid"]


def classification_issues(record, claims, assumptions):
    fields = set(CLASSIFICATION_FIELDS) | {"rationale"}
    issues = _shape(record, fields, "FrontendDesignPackage.classification")
    if not isinstance(record, dict):
        return issues
    for field in CLASSIFICATION_FIELDS:
        issues.extend(_classified_value_issues(field, record.get(field), claims, assumptions))
    issues.extend(_text(record.get("rationale"), "FrontendDesignPackage.classification.rationale", 8192))
    values = {field: record.get(field, {}).get("value") if isinstance(record.get(field), dict) else None
              for field in CLASSIFICATION_FIELDS}
    checks = (("profile_id", PROFILE_IDS), ("page_pattern", PAGE_PATTERN_IDS),
              ("platform", PLATFORMS), ("density", DENSITIES),
              ("operation_frequency", FREQUENCIES), ("data_density", FREQUENCIES),
              ("risk_level", RISK_LEVELS))
    for field, allowed in checks:
        if not isinstance(values[field], str) or values[field] not in allowed:
            issues.append(f"FrontendDesignPackage.classification.{field}.value: unknown catalog value")
    return issues


def _flow_step_issues(step, label, actions, state_ids):
    issues = _shape(step, FLOW_STEP_FIELDS, label)
    if not isinstance(step, dict):
        return issues
    action_id, expected_state = step.get("action_id"), step.get("expected_state")
    action = actions.get(action_id) if isinstance(action_id, str) else None
    if action is None:
        issues.append(f"{label}.action_id: unknown action")
    if not isinstance(expected_state, str) or expected_state not in state_ids:
        issues.append(f"{label}.expected_state: unknown state")
    elif action is not None and expected_state not in action["next_states"]:
        issues.append(f"{label}.expected_state: not allowed by action.next_states")
    issues.extend(_text(step.get("expected_feedback"), f"{label}.expected_feedback"))
    if not isinstance(step.get("context_preserved"), bool):
        issues.append(f"{label}.context_preserved: expected boolean")
    return issues


def flow_issues(records, claims, actions, state_ids):
    issues, kinds = [], set()
    id_issues, _ = unique_id_issues(records, "FrontendDesignPackage.flows", "flow")
    issues.extend(id_issues)
    if not isinstance(records, list) or not records or len(records) > 64:
        return issues + ["FrontendDesignPackage.flows: expected 1..64 records"]
    for index, record in enumerate(records):
        label = f"FrontendDesignPackage.flows[{index}]"
        issues.extend(_flow_record_issues(record, label, claims, actions, state_ids))
        if isinstance(record, dict) and isinstance(record.get("kind"), str) \
                and record.get("kind") in FLOW_KINDS:
            kinds.add(record["kind"])
    if "primary" not in kinds:
        issues.append("FrontendDesignPackage.flows: at least one primary flow is required")
    return issues


def _flow_record_issues(record, label, claims, actions, state_ids):
    issues = _shape(record, FLOW_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    flow_id = record.get("id")
    for field in ("id", "actor", "goal", "entry", "trigger", "terminal_outcome", "context_preservation"):
        issues.extend(_text(record.get(field), f"{label}.{field}"))
    if not isinstance(record.get("kind"), str) or record.get("kind") not in FLOW_KINDS:
        issues.append(f"{label}.kind: invalid")
    for field, required in (("preconditions", False), ("permissions", True)):
        issues.extend(_strings(record.get(field), f"{label}.{field}", non_empty=required))
    risk_level = record.get("risk_level")
    if not isinstance(risk_level, str) or risk_level not in RISK_LEVELS:
        issues.append(f"{label}.risk_level: invalid")
    steps = record.get("steps")
    if not isinstance(steps, list) or not steps:
        issues.append(f"{label}.steps: expected non-empty list")
    else:
        previous_state = None
        for step_index, step in enumerate(steps):
            issues.extend(_flow_step_issues(step, f"{label}.steps[{step_index}]", actions, state_ids))
            if not isinstance(step, dict):
                continue
            action_id = step.get("action_id")
            action = actions.get(action_id) if isinstance(action_id, str) else None
            if previous_state is not None and action is not None \
                    and action.get("source_state") != previous_state:
                issues.append(f"{label}.steps[{step_index}].action_id: action is unavailable from prior expected state")
            expected_state = step.get("expected_state")
            previous_state = expected_state if isinstance(expected_state, str) \
                and expected_state in state_ids else previous_state
    issues.extend(subject_claim_issues(record.get("proof_claim_ids"), f"{label}.proof_claim_ids",
                                       claims, "flow", flow_id, {"operation_flow", "recovery_flow"}))
    return issues


def state_model_issues(record, claims):
    issues = _shape(record, STATE_MODEL_FIELDS, "FrontendDesignPackage.state_model")
    if not isinstance(record, dict):
        return issues, set(), {}, set()
    states = record.get("states")
    id_issues, state_ids = unique_id_issues(states, "FrontendDesignPackage.state_model.states", "state")
    issues.extend(id_issues)
    if not isinstance(states, list) or not states:
        return issues + ["FrontendDesignPackage.state_model.states: expected non-empty list"], set(), {}, set()
    if not isinstance(record.get("initial_state"), str) or record.get("initial_state") not in state_ids:
        issues.append("FrontendDesignPackage.state_model.initial_state: unknown state")
    actions, high_risk = {}, set()
    for index, state in enumerate(states):
        added, risky, state_issues = _state_issues(state, index, state_ids, set(actions))
        issues.extend(state_issues)
        actions.update(added)
        high_risk.update(risky)
    issues.extend(subject_claim_issues(record.get("proof_claim_ids"),
                                       "FrontendDesignPackage.state_model.proof_claim_ids", claims,
                                       "state_model", record.get("id"),
                                       {"state_action_matrix", "permission_action_review"}))
    return issues, state_ids, actions, high_risk


def _state_issues(state, index, state_ids, seen_actions):
    label, issues, added, risky = f"FrontendDesignPackage.state_model.states[{index}]", [], {}, set()
    issues.extend(_shape(state, STATE_FIELDS, label))
    if not isinstance(state, dict):
        return added, risky, issues
    issues.extend(_text(state.get("id"), f"{label}.id", 128))
    issues.extend(_text(state.get("label"), f"{label}.label", 256))
    terminal = state.get("terminal")
    if not isinstance(terminal, bool):
        issues.append(f"{label}.terminal: expected boolean")
    actions = state.get("actions")
    if not isinstance(actions, list):
        return added, risky, issues + [f"{label}.actions: expected list"]
    for action_index, action in enumerate(actions):
        action_label = f"{label}.actions[{action_index}]"
        issues.extend(_action_issues(action, action_label, state_ids))
        action_id = action.get("id") if isinstance(action, dict) else None
        if isinstance(action_id, str):
            if action_id in seen_actions or action_id in added:
                issues.append(f"{action_label}.id: duplicate action id {action_id!r}")
            next_states = action.get("next_states")
            normalized = set(next_states) if isinstance(next_states, list) \
                and all(isinstance(item, str) for item in next_states) else set()
            added[action_id] = {"source_state": state.get("id"), "next_states": normalized,
                                "effect_class": action.get("effect_class")}
            if terminal is True and any(item != state.get("id") for item in normalized):
                issues.append(f"{action_label}.next_states: terminal state cannot transition to another state")
            if isinstance(action.get("effect_class"), str) and action.get("effect_class") in HIGH_RISK_EFFECTS:
                risky.add(action_id)
    return added, risky, issues


def _action_issues(action, label, state_ids):
    issues = _shape(action, ACTION_FIELDS, label)
    if not isinstance(action, dict):
        return issues
    for field in ("id", "label", "feedback", "recovery"):
        issues.extend(_text(action.get(field), f"{label}.{field}", 4096 if field in {"feedback", "recovery"} else 256))
    effect_class = action.get("effect_class")
    if not isinstance(effect_class, str) or effect_class not in EFFECT_CLASSES:
        issues.append(f"{label}.effect_class: invalid")
    for field in ("permissions", "data_guards", "system_guards"):
        issues.extend(_strings(action.get(field), f"{label}.{field}", non_empty=True))
    next_states = action.get("next_states")
    issues.extend(_strings(next_states, f"{label}.next_states"))
    if isinstance(next_states, list) and all(isinstance(item, str) for item in next_states):
        missing = set(next_states) - state_ids
        if missing:
            issues.append(f"{label}.next_states: unknown states {sorted(missing)}")
    return issues


def verification_case_issues(records, artifacts, claims, flow_ids, state_ids, source_tree, repo_root):
    issues, capture_fingerprints = [], {}
    id_issues, _ = unique_id_issues(records, "FrontendDesignPackage.verification_cases", "verification case")
    issues.extend(id_issues)
    if not isinstance(records, list) or len(records) > 128:
        return issues + ["FrontendDesignPackage.verification_cases: expected list <= 128 items"]
    for index, record in enumerate(records):
        label = f"FrontendDesignPackage.verification_cases[{index}]"
        issues.extend(_verification_case_record_issues(record, label, artifacts, claims,
                                                        flow_ids, state_ids, source_tree, repo_root))
        if isinstance(record, dict) and record.get("kind") == "capture":
            for ref in record.get("artifact_ids", []) if isinstance(record.get("artifact_ids"), list) else []:
                artifact = artifacts.get(ref) if isinstance(ref, str) else None
                if not isinstance(artifact, dict) or artifact.get("kind") not in {"screenshot", "visual_diff"}:
                    continue
                digest = artifact.get("content_sha256")
                if not isinstance(digest, str):
                    continue
                fingerprint = (artifact.get("kind"), digest)
                prior = capture_fingerprints.setdefault(fingerprint, record.get("id"))
                if prior != record.get("id"):
                    issues.append(f"{label}.artifact_ids: identical capture bytes reused from case {prior!r}")
    return issues


def _verification_case_record_issues(record, label, artifacts, claims, flow_ids, state_ids,
                                     source_tree, repo_root):
    issues = _shape(record, CASE_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    case_id, kind = record.get("id"), record.get("kind")
    issues.extend(_text(case_id, f"{label}.id", 128))
    issues.extend(_text(record.get("fixture_id"), f"{label}.fixture_id", 128))
    if not isinstance(kind, str) or kind not in VERIFICATION_KINDS:
        issues.append(f"{label}.kind: invalid")
    subject_ids = flow_ids if kind == "interaction" else state_ids
    if not isinstance(record.get("subject_id"), str) or record.get("subject_id") not in subject_ids:
        issues.append(f"{label}.subject_id: does not resolve for {kind!r}")
    if record.get("source_tree_sha256") != source_tree:
        issues.append(f"{label}.source_tree_sha256: does not match package")
    if not isinstance(record.get("build_sha256"), str) or not DIGEST.fullmatch(record["build_sha256"]):
        issues.append(f"{label}.build_sha256: invalid")
    issues.extend(_environment_issues(record.get("environment"), f"{label}.environment"))
    refs = record.get("artifact_ids")
    issues.extend(_strings(refs, f"{label}.artifact_ids", non_empty=True))
    if isinstance(refs, list) and all(isinstance(ref, str) for ref in refs):
        missing = set(refs) - set(artifacts)
        if missing:
            issues.append(f"{label}.artifact_ids: unknown artifact ids {sorted(missing)}")
        kinds = {artifacts[ref].get("kind") for ref in refs
                 if ref in artifacts and isinstance(artifacts[ref], dict)
                 and isinstance(artifacts[ref].get("kind"), str)}
        required_kind = "trace" if kind == "interaction" else "screenshot"
        if required_kind not in kinds:
            issues.append(f"{label}.artifact_ids: {kind} requires {required_kind} artifact")
        if kind == "capture":
            issues.extend(_capture_dimension_issues(record, refs, artifacts, label, repo_root))
    proof = {"interaction_execution_receipts"} if kind == "interaction" \
        else {"capture_receipt", "visual_diff_receipts"}
    allowed_negative = {"geometry_measurement_receipts"} if kind == "capture" else set()
    issues.extend(subject_claim_issues(
        record.get("proof_claim_ids"), f"{label}.proof_claim_ids", claims,
        "verification_case", case_id, proof,
        allowed_negative_types=allowed_negative,
    ))
    issues.extend(_case_claim_binding_issues(record, claims, label))
    return issues


def _case_claim_binding_issues(record, claims, label):
    artifact_ids = record.get("artifact_ids")
    declared = set(artifact_ids) if isinstance(artifact_ids, list) \
        and all(isinstance(item, str) for item in artifact_ids) else set()
    claimed = set()
    proof_claim_ids = record.get("proof_claim_ids")
    for claim_id in proof_claim_ids if isinstance(proof_claim_ids, list) else []:
        claim = claims.get(claim_id) if isinstance(claim_id, str) else None
        if isinstance(claim, dict) and isinstance(claim.get("artifact_ids"), list):
            claimed.update(item for item in claim["artifact_ids"] if isinstance(item, str))
    missing = declared - claimed
    extra = claimed - declared
    issues = []
    if missing:
        issues.append(f"{label}: case artifacts are not bound by its proof claims {sorted(missing)}")
    if extra:
        issues.append(f"{label}: claim artifacts are not declared by the case {sorted(extra)}")
    return issues


def _capture_dimension_issues(record, refs, artifacts, label, repo_root):
    screenshot = next((artifacts[ref] for ref in refs
                       if ref in artifacts and artifacts[ref].get("kind") == "screenshot"), None)
    environment = record.get("environment")
    if not isinstance(screenshot, dict) or not isinstance(environment, dict):
        return []
    viewport = environment.get("viewport")
    match = re.fullmatch(r"([1-9][0-9]{0,4})x([1-9][0-9]{0,4})", viewport) \
        if isinstance(viewport, str) else None
    dpr = environment.get("dpr")
    if not match or not isinstance(dpr, (int, float)) or isinstance(dpr, bool) \
            or not math.isfinite(dpr) or not 0 < dpr <= 8:
        return [f"{label}.environment.viewport: capture requires WIDTHxHEIGHT"]
    try:
        actual = png_dimensions(repo_root / screenshot["locator"])
    except (KeyError, OSError, ValueError, TypeError):
        return []  # The evidence validator reports malformed or unreadable artifacts.
    expected = (round(int(match.group(1)) * dpr), round(int(match.group(2)) * dpr))
    return [] if actual == expected else [f"{label}: PNG dimensions {actual} do not match viewport×DPR {expected}"]


def _environment_issues(record, label):
    issues = _shape(record, ENV_FIELDS, label)
    if not isinstance(record, dict):
        return issues
    for field in ENV_FIELDS - {"dpr", "text_scale", "reduced_motion"}:
        issues.extend(_text(record.get(field), f"{label}.{field}", 256))
    if isinstance(record.get("fonts_digest"), str) and not DIGEST.fullmatch(record["fonts_digest"]):
        issues.append(f"{label}.fonts_digest: invalid digest")
    for field in ("dpr", "text_scale"):
        value = record.get(field)
        maximum = 8 if field == "dpr" else 10
        if not isinstance(value, (int, float)) or isinstance(value, bool) \
                or not math.isfinite(value) or not 0 < value <= maximum:
            issues.append(f"{label}.{field}: expected finite positive number <= {maximum}")
    if not isinstance(record.get("reduced_motion"), bool):
        issues.append(f"{label}.reduced_motion: expected boolean")
    return issues
