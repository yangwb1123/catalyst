#!/usr/bin/env python3
"""Integration tests for Evidence/Claim wiring in Agent Engineering."""
import json
import shutil
import unittest

import yaml

from test_agent_engineering_check import engineering, make_temp_repo, replace_once
import governance_engineering_check as governance


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
        replace_once(path, "persistence: local_append_only_exact_evidence_claim_journal",
                     "persistence: authoritative_truth_ledger")
        self.assertTrue(any("protected governance contract policy" in issue for issue in self.issues()))

    def test_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "governance-evidence-claim-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("schema_sha256" in issue for issue in self.issues()))

    def test_journal_schema_pin_is_enforced(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        self.assertTrue(any("journal_schema_sha256" in issue for issue in self.issues()))

    def test_missing_journal_schema_is_rejected(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        path.unlink()
        self.assertTrue(any("required pin target missing" in issue for issue in self.issues()))

    def test_journal_schema_registers_the_bounded_list_envelope(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        self.assertIn(
            {"$ref": "#/$defs/record_inspection_list"},
            schema["oneOf"],
        )
        record_list = schema["$defs"]["record_inspection_list"]
        self.assertEqual(record_list["required"], ["api_version", "kind", "records"])
        self.assertEqual(record_list["properties"]["records"]["maxItems"], 100)
        self.assertEqual(
            record_list["properties"]["records"]["items"],
            {"$ref": "#/$defs/record_inspection"},
        )

    def test_journal_schema_freezes_reference_closure_admissibility_limits(self):
        path = self.repo / "docs" / "contracts" / "governance-record-journal-v1.schema.json"
        schema = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(
            schema["x-forgeos-reference-closure-limits"],
            governance.SCHEMA_CLOSURE_LIMITS,
        )

    def test_registry_freezes_closure_limits_and_runtime_delivery(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        limits = data["journal"]["limits"]
        for field, expected in governance.REFERENCE_CLOSURE_LIMITS.items():
            self.assertEqual(limits[field], expected)
        self.assertEqual(data["journal"]["runtime_delivery"], governance.RUNTIME_DELIVERY)

    def test_reference_closure_limit_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "max_dependency_records: 1024", "max_dependency_records: 1025")
        self.assertTrue(any("max_dependency_records must remain 1024" in issue
                            for issue in self.issues()))

    def test_skill_requires_real_runtime_and_honest_scaffold_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "forge-runtime governance journal show",
                     "forge governance journal show")
        self.assertTrue(any("compatible forge-runtime CLI" in issue for issue in self.issues()))

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
