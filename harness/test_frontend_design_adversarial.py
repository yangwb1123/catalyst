#!/usr/bin/env python3
"""Regression tests for fail-closed FrontendDesignPackage validation."""
import copy
import hashlib
import math
import shutil
import struct
import sys
import unittest
import zlib
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
import frontend_design_check as frontend  # noqa: E402
import test_frontend_design_check as fixtures  # noqa: E402


def artifact(package, artifact_id):
    return next(item for item in package["evidence_artifacts"] if item["id"] == artifact_id)


def append_artifact(package, repo, artifact_id, kind, data, media_type,
                    producer="tool", producer_id="fixture-tool", locator=None):
    relative = locator or f"evidence/{artifact_id}.bin"
    target = repo / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    package["evidence_artifacts"].append({
        "id": artifact_id, "kind": kind, "media_type": media_type,
        "locator": relative,
        "content_sha256": "sha256:" + hashlib.sha256(data).hexdigest(),
        "bytes": len(data), "source_revision": package["source_revision"],
        "integrity": "digest_bound", "provenance": "declarative",
        "producer": producer, "producer_id": producer_id,
    })
    return artifact_id


def append_claim(package, claim_id, claim_class, proof_types, subject_type, subject_id,
                 artifact_ids, claimant=None, claimant_id=None):
    defaults = {
        "source_observation": ("repository", "fixture-repository", "observed"),
        "execution_observation": ("tool", "fixture-tool", "passed"),
        "review_observation": ("operator", "reviewer-1", "observed"),
    }
    default_claimant, default_id, result = defaults[claim_class]
    package["proof_claims"].append({
        "id": claim_id, "claim_class": claim_class,
        "proof_types": sorted(proof_types), "subject_type": subject_type,
        "subject_id": subject_id, "artifact_ids": artifact_ids,
        "claimant": claimant or default_claimant,
        "claimant_id": claimant_id or default_id, "result": result,
    })
    return claim_id


def png_with_text(data, text_value):
    payload = b"Comment\x00" + text_value.encode("ascii")
    typed = b"tEXt" + payload
    chunk = struct.pack(">I", len(payload)) + typed
    chunk += struct.pack(">I", zlib.crc32(typed) & 0xFFFFFFFF)
    return data[:-12] + chunk + data[-12:]


def append_flow(package, flow_id, kind, action_id="view", risk="low"):
    claim_id = append_claim(
        package, f"flow-{flow_id}-claim", "source_observation",
        {"operation_flow", "recovery_flow"}, "flow", flow_id, ["source"],
    )
    base = copy.deepcopy(package["flows"][0])
    base.update(id=flow_id, kind=kind, risk_level=risk, proof_claim_ids=[claim_id])
    base["steps"][0]["action_id"] = action_id
    package["flows"].append(base)


def add_controlled_decision(package, repo, dimension="business_task"):
    record = fixtures.decision(package, dimension)
    record.update(
        reversibility="low", alternatives=["Keep", "Replace"],
        migration_cost="A coordinated migration is required.",
        blast_radius="The shared client surface is affected.",
        adr_ref="docs/adr/frontend-test.md", reviewer_id="reviewer-1",
        revisit_trigger="Revisit when the public contract changes.",
    )
    artifact_id = append_artifact(
        package, repo, "frontend-test-adr", "source", b"# Test ADR\n",
        "text/markdown", "repository", "fixture-repository", record["adr_ref"],
    )
    claim_id = append_claim(
        package, "frontend-test-adr-claim", "source_observation",
        {"architecture_decision_record"}, "decision", dimension, [artifact_id],
    )
    record["proof_claim_ids"].append(claim_id)
    return record


