#!/usr/bin/env python3
"""Governance integration tests for the ADR-0055 ContextPackage contract."""

import copy
import json
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.context_package import (
    CANONICAL_REFS, CONTEXT_PACKAGE, NON_CAPABILITY, PACKAGE_MANIFEST_SHA256,
    PORTABLE_DETECTOR, SCHEMA_RELATIVE, adr_issues, detector_issues,
    documentation_issues, package_issues, registry_issues, schema_issues,
    skill_issues, wiring_issues,
)


class ContextPackageGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.source_root = Path(__file__).resolve().parents[2]
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        self.schema = self.repo / SCHEMA_RELATIVE
        self.schema.parent.mkdir(parents=True)
        shutil.copy2(self.source_root / SCHEMA_RELATIVE, self.schema)

    def test_current_schema_metadata_is_frozen(self):
        self.assertEqual(schema_issues(self.repo), [])

    def test_schema_semantics_drift_is_rejected(self):
        value = json.loads(self.schema.read_text(encoding="utf-8"))
        value["x-forgeos-context-semantics"]["instruction_allowed"] = True
        self.schema.write_text(json.dumps(value), encoding="utf-8")
        issues = schema_issues(self.repo)
        self.assertTrue(any("x-forgeos-context-semantics drifted" in issue
                            for issue in issues), issues)

    def test_schema_duplicate_key_is_rejected(self):
        raw = self.schema.read_text(encoding="utf-8")
        raw = raw.replace('"title": "ForgeOS ContextPackage',
                          '"title": "duplicate",\n  "title": "ForgeOS ContextPackage', 1)
        self.schema.write_text(raw, encoding="utf-8")
        issues = schema_issues(self.repo)
        self.assertTrue(any("duplicate JSON key" in issue for issue in issues), issues)

    def test_schema_forbidden_scalar_patterns_match_trailing_newline(self):
        schema = json.loads(self.schema.read_text(encoding="utf-8"))
        definitions = schema["$defs"]
        self.assertIsNotNone(re.search(definitions["text"]["not"]["pattern"], "task\n"))
        self.assertIsNotNone(re.search(definitions["content"]["not"]["pattern"], "body\r"))
        self.assertEqual(definitions["hash"]["maxLength"], 64)

    def test_schema_forbids_required_snippet_truncation(self):
        definitions = json.loads(self.schema.read_text(encoding="utf-8"))["$defs"]
        snippet_rules = definitions["snippet"]["allOf"]
        required_rule = snippet_rules[-1]
        self.assertEqual(required_rule["if"]["properties"]["required"], {"const": True})
        self.assertEqual(
            required_rule["then"]["properties"]["truncation"], {"type": "null"}
        )
        self.assertEqual(
            definitions["truncation_receipt"]["properties"]["retained_bytes"]["minimum"], 1
        )
        self.assertEqual(
            definitions["accounting"]["properties"]["candidate_count"]["minimum"], 1
        )

    def test_current_registry_and_detector_match_contract(self):
        policy_path = (self.source_root / ".agent" / "engineering" /
                       "governance-contracts.yml")
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertEqual(policy["context_package"], CONTEXT_PACKAGE)
        self.assertEqual(registry_issues(policy, policy_path), [])
        self.assertEqual(detector_issues(self.source_root / ".agent"), [])

    def test_context_skill_keeps_non_authority_markers(self):
        self.assertEqual(skill_issues(self.source_root), [])

    def test_registry_adds_only_closed_portable_delivery_without_authority(self):
        policy_path = self.source_root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        portable = policy["context_package"]["portable_package"]
        self.assertEqual(policy["version"], 39)
        self.assertEqual(portable["assembler_arguments"], "zero")
        self.assertEqual(portable["checker_package_root_argument"], "zero_or_one")
        self.assertFalse(portable["reads_ambient_sources"])
        self.assertFalse(portable["invokes_provider_or_model"])
        self.assertEqual(portable["grant_pdp_authority"], "unavailable")
        self.assertEqual(portable["persistence"], "none")
        self.assertEqual(policy["contract_pins"][
            "context_package_package_manifest_sha256"], PACKAGE_MANIFEST_SHA256)
        self.assertIn(NON_CAPABILITY, policy["non_capabilities"])

    def test_portable_delivery_or_authority_drift_fails_closed(self):
        policy_path = self.source_root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        mutated = copy.deepcopy(policy)
        mutated["context_package"]["portable_package"][
            "invokes_provider_or_model"] = True
        mutated["contract_pins"][
            "context_package_package_manifest_sha256"] = "0" * 64
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, policy_path)
        self.assertTrue(any("contract drifted" in issue for issue in issues), issues)
        self.assertTrue(any("manifest pin drifted" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_portable_package_adr_docs_and_wiring_are_frozen(self):
        self.assertEqual(package_issues(self.source_root), [])
        self.assertEqual(adr_issues(self.source_root), [])
        self.assertEqual(documentation_issues(self.source_root), [])
        self.assertEqual(wiring_issues(self.source_root,
                                       self.source_root / ".agent"), [])

    def test_activation_refs_and_detector_remain_non_load_bearing(self):
        from agent_engineering.contract import EXTENSION_REFS
        expected = {
            "context_package_portable_skill": CANONICAL_REFS[
                "context_package_portable_skill"],
            "context_package_package_manifest": CANONICAL_REFS[
                "context_package_package_manifest"],
            "context_engineering_skill_decision": CANONICAL_REFS[
                "context_engineering_skill_decision"],
        }
        self.assertEqual({key: EXTENSION_REFS.get(key) for key in expected}, expected)
        detectors = yaml.safe_load((self.source_root /
            ".agent/engineering/detectors.yml").read_text(encoding="utf-8"))
        detector = next(item for item in detectors["detectors"] if item["id"] ==
                        "governance.context_engineering_portable_package")
        self.assertEqual(detector["implementation"]["argv"],
                         PORTABLE_DETECTOR["argv"])
        self.assertEqual(detector["state"], "shadow")
        self.assertFalse(detector["invocation"]["load_bearing"])


if __name__ == "__main__":
    unittest.main()
