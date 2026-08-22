#!/usr/bin/env python3
"""Governance regressions for ADR-0070 Project Source Snapshot."""

import copy
import sys
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.project_source_snapshot import (
    CANONICAL_REFS, NON_CAPABILITY, PROJECT_SOURCE_SNAPSHOT, adr_issues,
    detector_issues,
    package_issues, registry_issues, schema_and_fixture_issues,
    skill_and_documentation_issues, wiring_issues,
)


class ProjectSourceSnapshotGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_classifies_evaluator_and_linux_producer_without_authority(self):
        self.assertEqual(self.policy["project_source_snapshot"],
                         PROJECT_SOURCE_SNAPSHOT)
        self.assertIn("project_source_snapshot",
                      self.policy["scope"]["shipped_evaluators"])
        self.assertIn("local_project_source_snapshot_producer",
                      self.policy["scope"]["shipped_producers"])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])

    def test_authority_platform_and_scaffold_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["project_source_snapshot"]["authority_semantics"][
            "effect_attestation"] = True
        mutated["project_source_snapshot"]["runtime_platform"][
            "live_producer_host"] = "portable"
        mutated["project_source_snapshot"]["portable_package"][
            "copies_catalyst_go_runtime"] = True
        mutated["scope"]["shipped_runtime_profiles"].append(
            "project_source_snapshot")
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract drifted" in issue for issue in issues), issues)
        self.assertTrue(any("authority or projection" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_portable_runtime_and_package_failure_states_are_distinct(self):
        runtime = self.policy["project_source_snapshot"]["runtime_platform"]
        package = self.policy["project_source_snapshot"]["portable_package"]
        self.assertEqual(runtime["python_decoder_portability"],
                         "source_portable_no_live_capture")
        self.assertEqual(runtime["unsupported_host_adapter_result"],
                         "exit_3_not_executed_before_runtime_access")
        self.assertEqual(runtime["python_entrypoint_startup"],
                         "isolated_required_before_non_builtin_import")
        self.assertEqual(package["absent_named_runtime_result"],
                         "exit_3_not_executed")
        self.assertEqual(package[
            "existing_incompatible_or_execution_failure_result"],
            "exit_1_failure")
        self.assertEqual(package[
            "package_integrity_nofollow_unavailable_result"],
            "exit_1_fail_closed")
        self.assertEqual(package["vendored_contract_loading"],
                         "adapter_anchored_exact_file_location_without_sys_path_mutation")
        self.assertEqual(package["package_check_capture_consistency"],
                         "separate_non_atomic_operations")
        self.assertIs(package["package_check_authenticates_publisher"], False)
        self.assertEqual(package["output_target"], "outside_captured_root")

    def test_schema_golden_package_adr_skill_and_wiring_are_frozen(self):
        self.assertEqual(schema_and_fixture_issues(self.repo), [])
        self.assertEqual(package_issues(self.repo), [])
        self.assertEqual(adr_issues(self.repo), [])
        self.assertEqual(skill_and_documentation_issues(self.repo), [])
        self.assertEqual(detector_issues(self.agent), [])
        self.assertEqual(wiring_issues(self.repo, self.agent), [])

    def test_roadmap_parent_remains_open_and_nested_item_is_closed(self):
        self.assertEqual(skill_and_documentation_issues(self.repo), [])

    def test_agent_engineering_registry_freezes_project_activation_refs(self):
        from agent_engineering.contract import EXTENSION_REFS
        expected = {
            "project_source_snapshot_schema": CANONICAL_REFS[
                "project_source_snapshot_schema"],
            "project_source_snapshot_fixture": CANONICAL_REFS[
                "project_source_snapshot_golden_fixture"],
            "project_source_snapshot_checker": CANONICAL_REFS[
                "project_source_snapshot_checker"],
            "project_source_snapshot_skill": CANONICAL_REFS[
                "project_source_snapshot_skill"],
            "project_source_snapshot_portable_skill": CANONICAL_REFS[
                "project_source_snapshot_portable_skill"],
            "project_source_snapshot_package_manifest": CANONICAL_REFS[
                "project_source_snapshot_package_manifest"],
            "project_source_snapshot_decision": CANONICAL_REFS[
                "project_source_snapshot_decision"],
        }
        self.assertEqual({key: EXTENSION_REFS.get(key) for key in expected},
                         expected)

    def test_detector_keeps_repo_relative_entrypoint_at_argv_one(self):
        detector = next(item for item in yaml.safe_load(
            (self.agent / "engineering/detectors.yml").read_text(encoding="utf-8")
        )["detectors"] if item["id"] == "governance.project_source_snapshot")
        self.assertEqual(detector["implementation"]["argv"], [
            "python3", "harness/project_source_snapshot_contract/check.py",
            "--golden", "repo_root",
        ])

    def test_adr_does_not_reject_distinct_valid_live_observations(self):
        self.assertEqual(adr_issues(self.repo), [])
        path = self.repo / "docs/adr/ADR-0070-local-project-source-snapshot-v1.md"
        text = path.read_text(encoding="utf-8")
        self.assertIn("same supplied facts", text)
        self.assertIn("distinct valid live observation", text)
        self.assertIn("nonzero index debug", text)
        self.assertIn("intent-to-add", text)
        self.assertNotIn("reject all resealed mutations", text)
        self.assertNotIn("ls-files -v", text)


if __name__ == "__main__":
    unittest.main()
