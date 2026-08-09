#!/usr/bin/env python3
"""Validate business-bound UI composition artifacts without judging aesthetics."""
from engineering_check_support import repo_path_issue
from .contract import BUSINESS_UI_COMPOSITION_MEDIA_TYPE, COMPOSITION_TRIGGERS
from .composition_support import (
    ACTION_RISK_TRIGGERS, AXIS_FIELDS, COLLECTIONS, DATA_FIELDS, DATA_KINDS,
    DATA_SEMANTIC_TRIGGERS, DISPOSITIONS, ELEMENT_FIELDS, GROUP_FIELDS, NULL_SEMANTICS,
    HIGH_RISK_PAGE_STATES, NON_NORMAL_DATA_STATES, OPTICAL_FIELDS, PAGE_STATE_FIELDS,
    PAGE_STATE_KINDS, REGION_FIELDS, RESPONSIVE_FIELDS,
    SHAPE_FIELDS, SPACING_FIELDS, STROKE_FIELDS, TOP_FIELDS, VIEW_FIELDS, WORK_MODES,
    canonical_sha256, index_records as _index, inside_region as _inside_region,
    exact_axis_membership_issues, page_state_action_issues,
    record_list_issues as _records, reference_issues as _refs,
    region_cycle_issues as _region_cycle_issues, shape_issues as _shape,
    strict_json_file, string_list_issues as _strings, text_issues as _text,
    token_issues as _token,
)


