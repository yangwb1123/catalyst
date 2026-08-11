#!/usr/bin/env python3
"""Integration tests for ADR-0050's governance and Skill wiring."""

import json
import shutil
import sys
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

import governance_engineering_check as governance
from test_agent_engineering_check import engineering, make_temp_repo, replace_once


class GovernanceEvolveLocatorIntegrationTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_registry_freezes_exact_v11_adapter(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 11)
        self.assertEqual(
            data["evolve_repo_locator_evidence_adapter"],
            governance.EVOLVE_REPO_LOCATOR_EVIDENCE_ADAPTER,
        )

    def test_schema_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" /
                "evolve-repo-locator-evidence-adapter-v1.schema.json")
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        issues = self.issues()
        self.assertTrue(any(
            "evolve_repo_locator_evidence_adapter_schema_sha256" in issue
            for issue in issues
        ), issues)

    def test_golden_pin_is_enforced(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "evolve-repo-locator-evidence-adapter-v1.json")
        replace_once(path, '"run_id": "run-evolve-0050"',
                     '"run_id": "run-evolve-drifted"')
        issues = self.issues()
        self.assertTrue(any(
            "evolve_repo_locator_evidence_adapter_golden_fixture_sha256" in issue
            for issue in issues
        ), issues)

    def test_registry_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "    unavailable_projectable: false",
                     "    unavailable_projectable: true")
        issues = self.issues()
        self.assertTrue(any(
            "evolve_repo_locator_evidence_adapter contract drifted" in issue
            for issue in issues
        ), issues)

    def test_schema_mapping_drift_is_rejected(self):
        path = (self.repo / "docs" / "contracts" /
                "evolve-repo-locator-evidence-adapter-v1.schema.json")
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-mapping"]["source_trust"] = "authoritative"
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("x-forgeos-mapping drifted" in issue for issue in issues),
                        issues)

    def test_golden_validator_is_integrated(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "evolve-repo-locator-evidence-adapter-v1.json")
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["request_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "golden.expected.request_sha256" in issue for issue in issues
        ), issues)

    def test_skill_requires_branch_and_non_capability_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "### Evolve repository locator adapter 分支",
                     "### Removed Evolve locator adapter branch")
        issues = self.issues()
        self.assertTrue(any(
            "Evolve locator Evidence adapter guidance" in issue for issue in issues
        ), issues)

    def test_skill_cannot_claim_scan_judgment(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "不会确认 finding、clear 或 opportunity",
                     "会确认 finding、clear 或 opportunity")
        issues = self.issues()
        self.assertTrue(any(
            "Evolve locator Evidence adapter guidance" in issue for issue in issues
        ), issues)

    def test_detector_requires_exact_arguments(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(
            item for item in data["detectors"]
            if item["id"] == "governance.evolve_repo_locator_evidence_adapter"
        )
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "Evolve locator Evidence adapter detector requires exact arguments" in issue
            for issue in issues
        ), issues)

    def test_detector_must_remain_shadow_non_load_bearing(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(
            item for item in data["detectors"]
            if item["id"] == "governance.evolve_repo_locator_evidence_adapter"
        )
        detector["state"] = "enforced"
        detector["invocation"]["owner"] = "forge_accept"
        detector["invocation"]["load_bearing"] = True
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "Evolve locator Evidence adapter detector requires exact shadow binding" in issue
            for issue in issues
        ), issues)


if __name__ == "__main__":
    unittest.main()
