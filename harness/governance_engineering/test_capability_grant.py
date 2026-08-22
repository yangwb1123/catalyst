#!/usr/bin/env python3
"""Governance regression tests for ADR-0056 contract-only delivery."""

import copy
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.capability_grant import (
    CAPABILITY_GRANT,
    FIXTURE_RELATIVE,
    RESULT,
    SCHEMA_RELATIVE,
    SKILL_RELATIVE,
    detector_issues,
    fixture_issues,
    registry_issues,
    schema_issues,
    skill_issues,
)
from engineering_routing.contract import ROUTE_INCLUDE_FLOORS


class CapabilityGrantGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.source_root = Path(__file__).resolve().parents[2]
        self.policy_path = (self.source_root / ".agent" / "engineering" /
                            "governance-contracts.yml")
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_current_registry_detector_and_skill_are_contract_only(self):
        self.assertEqual(self.policy["capability_grant"], CAPABILITY_GRANT)
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(detector_issues(self.source_root / ".agent"), [])
        self.assertEqual(skill_issues(self.source_root), [])
        self.assertEqual(
            self.policy["capability_grant"]["positive_result"], RESULT
        )

    def test_kind_delivery_sets_are_disjoint(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_kinds"].append("CapabilityGrant")
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("cannot be shipped-runtime" in issue for issue in issues), issues)
        self.assertTrue(any("must be disjoint" in issue for issue in issues), issues)

        duplicated = copy.deepcopy(self.policy)
        duplicated["scope"]["shipped_contract_only_kinds"].append("CapabilityGrant")
        issues = registry_issues(duplicated, self.policy_path)
        self.assertTrue(any("duplicate kinds" in issue for issue in issues), issues)

    def test_capability_grant_cannot_remain_planned(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["planned_kinds"].append("CapabilityGrant")
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("cannot be shipped-runtime or planned" in issue
                            for issue in issues), issues)

    def test_governance_route_pins_skill_and_schema_lanes(self):
        path = self.source_root / ".agent" / "engineering" / "context-routes.yml"
        registry = yaml.safe_load(path.read_text(encoding="utf-8"))
        route = next(item for item in registry["routes"] if item["id"] == "governance")
        by_ref = {item["ref"]: item for item in route["include"]}
        expected = {
            ".agent/skills/policy-authority.md": "instruction",
            "docs/contracts/capability-grant-v1.schema.json": "trusted_context",
        }
        for ref, lane in expected.items():
            self.assertEqual(ROUTE_INCLUDE_FLOORS["governance"][ref], lane)
            self.assertEqual(by_ref[ref]["lane"], lane)
            self.assertIs(by_ref[ref]["required"], True)

    def test_fixture_freezes_vocabulary_and_non_authority_result(self):
        self.assertEqual(fixture_issues(self.source_root), [])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.source_root / FIXTURE_RELATIVE
            target = root / FIXTURE_RELATIVE
            target.parent.mkdir(parents=True)
            fixture = json.loads(source.read_text(encoding="utf-8"))
            fixture["effect_vocabulary"]["effects"].reverse()
            fixture["expected_assessment"]["permission_attestation"] = True
            target.write_text(json.dumps(fixture), encoding="utf-8")
            issues = fixture_issues(root)
            self.assertTrue(any("21-effect" in issue for issue in issues), issues)
            self.assertTrue(any("permission_attestation" in issue
                                for issue in issues), issues)

    def test_schema_freezes_contract_only_authority_and_scope(self):
        self.assertEqual(schema_issues(self.source_root), [])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.source_root / SCHEMA_RELATIVE
            target = root / SCHEMA_RELATIVE
            target.parent.mkdir(parents=True)
            schema = json.loads(source.read_text(encoding="utf-8"))
            schema["x-forgeos-authority-semantics"]["production_effects"] = "allowed"
            schema["$defs"]["scope"]["properties"]["deny"]["items"] = {
                "$ref": "#/$defs/scope_clause"}
            schema["$defs"]["principal"]["properties"]["authority_class"] = {
                "const": "external_operator"}
            target.write_text(json.dumps(schema), encoding="utf-8")
            issues = schema_issues(root)
            self.assertTrue(any("authority-semantics" in issue for issue in issues), issues)
            self.assertTrue(any("flat deny" in issue for issue in issues), issues)
            self.assertTrue(any("issuer authority_class only" in issue
                                for issue in issues), issues)

    def test_runtime_or_attestation_escalation_is_rejected(self):
        mutated = copy.deepcopy(self.policy)
        mutated["capability_grant"]["unavailable_runtime"][
            "issuer_authentication"] = "available"
        mutated["capability_grant"]["assessment_constants"][
            "permission_attestation"] = True
        mutated["capability_grant"]["unavailable_runtime"][
            "context_package_integration"] = "available"
        mutated["capability_grant"]["production_effects"] = "allowed"
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract-only boundary drifted" in issue
                            for issue in issues), issues)

    def test_load_bearing_detector_escalation_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.source_root / ".agent" / "engineering" / "detectors.yml"
            target = root / "engineering" / "detectors.yml"
            target.parent.mkdir(parents=True)
            detectors = yaml.safe_load(source.read_text(encoding="utf-8"))
            entry = next(item for item in detectors["detectors"]
                         if item["id"] == "governance.capability_grant_contract")
            entry["state"] = "enforced"
            entry["invocation"]["load_bearing"] = True
            target.write_text(yaml.safe_dump(detectors, sort_keys=False), encoding="utf-8")
            issues = detector_issues(root)
            self.assertTrue(any("shadow" in issue for issue in issues), issues)
            self.assertTrue(any("load-bearing" in issue for issue in issues), issues)

    def test_skill_cannot_drop_authority_denials(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.source_root / SKILL_RELATIVE
            target = root / SKILL_RELATIVE
            target.parent.mkdir(parents=True)
            shutil.copy2(source, target)
            text = target.read_text(encoding="utf-8")
            target.write_text(text.replace("authorization_decision=none", "decision omitted"),
                              encoding="utf-8")
            issues = skill_issues(root)
            self.assertTrue(any("authorization_decision=none" in issue
                                for issue in issues), issues)


if __name__ == "__main__":
    unittest.main()
