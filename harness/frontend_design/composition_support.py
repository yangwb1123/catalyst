#!/usr/bin/env python3
"""Bounded parsing and primitive contracts shared by UI composition validation."""
import hashlib
import json
import re

from engineering_check_support import unknown_field_issues

FORBIDDEN_KEYS = {"accepted", "approved", "completed", "gate_result", "geometry_passed",
                  "quality_passed", "verdict"}
TOP_FIELDS = {
    "api_version", "id", "surface_id", "views", "data_semantics", "page_states", "regions",
    "axes", "groups", "spacing_relations", "strokes", "shape_rules", "responsive_variants",
    "load_bearing_elements", "optical_adjustments"}
VIEW_FIELDS = {"id", "actor", "work_mode", "flow_ids", "primary_questions"}
DATA_FIELDS = {
    "id", "kind", "source_authority", "definition", "unit", "time_basis", "freshness_policy",
    "null_semantics", "access_semantics", "uncertainty_policy", "explanation_policy",
    "human_confirmation"}
PAGE_STATE_FIELDS = {"id", "kind", "scope_region_ids", "business_state_ids",
                     "retains_previous_data", "action_ids", "recovery_action_ids", "feedback"}
REGION_FIELDS = {"id", "parent_id", "semantic_role", "view_ids", "flow_ids", "action_ids",
                 "data_ids", "priority", "axis_refs"}
AXIS_FIELDS = {"id", "orientation", "alignment_edge", "priority", "scope_region_id", "member_refs"}
GROUP_FIELDS = {"id", "region_id", "purpose", "primary_axis_ref", "member_refs"}
SPACING_FIELDS = {"id", "from_ref", "to_ref", "relationship", "token_ref"}
STROKE_FIELDS = {"id", "purpose", "start_anchor_ref", "end_anchor_ref", "token_ref"}
SHAPE_FIELDS = {"id", "semantic_role", "subject_refs", "family_token_ref"}
RESPONSIVE_FIELDS = {"id", "environment_ref", "region_dispositions", "reason"}
ELEMENT_FIELDS = {"id", "region_id", "view_ids", "flow_ids", "action_ids", "data_ids",
                  "feedback_state_ids", "axis_refs"}
OPTICAL_FIELDS = {"subject_ref", "axis", "policy_or_token_ref", "reason", "reviewer_ref"}
COLLECTIONS = {
    "views": (VIEW_FIELDS, True), "data_semantics": (DATA_FIELDS, False),
    "page_states": (PAGE_STATE_FIELDS, False), "regions": (REGION_FIELDS, True),
    "axes": (AXIS_FIELDS, True), "groups": (GROUP_FIELDS, False),
    "spacing_relations": (SPACING_FIELDS, True), "strokes": (STROKE_FIELDS, False),
    "shape_rules": (SHAPE_FIELDS, False), "responsive_variants": (RESPONSIVE_FIELDS, True),
    "load_bearing_elements": (ELEMENT_FIELDS, True), "optical_adjustments": (OPTICAL_FIELDS, False)}
WORK_MODES = {"operation", "supervision", "approval", "analysis", "configuration", "audit",
              "conversion", "exploration"}
DATA_KINDS = {"business_fact", "computed_judgment", "ai_recommendation", "derived_display"}
NULL_SEMANTICS = {"value", "zero", "unknown", "unavailable", "unauthorized", "not_applicable",
                  "not_calculated"}
PAGE_STATE_KINDS = {
    "initial", "loading", "empty", "partial", "stale", "denied", "precondition_blocked",
    "conflict", "offline", "success", "failure", "background_refresh", "unknown_result"}
DISPOSITIONS = {"present", "deferred", "omitted_with_reason"}
TOKEN_REF = re.compile(r"(?:token|policy|profile):[A-Za-z0-9][A-Za-z0-9_.:/-]{0,255}\Z")
DATA_SEMANTIC_TRIGGERS = {
    "form_or_table", "data_intensive", "high_risk_action", "authentication_or_payment",
    "destructive_user_data", "regulated_commitment", "safety_critical_surface"}
ACTION_RISK_TRIGGERS = {
    "high_risk_action", "authentication_or_payment", "destructive_user_data",
    "regulated_commitment"}
NON_NORMAL_DATA_STATES = {
    "loading", "empty", "partial", "stale", "failure", "background_refresh", "offline",
    "unknown_result"}
HIGH_RISK_PAGE_STATES = {"denied", "precondition_blocked", "conflict", "failure", "unknown_result"}


def shape_issues(value, fields, label):
    if not isinstance(value, dict):
        return [f"{label}: expected mapping"]
    issues = unknown_field_issues(value, fields, label)
    missing = fields - set(value)
    if missing:
        issues.append(f"{label}: missing fields {sorted(missing)}")
    return issues


def text_issues(value, label, maximum=4096, *, empty=False):
    if not isinstance(value, str) or len(value) > maximum or (not empty and not value.strip()):
        kind = "possibly empty " if empty else "non-empty "
        return [f"{label}: must be {kind}string <= {maximum} characters"]
    return []


def string_list_issues(value, label, *, non_empty=False, allowed=None):
    if not isinstance(value, list) or (non_empty and not value) or len(value) > 256:
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list <= 256 items"]
    if not all(isinstance(item, str) and item.strip() for item in value):
        return [f"{label}: values must be non-empty strings"]
    issues = [f"{label}: values must be unique"] if len(value) != len(set(value)) else []
    if allowed is not None and set(value) - set(allowed):
        issues.append(f"{label}: contains unknown values {sorted(set(value) - set(allowed))}")
    return issues


