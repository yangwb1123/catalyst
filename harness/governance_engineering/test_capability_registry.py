#!/usr/bin/env python3
"""Governance regressions for ADR-0068 Capability Registry v1."""

import copy
import sys
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.capability_registry import (
    CAPABILITY_REGISTRY,
    NON_CAPABILITY,
    SCAFFOLD_MARKERS,
    detector_issues,
    documentation_issues,
    registry_issues,
    schema_issues,
    skill_issues,
    wiring_issues,
)


class CapabilityRegistryGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_classifies_exact_resolver_without_authority(self):
        self.assertEqual(self.policy["capability_registry"], CAPABILITY_REGISTRY)
        self.assertIn("capability_registry", self.policy["scope"]["shipped_evaluators"])
        for field in ("shipped_kinds", "shipped_contract_only_kinds",
                      "shipped_producers", "shipped_projectors",
                      "shipped_runtime_profiles"):
            self.assertNotIn("capability_registry", self.policy["scope"][field])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])

    def test_scope_authority_and_catalog_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_runtime_profiles"].append("capability_registry")
        mutated["capability_registry"]["authority_semantics"][
            "capability_grant_activation"] = True
        mutated["capability_registry"]["coverage"]["planning_catalog_projection"] = True
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("runtime authority" in issue for issue in issues), issues)
        self.assertTrue(any("evaluator contract" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_schema_fixture_skill_and_wiring_are_frozen(self):
        self.assertEqual(schema_issues(self.repo), [])
        self.assertEqual(skill_issues(self.repo), [])
        self.assertEqual(wiring_issues(self.repo, self.agent), [])
        old = "harness/scaffold/test_forge-upgrade-engineering.mjs"
        owner = "harness/scaffold/engineering-upgrade-fixture.mjs"
        self.assertNotIn(old, SCAFFOLD_MARKERS)
        self.assertEqual(SCAFFOLD_MARKERS[owner], [
            "assertCapabilityRegistryScaffold", "CAPABILITY_REGISTRY_LEGACY_FILES",
        ])

    def test_detector_is_shadow_and_exact(self):
        self.assertEqual(detector_issues(self.agent), [])

    def test_promotion_closes_only_minimal_registry_items(self):
        self.assertEqual(documentation_issues(self.repo), [])


if __name__ == "__main__":
    unittest.main()
