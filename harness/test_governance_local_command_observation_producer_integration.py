#!/usr/bin/env python3
"""Integration tests for ADR-0051 governance, Skill, and reference wiring."""

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


class GovernanceLocalCommandProducerIntegrationTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def test_registry_freezes_exact_v11_shipped_scope(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 11)
        self.assertEqual(
            data["local_gate_command_observation_producer"],
            governance.LOCAL_GATE_COMMAND_OBSERVATION_PRODUCER,
        )
        self.assertEqual(
            data["scope"]["shipped_producers"],
            [
                "local_gate_command_observation_producer",
                "local_evolve_repo_locator_observation_producer",
                "local_go_package_dependency_graph_observation_producer",
            ],
        )
        self.assertEqual(
            data["scope"]["staged_producers"],
            [],
        )

    def test_schema_pin_is_enforced(self):
        path = (
            self.repo / "docs" / "contracts" /
            "local-gate-command-observation-producer-v1.schema.json"
        )
        replace_once(path, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        issues = self.issues()
        self.assertTrue(any(
            "local_gate_command_observation_producer_schema_sha256" in issue
            for issue in issues
        ), issues)

    def test_golden_pin_is_enforced(self):
        path = (
            self.repo / "docs" / "contracts" / "fixtures" /
            "local-gate-command-observation-producer-v1.json"
        )
        replace_once(
            path,
            "PURE_CONTRACT_FIXTURE",
            "DRIFTED_CONTRACT_FIXTURE",
        )
        issues = self.issues()
        self.assertTrue(any(
            "local_gate_command_observation_producer_golden_fixture_sha256" in issue
            for issue in issues
        ), issues)

    def test_registry_drift_is_rejected(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        replace_once(path, "    capture_default: disabled",
                     "    capture_default: enabled")
        issues = self.issues()
        self.assertTrue(any(
            "local_gate_command_observation_producer contract drifted" in issue
            for issue in issues
        ), issues)

    def test_golden_validator_is_integrated(self):
        path = (
            self.repo / "docs" / "contracts" / "fixtures" /
            "local-gate-command-observation-producer-v1.json"
        )
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["production_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "expected.production_sha256" in issue for issue in issues
        ), issues)

    def test_schema_capability_boundary_drift_is_rejected(self):
        path = (
            self.repo / "docs" / "contracts" /
            "local-gate-command-observation-producer-v1.schema.json"
        )
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-capability-boundary"]["journal_append"] = True
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "x-forgeos-capability-boundary drifted" in issue for issue in issues
        ), issues)

    def test_missing_fixture_is_rejected_as_required_reference(self):
        path = (
            self.repo / "docs" / "contracts" / "fixtures" /
            "local-gate-command-observation-producer-v1.json"
        )
        path.unlink()
        issues = self.issues()
        self.assertTrue(any(
            "required pin target missing" in issue and path.name in issue
            for issue in issues
        ), issues)

    def test_skill_requires_explicit_opt_in_branch(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(
            path,
            "### Local gate command observation producer 分支",
            "### Removed local producer branch",
        )
        issues = self.issues()
        self.assertTrue(any(
            "local command observation producer guidance" in issue
            for issue in issues
        ), issues)

    def test_skill_cannot_claim_exit_zero_is_pass(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "不会把 exit=0 当作 PASS", "会把 exit=0 当作 PASS")
        issues = self.issues()
        self.assertTrue(any(
            "local command observation producer guidance" in issue
            for issue in issues
        ), issues)


if __name__ == "__main__":
    unittest.main()
