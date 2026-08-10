#!/usr/bin/env python3
"""Adversarial tests for the BackendDecisionPackage shadow contract."""
import copy
import hashlib
import io
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
sys.path.insert(0, str(HARNESS_DIR))
import backend_decision_check as backend  # noqa: E402


def make_temp_repo():
    root = Path(tempfile.mkdtemp(prefix="forge-backend-decision-"))
    shutil.copytree(REPO_ROOT / ".agent", root / ".agent")
    standard = root / backend.STANDARD_REF
    standard.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / backend.STANDARD_REF, standard)
    return root


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise AssertionError(f"fixture token not found: {old}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def evidence_record(record_id, path, evidence_class, proof_types, subject_type, subject_id):
    target = REPO_ROOT / path
    producer = {"source_artifact": "repository", "tool_receipt": "tool",
                "review_receipt": "operator"}[evidence_class]
    result = "passed" if evidence_class == "tool_receipt" else "observed"
    return {
        "id": record_id, "kind": "repository_file", "evidence_class": evidence_class,
        "proof_types": sorted(proof_types), "subject_type": subject_type, "subject_id": subject_id,
        "locator": path, "content_sha256": "sha256:" + hashlib.sha256(target.read_bytes()).hexdigest(),
        "source_revision": "working-tree", "producer": producer,
        "producer_id": f"fixture-{producer}", "result": result,
    }


def subject_evidence(prefix, subject_type, subject_id, proof_types):
    tool_types = set(proof_types) & backend.TOOL_PROOF_TYPES
    source_types = set(proof_types) - tool_types
    records = []
    if source_types:
        records.append(evidence_record(f"{prefix}-source", ".agent/AGENTS.md", "source_artifact",
                                       source_types, subject_type, subject_id))
    if tool_types:
        records.append(evidence_record(f"{prefix}-tool", "harness/test_backend_decision_check.py",
                                       "tool_receipt", tool_types, subject_type, subject_id))
    return records


def valid_decision_bundle():
    decisions, evidence = [], []
    for dimension in backend.DIMENSION_OWNERS:
        records = subject_evidence(f"decision-{dimension}", "decision", dimension,
                                   backend.DIMENSION_PROOF_TYPES[dimension] | {"source_fact"})
        evidence.extend(records)
        proof_refs = [item["id"] for item in records]
        decisions.append({
            "id": dimension,
            "status": "addressed",
            "facts": [{
                "claim_type": "fact",
                "statement": f"Current fact for {dimension}",
                "evidence_id": f"decision-{dimension}-source",
                "confidence": 1.0,
            }],
            "decision": f"Bounded decision for {dimension}",
            "alternatives": ["Maintain current design"],
            "rationale": "The cited current state and task boundary support this choice.",
            "proof_refs": proof_refs,
            "open_questions": [],
            "residual_risks": [],
            "decision_kinds": ["other"],
            "reversibility": "high",
            "migration_cost": "",
            "blast_radius": "",
            "adr_ref": "",
            "reviewer_id": "reviewer-1",
            "revisit_trigger": "",
        })
    return decisions, evidence


def valid_readiness_bundle():
    readiness, evidence = [], []
    for dimension in backend.READINESS_DIMENSIONS:
        records = subject_evidence(f"readiness-{dimension}", "readiness", dimension,
                                   backend.READINESS_PROOF_TYPES[dimension])
        evidence.extend(records)
        readiness.append({"id": dimension, "result": "ready", "rationale": "Typed proof is positive.",
                          "proof_refs": [item["id"] for item in records]})
    return readiness, evidence


def valid_package():
    decisions, decision_evidence = valid_decision_bundle()
    readiness, readiness_evidence = valid_readiness_bundle()
    review_evidence = evidence_record("independent-review", ".agent/engineering/README.md",
                                      "review_receipt", {"independent_review"}, "review", "reviewer-1")
    return {
        "api_version": "forgeos.backend-decision/v1",
        "task_id": "TASK-42",
        "source_revision": "working-tree",
        "source_tree_sha256": "sha256:" + "1" * 64,
        "context_sha256": "sha256:" + "2" * 64,
        "requirements_sha256": "sha256:" + "3" * 64,
        "policy_sha256": "sha256:" + backend.POLICY_SHA256,
        "schema_sha256": "sha256:" + backend.SCHEMA_SHA256,
        "principals": {"package_author_id": "author-1", "implementer_id": "implementer-1"},
        "materiality": "L3",
        "workflow_profile": "W3_systemic",
        "change_kinds": ["backend_behavior", "persistence_schema"],
        "applicability": {
            "status": "required", "reason": "", "source_evidence_ids": [],
            "reviewer_id": "reviewer-1", "reviewer_independent": True,
        },
        "review": {"reviewer_id": "reviewer-1", "independent": True,
                   "evidence_ids": [review_evidence["id"]]},
        "decisions": decisions,
        "readiness": readiness,
        "assumptions": [{
            "id": "A-1",
            "statement": "Peak load remains within the observed envelope.",
            "confidence": 0.9,
            "impact_if_wrong": 0.2,
            "reversibility": 0.8,
            "affected_dimensions": ["performance_capacity"],
            "verification_plan": "Compare production traffic and saturation telemetry.",
            "proof_refs": [],
            "status": "unverified",
        }],
        "evidence": decision_evidence + readiness_evidence + [review_evidence],
        "residual_risks": [{
            "id": "R-1",
            "severity": "low",
            "statement": "The load assumption can drift.",
            "mitigation": "Alert on the documented scale trigger.",
            "proof_refs": [],
        }],
    }


def decision(package, dimension):
    return next(item for item in package["decisions"] if item["id"] == dimension)


class BackendContractTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self):
        return backend.check_backend_decision_contract(self.repo)

    def test_live_backend_contract_is_valid(self):
        self.assertEqual(self.issues(), [])
    def test_policy_cannot_claim_runtime_enforcement(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "runtime_binding: pre_code_shadow_review_only", "runtime_binding: enforced")
        self.assertTrue(any("runtime_binding" in issue for issue in self.issues()))
    def test_workflow_floor_cannot_be_lowered(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "workflow_floor: W1_standard", "workflow_floor: W0_direct")
        self.assertTrue(any("workflow_floor" in issue for issue in self.issues()))
    def test_trigger_cannot_be_deleted(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "    - destructive_migration\n", "")
        self.assertTrue(any("canonical set" in issue for issue in self.issues()))
    def test_model_separation_default_cannot_be_weakened(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "default: separate_when_owner_or_change_reason_differs", "default: reuse_everything")
        self.assertTrue(any("separation default" in issue for issue in self.issues()))
    def test_direct_orm_exposure_cannot_be_allowed(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "direct_orm_exposure_to_public_contract: prohibited", "direct_orm_exposure_to_public_contract: allowed")
        self.assertTrue(any("ORM exposure" in issue for issue in self.issues()))

    def test_dimension_owner_cannot_drift(self):
        path = self.repo / backend.POLICY_REF
        replace_once(path, "owner_skill: data-modeling-transactions", "owner_skill: backend-engineering")
        self.assertTrue(any("wrong owner Skill" in issue for issue in self.issues()))

    def test_skill_structure_is_required(self):
        path = self.repo / ".agent/skills/backend-engineering.md"
        replace_once(path, "## 输入契约 (Inputs)", "## Missing")
        self.assertTrue(any("missing required section '输入契约'" in issue for issue in self.issues()))

    def test_schema_cannot_add_self_completion(self):
        path = self.repo / backend.SCHEMA_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["artifact"]["forbidden_fields"].remove("accepted")
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("forbidden fields changed" in issue for issue in self.issues()))

    def test_schema_dimension_coverage_rule_is_required(self):
        path = self.repo / backend.SCHEMA_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["semantic_rules"][0]["id"] = "omitted_dimension_coverage"
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("semantic rules must be exactly" in issue for issue in self.issues()))

    def test_policy_nested_semantics_are_byte_pinned(self):
        path = self.repo / backend.POLICY_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["invariants"] = []
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical policy bytes changed" in issue for issue in self.issues()))

    def test_schema_nested_bounds_are_byte_pinned(self):
        path = self.repo / backend.SCHEMA_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["records"]["sourced_claim"]["fields"]["confidence"]["maximum"] = 999
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical schema bytes changed" in issue for issue in self.issues()))

    def test_policy_nested_wrong_types_fail_without_traceback(self):
        path = self.repo / backend.POLICY_REF
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["applicability"]["triggers"] = [{"malformed": True}]
        issues = backend.validate_backend_policy(data, self.repo)
        self.assertTrue(any("canonical set" in issue for issue in issues))

    def test_policy_deep_malformed_values_fail_without_exception(self):
        source = yaml.safe_load((self.repo / backend.POLICY_REF).read_text(encoding="utf-8"))
        mutations = (("dimensions", 0, "id", {}), ("dimensions", 1, "id", []),
                     ("dimensions", 0, "id", [{}]), ("primary_sources", 0, "url", None),
                     ("primary_sources", 1, "url", {}), ("primary_sources", 0, "url", []),
                     ("invariants", 0, "rule", None), ("invariants", 1, "rule", {}),
                     ("invariants", 2, "rule", []))
        for collection, index, field, value in mutations:
            with self.subTest(collection=collection, field=field, value=value):
                data = copy.deepcopy(source)
                data[collection][index][field] = value
                self.assertTrue(backend.validate_backend_policy(data, self.repo))

    def test_cli_fails_closed_on_deep_malformed_policy(self):
        path = self.repo / backend.POLICY_REF
        source = yaml.safe_load(path.read_text(encoding="utf-8"))
        mutations = (("dimensions", 0, "id", {}), ("dimensions", 1, "id", []),
                     ("primary_sources", 0, "url", None), ("primary_sources", 1, "url", {}))
        for collection, index, field, value in mutations:
            with self.subTest(collection=collection, field=field, value=value):
                data = copy.deepcopy(source)
                data[collection][index][field] = value
                path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
                with redirect_stdout(io.StringIO()) as output:
                    result = backend.main(["backend-decision-check", str(self.repo)])
                self.assertEqual(result, 1)
                self.assertIn("backend-decision-check: FAIL", output.getvalue())


