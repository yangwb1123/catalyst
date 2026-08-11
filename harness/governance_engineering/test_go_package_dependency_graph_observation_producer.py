#!/usr/bin/env python3
"""ADR-0053 shipped governance, Skill, and reference integration tests."""

import json
import shutil
import sys
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

import governance_engineering_check as governance
from test_agent_engineering_check import engineering, make_temp_repo, replace_once


class GoDependencyProducerGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def policy(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        return path, yaml.safe_load(path.read_text(encoding="utf-8"))

    def test_registry_freezes_exact_v11_shipped_producer(self):
        _, data = self.policy()
        self.assertEqual(data["version"], 11)
        self.assertEqual(
            data["local_go_package_dependency_graph_observation_producer"],
            governance.LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER,
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

    def test_shipped_producer_cannot_be_demoted_by_registry_edit(self):
        path, data = self.policy()
        key = "local_go_package_dependency_graph_observation_producer"
        data["scope"]["shipped_producers"].remove(key)
        data["scope"]["staged_producers"].append(key)
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "shipped/staged producer scope drifted" in issue for issue in issues
        ), issues)

    def test_schema_and_fixture_pins_are_enforced(self):
        schema = (
            self.repo / "docs" / "contracts" /
            "local-go-package-dependency-graph-observation-producer-v1.schema.json"
        )
        replace_once(schema, '"title": "ForgeOS', '"title": "Drifted ForgeOS')
        fixture = (
            self.repo / "docs" / "contracts" / "fixtures" /
            "local-go-package-dependency-graph-observation-producer-v1.json"
        )
        replace_once(
            fixture, "PURE_CONTRACT_FIXTURE", "DRIFTED_CONTRACT_FIXTURE"
        )
        issues = self.issues()
        self.assertTrue(any(
            "local_go_package_dependency_graph_observation_producer_schema_sha256"
            in issue for issue in issues
        ), issues)
        self.assertTrue(any(
            "local_go_package_dependency_graph_observation_producer_"
            "golden_fixture_sha256" in issue for issue in issues
        ), issues)

    def test_registry_contract_drift_is_rejected(self):
        path, data = self.policy()
        data["local_go_package_dependency_graph_observation_producer"][
            "exact_capture"
        ]["capture_default"] = "enabled"
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "local_go_package_dependency_graph_observation_producer contract drifted"
            in issue for issue in issues
        ), issues)

    def test_schema_capability_boundary_drift_is_rejected(self):
        path = (
            self.repo / "docs" / "contracts" /
            "local-go-package-dependency-graph-observation-producer-v1.schema.json"
        )
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-capability-boundary"]["selected_build_semantics"] = (
            "resolved"
        )
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "x-forgeos-capability-boundary drifted" in issue for issue in issues
        ), issues)

    def test_schema_runtime_platform_drift_is_rejected(self):
        path = (
            self.repo / "docs" / "contracts" /
            "local-go-package-dependency-graph-observation-producer-v1.schema.json"
        )
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema["x-forgeos-runtime-platform"]["go_commands"] = "ambient"
        path.write_text(json.dumps(schema, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "x-forgeos-runtime-platform drifted" in issue for issue in issues
        ), issues)

    def test_golden_semantics_validator_is_integrated(self):
        path = (
            self.repo / "docs" / "contracts" / "fixtures" /
            "local-go-package-dependency-graph-observation-producer-v1.json"
        )
        fixture = json.loads(path.read_text(encoding="utf-8"))
        fixture["expected"]["production_sha256"] = "0" * 64
        path.write_text(json.dumps(fixture, ensure_ascii=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "golden.expected.production_sha256" in issue for issue in issues
        ), issues)

    def test_skill_requires_shipped_branch_and_non_capability_boundary(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(
            path,
            "selected-module-all-regular-go-files-union-v1",
            "drifted-selection-profile",
        )
        issues = self.issues()
        self.assertTrue(any(
            "local Go dependency graph producer guidance" in issue
            for issue in issues
        ), issues)

    def test_skill_shipped_status_cannot_claim_staged(self):
        path = self.agent_root / "skills" / "evidence-claim-management.md"
        replace_once(path, "已交付", "`ADOPTED-STAGED`")
        issues = self.issues()
        self.assertTrue(any(
            "local Go dependency graph producer guidance" in issue
            for issue in issues
        ), issues)

    def test_registry_freezes_reference_projection_and_no_go_tools(self):
        _, data = self.policy()
        implementation = data["reference_implementations"][
            "local_go_package_dependency_graph_observation_producer_go"
        ]
        self.assertEqual(
            implementation,
            governance.GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_REFERENCE_IMPLEMENTATION,
        )
        producer = governance.LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH_OBSERVATION_PRODUCER
        self.assertEqual(producer["runtime_platform"]["go_commands"], "none")
        self.assertEqual(producer["runtime_platform"]["network_access"],
                         "none_by_profile")
        self.assertEqual(producer["attests"], [])
        self.assertEqual(producer["persistence"], "none")

    def test_registry_requires_complete_non_capability_boundary(self):
        path, data = self.policy()
        data["non_capabilities"].remove(
            governance.GO_PACKAGE_DEPENDENCY_GRAPH_PRODUCER_NON_CAPABILITY
        )
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any(
            "local Go dependency non-capability drifted" in issue
            for issue in issues
        ), issues)


if __name__ == "__main__":
    unittest.main()
