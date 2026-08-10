#!/usr/bin/env python3
"""Adversarial tests for semantic floors and spatial ownership in UI composition."""
import copy
import hashlib
import json
import shutil
import sys
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
import frontend_design_check as frontend  # noqa: E402
import frontend_design_test_support as fixtures  # noqa: E402


def artifact(package, artifact_id):
    return next(item for item in package["evidence_artifacts"] if item["id"] == artifact_id)


class BusinessUiCompositionBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.repo = fixtures.make_temp_repo()
        self.package = fixtures.valid_package(self.repo)
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self):
        return frontend.validate_frontend_package(self.package, self.repo)

    def assert_issue(self, text):
        issues = self.issues()
        self.assertTrue(any(text in issue for issue in issues), issues)

    def mutate_composition(self, mutate):
        record = artifact(self.package, "business-ui-composition")
        path = self.repo / record["locator"]
        value = json.loads(path.read_text(encoding="utf-8"))
        mutate(value)
        payload = json.dumps(value, sort_keys=True, separators=(",", ":"),
                             ensure_ascii=False).encode("utf-8")
        path.write_bytes(payload)
        record["bytes"] = len(payload)
        record["content_sha256"] = "sha256:" + hashlib.sha256(payload).hexdigest()

    def activate(self, trigger, *, systemic=False, critical=False):
        self.package["change_kinds"].append(trigger)
        if critical:
            self.package["materiality"] = "L4"
            self.package["workflow_profile"] = "W3_systemic"
        elif systemic:
            self.package["materiality"] = "L3"
            self.package["workflow_profile"] = "W3_systemic"

    def add_recovery_action(self):
        self.package["state_model"]["states"].append({
            "id": "blocked", "label": "Blocked", "terminal": False,
            "actions": [{
                "id": "recover", "label": "Recover", "effect_class": "read_only",
                "permissions": ["records.view"], "data_guards": ["record_loaded"],
                "system_guards": ["service_available"], "next_states": ["ready"],
                "feedback": "The record is restored.", "recovery": "Retry safely.",
            }],
        })

    def test_data_intensive_trigger_requires_authoritative_data_semantics(self):
        self.activate("data_intensive")

        def mutate(value):
            value["data_semantics"] = []
            value["regions"][1]["data_ids"] = []
            value["load_bearing_elements"][0]["data_ids"] = []

        self.mutate_composition(mutate)
        self.assert_issue("require non-empty authoritative data semantics")

    def test_data_bearing_trigger_requires_load_bearing_element_trace(self):
        self.activate("data_intensive")
        self.mutate_composition(
            lambda value: value["load_bearing_elements"][0].update(data_ids=[])
        )
        self.assert_issue("data-bearing triggers require load-bearing element data trace")

    def test_data_intensive_trigger_requires_recoverable_non_normal_state(self):
        self.activate("data_intensive")
        self.assert_issue("data_intensive requires at least one recoverable non-normal data state")

    def test_data_intensive_semantic_floor_accepts_traced_data_and_recovery_state(self):
        self.activate("data_intensive")

        def mutate(value):
            value["page_states"].append({
                "id": "load-failed", "kind": "failure", "scope_region_ids": ["results"],
                "business_state_ids": ["ready"], "retains_previous_data": True,
                "action_ids": [], "recovery_action_ids": ["view"],
                "feedback": "Keep prior results and allow retry.",
            })
            value["load_bearing_elements"][0]["feedback_state_ids"].append("load-failed")

        self.mutate_composition(mutate)
        issues = self.issues()
        forbidden = ("require non-empty authoritative data semantics",
                     "data-bearing triggers require load-bearing element data trace",
                     "data_intensive requires at least one recoverable non-normal data state")
        self.assertFalse(any(text in issue for text in forbidden for issue in issues), issues)

    def test_declared_data_semantic_cannot_be_orphaned(self):
        def mutate(value):
            orphan = copy.deepcopy(value["data_semantics"][0])
            orphan["id"] = "orphan-risk"
            value["data_semantics"].append(orphan)

        self.mutate_composition(mutate)
        self.assert_issue("data semantics lack spatial trace ['orphan-risk']")

    def test_multi_role_trigger_requires_multiple_authoritative_flow_actors(self):
        self.activate("multi_role_permission", systemic=True)
        self.assert_issue("requires at least two authoritative flow actors with role views")
        self.assert_issue("multi_role_permission requires a recoverable denied state")

    def test_multi_role_view_flow_pair_requires_load_bearing_spatial_trace(self):
        self.activate("multi_role_permission", systemic=True)
        second = copy.deepcopy(self.package["flows"][0])
        second.update(id="audit-flow", kind="alternate", actor="auditor")
        self.package["flows"].append(second)

        def mutate(value):
            value["views"].append({
                "id": "auditor-view", "actor": "auditor", "work_mode": "audit",
                "flow_ids": ["audit-flow"], "primary_questions": ["Who changed the record?"],
            })

        self.mutate_composition(mutate)
        self.assert_issue("view 'auditor-view' flow 'audit-flow' lacks load-bearing spatial trace")

    def test_high_risk_trigger_requires_authoritative_state_model_action(self):
        self.activate("high_risk_action", systemic=True)
        self.assert_issue("require an authoritative high-risk state-model action")
        self.assert_issue("high-risk UI requires at least one recoverable blocked")

    def test_authentication_or_payment_requires_high_risk_action_and_recovery_state(self):
        self.activate("authentication_or_payment", critical=True)
        self.assert_issue("require an authoritative high-risk state-model action")
        self.assert_issue("high-risk UI requires at least one recoverable blocked")

    def test_read_only_safety_surface_does_not_invent_high_risk_action(self):
        self.activate("safety_critical_surface", critical=True)
        issues = self.issues()
        forbidden = ("require an authoritative high-risk state-model action",
                     "high-risk UI requires at least one recoverable blocked")
        self.assertFalse(any(text in issue for text in forbidden for issue in issues), issues)
        self.assertTrue(any("safety_critical_surface requires at least one recoverable" in issue
                            for issue in issues), issues)

    def test_page_state_actions_require_explicit_canonical_business_state(self):
        self.mutate_composition(
            lambda value: value["page_states"][0].update(business_state_ids=[])
        )
        self.assert_issue("business_state_ids: actions require at least one canonical business state")

    def test_waiting_state_without_actions_may_precede_business_object(self):
        def mutate(value):
            value["page_states"].append({
                "id": "initial-load", "kind": "loading", "scope_region_ids": ["results"],
                "business_state_ids": [], "retains_previous_data": False,
                "action_ids": [], "recovery_action_ids": [],
                "feedback": "The first record load is in progress.",
            })

        self.mutate_composition(mutate)
        issues = self.issues()
        self.assertFalse(any("page_states['initial-load'].business_state_ids: actions require"
                             in issue for issue in issues), issues)

    def test_recovery_action_must_be_available_from_declared_business_state(self):
        self.add_recovery_action()
        self.mutate_composition(
            lambda value: value["page_states"][0].update(recovery_action_ids=["recover"])
        )
        self.assert_issue("recovery_action_ids: action 'recover' is unavailable")

    def test_recovery_actions_must_cover_every_declared_business_state(self):
        self.add_recovery_action()
        self.mutate_composition(
            lambda value: value["page_states"][0].update(
                business_state_ids=["ready", "blocked"], recovery_action_ids=["view"])
        )
        self.assert_issue("no executable recovery for business states ['blocked']")

    def test_non_recoverable_normal_state_may_omit_recovery_actions(self):
        def mutate(value):
            value["page_states"].append({
                "id": "saved", "kind": "success", "scope_region_ids": ["results"],
                "business_state_ids": ["ready"], "retains_previous_data": True,
                "action_ids": [], "recovery_action_ids": [],
                "feedback": "The current record is saved.",
            })

        self.mutate_composition(mutate)
        issues = self.issues()
        self.assertFalse(any("page_states['saved'].recovery_action_ids: no executable" in issue
                             for issue in issues), issues)

    def test_region_and_axis_members_cannot_escape_axis_scope(self):
        self.mutate_composition(
            lambda value: value["axes"][0].update(scope_region_id="results")
        )
        issues = self.issues()
        self.assertTrue(any("scope is outside region ownership" in issue for issue in issues), issues)
        self.assertTrue(any("crosses axis scope ownership" in issue for issue in issues), issues)

    def test_axis_membership_requires_element_to_axis_reciprocity(self):
        self.mutate_composition(
            lambda value: value["axes"][0]["member_refs"].remove("record-row")
        )
        self.assert_issue("'record-row' declares axis 'content-left' but is absent")

    def test_axis_membership_requires_axis_to_element_reciprocity(self):
        self.mutate_composition(
            lambda value: value["load_bearing_elements"][0]["axis_refs"].remove("content-right")
        )
        self.assert_issue("axis 'content-right' lists 'record-row' without a reciprocal")

    def test_group_primary_axis_requires_exact_reciprocal_membership(self):
        self.mutate_composition(
            lambda value: value["axes"][0]["member_refs"].remove("result-group")
        )
        self.assert_issue("group 'result-group' declares primary axis 'content-left' but is absent")

    def test_group_member_cannot_cross_to_sibling_region(self):
        def mutate(value):
            value["regions"].append({
                "id": "sidebar", "parent_id": "page", "semantic_role": "context",
                "view_ids": ["operator-view"], "flow_ids": [], "action_ids": [],
                "data_ids": [], "priority": "contextual",
                "axis_refs": ["content-left", "content-right"],
            })
            for variant in value["responsive_variants"]:
                variant["region_dispositions"]["sidebar"] = "present"
            value["groups"][0]["region_id"] = "sidebar"

        self.mutate_composition(mutate)
        self.assert_issue("'record-row' crosses group region ownership")

    def test_load_bearing_element_requires_visual_group_ownership(self):
        self.mutate_composition(lambda value: value.update(groups=[]))
        self.assert_issue("load-bearing elements lack visual group ownership ['record-row']")

    def test_region_trace_cannot_substitute_for_high_risk_element_trace(self):
        self.activate("high_risk_action", systemic=True)
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["effect_class"] = "external_commit"
        self.mutate_composition(
            lambda value: value["load_bearing_elements"][0].update(action_ids=[])
        )
        self.assert_issue("high-risk actions lack load-bearing UI trace ['view']")

    def test_high_risk_element_requires_risk_feedback_state_trace(self):
        self.activate("high_risk_action", systemic=True)
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["effect_class"] = "external_commit"
        self.assert_issue("high-risk action 'view' lacks load-bearing risk feedback state trace")


if __name__ == "__main__":
    unittest.main()