class _CompositionValidator:
    def __init__(self, data, label, flows, state_ids, actions, high_risk, triggers,
                 reviewer_id, issues):
        self.data, self.label, self.flows = data, label, flows
        self.state_ids, self.actions = set(state_ids), actions
        self.high_risk, self.reviewer_id = set(high_risk), reviewer_id
        self.triggers = set(triggers)
        self.issues, self.indexes = issues, {}
        self.flow_ids, self.action_ids = set(flows), set(actions)
        self.views = self.data_items = self.page_states = self.regions = {}
        self.axes = self.groups = self.elements = {}
        self.spatial_ids = self.all_refs = set()

    def _spatial_region(self, ref):
        if ref in self.regions:
            return ref
        record = self.groups.get(ref) or self.elements.get(ref)
        return record.get("region_id") if isinstance(record, dict) else None

    def _scope_compatible(self, region_id, axis_id):
        axis = self.axes.get(axis_id)
        scope = axis.get("scope_region_id") if isinstance(axis, dict) else None
        return bool(region_id in self.regions and scope in self.regions
                    and _inside_region(region_id, scope, self.regions))

    def _header_and_indexes(self):
        if self.data.get("api_version") != "forgeos.business-ui-composition/v1":
            self.issues.append(f"{self.label}.api_version: unsupported")
        for field in ("id", "surface_id"):
            self.issues.extend(_text(self.data.get(field), f"{self.label}.{field}", 128))
        for field, (_, required) in COLLECTIONS.items():
            self.issues.extend(_records(self.data.get(field), f"{self.label}.{field}", non_empty=required))
            if field == "optical_adjustments":
                continue
            added, self.indexes[field] = _index(self.data.get(field), f"{self.label}.{field}")
            self.issues.extend(added)
        self.views, self.data_items = self.indexes.get("views", {}), self.indexes.get("data_semantics", {})
        self.page_states, self.regions = self.indexes.get("page_states", {}), self.indexes.get("regions", {})
        self.axes, self.groups = self.indexes.get("axes", {}), self.indexes.get("groups", {})
        self.elements = self.indexes.get("load_bearing_elements", {})
        spatial_ids = list(self.regions) + list(self.groups) + list(self.elements)
        reference_ids = spatial_ids + list(self.axes)
        if len(reference_ids) != len(set(reference_ids)):
            self.issues.append(f"{self.label}: region/group/element/axis ids share an ambiguous reference")
        self.spatial_ids, self.all_refs = set(spatial_ids), set(reference_ids)

    def _view_issues(self):
        for item_id, record in self.views.items():
            item = f"{self.label}.views[{item_id!r}]"
            self.issues.extend(_shape(record, VIEW_FIELDS, item))
            self.issues.extend(_text(record.get("actor"), f"{item}.actor", 128))
            if record.get("work_mode") not in WORK_MODES:
                self.issues.append(f"{item}.work_mode: invalid")
            self.issues.extend(_refs(record.get("flow_ids"), self.flow_ids, f"{item}.flow_ids", non_empty=True))
            self.issues.extend(_strings(record.get("primary_questions"), f"{item}.primary_questions", non_empty=True))
            refs = record.get("flow_ids")
            mismatched = [ref for ref in refs if ref in self.flows
                          and self.flows[ref].get("actor") != record.get("actor")] \
                if isinstance(refs, list) else []
            if mismatched:
                self.issues.append(f"{item}.actor: does not own referenced flows {sorted(mismatched)}")

    def _data_issues(self):
        for item_id, record in self.data_items.items():
            item = f"{self.label}.data_semantics[{item_id!r}]"
            self.issues.extend(_shape(record, DATA_FIELDS, item))
            if record.get("kind") not in DATA_KINDS:
                self.issues.append(f"{item}.kind: invalid")
            fields = ("source_authority", "definition", "unit", "time_basis", "freshness_policy",
                      "uncertainty_policy", "explanation_policy")
            for field in fields:
                self.issues.extend(_text(record.get(field), f"{item}.{field}"))
            self.issues.extend(_strings(record.get("null_semantics"), f"{item}.null_semantics",
                                        non_empty=True, allowed=NULL_SEMANTICS))
            if record.get("access_semantics") not in {"visible", "masked", "redacted", "permission_gated"}:
                self.issues.append(f"{item}.access_semantics: invalid")
            confirmation = record.get("human_confirmation")
            if confirmation not in {"not_applicable", "optional", "required"}:
                self.issues.append(f"{item}.human_confirmation: invalid")
            if record.get("kind") == "ai_recommendation" and confirmation != "required":
                self.issues.append(f"{item}: AI recommendation requires human confirmation")

    def _page_state_issues(self):
        region_ids = set(self.regions)
        for item_id, record in self.page_states.items():
            item = f"{self.label}.page_states[{item_id!r}]"
            self.issues.extend(_shape(record, PAGE_STATE_FIELDS, item))
            if record.get("kind") not in PAGE_STATE_KINDS:
                self.issues.append(f"{item}.kind: invalid")
            self.issues.extend(_refs(record.get("scope_region_ids"), region_ids,
                                     f"{item}.scope_region_ids", non_empty=True))
            self.issues.extend(_refs(record.get("business_state_ids"), self.state_ids,
                                     f"{item}.business_state_ids"))
            self.issues.extend(_refs(record.get("action_ids"), self.action_ids, f"{item}.action_ids"))
            self.issues.extend(_refs(record.get("recovery_action_ids"), self.action_ids,
                                     f"{item}.recovery_action_ids"))
            self.issues.extend(page_state_action_issues(record, self.actions, item))
            if not isinstance(record.get("retains_previous_data"), bool):
                self.issues.append(f"{item}.retains_previous_data: expected boolean")
            self.issues.extend(_text(record.get("feedback"), f"{item}.feedback"))

    def _region_issues(self):
        for item_id, record in self.regions.items():
            item, parent = f"{self.label}.regions[{item_id!r}]", record.get("parent_id")
            self.issues.extend(_shape(record, REGION_FIELDS, item))
            self.issues.extend(_text(parent, f"{item}.parent_id", 128, empty=True))
            if isinstance(parent, str) and parent and parent not in self.regions:
                self.issues.append(f"{item}.parent_id: unknown region")
            self.issues.extend(_text(record.get("semantic_role"), f"{item}.semantic_role", 256))
            for field, known in (("view_ids", self.views), ("flow_ids", self.flow_ids),
                                 ("action_ids", self.action_ids), ("data_ids", self.data_items)):
                self.issues.extend(_refs(record.get(field), known, f"{item}.{field}"))
            self.issues.extend(_refs(record.get("axis_refs"), self.axes, f"{item}.axis_refs", non_empty=True))
            for axis_id in record.get("axis_refs", []) if isinstance(record.get("axis_refs"), list) else []:
                if axis_id in self.axes and not self._scope_compatible(item_id, axis_id):
                    self.issues.append(f"{item}.axis_refs: axis {axis_id!r} scope is outside region ownership")
            if record.get("priority") not in {"primary", "secondary", "contextual", "deferred"}:
                self.issues.append(f"{item}.priority: invalid")
        self.issues.extend(_region_cycle_issues(self.regions, f"{self.label}.regions"))

    def _axis_and_group_issues(self):
        for item_id, record in self.axes.items():
            item = f"{self.label}.axes[{item_id!r}]"
            self.issues.extend(_shape(record, AXIS_FIELDS, item))
            if record.get("orientation") not in {"vertical", "horizontal"}:
                self.issues.append(f"{item}.orientation: invalid")
            if record.get("alignment_edge") not in {"left", "right", "top", "bottom", "center", "baseline"}:
                self.issues.append(f"{item}.alignment_edge: invalid")
            priority = record.get("priority")
            if not isinstance(priority, int) or isinstance(priority, bool) or not 1 <= priority <= 9:
                self.issues.append(f"{item}.priority: expected integer 1..9")
            if record.get("scope_region_id") not in self.regions:
                self.issues.append(f"{item}.scope_region_id: unknown region")
            self.issues.extend(_refs(record.get("member_refs"), self.spatial_ids,
                                     f"{item}.member_refs", non_empty=True))
            scope = record.get("scope_region_id")
            for ref in record.get("member_refs", []) if isinstance(record.get("member_refs"), list) else []:
                member_region = self._spatial_region(ref)
                if scope in self.regions and member_region in self.regions \
                        and not _inside_region(member_region, scope, self.regions):
                    self.issues.append(f"{item}.member_refs: {ref!r} crosses axis scope ownership")
        for item_id, record in self.groups.items():
            item = f"{self.label}.groups[{item_id!r}]"
            self.issues.extend(_shape(record, GROUP_FIELDS, item))
            if record.get("region_id") not in self.regions:
                self.issues.append(f"{item}.region_id: unknown region")
            self.issues.extend(_text(record.get("purpose"), f"{item}.purpose"))
            if record.get("primary_axis_ref") not in self.axes:
                self.issues.append(f"{item}.primary_axis_ref: unknown axis")
            elif record.get("region_id") in self.regions \
                    and not self._scope_compatible(record.get("region_id"), record.get("primary_axis_ref")):
                self.issues.append(f"{item}.primary_axis_ref: axis scope is outside group region ownership")
            self.issues.extend(_refs(record.get("member_refs"), set(self.regions) | set(self.elements),
                                     f"{item}.member_refs", non_empty=True))
            owner = record.get("region_id")
            for ref in record.get("member_refs", []) if isinstance(record.get("member_refs"), list) else []:
                member_region = self._spatial_region(ref)
                if owner in self.regions and member_region in self.regions \
                        and not _inside_region(member_region, owner, self.regions):
                    self.issues.append(f"{item}.member_refs: {ref!r} crosses group region ownership")
        self.issues.extend(exact_axis_membership_issues(
            self.axes, self.regions, self.groups, self.elements, self.label,
        ))

    def _element_issues(self):
        trace_flows, trace_actions = set(), set()
        known_refs = (("view_ids", self.views), ("flow_ids", self.flow_ids),
                      ("action_ids", self.action_ids), ("data_ids", self.data_items),
                      ("feedback_state_ids", self.page_states), ("axis_refs", self.axes))
        for item_id, record in self.elements.items():
            item = f"{self.label}.load_bearing_elements[{item_id!r}]"
            self.issues.extend(_shape(record, ELEMENT_FIELDS, item))
            if record.get("region_id") not in self.regions:
                self.issues.append(f"{item}.region_id: unknown region")
            for field, known in known_refs:
                self.issues.extend(_refs(record.get(field), known, f"{item}.{field}"))
            for axis_id in record.get("axis_refs", []) if isinstance(record.get("axis_refs"), list) else []:
                if axis_id in self.axes and record.get("region_id") in self.regions \
                        and not self._scope_compatible(record.get("region_id"), axis_id):
                    self.issues.append(f"{item}.axis_refs: axis {axis_id!r} scope is outside element region ownership")
            trace_fields = ("flow_ids", "action_ids", "data_ids", "feedback_state_ids")
            if not any(record.get(field) for field in trace_fields):
                self.issues.append(f"{item}: must trace to a flow, action, data item, or feedback state")
            trace_flows.update(record.get("flow_ids") if isinstance(record.get("flow_ids"), list) else [])
            trace_actions.update(record.get("action_ids") if isinstance(record.get("action_ids"), list) else [])
        grouped = {ref for group in self.groups.values()
                   for ref in (group.get("member_refs") if isinstance(group.get("member_refs"), list) else [])}
        if set(self.elements) - grouped:
            self.issues.append(f"{self.label}: load-bearing elements lack visual group ownership "
                               f"{sorted(set(self.elements) - grouped)}")
        primary = {item_id for item_id, record in self.flows.items() if record.get("kind") == "primary"}
        if primary - trace_flows:
            self.issues.append(f"{self.label}: primary flows lack load-bearing UI trace {sorted(primary - trace_flows)}")
        if self.high_risk - trace_actions:
            self.issues.append(f"{self.label}: high-risk actions lack load-bearing UI trace {sorted(self.high_risk - trace_actions)}")
        return primary

    def _semantic_floor_issues(self):
        element_data = {ref for record in self.elements.values()
                        for ref in (record.get("data_ids") if isinstance(record.get("data_ids"), list) else [])}
        region_data = {ref for record in self.regions.values()
                       for ref in (record.get("data_ids") if isinstance(record.get("data_ids"), list) else [])}
        missing = set(self.data_items) - element_data - region_data
        if missing:
            self.issues.append(f"{self.label}: data semantics lack spatial trace {sorted(missing)}")
        semantic_triggers = self.triggers & DATA_SEMANTIC_TRIGGERS
        if semantic_triggers and not self.data_items:
            self.issues.append(f"{self.label}.data_semantics: triggers {sorted(semantic_triggers)} "
                               "require non-empty authoritative data semantics")
        if semantic_triggers and not element_data:
            self.issues.append(f"{self.label}: data-bearing triggers require load-bearing element data trace")
        actors = {record.get("actor") for record in self.views.values()
                  if isinstance(record.get("actor"), str) and record.get("actor").strip()}
        flow_actors = {record.get("actor") for record in self.flows.values()
                       if isinstance(record.get("actor"), str) and record.get("actor").strip()}
        if "multi_role_permission" in self.triggers and (len(actors) < 2 or len(flow_actors) < 2):
            self.issues.append(f"{self.label}.views: multi_role_permission requires at least two "
                               "authoritative flow actors with role views")
        if "multi_role_permission" in self.triggers and flow_actors - actors:
            self.issues.append(f"{self.label}.views: authoritative flow actors lack views "
                               f"{sorted(flow_actors - actors)}")
        if "multi_role_permission" in self.triggers:
            self._multi_role_trace_issues()
        action_triggers = self.triggers & ACTION_RISK_TRIGGERS
        if action_triggers and not self.high_risk:
            self.issues.append(f"{self.label}: triggers {sorted(action_triggers)} require an "
                               "authoritative high-risk state-model action")

    def _multi_role_trace_issues(self):
        for view_id, view in self.views.items():
            flow_ids = view.get("flow_ids") if isinstance(view.get("flow_ids"), list) else []
            for flow_id in flow_ids:
                traced = any(view_id in set(element.get("view_ids", []))
                             and flow_id in set(element.get("flow_ids", []))
                             for element in self.elements.values())
                if not traced:
                    self.issues.append(f"{self.label}: view {view_id!r} flow {flow_id!r} lacks "
                                       "load-bearing spatial trace")

    def _page_state_floor_issues(self):
        kinds = {item_id: record.get("kind") for item_id, record in self.page_states.items()}
        recoverable = {item_id for item_id, record in self.page_states.items()
                       if isinstance(record.get("recovery_action_ids"), list)
                       and record.get("recovery_action_ids")}
        if "data_intensive" in self.triggers \
                and not {item_id for item_id in recoverable if kinds.get(item_id) in NON_NORMAL_DATA_STATES}:
            self.issues.append(f"{self.label}.page_states: data_intensive requires at least one "
                               "recoverable non-normal data state")
        risk_required = bool(self.high_risk or self.triggers & ACTION_RISK_TRIGGERS)
        risk_states = {item_id for item_id in recoverable
                       if kinds.get(item_id) in HIGH_RISK_PAGE_STATES}
        if risk_required and not risk_states:
            self.issues.append(f"{self.label}.page_states: high-risk UI requires at least one "
                               "recoverable blocked, conflict, failure, or unknown-result state")
        denied_states = {item_id for item_id in recoverable if kinds.get(item_id) == "denied"}
        if "multi_role_permission" in self.triggers and not denied_states:
            self.issues.append(f"{self.label}.page_states: multi_role_permission requires a "
                               "recoverable denied state")
        safety_states = {item_id for item_id in recoverable
                         if kinds.get(item_id) in HIGH_RISK_PAGE_STATES | NON_NORMAL_DATA_STATES}
        if "safety_critical_surface" in self.triggers and not safety_states:
            self.issues.append(f"{self.label}.page_states: safety_critical_surface requires at "
                               "least one recoverable risk or non-normal state")
        for action_id in sorted(self.high_risk):
            matching = [record for record in self.elements.values()
                        if action_id in set(record.get("action_ids", []))]
            feedback = {state_id for record in matching
                        for state_id in (record.get("feedback_state_ids") or [])}
            if matching and not feedback & risk_states:
                self.issues.append(f"{self.label}: high-risk action {action_id!r} lacks "
                                   "load-bearing risk feedback state trace")

    def _spatial_rule_issues(self):
        fields_by_collection = (("spacing_relations", SPACING_FIELDS), ("strokes", STROKE_FIELDS),
                                ("shape_rules", SHAPE_FIELDS), ("responsive_variants", RESPONSIVE_FIELDS))
        for field, fields in fields_by_collection:
            for item_id, record in self.indexes.get(field, {}).items():
                self.issues.extend(_shape(record, fields, f"{self.label}.{field}[{item_id!r}]"))
        for item_id, record in self.indexes.get("spacing_relations", {}).items():
            item = f"{self.label}.spacing_relations[{item_id!r}]"
            for field in ("from_ref", "to_ref"):
                if record.get(field) not in self.all_refs:
                    self.issues.append(f"{item}.{field}: unknown spatial reference")
            self.issues.extend(_text(record.get("relationship"), f"{item}.relationship", 128))
            self.issues.extend(_token(record.get("token_ref"), f"{item}.token_ref"))
        for item_id, record in self.indexes.get("strokes", {}).items():
            item = f"{self.label}.strokes[{item_id!r}]"
            if record.get("purpose") not in {"boundary", "separator", "guide", "relationship", "emphasis"}:
                self.issues.append(f"{item}.purpose: invalid")
            for field in ("start_anchor_ref", "end_anchor_ref"):
                if record.get(field) not in self.all_refs:
                    self.issues.append(f"{item}.{field}: unknown anchor")
            self.issues.extend(_token(record.get("token_ref"), f"{item}.token_ref"))

    def _shape_and_responsive_issues(self):
        for item_id, record in self.indexes.get("shape_rules", {}).items():
            item = f"{self.label}.shape_rules[{item_id!r}]"
            self.issues.extend(_text(record.get("semantic_role"), f"{item}.semantic_role", 128))
            self.issues.extend(_refs(record.get("subject_refs"), self.spatial_ids,
                                     f"{item}.subject_refs", non_empty=True))
            self.issues.extend(_token(record.get("family_token_ref"), f"{item}.family_token_ref"))
        region_ids = set(self.regions)
        for item_id, record in self.indexes.get("responsive_variants", {}).items():
            item = f"{self.label}.responsive_variants[{item_id!r}]"
            self.issues.extend(_text(record.get("environment_ref"), f"{item}.environment_ref", 128))
            dispositions = record.get("region_dispositions")
            if not isinstance(dispositions, dict) or set(dispositions) != region_ids:
                self.issues.append(f"{item}.region_dispositions: must cover every region exactly once")
            elif set(dispositions.values()) - DISPOSITIONS:
                self.issues.append(f"{item}.region_dispositions: contains invalid disposition")
            self.issues.extend(_text(record.get("reason"), f"{item}.reason"))

    def _optical_and_ai_issues(self):
        optical = self.data.get("optical_adjustments")
        for position, record in enumerate(optical if isinstance(optical, list) else []):
            item = f"{self.label}.optical_adjustments[{position}]"
            self.issues.extend(_shape(record, OPTICAL_FIELDS, item))
            if not isinstance(record, dict):
                continue
            if record.get("subject_ref") not in self.spatial_ids:
                self.issues.append(f"{item}.subject_ref: unknown spatial reference")
            if record.get("axis") not in {"x", "y", "baseline", "center"}:
                self.issues.append(f"{item}.axis: invalid")
            self.issues.extend(_token(record.get("policy_or_token_ref"), f"{item}.policy_or_token_ref"))
            self.issues.extend(_text(record.get("reason"), f"{item}.reason"))
            if record.get("reviewer_ref") != self.reviewer_id:
                self.issues.append(f"{item}.reviewer_ref: must match independent package reviewer")
        ai_ids = {item_id for item_id, record in self.data_items.items()
                  if record.get("kind") == "ai_recommendation"}
        for record in self.elements.values() if ai_ids else []:
            if set(record.get("data_ids", [])) & ai_ids and set(record.get("action_ids", [])) & self.high_risk:
                for data_id in set(record["data_ids"]) & ai_ids:
                    if self.data_items[data_id].get("human_confirmation") != "required":
                        self.issues.append(f"{self.label}: high-risk AI recommendation {data_id!r} lacks human confirmation")

    def run(self):
        self._header_and_indexes()
        self._view_issues()
        self._data_issues()
        self._page_state_issues()
        self._region_issues()
        self._axis_and_group_issues()
        primary = self._element_issues()
        self._semantic_floor_issues()
        self._page_state_floor_issues()
        self._spatial_rule_issues()
        self._shape_and_responsive_issues()
        self._optical_and_ai_issues()
        return self.issues, self.all_refs, primary, self.high_risk


