"""Governance regressions for the accepted ADR-0061 contract-only slice."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from governance_engineering.knowledge_update_proposal import (
    FIXTURE_RELATIVE, KNOWLEDGE_UPDATE_PROPOSAL, PROMOTION_MARKERS,
    fixture_issues, promotion_issues, registry_issues, schema_issues,
)


ROOT = Path(__file__).resolve().parents[2]


class KnowledgeUpdateProposalGovernanceTest(unittest.TestCase):
    def policy(self):
        path = ROOT / ".agent/engineering/governance-contracts.yml"
        return path, yaml.safe_load(path.read_text(encoding="utf-8"))

    def test_registry_freezes_accepted_contract_only_boundary(self):
        path, data = self.policy()
        self.assertEqual(data["version"], 39)
        self.assertEqual(data["knowledge_update_proposal"],
                         KNOWLEDGE_UPDATE_PROPOSAL)
        self.assertEqual(data["scope"]["shipped_contract_only_kinds"], [
            "ApprovalRecord", "CapabilityGrant", "KnowledgeUpdateProposal",
            "TransitionReceipt",
        ])
        self.assertEqual(data["scope"]["planned_kinds"], [])
        self.assertNotIn("KnowledgeUpdateProposal", data["scope"]["shipped_kinds"])
        self.assertEqual(registry_issues(data, path), [])

    def test_registry_mutations_fail_closed(self):
        path, data = self.policy()
        for mutation in (
                lambda value: value["knowledge_update_proposal"].update(
                    {"persistence": "journal"}),
                lambda value: value["scope"].update(
                    {"planned_kinds": ["KnowledgeUpdateProposal"]}),
                lambda value: value["scope"]["shipped_kinds"].append(
                    "KnowledgeUpdateProposal")):
            mutated = copy.deepcopy(data)
            mutation(mutated)
            self.assertTrue(registry_issues(mutated, path))

    def test_schema_and_fixture_pins_are_frozen(self):
        self.assertEqual(schema_issues(ROOT), [])
        self.assertEqual(fixture_issues(ROOT), [])

    def test_fixture_hash_mutation_is_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / FIXTURE_RELATIVE
            target.parent.mkdir(parents=True)
            fixture = json.loads((ROOT / FIXTURE_RELATIVE).read_text(encoding="utf-8"))
            fixture["expected_assessment"]["assessment_sha256"] = "f" * 64
            target.write_text(json.dumps(fixture, separators=(",", ":")),
                              encoding="utf-8")
            issues = fixture_issues(root)
            self.assertTrue(any("fixture pin drifted" in issue for issue in issues))
            self.assertTrue(any("assessment golden hash drifted" in issue
                                for issue in issues))

    def test_accepted_promotion_markers_cannot_regress(self):
        self.assertEqual(promotion_issues(ROOT, optional=True), [])
        self.assertEqual(set(PROMOTION_MARKERS), {
            "docs/adr/0061-knowledge-update-proposal-v1-contract-only.md",
            ".agent/DECISIONS.md", ".agent/ROADMAP.md", ".agent/CURRENT_SPRINT.md",
            "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md",
        })
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative, marker in PROMOTION_MARKERS.items():
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(marker, encoding="utf-8")
            decision = root / "docs/adr/0061-knowledge-update-proposal-v1-contract-only.md"
            decision.write_text("- Status: Candidate", encoding="utf-8")
            issues = promotion_issues(root)
            self.assertEqual(len(issues), 1)
            self.assertIn("missing accepted ADR-0061 marker", issues[0])

    def test_optional_promotion_facts_skip_only_when_entirely_absent(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(len(promotion_issues(root)), len(PROMOTION_MARKERS))
            self.assertEqual(promotion_issues(root, optional=True), [])
            relative, marker = next(
                item for item in PROMOTION_MARKERS.items()
                if item[0] != "docs/adr/0061-knowledge-update-proposal-v1-contract-only.md"
            )
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(marker, encoding="utf-8")
            issues = promotion_issues(root, optional=True)
            self.assertEqual(len(issues), len(PROMOTION_MARKERS) - 1)
            self.assertTrue(all("cannot validate ADR-0061 promotion" in issue
                                for issue in issues))


if __name__ == "__main__":
    unittest.main()
