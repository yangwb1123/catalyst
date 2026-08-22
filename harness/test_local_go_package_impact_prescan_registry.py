#!/usr/bin/env python3
"""Governance registry, activation, and routing tests for ADR-0062."""

import shutil
import unittest

import yaml

from agent_engineering.support import engineering, make_temp_repo
from governance_engineering.registry_contract import (
    IMPACT_PRESCAN_NON_CAPABILITY,
    LOCAL_GO_PACKAGE_IMPACT_PRESCAN,
)


SCHEMA = "docs/contracts/local-go-package-impact-prescan-v1.schema.json"


class LocalGoPackageImpactPrescanRegistryTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.agent_root = self.repo / ".agent"

    def issues(self):
        return engineering.check_agent_engineering_spec(self.agent_root)

    def load_policy(self):
        path = self.agent_root / "engineering" / "governance-contracts.yml"
        return path, yaml.safe_load(path.read_text(encoding="utf-8"))

    def test_live_registry_is_pure_evaluator_not_producer(self):
        _, data = self.load_policy()
        self.assertEqual(data["version"], 39)
        self.assertEqual(data["scope"]["shipped_evaluators"],
                         ["local_go_package_impact_prescan", "graph_snapshot",
                          "graph_snapshot_test_source",
                          "architecture_decision_record_v2",
                          "capability_registry",
                          "planning_capability_ownership",
                          "project_source_snapshot"])
        self.assertNotIn("local_go_package_impact_prescan",
                         data["scope"]["shipped_producers"])
        self.assertEqual(data["local_go_package_impact_prescan"],
                         LOCAL_GO_PACKAGE_IMPACT_PRESCAN)

    def test_scope_and_non_capability_drift_fail_closed(self):
        path, data = self.load_policy()
        data["scope"]["shipped_evaluators"] = []
        data["non_capabilities"].remove(
            IMPACT_PRESCAN_NON_CAPABILITY)
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("shipped pure evaluator scope drifted" in issue
                            for issue in issues), issues)
        self.assertTrue(any("ImpactPreScan non-capability" in issue
                            for issue in issues), issues)

    def test_schema_pin_and_semantics_are_enforced(self):
        path = self.repo / SCHEMA
        text = path.read_text(encoding="utf-8")
        text = text.replace(
            '"delivery": "shipped_pure_local_runtime_and_strict_checker"',
            '"delivery": "full_impact_closure"', 1)
        path.write_text(text, encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("local_go_package_impact_prescan_schema_sha256" in issue
                            for issue in issues), issues)
        self.assertTrue(any("x-forgeos-authority-semantics drifted" in issue
                            for issue in issues), issues)

    def test_fixture_pin_and_exact_derivation_are_enforced(self):
        path = (self.repo / "docs" / "contracts" / "fixtures" /
                "local-go-package-impact-prescan-v1.json")
        text = path.read_text(encoding="utf-8")
        path.write_text(text.replace("impact-fixture-001",
                                     "impact-fixture-002", 1), encoding="utf-8")
        issues = self.issues()
        self.assertTrue(any("local_go_package_impact_prescan_golden_fixture_sha256"
                            in issue for issue in issues), issues)
        self.assertTrue(any("golden" in issue and "exact derived" in issue
                            for issue in issues), issues)

    def test_activation_ref_is_required(self):
        path = self.agent_root / "engineering" / "activation.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        del data["canonical_extension_refs"][
            "local_go_package_impact_prescan_checker"]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("canonical_extension_refs" in issue
                            for issue in self.issues()))

    def test_governance_and_architecture_routes_require_schema(self):
        path = self.agent_root / "engineering" / "context-routes.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        for route_id in ("governance", "architecture-boundary"):
            route = next(item for item in data["routes"]
                         if item["id"] == route_id)
            route["include"] = [item for item in route["include"]
                                if item["ref"] != SCHEMA]
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = self.issues()
        self.assertGreaterEqual(sum("weakens required context" in issue
                                    for issue in issues), 2, issues)


if __name__ == "__main__":
    unittest.main()