def _composition_record_issues(data, label, flows, state_ids, actions, high_risk, triggers,
                               reviewer_id):
    issues = _shape(data, TOP_FIELDS, label)
    if not isinstance(data, dict):
        return issues, set(), set(), set()
    validator = _CompositionValidator(
        data, label, flows, state_ids, actions, high_risk, triggers, reviewer_id, issues,
    )
    return validator.run()


def _selected_compositions(artifacts):
    return {item_id: record for item_id, record in artifacts.items()
            if isinstance(record, dict)
            and record.get("media_type") == BUSINESS_UI_COMPOSITION_MEDIA_TYPE}


def _layout_composition_claims(claims):
    return [claim for claim in claims.values() if isinstance(claim, dict)
            and claim.get("subject_type") == "decision"
            and claim.get("subject_id") == "layout_component_composition"
            and "business_ui_composition" in set(claim.get("proof_types", []))]


def _package_flows(package):
    records = package.get("flows")
    return {record.get("id"): record for record in records if isinstance(record, dict)
            and isinstance(record.get("id"), str)} if isinstance(records, list) else {}


def _artifact_composition(artifact_id, artifact, repo_root, referenced, context):
    label, issues = f"FrontendDesignPackage.evidence_artifacts[{artifact_id!r}]", []
    if artifact.get("kind") != "source":
        issues.append(f"{label}: business UI composition must be a source artifact")
    if artifact_id not in referenced:
        issues.append(f"{label}: composition is not bound to the layout decision")
    path_issue = repo_path_issue(repo_root, artifact.get("locator"), f"{label}.locator")
    if path_issue:
        return issues + [path_issue], None
    data, read_issues = strict_json_file(repo_root / artifact.get("locator"), label)
    issues.extend(read_issues)
    record_issues, refs, primary, risky = _composition_record_issues(
        data, label, context["flows"], context["state_ids"], context["actions"],
        context["high_risk"], context["triggers"], context["reviewer_id"],
    )
    issues.extend(record_issues)
    if not isinstance(data, dict):
        return issues, None
    parsed = {"artifact_id": artifact_id, "contract": data, "refs": refs,
              "primary_flows": primary, "high_risk_actions": risky}
    return issues, (data.get("surface_id"), artifact.get("content_sha256"), parsed)


