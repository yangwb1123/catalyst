#!/usr/bin/env python3
"""Governance regressions for ADR-0067 proposed-only ADR v2."""

import copy
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.architecture_decision_record_v2 import (
    ARCHITECTURE_DECISION_RECORD_V2,
    NON_CAPABILITY,
    detector_issues,
    documentation_issues,
    registry_issues,
    schema_issues,
    skill_issues,
)


class ArchitectureDecisionRecordV2GovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_classifies_proposed_only_evaluator_without_authority(self):
        self.assertEqual(
            self.policy["architecture_decision_record_v2"],
            ARCHITECTURE_DECISION_RECORD_V2,
        )
        self.assertIn(
            "architecture_decision_record_v2",
            self.policy["scope"]["shipped_evaluators"],
        )
        for field in (
                "shipped_kinds", "shipped_contract_only_kinds",
                "shipped_producers", "shipped_projectors",
                "shipped_runtime_profiles"):
            self.assertNotIn(
                "architecture_decision_record_v2", self.policy["scope"][field])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])

    def test_scope_authority_and_non_capability_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_producers"].append(
            "architecture_decision_record_v2")
        mutated["architecture_decision_record_v2"]["authority_semantics"][
            "approval_or_acceptance_attestation"] = True
        mutated["legacy"]["adr_import"] = "not_implemented"
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("producer" in issue for issue in issues), issues)
        self.assertTrue(any("evaluator contract" in issue for issue in issues), issues)
        self.assertTrue(any("legacy ADR import" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_schema_and_skill_freeze_proposed_only_boundary(self):
        self.assertEqual(schema_issues(self.repo), [])
        self.assertEqual(skill_issues(self.repo), [])

    def test_detector_is_shadow_and_exact(self):
        self.assertEqual(detector_issues(self.agent), [])

    def test_promotion_closes_only_proposed_frontmatter_item(self):
        self.assertEqual(documentation_issues(self.repo), [])

    def test_universal_scaffold_does_not_require_catalyst_promotion_docs(self):
        with tempfile.TemporaryDirectory() as raw:
            self.assertEqual(documentation_issues(Path(raw)), [])


if __name__ == "__main__":
    unittest.main()
