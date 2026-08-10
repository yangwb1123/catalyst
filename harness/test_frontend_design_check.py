#!/usr/bin/env python3
"""Adversarial tests for the FrontendDesignPackage shadow contract."""
import copy
import hashlib
import shutil
import sys
import unittest
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
import frontend_design_check as frontend  # noqa: E402
import frontend_design_test_support as fixture_support  # noqa: E402

PNG_1X1 = fixture_support.PNG_1X1


def make_temp_repo():
    return fixture_support.make_temp_repo()


def replace_once(path, old, new):
    fixture_support.replace_once(path, old, new)


def valid_package(root):
    return fixture_support.valid_package(root)


def decision(package, dimension):
    return next(item for item in package["decisions"] if item["id"] == dimension)


class FrontendContractTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self):
        return frontend.check_frontend_design_contract(self.repo)

    def test_live_contract_is_structurally_valid(self):
        self.assertEqual(self.issues(), [])

    def test_policy_cannot_claim_enforcement(self):
        replace_once(self.repo / frontend.POLICY_REF, "pre_code_shadow_review_only", "enforced")
        self.assertTrue(any("runtime_binding" in issue or "canonical policy bytes" in issue
                            for issue in self.issues()))

    def test_trigger_and_floor_are_pinned(self):
        replace_once(self.repo / frontend.POLICY_REF,
                     "shared_token_or_component: { materiality: L3",
                     "shared_token_or_component: { materiality: L1")
        self.assertTrue(any("trigger floors" in issue for issue in self.issues()))

    def test_profile_catalog_cannot_lose_profile(self):
        path = self.repo / frontend.PROFILE_REF
        replace_once(path, "  - id: ai_agent_workspace", "  - id: removed_agent_workspace")
        self.assertTrue(any("profile ids changed" in issue for issue in self.issues()))

    def test_schema_cannot_add_completion_authority(self):
        path = self.repo / frontend.SCHEMA_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["artifact"]["forbidden_fields"].remove("accepted")
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("forbidden completion fields" in issue for issue in self.issues()))

    def test_skill_structure_is_required(self):
        path = self.repo / ".agent/skills/frontend-client-engineering.md"
        replace_once(path, "## 输入契约", "## Missing")
        self.assertTrue(any("missing required section '输入契约'" in issue for issue in self.issues()))

    def test_ui_geometry_skill_structure_is_required(self):
        path = self.repo / ".agent/skills/ui-geometry.md"
        replace_once(path, "## 自动化与验收", "## Missing")
        self.assertTrue(any("ui-geometry.md: missing required section '自动化与验收'" in issue
                            for issue in self.issues()))


class FrontendPackageTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.package = valid_package(self.repo)
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self):
        return frontend.validate_frontend_package(self.package, self.repo)

    def test_valid_package_is_structurally_valid(self):
        self.assertEqual(self.issues(), [])
        self.assertEqual(frontend.classify_frontend_package(self.package, []), "STRUCTURALLY_VALID")

    def test_recursive_completion_claim_is_rejected(self):
        self.package["classification"]["approved"] = True
        self.assertTrue(any("forbidden completion-authority" in issue for issue in self.issues()))

    def test_malformed_nested_shapes_fail_closed_without_traceback(self):
        mutations = (("classification", []), ("state_model", []), ("flows", {}),
                     ("change_kinds", 42), ("verification_cases", {}))
        for field, value in mutations:
            with self.subTest(field=field):
                package = valid_package(self.repo)
                package[field] = value
                self.assertTrue(frontend.validate_frontend_package(package, self.repo))

    def test_trigger_floor_cannot_be_underreported(self):
        self.package["change_kinds"] = ["authentication_or_payment"]
        self.assertTrue(any("below activated trigger floor" in issue for issue in self.issues()))

    def test_unknown_profile_and_pattern_are_rejected(self):
        for field in ("profile_id", "page_pattern"):
            with self.subTest(field=field):
                package = copy.deepcopy(self.package)
                package["classification"][field]["value"] = "invented"
                self.assertTrue(frontend.validate_frontend_package(package, self.repo))

    def test_inference_requires_assumption(self):
        item = self.package["classification"]["profile_id"]
        item.update(claim_type="inference", confidence=0.7, proof_claim_id="", assumption_id="missing")
        self.assertTrue(any("must resolve to an assumption" in issue for issue in self.issues()))

    def test_missing_and_duplicate_dimension_are_rejected(self):
        self.package["decisions"].pop()
        self.assertTrue(any("cover every canonical dimension" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        self.package["decisions"][-1] = copy.deepcopy(self.package["decisions"][0])
        self.assertTrue(any("duplicate decision id" in issue for issue in self.issues()))

    def test_triggered_dimension_cannot_escape_as_na(self):
        item = decision(self.package, "business_task")
        item.update(status="not_applicable", facts=[], decision="", alternatives=[],
                    rationale="No task.", reversibility="not_applicable")
        self.assertTrue(any("triggered dimension 'business_task'" in issue for issue in self.issues()))

    def test_blocked_cannot_claim_decision(self):
        item = decision(self.package, "motion_performance")
        item.update(status="blocked", open_questions=["What budget applies?"], decision="chosen")
        self.assertTrue(any("cannot claim a decision" in issue for issue in self.issues()))

    def test_load_bearing_assumption_blocks_affected_dimension(self):
        self.package["assumptions"] = [{"id": "A-1", "statement": "Roles never change",
            "confidence": 0.2, "impact_if_wrong": 1.0, "reversibility": 0.0,
            "affected_dimensions": ["state_permission_action"],
            "verification_plan": "Verify the policy source.", "proof_claim_ids": [],
            "status": "unverified"}]
        self.assertTrue(any("must block 'state_permission_action'" in issue for issue in self.issues()))

    def test_flow_requires_primary_and_resolved_action(self):
        self.package["flows"][0]["kind"] = "alternative"
        self.assertTrue(any("primary flow" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        self.package["flows"][0]["steps"][0]["action_id"] = "missing"
        self.assertTrue(any("unknown action" in issue for issue in self.issues()))

    def test_mutation_requires_error_and_recovery(self):
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["effect_class"] = "reversible_write"
        self.assertTrue(any("mutating UI requires error and recovery" in issue for issue in self.issues()))

    def test_high_risk_action_requires_trigger_and_cancel_paths(self):
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["effect_class"] = "external_commit"
        issues = self.issues()
        self.assertTrue(any("high-risk action trigger" in issue for issue in issues))
        self.assertTrue(any("high-risk action requires cancel" in issue for issue in issues))

    def test_form_requires_cancel_flow(self):
        self.package["classification"]["page_pattern"]["value"] = "form"
        self.assertTrue(any("form requires a cancel flow" in issue for issue in self.issues()))

    def test_state_next_and_duplicate_action_are_rejected(self):
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["next_states"] = ["missing"]
        self.assertTrue(any("unknown states" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        state = self.package["state_model"]["states"][0]
        state["actions"].append(copy.deepcopy(state["actions"][0]))
        self.assertTrue(any("duplicate action id" in issue for issue in self.issues()))

    def test_self_review_is_rejected(self):
        self.package["review"]["reviewer_id"] = "author-1"
        self.assertTrue(any("reviewer must differ" in issue for issue in self.issues()))

    def test_digest_mismatch_and_path_escape_are_rejected(self):
        self.package["evidence_artifacts"][0]["content_sha256"] = "sha256:" + "0" * 64
        self.assertTrue(any("current file bytes" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        self.package["evidence_artifacts"][0]["locator"] = "../escape"
        self.assertTrue(any("unsafe repository path" in issue for issue in self.issues()))

    def test_source_claim_cannot_impersonate_execution(self):
        claim = next(item for item in self.package["proof_claims"]
                     if item["claim_class"] == "source_observation")
        claim["proof_types"] = ["interaction_execution_receipts"]
        self.assertTrue(any("do not match claim_class" in issue for issue in self.issues()))

    def test_non_positive_execution_cannot_prove_readiness(self):
        claim = next(item for item in self.package["proof_claims"]
                     if "accessibility_execution_receipts" in item["proof_types"])
        claim["result"] = "not_executed"
        self.assertTrue(any("requires observed/passed claims" in issue for issue in self.issues()))

    def test_subject_mismatch_is_rejected(self):
        claim = next(item for item in self.package["proof_claims"]
                     if "operation_flow" in item["proof_types"])
        claim["subject_id"] = "another-flow"
        self.assertTrue(any("different subject" in issue for issue in self.issues()))

    def test_non_png_screenshot_and_dimension_mismatch_are_rejected(self):
        artifact = next(item for item in self.package["evidence_artifacts"] if item["kind"] == "screenshot")
        target = self.repo / artifact["locator"]
        target.write_bytes(b"not png")
        artifact["bytes"] = 7
        artifact["content_sha256"] = "sha256:" + hashlib.sha256(b"not png").hexdigest()
        self.assertTrue(any("structurally decodable PNG" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        capture = next(item for item in self.package["verification_cases"] if item["kind"] == "capture")
        capture["environment"]["viewport"] = "2x2"
        self.assertTrue(any("viewport×DPR" in issue for issue in self.issues()))

    def test_screenshot_reuse_and_unbound_case_artifact_are_rejected(self):
        capture = copy.deepcopy(next(item for item in self.package["verification_cases"] if item["kind"] == "capture"))
        capture["id"] = "capture-2"
        for claim_id in capture["proof_claim_ids"]:
            claim = copy.deepcopy(next(item for item in self.package["proof_claims"] if item["id"] == claim_id))
            claim["id"] += "-2"
            claim["subject_id"] = "capture-2"
            self.package["proof_claims"].append(claim)
        capture["proof_claim_ids"] = [item + "-2" for item in capture["proof_claim_ids"]]
        self.package["verification_cases"].append(capture)
        self.assertTrue(any("identical capture bytes reused" in issue for issue in self.issues()))
        self.package = valid_package(self.repo)
        case = next(item for item in self.package["verification_cases"] if item["kind"] == "interaction")
        extra = next(item["id"] for item in self.package["evidence_artifacts"] if item["kind"] == "source")
        case["artifact_ids"].append(extra)
        self.assertTrue(any("not bound by its proof claims" in issue for issue in self.issues()))

    def test_ready_requires_interaction_and_capture_cases(self):
        self.package["verification_cases"] = []
        issues = self.issues()
        self.assertTrue(any("interaction readiness" in issue for issue in issues))
        self.assertTrue(any("visual readiness" in issue for issue in issues))

    def test_critical_risk_and_blocked_decision_classify_without_completion_claim(self):
        self.package["residual_risks"][0]["severity"] = "critical"
        self.assertEqual(frontend.classify_frontend_package(self.package, []), "VALID_NOT_READY")
        decision(self.package, "motion_performance")["status"] = "blocked"
        self.assertEqual(frontend.classify_frontend_package(self.package, []), "VALID_BLOCKED")
        self.assertEqual(frontend.classify_frontend_package(self.package, ["bad"]), "INVALID")


if __name__ == "__main__":
    unittest.main()
