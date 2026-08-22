#!/usr/bin/env python3
"""Governance regressions for ADR-0069 planning ownership projection."""

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.planning_capability_ownership import (
    NON_CAPABILITY,
    PLANNING_CAPABILITY_OWNERSHIP,
    adr_issues,
    detector_issues,
    documentation_issues,
    fixture_and_source_issues,
    registry_issues,
    schema_issues,
    skill_issues,
    wiring_issues,
)


class PlanningCapabilityOwnershipGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_classifies_planning_projector_without_authority(self):
        self.assertEqual(
            self.policy["planning_capability_ownership"],
            PLANNING_CAPABILITY_OWNERSHIP,
        )
        self.assertIn(
            "planning_capability_ownership",
            self.policy["scope"]["shipped_evaluators"],
        )
        for field in (
                "shipped_kinds", "shipped_contract_only_kinds", "shipped_producers",
                "shipped_projectors", "shipped_runtime_profiles", "planned_kinds"):
            self.assertNotIn("planning_capability_ownership", self.policy["scope"][field])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])

    def test_scope_physical_authority_and_roadmap_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_runtime_profiles"].append(
            "planning_capability_ownership")
        mutated["planning_capability_ownership"]["authority_semantics"][
            "registry_mutation"] = True
        mutated["planning_capability_ownership"]["scaffold_boundary"][
            "generates_declared_owner_skill_or_host_adapter_files"] = True
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("runtime/kind authority" in issue for issue in issues), issues)
        self.assertTrue(any("contract drifted" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_schema_golden_sources_adr_skill_and_wiring_are_frozen(self):
        self.assertEqual(schema_issues(self.repo), [])
        self.assertEqual(fixture_and_source_issues(self.repo), [])
        self.assertEqual(adr_issues(self.repo), [])
        self.assertEqual(skill_issues(self.repo), [])
        self.assertEqual(wiring_issues(self.repo, self.agent), [])

    def test_schema_scaffold_copy_and_generation_boundaries_fail_closed(self):
        source = self.repo / (
            "docs/contracts/planning-capability-ownership-projection-v1.schema.json")
        schema = json.loads(source.read_text(encoding="utf-8"))
        delivery = schema["x-forgeos-delivery"]
        delivery["scaffold_does_not_copy"].append("declared_owner_skill_packages")
        delivery["scaffold_does_not_generate"] = ["host_adapters"]
        delivery[
            "existing_same_name_markdown_is_physical_resolution_or_availability_evidence"
        ] = True
        delivery["named_file_input_nonclaims"] = []
        schema["x-forgeos-yaml-source-profile"]["block_node_placement"] = "ambient"
        schema["x-forgeos-source-semantics"]["ignored_source_field_shapes"][
            "catalog_mapping_fields"]["fields"].remove("gates")
        schema["x-forgeos-source-semantics"]["semantic_source_bounds"][
            "implementation_wave"] = "ambient"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / source.relative_to(self.repo)
            target.parent.mkdir(parents=True)
            target.write_text(json.dumps(schema), encoding="utf-8")
            issues = schema_issues(root)
        self.assertTrue(any("scaffold copy boundary" in issue for issue in issues), issues)
        self.assertTrue(any("scaffold generation boundary" in issue for issue in issues), issues)
        self.assertTrue(any("same-name non-evidence" in issue for issue in issues), issues)
        self.assertTrue(any("named file input nonclaims" in issue for issue in issues), issues)
        self.assertTrue(any("block_node_placement drifted" in issue for issue in issues), issues)
        self.assertTrue(any("ignored source shape" in issue for issue in issues), issues)
        self.assertTrue(any("semantic source bounds" in issue for issue in issues), issues)

    def test_detector_is_shadow_and_exact(self):
        self.assertEqual(detector_issues(self.agent), [])

    def test_promotion_closes_only_ownership_projection_roadmap_item(self):
        self.assertEqual(documentation_issues(self.repo), [])


if __name__ == "__main__":
    unittest.main()