class FrontendAdversarialTest(unittest.TestCase):
    def setUp(self):
        self.repo = fixtures.make_temp_repo()
        self.package = fixtures.valid_package(self.repo)
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def validate(self, package=None):
        return frontend.validate_frontend_package(package or self.package, self.repo)

    def assert_rejected(self, package=None, contains=None):
        issues = self.validate(package)
        self.assertIsInstance(issues, list)
        self.assertTrue(issues)
        if contains:
            self.assertTrue(any(contains in issue for issue in issues), issues)
        return issues

    def test_unhashable_nested_values_fail_closed_without_crash(self):
        mutations = (
            lambda item: item["principals"].__setitem__("package_author_id", []),
            lambda item: item["decisions"][0].__setitem__("id", []),
            lambda item: item["decisions"][0].__setitem__("decision_kinds", [[]]),
            lambda item: item["readiness"][0].__setitem__("id", {}),
            lambda item: item["proof_claims"][0].__setitem__("proof_types", [[]]),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                package = fixtures.valid_package(self.repo)
                mutate(package)
                self.assert_rejected(package)

    def test_nan_and_infinity_fail_closed_without_crash(self):
        for value in (math.nan, math.inf, -math.inf):
            with self.subTest(value=value):
                package = fixtures.valid_package(self.repo)
                package["classification"]["risk_level"]["confidence"] = value
                package["verification_cases"][1]["environment"]["dpr"] = value
                self.assert_rejected(package, "finite positive number")

    def test_deep_nesting_fails_closed_without_recursion_crash(self):
        nested = "leaf"
        for _ in range(1500):
            nested = {"next": nested}
        self.package["unexpected_deep_value"] = nested
        self.assert_rejected(contains="unknown fields")

    def test_flow_expected_state_must_follow_action_transition(self):
        self.package["state_model"]["states"].append({
            "id": "other", "label": "Other", "terminal": False, "actions": [],
        })
        self.package["flows"][0]["steps"][0]["expected_state"] = "other"
        self.assert_rejected(contains="not allowed by action.next_states")

    def test_terminal_state_cannot_transition_to_another_state(self):
        state = self.package["state_model"]["states"][0]
        state["terminal"] = True
        state["actions"][0]["next_states"] = ["other"]
        self.package["state_model"]["states"].append({
            "id": "other", "label": "Other", "terminal": False, "actions": [],
        })
        self.assert_rejected(contains="terminal state cannot transition")

    def test_uncovered_write_action_is_rejected(self):
        action = copy.deepcopy(self.package["state_model"]["states"][0]["actions"][0])
        action.update(id="write", label="Write", effect_class="reversible_write")
        self.package["state_model"]["states"][0]["actions"].append(action)
        self.assert_rejected(contains="mutating actions lack flow coverage ['write']")

    def test_high_risk_action_requires_its_own_interaction_evidence(self):
        action = copy.deepcopy(self.package["state_model"]["states"][0]["actions"][0])
        action.update(id="commit", label="Commit", effect_class="external_commit")
        self.package["state_model"]["states"][0]["actions"].append(action)
        self.package["change_kinds"].append("high_risk_action")
        self.package["materiality"], self.package["workflow_profile"] = "L3", "W3_systemic"
        self.package["classification"]["risk_level"]["value"] = "high"
        append_flow(self.package, "risky", "alternative", "commit", "high")
        for kind in ("cancel", "error", "recovery"):
            append_flow(self.package, kind, kind)
        self.assert_rejected(contains="high-risk action 'commit' lacks interaction evidence")

    def test_png_signature_without_decodable_image_is_rejected(self):
        image = next(item for item in self.package["evidence_artifacts"]
                     if item["kind"] == "screenshot")
        fake = b"\x89PNG\r\n\x1a\n" + b"\x00" * 16
        (self.repo / image["locator"]).write_bytes(fake)
        image["bytes"] = len(fake)
        image["content_sha256"] = "sha256:" + hashlib.sha256(fake).hexdigest()
        self.assert_rejected(contains="structurally decodable PNG")

    def test_capture_bytes_cannot_be_reused_under_a_different_artifact_id(self):
        capture = next(item for item in self.package["verification_cases"]
                       if item["kind"] == "capture")
        original = artifact(self.package, "capture-png")
        screenshot = append_artifact(
            self.package, self.repo, "capture-png-copy", "screenshot",
            (self.repo / original["locator"]).read_bytes(), "image/png",
        )
        visual_diff = append_artifact(
            self.package, self.repo, "capture-diff-copy", "visual_diff",
            b'{"different":true}', "application/json",
        )
        claim = append_claim(
            self.package, "capture-copy-claim", "execution_observation",
            {"capture_receipt", "visual_diff_receipts"}, "verification_case",
            "capture-copy", [screenshot, visual_diff],
        )
        copied = copy.deepcopy(capture)
        copied.update(id="capture-copy", artifact_ids=[screenshot, visual_diff],
                      proof_claim_ids=[claim])
        self.package["verification_cases"].append(copied)
        self.assert_rejected(contains="identical capture bytes reused")

    def test_claim_only_visual_diff_cannot_bypass_capture_reuse_check(self):
        capture = next(item for item in self.package["verification_cases"]
                       if item["kind"] == "capture")
        capture["artifact_ids"].remove("capture-diff")
        screenshot = append_artifact(
            self.package, self.repo, "capture-unique", "screenshot",
            png_with_text(fixtures.PNG_1X1, "unique"), "image/png",
        )
        visual_diff = append_artifact(
            self.package, self.repo, "capture-diff-copy", "visual_diff",
            b"{}", "application/json",
        )
        claim = append_claim(
            self.package, "capture-claim-only-copy", "execution_observation",
            {"capture_receipt", "visual_diff_receipts"}, "verification_case",
            "capture-claim-only", [screenshot, visual_diff],
        )
        copied = copy.deepcopy(capture)
        copied.update(id="capture-claim-only", artifact_ids=[screenshot],
                      proof_claim_ids=[claim])
        self.package["verification_cases"].append(copied)
        self.assert_rejected(contains="claim artifacts are not declared by the case")

    def test_visual_diff_receipt_requires_visual_diff_artifact(self):
        case = next(item for item in self.package["verification_cases"]
                    if item["kind"] == "capture")
        case["artifact_ids"].remove("capture-diff")
        claim = next(item for item in self.package["proof_claims"]
                     if item["id"] == case["proof_claim_ids"][0])
        claim["artifact_ids"].remove("capture-diff")
        self.package["evidence_artifacts"] = [
            item for item in self.package["evidence_artifacts"] if item["id"] != "capture-diff"
        ]
        self.assert_rejected(contains="requires artifact kind ['visual_diff']")

    def test_decision_premise_cannot_be_disguised_as_inference(self):
        fact = self.package["decisions"][0]["facts"][0]
        fact.update(claim_type="inference", confidence=0.7, proof_claim_id="")
        self.assert_rejected(contains="decision premises must be facts")

    def test_rejected_assumption_cannot_support_classification(self):
        self.package["assumptions"].append({
            "id": "A-rejected", "statement": "The page is always compact.",
            "confidence": 0.8, "impact_if_wrong": 0.1, "reversibility": 1.0,
            "affected_dimensions": ["scenario_profile"],
            "verification_plan": "Inspect the product profile.",
            "proof_claim_ids": [], "status": "rejected",
        })
        classified = self.package["classification"]["density"]
        classified.update(claim_type="inference", confidence=0.7,
                          proof_claim_id="", assumption_id="A-rejected")
        self.assert_rejected(contains="rejected assumption cannot support an inference")

    def test_profile_deviation_requires_exact_typed_override(self):
        self.package["classification"]["density"]["value"] = "compact"
        self.assert_rejected(contains="must exactly cover density/motion deviations")

    def test_digest_bound_profile_override_is_accepted(self):
        self.package["classification"]["density"]["value"] = "compact"
        output = append_artifact(
            self.package, self.repo, "profile-review", "review_output", b"reviewed",
            "text/plain", "operator", "reviewer-1",
        )
        claim = append_claim(
            self.package, "profile-review-claim", "review_observation",
            {"profile_override_review"}, "profile_override", "generic_saas:density", [output],
        )
        self.package["profile_overrides"] = [{
            "field": "density", "default": "standard", "selected": "compact",
            "reason": "Dense operational data needs compact rows.",
            "scope": "This route only.", "risk": "low",
            "compensating_proof_claim_ids": [claim], "reviewer_id": "reviewer-1",
            "expires_at": "2999-12-31",
        }]
        self.assertEqual(self.validate(), [])

    def test_critical_profile_override_raises_effective_risk_floor(self):
        self.package["classification"]["density"]["value"] = "compact"
        output = append_artifact(
            self.package, self.repo, "critical-profile-review", "review_output",
            b"critical review", "text/plain", "operator", "reviewer-1",
        )
        claim = append_claim(
            self.package, "critical-profile-review-claim", "review_observation",
            {"profile_override_review"}, "profile_override",
            "generic_saas:density", [output],
        )
        self.package["profile_overrides"] = [{
            "field": "density", "default": "standard", "selected": "compact",
            "reason": "The deviation has critical operational impact.",
            "scope": "This route only.", "risk": "critical",
            "compensating_proof_claim_ids": [claim], "reviewer_id": "reviewer-1",
            "expires_at": "2999-12-31",
        }]
        issues = self.assert_rejected(contains="materiality: below declared risk floor")
        self.assertTrue(any("workflow_profile: below declared risk floor" in issue
                            for issue in issues), issues)

    def test_public_contract_and_classifier_apis_fail_closed_on_malformed_input(self):
        schema_path = self.repo / frontend.SCHEMA_REF
        schema = yaml.safe_load(schema_path.read_text(encoding="utf-8"))
        schema["artifact"]["required_fields"] = [[]]
        issues = frontend.validate_frontend_schema(schema)
        self.assertTrue(issues)
        self.assertEqual(frontend.classify_frontend_package(None, []), "INVALID")
        self.assertEqual(frontend.classify_frontend_package({}, []), "INVALID")
        malformed = {field: None for field in frontend.PACKAGE_FIELDS}
        self.assertEqual(frontend.classify_frontend_package(malformed, []), "INVALID")

    def test_review_claimant_identity_must_match_package_reviewer(self):
        claim = next(item for item in self.package["proof_claims"]
                     if item["id"] == "overall-review")
        claim["claimant_id"] = "reviewer-2"
        artifact(self.package, claim["artifact_ids"][0])["producer_id"] = "reviewer-2"
        self.assert_rejected(contains="review claimant must match package reviewer")

    def test_controlled_decision_adr_is_bound_to_exact_repository_path(self):
        record = add_controlled_decision(self.package, self.repo)
        self.assertEqual(self.validate(), [])
        alternate = self.repo / "docs/adr/another.md"
        alternate.write_text("# Another ADR\n", encoding="utf-8")
        record["adr_ref"] = "docs/adr/another.md"
        self.assert_rejected(contains="ADR must be digest-bound")

    def test_declared_critical_risk_raises_materiality_and_workflow_floor(self):
        self.package["classification"]["risk_level"]["value"] = "critical"
        issues = self.assert_rejected(contains="materiality: below declared risk floor")
        self.assertTrue(any("workflow_profile: below declared risk floor" in issue for issue in issues), issues)


if __name__ == "__main__":
    unittest.main()