class BackendPackageTest(unittest.TestCase):
    def test_valid_backend_package_is_accepted(self):
        self.assertEqual(backend.validate_backend_package(valid_package()), [])

    def test_missing_dimension_is_rejected(self):
        package = valid_package()
        package["decisions"].pop()
        self.assertTrue(any("cover every canonical dimension" in issue for issue in backend.validate_backend_package(package)))

    def test_duplicate_dimension_is_rejected(self):
        package = valid_package()
        package["decisions"][-1] = copy.deepcopy(package["decisions"][0])
        self.assertTrue(any("duplicate decision id" in issue for issue in backend.validate_backend_package(package)))

    def test_triggered_persistence_cannot_claim_not_applicable(self):
        package = valid_package()
        item = decision(package, "persistence")
        item.update(status="not_applicable", facts=[], decision="", alternatives=[], rationale="No state.", proof_refs=["proof:persistence"])
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("triggered dimension 'persistence'" in issue for issue in issues))

    def test_not_applicable_requires_proof(self):
        package = valid_package()
        item = decision(package, "security_tenancy_privacy")
        item.update(status="not_applicable", facts=[], decision="", alternatives=[], rationale="No security boundary.", proof_refs=[])
        self.assertTrue(any("expected non-empty list" in issue for issue in backend.validate_backend_package(package)))

    def test_addressed_requires_sourced_fact(self):
        package = valid_package()
        decision(package, "persistence")["facts"] = []
        self.assertTrue(any("addressed requires facts" in issue for issue in backend.validate_backend_package(package)))

    def test_blocked_cannot_claim_decision(self):
        package = valid_package()
        item = decision(package, "evolution_economics")
        item.update(status="blocked", open_questions=["Who owns the migration?"], decision="Chosen anyway")
        self.assertTrue(any("blocked requires open_questions and cannot claim" in issue for issue in backend.validate_backend_package(package)))

    def test_dangling_proof_is_rejected(self):
        package = valid_package()
        decision(package, "persistence")["proof_refs"] = ["proof:invented"]
        self.assertTrue(any("unknown evidence ids" in issue for issue in backend.validate_backend_package(package)))

    def test_nested_completion_claim_is_rejected(self):
        package = valid_package()
        package["assumptions"][0]["approved"] = True
        self.assertTrue(any("forbidden completion-authority field 'approved'" in issue for issue in backend.validate_backend_package(package)))

    def test_malformed_source_digest_is_rejected(self):
        package = valid_package()
        package["source_tree_sha256"] = "sha256:trust-me"
        self.assertTrue(any("requires sha256" in issue for issue in backend.validate_backend_package(package)))

    def test_destructive_migration_requires_l4(self):
        package = valid_package()
        package["change_kinds"] = ["destructive_migration"]
        package["materiality"] = "L3"
        self.assertTrue(any("below activated trigger floor" in issue for issue in backend.validate_backend_package(package)))

    def test_assumption_probability_bounds_are_enforced(self):
        package = valid_package()
        package["assumptions"][0]["impact_if_wrong"] = 2.0
        self.assertTrue(any("impact_if_wrong" in issue for issue in backend.validate_backend_package(package)))

    def test_unknown_change_kind_is_rejected(self):
        package = valid_package()
        package["change_kinds"] = ["trust_the_agent"]
        self.assertTrue(any("unknown trigger" in issue for issue in backend.validate_backend_package(package)))

    def test_evidence_locator_must_exist(self):
        package = valid_package()
        package["evidence"][0]["locator"] = "invented-proof.txt"
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("referenced path does not exist" in issue for issue in issues))

    def test_evidence_digest_is_recomputed(self):
        package = valid_package()
        package["evidence"][0]["content_sha256"] = "sha256:" + "9" * 64
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("does not match current file bytes" in issue for issue in issues))

    def test_open_assumption_cannot_be_confirmed_fact(self):
        package = valid_package()
        package["assumptions"][0]["statement"] = decision(package, "persistence")["facts"][0]["statement"]
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("open assumption cannot also be a confirmed fact" in issue for issue in issues))

    def test_fact_confidence_cannot_be_zero(self):
        package = valid_package()
        decision(package, "persistence")["facts"][0]["confidence"] = 0
        self.assertTrue(any("confirmed fact requires confidence 1.0" in issue
                            for issue in backend.validate_backend_package(package)))

    def test_low_reversibility_requires_controls(self):
        package = valid_package()
        decision(package, "persistence")["reversibility"] = "low"
        self.assertTrue(any("irreversible control fields are incomplete" in issue
                            for issue in backend.validate_backend_package(package)))

    def test_generic_file_cannot_close_all_proof_obligations(self):
        package = valid_package()
        generic = package["evidence"][0]["id"]
        for item in package["decisions"]:
            item["proof_refs"] = [generic]
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("missing required proof types" in issue for issue in issues))
        self.assertTrue(any("bound to a different subject" in issue for issue in issues))

    def test_tool_proof_cannot_be_labeled_as_source_artifact(self):
        package = valid_package()
        tool = next(item for item in package["evidence"] if item["evidence_class"] == "tool_receipt")
        tool.update(evidence_class="source_artifact", producer="repository", result="observed")
        self.assertTrue(any("match evidence_class" in issue
                            for issue in backend.validate_backend_package(package)))

    def test_reviewer_must_differ_from_implementer(self):
        package = valid_package()
        package["principals"]["implementer_id"] = package["review"]["reviewer_id"]
        self.assertTrue(any("reviewer must differ" in issue
                            for issue in backend.validate_backend_package(package)))

    def test_irreversible_adr_must_exist_and_reviewer_must_match(self):
        package = valid_package()
        item = decision(package, "persistence")
        item.update(reversibility="low", alternatives=["A", "B"], migration_cost="high",
                    blast_radius="system", adr_ref="docs/adr/DOES-NOT-EXIST.md",
                    reviewer_id="executor-self", revisit_trigger="new evidence")
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("reviewer must match" in issue for issue in issues))
        self.assertTrue(any("DOES-NOT-EXIST" in issue for issue in issues))

    def test_irreversible_adr_is_resolved_and_digest_bound(self):
        package = valid_package()
        item = decision(package, "persistence")
        adr = evidence_record("persistence-adr", "docs/design/ai-engineering-os/backend-decision-standard.md",
                              "source_artifact", {"adr"}, "decision", "persistence")
        package["evidence"].append(adr)
        item.update(reversibility="low", decision_kinds=["database_identity"], alternatives=["A", "B"],
                    migration_cost="high", blast_radius="system", adr_ref=adr["locator"],
                    reviewer_id="reviewer-1", revisit_trigger="new evidence",
                    proof_refs=item["proof_refs"] + [adr["id"]])
        self.assertEqual(backend.validate_backend_package(package), [])

    def test_public_contract_trigger_requires_irreversible_kind(self):
        package = valid_package()
        package["change_kinds"] = ["public_contract"]
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("requires decision kind 'public_contract'" in issue for issue in issues))

    def test_ready_requires_addressed_decision_dependencies(self):
        package = valid_package()
        item = decision(package, "persistence")
        item.update(status="blocked", facts=[], decision="", alternatives=[], proof_refs=[],
                    open_questions=["Which storage boundary is authoritative?"])
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("ready requires addressed decisions" in issue for issue in issues))

    def test_one_locator_cannot_claim_incompatible_evidence_classes(self):
        package = valid_package()
        tool = next(item for item in package["evidence"] if item["evidence_class"] == "tool_receipt")
        tool["locator"] = package["evidence"][0]["locator"]
        tool["content_sha256"] = package["evidence"][0]["content_sha256"]
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("multiple evidence classes" in issue for issue in issues))

    def test_malformed_rank_values_fail_closed_without_exception(self):
        for field in ("materiality", "workflow_profile"):
            for value in ({}, []):
                package = valid_package()
                package[field] = value
                self.assertTrue(backend.validate_backend_package(package))

    def test_classifier_is_total_for_malformed_collections(self):
        for field in ("applicability", "decisions", "readiness", "residual_risks"):
            package = valid_package()
            package[field] = "malformed"
            self.assertEqual(backend.classify_backend_package(package), "INVALID")

    def test_nested_malformed_values_fail_without_exception(self):
        mutations = (("decisions", "id", {}), ("decisions", "facts", None),
                     ("readiness", "result", []), ("evidence", "result", {}),
                     ("assumptions", "status", []), ("residual_risks", "severity", {}))
        for collection, field, value in mutations:
            with self.subTest(collection=collection, field=field):
                package = valid_package()
                package[collection][0][field] = value
                self.assertTrue(backend.validate_backend_package(package))

    def test_cli_fails_closed_on_malformed_package_without_traceback(self):
        package = valid_package()
        package["decisions"] = "malformed"
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yml", encoding="utf-8") as handle:
            yaml.safe_dump(package, handle)
            handle.flush()
            with redirect_stdout(io.StringIO()) as output:
                result = backend.main(["backend-decision-check", str(REPO_ROOT), handle.name])
                self.assertEqual(result, 1)
                self.assertIn("backend-decision-check: FAIL", output.getvalue())
    def test_evidence_file_size_is_bounded_before_hashing(self):
        package = valid_package()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            oversized = root / "oversized.bin"
            oversized.write_bytes(b"x" * (backend.MAX_EVIDENCE_BYTES + 1))
            package["evidence"][0].update(locator="oversized.bin",
                                          content_sha256="sha256:" + "0" * 64)
            issues = backend.validate_backend_package(package, root)
        self.assertTrue(any("evidence file exceeds" in issue for issue in issues))
    def test_readiness_dimension_cannot_be_omitted(self):
        package = valid_package()
        package["readiness"].pop()
        self.assertTrue(any("readiness must cover every canonical dimension" in issue
                            for issue in backend.validate_backend_package(package)))
    def test_multi_tenant_trigger_requires_w3(self):
        package = valid_package()
        package.update(change_kinds=["multi_tenant"], materiality="L2", workflow_profile="W2_assured")
        issues = backend.validate_backend_package(package)
        self.assertTrue(any("below activated trigger floor" in issue for issue in issues))
    def test_blocked_package_is_not_printed_as_pass(self):
        package = valid_package()
        item = decision(package, "business_invariants")
        item.update(status="blocked", decision="", open_questions=["Which rule is authoritative?"])
        self.assertEqual(backend.classify_backend_package(package), "VALID_BLOCKED")
    def test_critical_risk_is_not_ready(self):
        package = valid_package()
        package["residual_risks"][0]["severity"] = "critical"
        self.assertEqual(backend.classify_backend_package(package), "VALID_NOT_READY")


if __name__ == "__main__":
    unittest.main()
