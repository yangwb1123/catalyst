#!/usr/bin/env python3
"""Shared valid AFDS fixtures for contract and adversarial tests."""
import base64
import hashlib
import json
import shutil
import tempfile
from pathlib import Path

import frontend_design_check as frontend
from frontend_design.composition import canonical_sha256

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
PNG_1X1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
)


def make_temp_repo():
    root = Path(tempfile.mkdtemp(prefix="forge-frontend-design-"))
    shutil.copytree(REPO_ROOT / ".agent", root / ".agent")
    target = root / frontend.STANDARD_REF
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / frontend.STANDARD_REF, target)
    (root / "evidence").mkdir()
    return root


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise AssertionError(f"fixture token not found: {old}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


class PackageBuilder:
    def __init__(self, root):
        self.root, self.artifacts, self.claims = root, [], []
        self.source_id = self.artifact("source", "source", b"authoritative source",
                                       producer="repository", producer_id="fixture-repository")

    def artifact(self, artifact_id, kind, data=b"fixture evidence", media_type="text/plain",
                 producer="tool", producer_id="fixture-tool"):
        suffix = ".png" if kind == "screenshot" else ".json" if (
            kind in {"trace", "visual_diff", "accessibility_report"} or "json" in media_type
        ) else ".txt"
        relative = f"evidence/{artifact_id}{suffix}"
        target = self.root / relative
        target.write_bytes(data)
        self.artifacts.append({
            "id": artifact_id, "kind": kind, "media_type": media_type,
            "locator": relative,
            "content_sha256": "sha256:" + hashlib.sha256(data).hexdigest(),
            "bytes": len(data), "source_revision": "working-tree",
            "integrity": "digest_bound", "provenance": "declarative",
            "producer": producer, "producer_id": producer_id,
        })
        return artifact_id

    @staticmethod
    def proof_artifact_kinds(proof_types):
        mapping = {
            "interaction_execution_receipts": "trace",
            "capture_receipt": "screenshot",
            "visual_diff_receipts": "visual_diff",
            "accessibility_execution_receipts": "accessibility_report",
            "geometry_measurement_receipts": "tool_output",
        }
        return {mapping.get(item, "tool_output") for item in proof_types}

    def claim(self, claim_id, claim_class, proof_types, subject_type, subject_id,
              artifact_ids=None, result=None):
        if artifact_ids is None:
            if claim_class == "source_observation":
                artifact_ids = [self.source_id]
            else:
                kinds = {"review_output"} if claim_class == "review_observation" \
                    else self.proof_artifact_kinds(proof_types)
                producer = "operator" if claim_class == "review_observation" else "tool"
                producer_id = "reviewer-1" if claim_class == "review_observation" else "fixture-tool"
                artifact_ids = [self.artifact(f"artifact-{claim_id}-{kind}", kind,
                                              PNG_1X1 if kind == "screenshot" else b"{}",
                                              "image/png" if kind == "screenshot" else "application/json",
                                              producer, producer_id)
                                for kind in sorted(kinds)]
        claimant = {"source_observation": "repository", "execution_observation": "tool",
                    "review_observation": "operator"}[claim_class]
        claimant_id = {"source_observation": "fixture-repository",
                       "execution_observation": "fixture-tool",
                       "review_observation": "reviewer-1"}[claim_class]
        if result is None:
            result = "passed" if claim_class == "execution_observation" else "observed"
        self.claims.append({
            "id": claim_id, "claim_class": claim_class,
            "proof_types": sorted(proof_types), "subject_type": subject_type,
            "subject_id": subject_id, "artifact_ids": artifact_ids,
            "claimant": claimant, "claimant_id": claimant_id, "result": result,
        })
        return claim_id

    def subject_claims(self, prefix, subject_type, subject_id, proof_types, source_artifacts=None):
        refs = []
        groups = (
            ("execution_observation", set(proof_types) & frontend.EXECUTION_PROOF_TYPES),
            ("review_observation", set(proof_types) & frontend.REVIEW_PROOF_TYPES),
        )
        consumed = set().union(*(group for _, group in groups))
        source_types = set(proof_types) - consumed
        if source_types:
            refs.append(self.claim(f"{prefix}-source", "source_observation", source_types,
                                   subject_type, subject_id,
                                   source_artifacts or [self.source_id]))
        for claim_class, values in groups:
            if values:
                refs.append(self.claim(f"{prefix}-{claim_class}", claim_class, values,
                                       subject_type, subject_id))
        return refs


def classified(builder, field, value):
    claim_id = builder.claim(f"class-{field}", "source_observation", {"classification_fact"},
                             "classification", field)
    return {"value": value, "claim_type": "fact", "confidence": 1.0,
            "proof_claim_id": claim_id, "assumption_id": ""}


def _composition_experience():
    return {
        "views": [{"id": "operator-view", "actor": "operator", "work_mode": "operation",
                   "flow_ids": ["primary"],
                   "primary_questions": ["Which record should be inspected?"]}],
        "data_semantics": [{
            "id": "record-status", "kind": "business_fact",
            "source_authority": "records-service", "definition": "Current record status.",
            "unit": "status-code", "time_basis": "current-version",
            "freshness_policy": "Refresh before effectful action.",
            "null_semantics": ["value", "unknown", "unavailable", "unauthorized"],
            "access_semantics": "permission_gated", "uncertainty_policy": "not_applicable",
            "explanation_policy": "Show the authoritative status label.",
            "human_confirmation": "not_applicable",
        }],
        "page_states": [{
            "id": "content-ready", "kind": "initial", "scope_region_ids": ["results"],
            "business_state_ids": ["ready"], "retains_previous_data": True,
            "action_ids": ["view"], "recovery_action_ids": ["view"],
            "feedback": "The selected record remains visible.",
        }],
    }


def _composition_structure():
    return {
        "regions": [
            {"id": "page", "parent_id": "", "semantic_role": "page-surface",
             "view_ids": ["operator-view"], "flow_ids": ["primary"],
             "action_ids": [], "data_ids": [], "priority": "primary",
             "axis_refs": ["content-left", "content-right"]},
            {"id": "results", "parent_id": "page", "semantic_role": "primary-results",
             "view_ids": ["operator-view"], "flow_ids": ["primary"],
             "action_ids": ["view"], "data_ids": ["record-status"], "priority": "primary",
             "axis_refs": ["content-left", "content-right"]},
        ],
        "axes": [
            {"id": "content-left", "orientation": "vertical", "alignment_edge": "left",
             "priority": 1, "scope_region_id": "page",
             "member_refs": ["page", "results", "result-group", "record-row"]},
            {"id": "content-right", "orientation": "vertical", "alignment_edge": "right",
             "priority": 1, "scope_region_id": "page",
             "member_refs": ["page", "results", "record-row"]},
        ],
        "groups": [{"id": "result-group", "region_id": "results",
                    "purpose": "Keep record identity and status together.",
                    "primary_axis_ref": "content-left", "member_refs": ["record-row"]}],
    }


def _composition_presentation_rules():
    return {
        "spacing_relations": [{"id": "page-to-results", "from_ref": "page",
                               "to_ref": "results", "relationship": "major-region",
                               "token_ref": "token:space.region.medium"}],
        "strokes": [{"id": "results-boundary", "purpose": "boundary",
                     "start_anchor_ref": "content-left", "end_anchor_ref": "content-right",
                     "token_ref": "token:stroke.subtle"}],
        "shape_rules": [{"id": "result-shape", "semantic_role": "interactive-row",
                         "subject_refs": ["record-row"],
                         "family_token_ref": "token:shape.result-row"}],
        "responsive_variants": [
            {"id": "desktop", "environment_ref": "policy:viewport.desktop",
             "region_dispositions": {"page": "present", "results": "present"},
             "reason": "The primary task remains available."},
            {"id": "narrow", "environment_ref": "policy:viewport.narrow",
             "region_dispositions": {"page": "present", "results": "present"},
             "reason": "The result list reflows without losing the primary task."},
        ],
        "load_bearing_elements": [{
            "id": "record-row", "region_id": "results", "view_ids": ["operator-view"],
            "flow_ids": ["primary"], "action_ids": ["view"],
            "data_ids": ["record-status"], "feedback_state_ids": ["content-ready"],
            "axis_refs": ["content-left", "content-right"],
        }],
        "optical_adjustments": [],
    }


def valid_composition(builder):
    contract = {
        "api_version": "forgeos.business-ui-composition/v1",
        "id": "records-list-composition", "surface_id": "records-list",
        **_composition_experience(),
        **_composition_structure(),
        **_composition_presentation_rules(),
    }
    payload = json.dumps(contract, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=False).encode("utf-8")
    artifact_id = builder.artifact(
        "business-ui-composition", "source", payload,
        frontend.BUSINESS_UI_COMPOSITION_MEDIA_TYPE,
        producer="repository", producer_id="fixture-repository",
    )
    artifact = next(item for item in builder.artifacts if item["id"] == artifact_id)
    return artifact_id, artifact["content_sha256"], contract


def valid_decisions(builder, composition_artifact_id):
    records = []
    for dimension in frontend.DIMENSION_OWNERS:
        source_artifacts = [builder.source_id, composition_artifact_id] \
            if dimension == "layout_component_composition" else None
        proof_refs = builder.subject_claims(
            f"decision-{dimension}", "decision", dimension,
            frontend.DIMENSION_PROOF_TYPES[dimension], source_artifacts,
        )
        fact_ref = builder.claim(f"decision-{dimension}-fact", "source_observation",
                                 {"source_fact"}, "decision", dimension)
        records.append({
            "id": dimension, "status": "addressed",
            "facts": [{"claim_type": "fact", "statement": f"Fact for {dimension}",
                       "proof_claim_id": fact_ref, "confidence": 1.0}],
            "decision": f"Bounded choice for {dimension}",
            "alternatives": ["Keep the current bounded behavior"],
            "rationale": "Current source and task scope support this choice.",
            "proof_claim_ids": proof_refs, "open_questions": [], "residual_risks": [],
            "reversibility": "high", "decision_kinds": ["other"],
            "migration_cost": "", "blast_radius": "", "adr_ref": "",
            "reviewer_id": "reviewer-1", "revisit_trigger": "",
        })
    return records


def valid_readiness(builder):
    records = []
    for dimension in frontend.READINESS_DIMENSIONS:
        refs = builder.subject_claims(f"readiness-{dimension}", "readiness", dimension,
                                      frontend.READINESS_PROOF_TYPES[dimension])
        records.append({"id": dimension, "result": "ready",
                        "rationale": "All typed observations are positive.",
                        "proof_claim_ids": refs})
    return records


def valid_classification(builder):
    values = {
        "product_type": "saas", "business_domain": "test", "page_pattern": "list",
        "profile_id": "generic_saas", "platform": "web_responsive", "density": "standard",
        "motion_level": 1, "operation_frequency": "medium", "data_density": "medium",
        "risk_level": "medium", "primary_user": "operator", "primary_task": "inspect records",
    }
    classification = {field: classified(builder, field, value) for field, value in values.items()}
    classification["rationale"] = "The requirement and current surface establish this profile."
    return classification


def valid_experience(builder):
    state_refs = builder.subject_claims("state-model", "state_model", "main",
                                        {"state_action_matrix", "permission_action_review"})
    state_model = {
        "id": "main", "initial_state": "ready", "proof_claim_ids": state_refs,
        "states": [{"id": "ready", "label": "Ready", "terminal": False,
                    "actions": [{"id": "view", "label": "View", "effect_class": "read_only",
                                 "permissions": ["records.view"], "data_guards": ["record_loaded"],
                                 "system_guards": ["service_available"], "next_states": ["ready"],
                                 "feedback": "The selected record is displayed.",
                                 "recovery": "Keep the list context and allow retry."}]}],
    }
    flow_refs = builder.subject_claims("flow-primary", "flow", "primary",
                                       {"operation_flow", "recovery_flow"})
    flows = [{
        "id": "primary", "kind": "primary", "actor": "operator",
        "goal": "Inspect one record", "entry": "records list", "trigger": "select record",
        "preconditions": ["records loaded"],
        "steps": [{"action_id": "view", "expected_feedback": "detail visible",
                   "expected_state": "ready", "context_preserved": True}],
        "terminal_outcome": "The requested record is visible.",
        "context_preservation": "Filters and list position are retained.",
        "permissions": ["records.view"], "risk_level": "low", "proof_claim_ids": flow_refs,
    }]
    return state_model, flows


def valid_cases(builder, source_tree, composition_sha256):
    build = "sha256:" + "4" * 64
    trace = builder.artifact("trace-primary", "trace")
    interaction_claim = builder.claim("case-interaction-claim", "execution_observation",
                                      {"interaction_execution_receipts"}, "verification_case",
                                      "interaction-1", [trace])
    screenshot = builder.artifact("capture-png", "screenshot", PNG_1X1, "image/png")
    diff = builder.artifact("capture-diff", "visual_diff", b"{}", "application/json")
    environment = {"platform": "chromium", "runtime": "playwright", "viewport": "1x1",
                   "dpr": 1, "theme": "light", "locale": "en-US", "timezone": "UTC",
                   "color_scheme": "light", "reduced_motion": True, "text_scale": 1,
                   "fonts_digest": "sha256:" + "5" * 64}
    report = {
        "api_version": "forgeos.ui-geometry-report/v1",
        "contract_sha256": composition_sha256, "case_id": "capture-1",
        "source_tree_sha256": source_tree, "build_sha256": build,
        "fixture_id": "fixture-1", "environment_sha256": canonical_sha256(environment),
        "coordinate_space": {
            "unit": "css_px", "origin": "capture_viewport_top_left",
            "axis_orientation": "x_right_y_down", "device_pixels_per_unit": 1,
        },
        "runner": {"name": "fixture-geometry-runner", "version": "1.0.0"},
        "assertions": [{
            "id": "content-axis", "type": "axis_alignment",
            "subject_refs": ["page", "results"], "required": True,
            "tolerance": {"value": 1.5, "policy_ref": "policy:geometry.axis-tolerance"},
            "observations": [{"subject_ref": "page", "value": 16.0},
                             {"subject_ref": "results", "value": 16.0}],
            "result": "passed",
        }],
    }
    report_bytes = json.dumps(report, sort_keys=True, separators=(",", ":")).encode("utf-8")
    report_id = builder.artifact("geometry-report", "tool_output", report_bytes,
                                 frontend.GEOMETRY_REPORT_MEDIA_TYPE)
    capture_claim = builder.claim("case-capture-claim", "execution_observation",
                                  {"capture_receipt", "visual_diff_receipts"},
                                  "verification_case", "capture-1", [screenshot, diff])
    geometry_claim = builder.claim("case-geometry-claim", "execution_observation",
                                   {"geometry_measurement_receipts"}, "verification_case",
                                   "capture-1", [report_id])
    return [
        {"id": "interaction-1", "kind": "interaction", "subject_id": "primary",
         "source_tree_sha256": source_tree, "build_sha256": build, "fixture_id": "fixture-1",
         "environment": environment, "artifact_ids": [trace],
         "proof_claim_ids": [interaction_claim]},
        {"id": "capture-1", "kind": "capture", "subject_id": "ready",
         "source_tree_sha256": source_tree, "build_sha256": build, "fixture_id": "fixture-1",
         "environment": environment, "artifact_ids": [screenshot, diff, report_id],
         "proof_claim_ids": [capture_claim, geometry_claim]},
    ]


def valid_package(root):
    builder = PackageBuilder(root)
    source_tree = "sha256:" + "1" * 64
    classification = valid_classification(builder)
    state_model, flows = valid_experience(builder)
    composition_id, composition_sha, _ = valid_composition(builder)
    cases = valid_cases(builder, source_tree, composition_sha)
    review_claim = builder.claim("overall-review", "review_observation", {"independent_review"},
                                 "review", "reviewer-1")
    return {
        "api_version": "forgeos.frontend-design/v1", "task_id": "TASK-UI-42",
        "source_revision": "working-tree", "source_tree_sha256": source_tree,
        "context_sha256": "sha256:" + "2" * 64,
        "requirements_sha256": "sha256:" + "3" * 64,
        "policy_sha256": "sha256:" + frontend.POLICY_SHA256,
        "profile_catalog_sha256": "sha256:" + frontend.PROFILE_SHA256,
        "schema_sha256": "sha256:" + frontend.SCHEMA_SHA256,
        "principals": {"package_author_id": "author-1", "implementer_id": "implementer-1"},
        "materiality": "L2", "workflow_profile": "W2_assured",
        "change_kinds": ["page_or_route"],
        "applicability": {"status": "required", "reason": "", "source_claim_ids": [],
                          "reviewer_id": "reviewer-1", "reviewer_independent": True},
        "review": {"reviewer_id": "reviewer-1", "independent": True,
                   "proof_claim_ids": [review_claim]},
        "classification": classification, "profile_overrides": [],
        "flows": flows, "state_model": state_model,
        "decisions": valid_decisions(builder, composition_id),
        "readiness": valid_readiness(builder), "verification_cases": cases,
        "assumptions": [], "evidence_artifacts": builder.artifacts,
        "proof_claims": builder.claims,
        "residual_risks": [{"id": "R-1", "severity": "low",
                            "statement": "Visual rendering can drift by environment.",
                            "mitigation": "Keep the capture environment pinned.",
                            "proof_claim_ids": []}],
    }


def decision(package, dimension):
    return next(item for item in package["decisions"] if item["id"] == dimension)
