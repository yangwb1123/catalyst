#!/usr/bin/env python3
"""Governance integration tests for ADR-0065/0066 GraphSnapshot profiles."""

import copy
import sys
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.graph_snapshot import (
    GRAPH_SNAPSHOT,
    GRAPH_SNAPSHOT_TEST_SOURCE,
    NON_CAPABILITY,
    TEST_SOURCE_NON_CAPABILITY,
    detector_issues,
    registry_issues,
    schema_issues,
    skill_issues,
    test_source_schema_issues as source_schema_issues,
)


class GraphSnapshotGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_classifies_partial_projector_without_authority(self):
        self.assertEqual(self.policy["graph_snapshot"], GRAPH_SNAPSHOT)
        self.assertEqual(self.policy["graph_snapshot_test_source"],
                         GRAPH_SNAPSHOT_TEST_SOURCE)
        self.assertEqual(self.policy["scope"]["shipped_projectors"],
                         ["graph_snapshot", "graph_snapshot_test_source"])
        self.assertIn("graph_snapshot",
                      self.policy["scope"]["shipped_evaluators"])
        self.assertNotIn("graph_snapshot",
                         self.policy["scope"]["shipped_producers"])
        self.assertNotIn("graph_snapshot_test_source",
                         self.policy["scope"]["shipped_producers"])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertIn(TEST_SOURCE_NON_CAPABILITY,
                      self.policy["non_capabilities"])
        self.assertEqual(registry_issues(
            self.policy, self.policy_path), [])

    def test_scope_authority_and_non_capability_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_producers"].append(
            "graph_snapshot_test_source")
        mutated["graph_snapshot_test_source"]["authority_semantics"][
            "satisfies_g3_or_assessment_join"] = True
        mutated["non_capabilities"].remove(TEST_SOURCE_NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("producer or authority" in issue for issue in issues), issues)
        self.assertTrue(any("test-source projector contract" in issue
                            for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_schema_taxonomy_limits_and_unknown_boundary_are_frozen(self):
        self.assertEqual(schema_issues(self.repo), [])
        self.assertEqual(source_schema_issues(self.repo), [])

    def test_detector_is_shadow_and_exact(self):
        self.assertEqual(detector_issues(self.agent), [])

    def test_skill_keeps_partial_unknown_and_g3_boundary(self):
        self.assertEqual(skill_issues(self.repo), [])


if __name__ == "__main__":
    unittest.main()