def composition_issues(package, repo_root, artifacts, claims, state_ids, actions, high_risk):
    """Validate applicable composition artifacts and return them by digest."""
    issues, parsed, surface_ids = [], {}, set()
    triggers = set(package.get("change_kinds", [])) \
        if isinstance(package.get("change_kinds"), list) else set()
    required, selected = bool(triggers & COMPOSITION_TRIGGERS), _selected_compositions(artifacts)
    if required and not selected:
        issues.append("FrontendDesignPackage: applicable UI change requires a business UI composition artifact")
    if len(selected) > 32:
        issues.append("FrontendDesignPackage: at most 32 business UI composition artifacts are allowed")
    layout_claims = _layout_composition_claims(claims)
    referenced = {ref for claim in layout_claims for ref in claim.get("artifact_ids", [])
                  if isinstance(ref, str)}
    review = package.get("review")
    context = {"flows": _package_flows(package), "state_ids": state_ids, "actions": actions,
               "high_risk": high_risk,
               "triggers": triggers,
               "reviewer_id": review.get("reviewer_id") if isinstance(review, dict) else None}
    for artifact_id, artifact in selected.items():
        artifact_issues, result = _artifact_composition(
            artifact_id, artifact, repo_root, referenced, context,
        )
        issues.extend(artifact_issues)
        if result is None:
            continue
        surface_id, digest, record = result
        if isinstance(surface_id, str):
            if surface_id in surface_ids:
                label = f"FrontendDesignPackage.evidence_artifacts[{artifact_id!r}]"
                issues.append(f"{label}.surface_id: duplicate across composition artifacts")
            surface_ids.add(surface_id)
        if isinstance(digest, str):
            parsed[digest] = record
    if required and not layout_claims:
        issues.append("FrontendDesignPackage: layout decision lacks business_ui_composition proof")
    return issues, parsed
