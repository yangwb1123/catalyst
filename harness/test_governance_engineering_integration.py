#!/usr/bin/env python3
"""Integration tests for Evidence/Claim wiring in Agent Engineering."""
import shutil
import unittest

import yaml

from test_agent_engineering_check import engineering, make_temp_repo, replace_once


class GovernanceEngineeringIntegrationTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_extension_cannot_leave_registry(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"]["governance_contract_schema"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue for issue in self.issues()))

    def test_policy_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "persistence: forbidden", "persistence: allowed")
        self.assertTrue(any("protected governance contract policy" in issue for issue in self.issues()))

    def test_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "governance-evidence-claim-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("schema_sha256" in issue for issue in self.issues()))

    def test_oversized_contract_json_is_rejected_before_hashing(self):
        path = self.repo / "docs" / "contracts" / "governance-evidence-claim-v1.schema.json"
        with path.open("wb") as stream:
            stream.truncate(1_048_577)
        issues = self.issues()
        self.assertTrue(any("schema.json exceeds 1048576 bytes" in issue
                            for issue in issues), issues)

    def test_oversized_registry_is_rejected_before_yaml_parsing(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        with path.open("wb") as stream:
            stream.truncate(524_289)
        issues = self.issues()
        self.assertTrue(any("file exceeds 524288 bytes" in issue for issue in issues), issues)

    def test_skill_floor_is_required(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "## 自动化与验收", "## Removed verification")
        self.assertTrue(any("missing required section '自动化与验收'" in issue for issue in self.issues()))

    def test_detector_requires_record_set_argument(self):
        path = self.agent_root / "engineering" / "detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == "governance.evidence_claim_contract")
        detector["implementation"]["argv"].pop()
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("requires exact record-set arguments" in issue
                            for issue in self.issues()))


if __name__ == "__main__":
    unittest.main()
