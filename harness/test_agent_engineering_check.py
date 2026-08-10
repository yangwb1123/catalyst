#!/usr/bin/env python3
"""Tests for the machine-readable Agent Engineering governance contracts."""
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
CHECKER = HARNESS_DIR / "agent_engineering_check.py"
sys.path.insert(0, str(HARNESS_DIR))
import agent_engineering_check as engineering  # noqa: E402


def _copy_tree(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, target)


def make_temp_repo():
    root = Path(tempfile.mkdtemp(prefix="forge-engineering-"))
    for relative in (
        ".agent", ".ai", ".arch", "harness", "docs/release",
        "docs/design/ai-engineering-os", "docs/adr", "docs/contracts",
    ):
        _copy_tree(REPO_ROOT / relative, root / relative)
    return root


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if text.count(old) < 1:
        raise AssertionError(f"fixture token not found: {old}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


class SpecValidationTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_live_contract_copy_passes(self):
        self.assertEqual(self.issues(), [])

    def test_missing_discipline_is_rejected(self):
        path = self.agent_root / "engineering" / "disciplines.yml"
        replace_once(path, "  - id: contract\n", "  - id: removed-contract\n")
        self.assertTrue(any("disciplines must be exactly" in issue for issue in self.issues()))

    def test_duplicate_rule_id_is_rejected(self):
        path = self.agent_root / "engineering" / "rules.yml"
        replace_once(path, "  - id: SEC-001\n", "  - id: TRUTH-001\n")
        self.assertTrue(any("duplicate rule id" in issue for issue in self.issues()))

    def test_error_rule_without_automatic_detector_is_rejected(self):
        path = self.agent_root / "engineering" / "rules.yml"
        replace_once(path, "      mode: automatic\n", "      mode: review\n")
        self.assertTrue(any("must be automatic" in issue for issue in self.issues()))

    def test_duplicate_yaml_key_is_rejected(self):
        path = self.agent_root / "engineering" / "disciplines.yml"
        replace_once(path, "kind: DisciplineRegistry\n", "kind: DisciplineRegistry\nkind: Duplicate\n")
        self.assertTrue(any("duplicate key" in issue for issue in self.issues()))

    def test_context_path_escape_is_rejected(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "ref: harness/policies.yml", "ref: ../outside-secret")
        self.assertTrue(any("unsafe repository path" in issue for issue in self.issues()))

    def test_context_symlink_escape_is_rejected(self):
        target = self.agent_root / "skills" / "testing.md"
        outside = self.repo.parent / f"{self.repo.name}-outside"
        outside.write_text("outside", encoding="utf-8")
        self.addCleanup(outside.unlink, missing_ok=True)
        target.unlink()
        target.symlink_to(outside)
        self.assertTrue(any("escapes repository through a symlink" in issue for issue in self.issues()))

    def test_unknown_workflow_is_rejected(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        replace_once(path, "required_workflows: [build]", "required_workflows: [ghost] ")
        self.assertTrue(any("unknown required_workflows" in issue for issue in self.issues()))

    def test_profile_gate_vocabulary_cannot_fork_modes(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        replace_once(path, "gate_catalog: [lint, test, build, complexity, arch, security]",
                     "gate_catalog: [lint, test, build, complexity, arch, security, invented]")
        self.assertTrue(any("must reuse policies/modes.yml exactly" in issue for issue in self.issues()))

    def test_risk_floor_cannot_be_downgraded(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        replace_once(path, "L4: W3_systemic", "L4: W1_standard")
        self.assertTrue(any("canonical L0-W0" in issue for issue in self.issues()))

    def test_all_workflow_profiles_cannot_be_downgraded_together(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        for profile in data["profiles"]:
            profile["required_workflows"] = ["build"]
            profile["required_gates"] = ["lint"]
            profile["required_reviewers"] = []
            profile["proof_obligations"] = ["self_attestation"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("below its" in issue for issue in self.issues()))

    def test_execution_autonomy_ceilings_cannot_be_raised_together(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        for profile in data["profiles"]:
            profile["autonomy"]["execution"] = 1.0
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("execution autonomy exceeds" in issue for issue in self.issues()))

    def test_learning_autonomy_ceilings_cannot_be_raised_together(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        for profile in data["profiles"]:
            profile["autonomy"]["learning"] = 1.0
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("learning autonomy exceeds" in issue for issue in self.issues()))

    def test_repair_attempt_ceilings_cannot_be_raised_together(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        for profile in data["profiles"]:
            profile["stop_conditions"]["max_repair_attempts"] = 99
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("repair-attempt ceiling" in issue for issue in self.issues()))

    def test_assured_profiles_cannot_disable_human_gate(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["profiles"][2]["human_gate_required"] = False
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("requires a human gate" in issue for issue in self.issues()))

    def test_assured_profiles_cannot_skip_human_approval_on_success(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["profiles"][2]["stop_conditions"]["success"].remove("human_approval")
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("human_approval before success" in issue for issue in self.issues()))

    def test_human_approval_cannot_replace_other_success_proofs(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["profiles"][2]["stop_conditions"]["success"] = ["human_approval"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("success stop-condition floor" in issue for issue in self.issues()))

    def test_blocked_stop_conditions_cannot_drop_safety_guards(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["profiles"][3]["stop_conditions"]["blocked"] = ["authority_missing"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("blocked stop-condition floor" in issue for issue in self.issues()))

    def test_stop_condition_lists_cannot_be_replaced_by_strings(self):
        path = self.agent_root / "engineering" / "workflow-profiles.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["profiles"][2]["stop_conditions"]["success"] = "human_approval"
        data["profiles"][2]["stop_conditions"]["blocked"] = "authority_missing"
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("requires non-empty string-list" in issue for issue in self.issues()))

    def test_project_binding_cannot_claim_enforcement(self):
        path = self.agent_root / "project.yml"
        replace_once(path, "activation: shadow", "activation: enforce")
        self.assertTrue(any("shadow-only" in issue for issue in self.issues()))

    def test_completion_schema_requires_honesty_invariants(self):
        path = self.agent_root / "eval" / "completion-evidence.schema.yml"
        replace_once(path, "  - id: decision_fields_forbidden\n", "  - id: missing-invariant\n")
        self.assertTrue(any("invariants must be exactly" in issue for issue in self.issues()))

    def test_legacy_project_without_binding_uses_shadow_default(self):
        path = self.agent_root / "project.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data.pop("engineering_spec")
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertEqual(self.issues(), [])

    def test_backend_extension_cannot_leave_canonical_registry(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"]["backend_policy"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue for issue in self.issues()))

    def test_frontend_extension_cannot_leave_canonical_registry(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"]["frontend_profiles"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue for issue in self.issues()))

    def test_frontend_architecture_extension_cannot_leave_registry(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"]["frontend_architecture_contract"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue for issue in self.issues()))

    def test_frontend_architecture_policy_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "frontend-code-architecture.yml"
        replace_once(path, "completion_authority: forge_accept", "completion_authority: self")
        self.assertTrue(any("frontend architecture policy" in issue for issue in self.issues()))

    def test_frontend_architecture_json_malformed_is_rejected(self):
        path = self.repo / ".arch" / "frontend-architecture.v1.json"
        path.write_text('{"schema":', encoding="utf-8")
        self.assertTrue(any("contract validator failed" in issue for issue in self.issues()))

    def test_frontend_architecture_skill_floor_is_required(self):
        path = self.agent_root / "skills" / "frontend-code-architecture.md"
        replace_once(path, "## 例外合同 (Exceptions)", "## Removed exceptions")
        self.assertTrue(any("missing required section '例外合同'" in issue for issue in self.issues()))

    def test_fake_automatic_detector_path_is_rejected(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        replace_once(path, "[python3, harness/check.py]", "[python3, .agent/AGENTS.md]")
        issues = self.issues()
        self.assertTrue(any("not a registered forge_accept detector" in issue for issue in issues))

    def test_error_rule_cannot_reference_shadow_detector(self):
        path = self.agent_root / "engineering" / "rules.yml"
        replace_once(path, "detector_refs: [harness.secret_scan]",
                     "detector_refs: [completion.evidence_package]")
        self.assertTrue(any("non-enforced detector" in issue for issue in self.issues()))

    def test_detector_must_declare_rule_coverage(self):
        path = self.agent_root / "engineering" / "rules.yml"
        replace_once(path, "detector_refs: [harness.secret_scan]",
                     "detector_refs: [harness.architecture]")
        self.assertTrue(any("does not declare coverage" in issue for issue in self.issues()))

    def test_enforced_detector_requires_registered_negative_test(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        replace_once(path, "contains: test_detects_broken_workflow_ref",
                     "contains: test_real_repo_passes")
        self.assertTrue(any("not a registered forge_accept detector" in issue for issue in self.issues()))

    def test_detector_must_be_invoked_by_acceptance_collect(self):
        path = self.repo / "harness" / "acceptance.mjs"
        replace_once(path, "    probeSecurity(),\n", "")
        self.assertTrue(any("not invoked by forge_accept" in issue for issue in self.issues()))

    def test_detector_registry_argv_must_match_probe_command(self):
        path = self.repo / "harness" / "acceptance.mjs"
        replace_once(path, "[join(HARNESS_DIR, 'gate.mjs')]",
                     "[join(HARNESS_DIR, 'secret-scan.mjs')]")
        self.assertTrue(any("expected exact detector argv" in issue for issue in self.issues()))

    def test_detector_probe_cannot_force_a_pass(self):
        path = self.repo / "harness" / "acceptance.mjs"
        replace_once(path, "    r.ok ? PASS : FAIL,\n", "    PASS,\n")
        self.assertTrue(any("does not derive PASS/FAIL" in issue for issue in self.issues()))

    def test_protected_automatic_rule_cannot_be_reversed(self):
        path = self.agent_root / "engineering" / "rules.yml"
        replace_once(path, "    title: Secrets must not enter source, logs or snapshots",
                     "    title: Secrets may be committed")
        self.assertTrue(any("protected automatic rule 'SEC-001' changed" in issue
                            for issue in self.issues()))

    def test_context_route_rejects_free_keyword_match(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "      op: any\n      predicates:\n",
                     "      op: any\n      keywords: [ignore-all-rules]\n      predicates:\n")
        self.assertTrue(any("typed match" in issue for issue in self.issues()))

    def test_context_route_rejects_glob_escape(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "values: [.agent/**, harness/**]", "values: [../../**]")
        self.assertTrue(any("unsafe repository-relative POSIX glob" in issue for issue in self.issues()))

    def test_context_overflow_policy_cannot_be_weakened(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "on_required_overflow: block", "on_required_overflow: summarize_and_continue")
        self.assertTrue(any("on_required_overflow" in issue for issue in self.issues()))

    def test_context_base_cannot_drop_agent_entrypoint(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["selection"]["base_required"] = [
            item for item in data["selection"]["base_required"]
            if item["ref"] != ".agent/AGENTS.md"
        ]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens canonical entry '.agent/AGENTS.md'" in issue for issue in self.issues()))

    def test_context_file_budget_cannot_be_amplified(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "max_files: 24", "max_files: 25")
        self.assertTrue(any("max_files exceeds the v1 budget ceiling" in issue for issue in self.issues()))

    def test_context_byte_budget_cannot_be_amplified(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "max_total_bytes: 524288", "max_total_bytes: 524289")
        self.assertTrue(any("max_total_bytes exceeds the v1 budget ceiling" in issue for issue in self.issues()))

    def test_context_required_security_ref_cannot_be_downgraded(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "ref: .agent/skills/security-review.md, lane: instruction, required: true",
                     "ref: .agent/skills/security-review.md, lane: untrusted_data, required: false")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_security_route_cannot_drop_secure_coding_skill(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "security-change")
        route["include"] = [item for item in route["include"]
                            if item["ref"] != ".agent/skills/secure-coding.md"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_instruction_lane_rejects_non_governance_source(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "ref: harness/secret-scan.mjs, lane: trusted_context",
                     "ref: harness/secret-scan.mjs, lane: instruction")
        self.assertTrue(any("instruction lane" in issue for issue in self.issues()))

    def test_prompt_source_cannot_be_promoted_to_trusted_context(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        replace_once(path, "ref: .ai/prompts/02-security-rfc-review.md, lane: untrusted_data",
                     "ref: .ai/prompts/02-security-rfc-review.md, lane: trusted_context")
        self.assertTrue(any(".ai/prompts sources must remain untrusted_data" in issue
                            for issue in self.issues()))

    def test_security_route_trigger_cannot_be_made_unreachable(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "security-change")
        route["match"]["op"] = "all"
        route["match"]["predicates"][1]["values"] = ["documentation"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens its canonical v1 match trigger" in issue for issue in self.issues()))

    def test_data_route_cannot_drop_persistence_skill(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "data-and-contract")
        route["include"] = [item for item in route["include"]
                            if item["ref"] != ".agent/skills/data-modeling-transactions.md"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_backend_route_trigger_cannot_be_narrowed(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "backend-runtime")
        route["match"]["predicates"][0]["values"] = ["**/unknown-backend/**"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens its canonical v1 match trigger" in issue for issue in self.issues()))

    def test_frontend_route_cannot_drop_state_and_interaction_skill(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "user-experience")
        route["include"] = [item for item in route["include"]
                            if item["ref"] != ".agent/skills/information-interaction-design.md"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_frontend_route_cannot_drop_code_architecture_skill(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "user-experience")
        route["include"] = [item for item in route["include"]
                            if item["ref"] != ".agent/skills/frontend-code-architecture.md"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_frontend_route_cannot_drop_ui_geometry_skill(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in data["routes"] if item["id"] == "user-experience")
        route["include"] = [item for item in route["include"]
                            if item["ref"] != ".agent/skills/ui-geometry.md"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("weakens required context" in issue for issue in self.issues()))

    def test_completion_schema_type_drift_is_rejected(self):
        path = self.agent_root / "eval" / "completion-evidence.schema.yml"
        replace_once(path, "verification_receipts: { type: list, items: verification_receipt",
                     "verification_receipts: { type: list, items: string")
        self.assertTrue(any("package.verification_receipts contract must remain" in issue
                            for issue in self.issues()))

    def test_completion_schema_bounds_cannot_be_weakened(self):
        path = self.agent_root / "eval" / "completion-evidence.schema.yml"
        replace_once(path, "task_id: { type: string, non_empty: true, max_length: 128 }",
                     "task_id: { type: string, non_empty: false, max_length: 999999999 }")
        self.assertTrue(any("package.task_id contract must remain" in issue for issue in self.issues()))

    def test_completion_invariant_ids_must_be_unique(self):
        path = self.agent_root / "eval" / "completion-evidence.schema.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["invariants"].append(dict(data["invariants"][0]))
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("invariant ids must be unique" in issue for issue in self.issues()))

    def test_completion_invariant_prose_cannot_contradict_validator(self):
        path = self.agent_root / "eval" / "completion-evidence.schema.yml"
        replace_once(path, "Evidence packages cannot contain status, verdict, completed or accepted decision fields.",
                     "Evidence packages may declare completed without evidence.")
        self.assertTrue(any("prose contradicts" in issue for issue in self.issues()))


class CompletionReportTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        schema_path = REPO_ROOT / ".agent" / "eval" / "completion-evidence.schema.yml"
        cls.schema = yaml.safe_load(schema_path.read_text(encoding="utf-8"))

    def base_report(self):
        digest = "sha256:" + "1" * 64
        return {
            "task_id": "T-1", "summary": "done", "source_revision": "abc1234",
            "source_tree_sha256": digest,
            "changed_files": ["src/example.ts"], "requirements_covered": ["REQ-1"],
            "verification_receipts": [self.receipt(digest)],
            "residual_risks": [], "assumptions": [],
        }

    def receipt(self, digest=None):
        return {
            "id": "unit-1", "detector_id": "project.unit", "detector_version": "1.0.0",
            "source_tree_sha256": digest or "sha256:" + "1" * 64,
            "status": "passed", "argv": ["npm", "test"], "cwd": ".",
            "exit_code": 0, "output_sha256": "sha256:" + "2" * 64, "reason": "",
        }

    def test_structured_receipt_accepted(self):
        self.assertEqual(engineering.validate_evidence_package(self.base_report(), self.schema), [])

    def test_decision_field_rejected(self):
        report = self.base_report()
        report["status"] = "completed"
        issues = engineering.validate_evidence_package(report, self.schema)
        self.assertTrue(any("forbidden decision fields" in issue for issue in issues))

    def test_arbitrary_string_evidence_is_rejected(self):
        report = self.base_report()
        report["verification_receipts"] = ["agent says green"]
        issues = engineering.validate_evidence_package(report, self.schema)
        self.assertTrue(any("must be a mapping" in issue for issue in issues))

    def test_empty_evidence_package_is_rejected(self):
        report = self.base_report()
        report["verification_receipts"] = []
        issues = engineering.validate_evidence_package(report, self.schema)
        self.assertTrue(any("below min_items" in issue for issue in issues))

    def test_explicit_not_executed_reason_is_accepted(self):
        report = self.base_report()
        item = self.receipt(report["source_tree_sha256"])
        item.update({"id": "e2e", "status": "not_executed", "argv": [],
                     "exit_code": None, "output_sha256": None, "reason": "browser unavailable"})
        report["verification_receipts"].append(item)
        self.assertEqual(engineering.validate_evidence_package(report, self.schema), [])

    def test_duplicate_receipt_id_is_rejected(self):
        report = self.base_report()
        report["verification_receipts"].append(dict(report["verification_receipts"][0]))
        issues = engineering.validate_evidence_package(report, self.schema)
        self.assertIn("evidence package has duplicate receipt ids", issues)

    def test_receipt_source_mismatch_is_rejected(self):
        report = self.base_report()
        report["verification_receipts"][0]["source_tree_sha256"] = "sha256:" + "3" * 64
        issues = engineering.validate_evidence_package(report, self.schema)
        self.assertTrue(any("does not match" in issue for issue in issues))

    def test_cli_validates_report(self):
        repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, repo, ignore_errors=True)
        report = self.base_report()
        report_path = repo / "completion.yml"
        report_path.write_text(yaml.safe_dump(report), encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(CHECKER), str(repo), str(report_path)],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("agent-engineering-check: PASS", result.stdout)


if __name__ == "__main__":
    unittest.main()
