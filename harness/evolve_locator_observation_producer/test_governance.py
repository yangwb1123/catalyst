#!/usr/bin/env python3
"""ADR-0052 shipped governance, Skill, and reference integration tests."""

import json
import shutil
import sys
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

import governance_engineering_check as governance
from agent_engineering.support import engineering, make_temp_repo, replace_once


class EvolveLocatorProducerGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_registry_freezes_exact_v10_shipped_scope(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 39)
        self.assertEqual(
            data["local_evolve_repo_locator_observation_producer"],
            governance.LOCAL_EVOLVE_LOCATOR_OBSERVATION_PRODUCER,
        )
        self.assertEqual(
            data["scope"]["shipped_producers"],
            [
                "local_gate_command_observation_producer",
                "local_evolve_repo_locator_observation_producer",
                "local_go_package_dependency_graph_observation_producer",
                "local_project_source_snapshot_producer",
            ],
        )
        self.assertEqual(
            data["scope"]["staged_producers"],
            [],
        )

    def test_shipped_producer_cannot_be_demoted_by_registry_edit(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["scope"]["shipped_producers"].remove(
            "local_evolve_repo_locator_observation_producer"
        )
        data["scope"]["staged_producers"] = [
            "local_evolve_repo_locator_observation_producer"
        ]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("shipped/staged producer scope drifted" in issue
                            for issue in issues), issues)

    def test_schema_and_fixture_pins_are_enforced(self):
        schema = (self.repo / "docs" / "contracts" /
                  "local-evolve-repo-locator-observation-producer-v1.schema.json")
        replace_once(schema, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        fixture = (self.repo / "docs" / "contracts" / "fixtures" /
                   "local-evolve-repo-locator-observation-producer-v1.json")
        replace_once(fixture, "PURE_CONTRACT_FIXTURE", "DRIFTED_CONTRACT_FIXTURE")
        issues = self.issues()
        self.assertTrue(any(
            "local_evolve_repo_locator_observation_producer_schema_sha256" in issue
            for issue in issues
        ), issues)
        self.assertTrue(any(
            "local_evolve_repo_locator_observation_producer_golden_fixture_sha256"
            in issue for issue in issues
        ), issues)

    def test_registry_contract_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        data["local_evolve_repo_locator_observation_producer"][
            "exact_capture"
        ]["capture_default"] = "enabled"
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "local_evolve_repo_locator_observation_producer contract drifted" in issue
            for issue in issues
        ), issues)

    def test_schema_capability_boundary_drift_is_rejected(self):
        path = (self.repo / "docs" / "contracts" /
                "local-evolve-repo-locator-observation-producer-v1.schema.json")
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-capability-boundary"]["automatic_adr0050_binding"] = True
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("x-forgeos-capability-boundary drifted" in issue
                            for issue in issues), issues)

    def test_golden_semantics_validator_is_integrated(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "local-evolve-repo-locator-observation-producer-v1.json")
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["production_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("expected.production_sha256" in issue
                            for issue in issues), issues)

    def test_skill_requires_non_capability_markers(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "不自动调用 ADR-0050", "自动调用 ADR-0050")
        issues = self.issues()
        self.assertTrue(any(
            "local Evolve locator observation producer guidance" in issue
            for issue in issues
        ), issues)


if __name__ == "__main__":
    unittest.main()