def record_list_issues(value, label, maximum=256, *, non_empty=False):
    if not isinstance(value, list) or (non_empty and not value) or len(value) > maximum:
        return [f"{label}: expected {'non-empty ' if non_empty else ''}list <= {maximum} items"]
    return []


def index_records(records, label):
    issues, result = [], {}
    if not isinstance(records, list):
        return issues, result
    for position, record in enumerate(records):
        if not isinstance(record, dict):
            continue
        item_label, value = f"{label}[{position}]", record.get("id")
        issues.extend(text_issues(value, f"{item_label}.id", 128))
        if isinstance(value, str):
            if value in result:
                issues.append(f"{item_label}.id: duplicate id {value!r}")
            result[value] = record
    return issues, result


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _reject_constant(value):
    raise ValueError(f"non-finite JSON number {value!r}")


def strict_json_file(path, label, maximum=262144):
    """Read bounded JSON input while rejecting ambiguous structures."""
    try:
        raw = path.read_bytes()
        if len(raw) > maximum:
            return None, [f"{label}: exceeds {maximum} bytes"]
        data = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object,
                          parse_constant=_reject_constant)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        return None, [f"{label}: invalid strict JSON ({exc})"]
    issues, stack, nodes = [], [(data, 0)], 0
    while stack:
        value, depth = stack.pop()
        nodes += 1
        if depth > 20 or nodes > 20000:
            return None, [f"{label}: exceeds depth or node budget"]
        if isinstance(value, dict):
            forbidden = set(value) & FORBIDDEN_KEYS
            if forbidden:
                issues.append(f"{label}: forbidden completion-authority fields {sorted(forbidden)}")
            stack.extend((child, depth + 1) for child in value.values())
        elif isinstance(value, list):
            stack.extend((child, depth + 1) for child in value)
    return data, issues


def canonical_sha256(value):
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":"), allow_nan=False).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def reference_issues(values, known, label, *, non_empty=False):
    issues = string_list_issues(values, label, non_empty=non_empty)
    if isinstance(values, list) and all(isinstance(item, str) for item in values):
        missing = set(values) - set(known)
        if missing:
            issues.append(f"{label}: unknown references {sorted(missing)}")
    return issues


def token_issues(value, label):
    return [] if isinstance(value, str) and TOKEN_REF.fullmatch(value) else [
        f"{label}: requires token:/policy:/profile: symbolic reference"]


def region_cycle_issues(regions, label):
    issues = []
    for start in regions:
        seen, current = set(), start
        while current:
            if current in seen:
                issues.append(f"{label}: parent cycle contains region {current!r}")
                break
            seen.add(current)
            record = regions.get(current)
            current = record.get("parent_id") if isinstance(record, dict) else ""
            if current and current not in regions:
                break
    return sorted(set(issues))


def inside_region(region_id, scope_id, regions):
    """Return whether a region is the scope or one of its descendants."""
    seen, current = set(), region_id
    while isinstance(current, str) and current and current not in seen:
        if current == scope_id:
            return True
        seen.add(current)
        record = regions.get(current)
        current = record.get("parent_id") if isinstance(record, dict) else None
    return False


def page_state_action_issues(record, actions, label):
    issues = []
    business_states = set(record.get("business_state_ids", [])) \
        if isinstance(record.get("business_state_ids"), list) else set()
    if (record.get("action_ids") or record.get("recovery_action_ids")) and not business_states:
        issues.append(f"{label}.business_state_ids: actions require at least one canonical business state")
    for field in ("action_ids", "recovery_action_ids"):
        for action_id in record.get(field, []) if isinstance(record.get(field), list) else []:
            action = actions.get(action_id)
            source = action.get("source_state") if isinstance(action, dict) else None
            if source is not None and source not in business_states:
                issues.append(f"{label}.{field}: action {action_id!r} is unavailable "
                              "from the declared business states")
    recovery_sources = {actions[action_id].get("source_state")
                        for action_id in record.get("recovery_action_ids", [])
                        if action_id in actions}
    if record.get("recovery_action_ids") and business_states - recovery_sources:
        issues.append(f"{label}.recovery_action_ids: no executable recovery for "
                      f"business states {sorted(business_states - recovery_sources)}")
    return issues


def exact_axis_membership_issues(axes, regions, groups, elements, label):
    issues = []
    for ref, record in {**regions, **elements}.items():
        for axis_id in record.get("axis_refs", []) if isinstance(record.get("axis_refs"), list) else []:
            axis = axes.get(axis_id)
            members = axis.get("member_refs") if isinstance(axis, dict) else []
            if isinstance(members, list) and ref not in members:
                issues.append(f"{label}: {ref!r} declares axis {axis_id!r} "
                              "but is absent from its exact member_refs")
    for ref, group in groups.items():
        axis_id, axis = group.get("primary_axis_ref"), axes.get(group.get("primary_axis_ref"))
        members = axis.get("member_refs") if isinstance(axis, dict) else []
        if isinstance(members, list) and axis_id in axes and ref not in members:
            issues.append(f"{label}: group {ref!r} declares primary axis {axis_id!r} "
                          "but is absent from its exact member_refs")
    for axis_id, axis in axes.items():
        for ref in axis.get("member_refs", []) if isinstance(axis.get("member_refs"), list) else []:
            record = regions.get(ref) or elements.get(ref)
            if isinstance(record, dict) and axis_id not in record.get("axis_refs", []):
                issues.append(f"{label}: axis {axis_id!r} lists {ref!r} without "
                              "a reciprocal axis reference")
            elif ref in groups and groups[ref].get("primary_axis_ref") != axis_id:
                issues.append(f"{label}: axis {axis_id!r} lists group {ref!r} without "
                              "a reciprocal primary-axis reference")
    return issues
