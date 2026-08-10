#!/usr/bin/env python3
"""Adversarial coverage for business UI composition and geometry reports."""
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


def json_artifact(package, repo, artifact_id):
    record = artifact(package, artifact_id)
    return record, json.loads((repo / record["locator"]).read_text(encoding="utf-8"))


def replace_json(package, repo, artifact_id, value):
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=False).encode("utf-8")
    return replace_bytes(package, repo, artifact_id, payload)


def replace_bytes(package, repo, artifact_id, payload):
    record = artifact(package, artifact_id)
    (repo / record["locator"]).write_bytes(payload)
    record["bytes"] = len(payload)
    record["content_sha256"] = "sha256:" + hashlib.sha256(payload).hexdigest()
    return record["content_sha256"]


class BusinessUiGeometryTest(unittest.TestCase):
    def setUp(self):
        self.repo = fixtures.make_temp_repo()
        self.package = fixtures.valid_package(self.repo)
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self, package=None):
        return frontend.validate_frontend_package(package or self.package, self.repo)

    def assert_issue(self, text, package=None):
        issues = self.issues(package)
        self.assertTrue(any(text in issue for issue in issues), issues)

    def mutate_composition(self, mutate):
        _, value = json_artifact(self.package, self.repo, "business-ui-composition")
        mutate(value)
        return replace_json(self.package, self.repo, "business-ui-composition", value)

    def mutate_report(self, mutate):
        _, value = json_artifact(self.package, self.repo, "geometry-report")
        mutate(value)
        replace_json(self.package, self.repo, "geometry-report", value)

    def test_valid_composition_and_report_are_structurally_valid(self):
        self.assertEqual(self.issues(), [])

    def test_applicable_page_requires_composition_artifact(self):
        self.package["evidence_artifacts"] = [item for item in self.package["evidence_artifacts"]
                                              if item["id"] != "business-ui-composition"]
        self.assert_issue("requires a business UI composition artifact")

    def test_composition_must_be_bound_to_layout_decision(self):
        claim = next(item for item in self.package["proof_claims"]
                     if "business_ui_composition" in item["proof_types"])
        claim["artifact_ids"].remove("business-ui-composition")
        self.assert_issue("composition is not bound to the layout decision")

    def test_composition_rejects_duplicate_json_keys(self):
        replace_bytes(self.package, self.repo, "business-ui-composition", b'{"id":"a","id":"b"}')
        self.assert_issue("duplicate JSON key 'id'")

    def test_composition_rejects_non_finite_json_numbers(self):
        replace_bytes(self.package, self.repo, "business-ui-composition", b'{"value":NaN}')
        self.assert_issue("non-finite JSON number 'NaN'")

    def test_duplicate_and_cyclic_regions_fail_closed(self):
        def mutate(value):
            value["regions"][0]["parent_id"] = "results"
            value["regions"][1]["parent_id"] = "page"
            value["regions"].append(copy.deepcopy(value["regions"][1]))

        self.mutate_composition(mutate)
        issues = self.issues()
        self.assertTrue(any("duplicate id" in issue for issue in issues), issues)
        self.assertTrue(any("parent cycle" in issue for issue in issues), issues)

    def test_dangling_axis_and_raw_spacing_value_are_rejected(self):
        def mutate(value):
            value["regions"][0]["axis_refs"] = ["missing-axis"]
            value["spacing_relations"][0]["token_ref"] = "13px"

        self.mutate_composition(mutate)
        self.assert_issue("unknown references ['missing-axis']")
        self.assert_issue("requires token:/policy:/profile: symbolic reference")

    def test_axis_and_spatial_ids_cannot_be_ambiguous(self):
        self.mutate_composition(lambda value: value["axes"][0].update(id="page"))
        self.assert_issue("region/group/element/axis ids share an ambiguous reference")

    def test_primary_flow_requires_load_bearing_trace(self):
        def mutate(value):
            for region in value["regions"]:
                region["flow_ids"] = []
            value["load_bearing_elements"][0]["flow_ids"] = []

        self.mutate_composition(mutate)
        self.assert_issue("primary flows lack load-bearing UI trace")

    def test_high_risk_action_requires_load_bearing_trace(self):
        self.package["change_kinds"].append("high_risk_action")
        self.package["materiality"] = "L3"
        self.package["workflow_profile"] = "W3_systemic"
        action = self.package["state_model"]["states"][0]["actions"][0]
        action["effect_class"] = "external_commit"

        def mutate(value):
            for region in value["regions"]:
                region["action_ids"] = []
            value["load_bearing_elements"][0]["action_ids"] = []

        self.mutate_composition(mutate)
        self.assert_issue("high-risk actions lack load-bearing UI trace")

    def test_ai_recommendation_cannot_omit_human_confirmation(self):
        def mutate(value):
            item = value["data_semantics"][0]
            item["kind"] = "ai_recommendation"
            item["human_confirmation"] = "optional"

        self.mutate_composition(mutate)
        self.assert_issue("AI recommendation requires human confirmation")

    def test_ambiguous_null_semantic_is_rejected(self):
        self.mutate_composition(
            lambda value: value["data_semantics"][0].update(null_semantics=["value", "empty"])
        )
        self.assert_issue("contains unknown values ['empty']")

    def test_responsive_variant_must_place_every_region(self):
        self.mutate_composition(
            lambda value: value["responsive_variants"][0]["region_dispositions"].pop("results")
        )
        self.assert_issue("must cover every region exactly once")

    def test_optical_adjustment_requires_symbolic_source_and_independent_reviewer(self):
        def mutate(value):
            value["optical_adjustments"] = [{
                "subject_ref": "record-row", "axis": "y", "policy_or_token_ref": "2px",
                "reason": "Asymmetric icon box.", "reviewer_ref": "author-1",
            }]

        self.mutate_composition(mutate)
        issues = self.issues()
        self.assertTrue(any("requires token:/policy:/profile" in issue for issue in issues), issues)
        self.assertTrue(any("must match independent package reviewer" in issue for issue in issues), issues)

    def test_geometry_report_context_must_match_capture_case(self):
        self.mutate_report(lambda value: value.update(fixture_id="other-fixture"))
        self.assert_issue("fixture_id: does not match capture case")

    def test_geometry_report_contract_digest_must_resolve(self):
        self.mutate_report(lambda value: value.update(contract_sha256="sha256:" + "9" * 64))
        self.assert_issue("does not resolve to a composition artifact")

    def test_geometry_report_rejects_score_only_or_unknown_fields(self):
        self.mutate_report(lambda value: value.update(score=100))
        self.assert_issue("unknown fields ['score']")

    def test_passed_claim_cannot_hide_required_failure(self):
        self.mutate_report(lambda value: value["assertions"][0].update(result="failed"))
        self.assert_issue("passed claim cannot hide failed/inconclusive required assertions")

    def test_honest_negative_geometry_is_valid_when_visual_evidence_is_not_ready(self):
        for result in ("failed", "inconclusive", "not_executed"):
            with self.subTest(result=result):
                repo = fixtures.make_temp_repo()
                self.addCleanup(shutil.rmtree, repo, ignore_errors=True)
                package = fixtures.valid_package(repo)
                _, report = json_artifact(package, repo, "geometry-report")
                report["assertions"][0]["result"] = result
                if result == "not_executed":
                    report["assertions"][0]["observations"] = []
                replace_json(package, repo, "geometry-report", report)
                claim = next(item for item in package["proof_claims"]
                             if "geometry_measurement_receipts" in item["proof_types"])
                claim["result"] = result
                readiness = next(item for item in package["readiness"]
                                 if item["id"] == "visual_evidence")
                readiness.update(result="not_ready", rationale="Geometry evidence is not positive.",
                                 proof_claim_ids=[claim["id"]])
                issues = frontend.validate_frontend_package(package, repo)
                self.assertEqual(issues, [])
                self.assertEqual(frontend.classify_frontend_package(package, issues),
                                 "VALID_NOT_READY")

    def test_non_geometry_verification_claim_still_requires_positive_result(self):
        claim = next(item for item in self.package["proof_claims"]
                     if "interaction_execution_receipts" in item["proof_types"]
                     and item["subject_type"] == "verification_case")
        claim["result"] = "failed"
        self.assert_issue("requires observed/passed claims")

    def test_honest_failed_geometry_cannot_support_ready_visual_state(self):
        self.mutate_report(lambda value: value["assertions"][0].update(result="failed"))
        claim = next(item for item in self.package["proof_claims"]
                     if "geometry_measurement_receipts" in item["proof_types"])
        claim["result"] = "failed"
        self.assert_issue("visual readiness lacks geometry reports")

    def test_negative_claim_must_match_required_assertion_aggregate(self):
        self.mutate_report(lambda value: value["assertions"][0].update(result="inconclusive"))
        claim = next(item for item in self.package["proof_claims"]
                     if "geometry_measurement_receipts" in item["proof_types"])
        claim["result"] = "failed"
        self.assert_issue("does not match required assertion result 'inconclusive'")

    def test_required_assertion_aggregate_uses_failure_precedence(self):
        def mutate(value):
            template = value["assertions"][0]
            template.update(id="not-run", result="not_executed", observations=[])
            for assertion_id, result in (("uncertain", "inconclusive"), ("failed", "failed")):
                assertion = copy.deepcopy(template)
                assertion.update(id=assertion_id, result=result,
                                 observations=[{"subject_ref": "page", "value": 16.0},
                                               {"subject_ref": "results", "value": 16.0}])
                value["assertions"].append(assertion)

        self.mutate_report(mutate)
        claim = next(item for item in self.package["proof_claims"]
                     if "geometry_measurement_receipts" in item["proof_types"])
        claim["result"] = "failed"
        readiness = next(item for item in self.package["readiness"]
                         if item["id"] == "visual_evidence")
        readiness.update(result="not_ready", rationale="Required geometry failed.",
                         proof_claim_ids=[claim["id"]])
        self.assertEqual(self.issues(), [])

    def test_geometry_report_cannot_make_every_assertion_optional(self):
        self.mutate_report(lambda value: value["assertions"][0].update(
            required=False, result="not_executed", observations=[]))
        self.assert_issue("at least one assertion must be required")

    def test_visual_readiness_requires_geometry_report_for_capture(self):
        case = next(item for item in self.package["verification_cases"] if item["kind"] == "capture")
        case["artifact_ids"].remove("geometry-report")
        case["proof_claim_ids"].remove("case-geometry-claim")
        self.package["proof_claims"] = [item for item in self.package["proof_claims"]
                                        if item["id"] != "case-geometry-claim"]
        self.package["evidence_artifacts"] = [item for item in self.package["evidence_artifacts"]
                                              if item["id"] != "geometry-report"]
        self.assert_issue("visual readiness lacks geometry reports")

    def test_executed_assertion_observes_every_declared_subject_once(self):
        def mutate(value):
            value["assertions"][0]["observations"] = [
                {"subject_ref": "page", "value": 16.0},
                {"subject_ref": "page", "value": 16.0},
                {"subject_ref": "record-row", "value": 16.0},
            ]

        self.mutate_report(mutate)
        issues = self.issues()
        self.assertTrue(any("subject_ref values must be unique" in issue for issue in issues), issues)
        self.assertTrue(any("missing asserted subjects ['results']" in issue for issue in issues), issues)
        self.assertTrue(any("subjects outside assertion ['record-row']" in issue for issue in issues), issues)

    def test_not_executed_assertion_rejects_observations(self):
        self.mutate_report(lambda value: value["assertions"][0].update(result="not_executed"))
        self.assert_issue("not_executed assertion cannot contain observations")

    def test_geometry_claim_requires_geometry_report_media_type(self):
        artifact(self.package, "geometry-report")["media_type"] = "application/json"
        self.assert_issue("geometry proof lacks geometry report")


if __name__ == "__main__":
    unittest.main()
