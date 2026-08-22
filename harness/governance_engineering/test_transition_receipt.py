"""ADR-0060 accepted TransitionReceipt governance integration tests."""

from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from governance_engineering.transition_receipt import (  # noqa: E402
    PROMOTION_MARKERS,
    RESULT,
    TRANSITION_RECEIPT,
    detector_issues,
    fixture_issues,
    promotion_issues,
    registry_issues,
    route_issues,
    schema_issues,
    skill_issues,
)


class TransitionReceiptGovernanceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = HARNESS.parent
        cls.agent = cls.root / ".agent"
        cls.policy_path = cls.agent / "engineering/governance-contracts.yml"
        cls.policy = yaml.safe_load(cls.policy_path.read_text(encoding="utf-8"))

    def test_accepted_registry_detector_route_and_skill_are_contract_only(self):
        self.assertEqual(self.policy["transition_receipt"], TRANSITION_RECEIPT)
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(detector_issues(self.agent), [])
        self.assertEqual(route_issues(self.root), [])
        self.assertEqual(skill_issues(self.root), [])
        self.assertEqual(promotion_issues(self.root, optional=True), [])
        self.assertEqual(self.policy["transition_receipt"]["positive_result"], RESULT)

    def test_schema_and_fixture_pins_are_frozen(self):
        self.assertEqual(schema_issues(self.root), [])
        self.assertEqual(fixture_issues(self.root), [])

    def test_transition_receipt_cannot_be_runtime_or_planned(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_kinds"].append("TransitionReceipt")
        mutated["scope"]["planned_kinds"].append("TransitionReceipt")
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("cannot be a shipped runtime" in issue for issue in issues))
        self.assertTrue(any("planned kinds must be empty" in issue for issue in issues))

    def test_authority_escalation_is_rejected(self):
        mutated = copy.deepcopy(self.policy)
        mutated["transition_receipt"]["unavailable_runtime"][
            "append_only_transition_ledger"] = "available"
        mutated["transition_receipt"]["assessment_constants"][
            "transition_attestation"] = True
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract-only boundary drifted" in issue for issue in issues))

    def test_detector_cannot_become_load_bearing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "engineering/detectors.yml"
            target.parent.mkdir(parents=True)
            data = yaml.safe_load((self.agent / "engineering/detectors.yml").read_text())
            detector = next(item for item in data["detectors"]
                            if item["id"] == "governance.transition_receipt_contract")
            detector["state"] = "enforced"
            detector["invocation"]["load_bearing"] = True
            target.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
            issues = detector_issues(root)
            self.assertTrue(any("shadow" in issue for issue in issues))
            self.assertTrue(any("load-bearing" in issue for issue in issues))

    def test_accepted_promotion_markers_cannot_regress(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative, marker in PROMOTION_MARKERS.items():
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(marker, encoding="utf-8")
            decision = root / "docs/adr/0060-transition-receipt-v1-contract-only.md"
            decision.write_text("- Status: Candidate", encoding="utf-8")
            issues = promotion_issues(root)
            self.assertEqual(len(issues), 1)
            self.assertIn("missing accepted ADR-0060 marker", issues[0])

    def test_optional_promotion_facts_skip_only_when_entirely_absent(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(len(promotion_issues(root)), len(PROMOTION_MARKERS))
            self.assertEqual(promotion_issues(root, optional=True), [])
            relative, marker = next(
                item for item in PROMOTION_MARKERS.items()
                if item[0] != "docs/adr/0060-transition-receipt-v1-contract-only.md"
            )
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(marker, encoding="utf-8")
            issues = promotion_issues(root, optional=True)
            self.assertEqual(len(issues), len(PROMOTION_MARKERS) - 1)
            self.assertTrue(all("cannot validate ADR-0060 promotion" in issue
                                for issue in issues))


if __name__ == "__main__":
    unittest.main()
