#!/usr/bin/env python3
"""Adversarial tests for the FrontendDesignPackage shadow contract."""
import base64
import copy
import hashlib
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
sys.path.insert(0, str(HARNESS_DIR))
import frontend_design_check as frontend  # noqa: E402

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
        suffix = ".png" if kind == "screenshot" else ".json" if kind in {
            "trace", "visual_diff", "accessibility_report"
        } else ".txt"
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

    def subject_claims(self, prefix, subject_type, subject_id, proof_types):
        refs = []
        groups = (
            ("execution_observation", set(proof_types) & frontend.EXECUTION_PROOF_TYPES),
            ("review_observation", set(proof_types) & frontend.REVIEW_PROOF_TYPES),
        )
        consumed = set().union(*(group for _, group in groups))
        source_types = set(proof_types) - consumed
        if source_types:
            refs.append(self.claim(f"{prefix}-source", "source_observation", source_types,
                                   subject_type, subject_id))
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


def valid_decisions(builder):
    records = []
    for dimension in frontend.DIMENSION_OWNERS:
        proof_refs = builder.subject_claims(f"decision-{dimension}", "decision", dimension,
                                            frontend.DIMENSION_PROOF_TYPES[dimension])
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


def valid_cases(builder, source_tree):
    build = "sha256:" + "4" * 64
    trace = builder.artifact("trace-primary", "trace")
    interaction_claim = builder.claim("case-interaction-claim", "execution_observation",
                                      {"interaction_execution_receipts"}, "verification_case",
                                      "interaction-1", [trace])
    screenshot = builder.artifact("capture-png", "screenshot", PNG_1X1, "image/png")
    diff = builder.artifact("capture-diff", "visual_diff", b"{}", "application/json")
    capture_claim = builder.claim("case-capture-claim", "execution_observation",
                                  {"capture_receipt", "visual_diff_receipts"},
                                  "verification_case", "capture-1", [screenshot, diff])
    environment = {"platform": "chromium", "runtime": "playwright", "viewport": "1x1",
                   "dpr": 1, "theme": "light", "locale": "en-US", "timezone": "UTC",
                   "color_scheme": "light", "reduced_motion": True, "text_scale": 1,
                   "fonts_digest": "sha256:" + "5" * 64}
    cases = [
        {"id": "interaction-1", "kind": "interaction", "subject_id": "primary",
         "source_tree_sha256": source_tree, "build_sha256": build, "fixture_id": "fixture-1",
         "environment": environment, "artifact_ids": [trace],
         "proof_claim_ids": [interaction_claim]},
        {"id": "capture-1", "kind": "capture", "subject_id": "ready",
         "source_tree_sha256": source_tree, "build_sha256": build, "fixture_id": "fixture-1",
         "environment": environment, "artifact_ids": [screenshot, diff],
         "proof_claim_ids": [capture_claim]},
    ]
    return cases


def valid_package(root):
    builder = PackageBuilder(root)
    source_tree = "sha256:" + "1" * 64
    classification = valid_classification(builder)
    state_model, flows = valid_experience(builder)
    cases = valid_cases(builder, source_tree)
    review_claim = builder.claim("overall-review", "review_observation", {"independent_review"},
                                 "review", "reviewer-1")
    package = {
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
        "decisions": valid_decisions(builder), "readiness": valid_readiness(builder),
        "verification_cases": cases, "assumptions": [],
        "evidence_artifacts": builder.artifacts, "proof_claims": builder.claims,
        "residual_risks": [{"id": "R-1", "severity": "low",
                            "statement": "Visual rendering can drift by environment.",
                            "mitigation": "Keep the capture environment pinned.", "proof_claim_ids": []}],
    }
    return package


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
