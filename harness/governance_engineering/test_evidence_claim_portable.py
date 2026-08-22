#!/usr/bin/env python3
"""Governance regression tests for ADR-0072 portable validation delivery."""

import copy
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.evidence_claim_portable import (
    ADAPTER_RELATIVE, CANONICAL_REFS, DELIVERY_DECISION_RELATIVE,
    NON_CAPABILITY, PACKAGE_MANIFEST_SHA256, PORTABLE_SKILL_RELATIVE,
    PORTABLE_VALIDATION, adr_issues, compatibility_issues,
    detector_issues, documentation_issues, package_issues, registry_issues,
    skill_issues, vendor_issues, wiring_issues,
)


class EvidenceClaimPortableGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        policy_path = self.root / ".agent/engineering/governance-contracts.yml"
        self.policy_path = policy_path
        self.policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))

    def test_current_registry_is_exact_and_scope_neutral(self):
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(self.policy["evidence_claim_portable_validation"],
                         PORTABLE_VALIDATION)
        self.assertEqual(self.policy["contract_pins"][
            "evidence_claim_package_manifest_sha256"], PACKAGE_MANIFEST_SHA256)
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_registry_ref_pin_contract_and_noncapability_drift_reject(self):
        mutated = copy.deepcopy(self.policy)
        mutated["evidence_claim_portable_validation"]["persistence"] = "sqlite"
        mutated["canonical_refs"]["evidence_claim_portable_skill"] = "wrong"
        mutated["contract_pins"]["evidence_claim_package_manifest_sha256"] = "0" * 64
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract drifted" in issue for issue in issues), issues)
        self.assertTrue(any("canonical_refs" in issue for issue in issues), issues)
        self.assertTrue(any("manifest pin" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_runtime_scope_promotion_rejects(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "evidence_claim_portable_validation"
        )
        self.assertTrue(any("must not expand runtime scope" in issue
                            for issue in registry_issues(mutated, self.policy_path)))

    def test_package_fixture_vendor_and_compatibility_are_exact(self):
        self.assertEqual(package_issues(self.root), [])
        self.assertEqual(compatibility_issues(self.root), [])
        self.assertEqual(vendor_issues(self.root), [])

    def test_detector_is_shadow_and_exact(self):
        self.assertEqual(detector_issues(self.root / ".agent"), [])
        detectors = yaml.safe_load((self.root / ".agent/engineering/detectors.yml")
                                   .read_text(encoding="utf-8"))
        commands = [item["implementation"]["argv"]
                    for item in detectors["detectors"]]
        self.assertFalse(any("skills/evidence-claim-management/scripts/validate.py"
                             in command for command in commands))

    def test_activation_route_disciplines_and_adapter_are_frozen(self):
        self.assertEqual(wiring_issues(self.root / ".agent"), [])
        self.assertEqual(skill_issues(self.root), [])
        activation = yaml.safe_load((self.root / ".agent/engineering/activation.yml")
                                    .read_text(encoding="utf-8"))
        refs = activation["canonical_extension_refs"]
        self.assertEqual({key: refs.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)

    def test_portable_skill_is_absent_from_every_route(self):
        routes = yaml.safe_load((self.root / ".agent/engineering/context-routes.yml")
                                .read_text(encoding="utf-8"))
        refs = {item["ref"] for route in routes["routes"]
                for item in route.get("include", [])}
        self.assertNotIn(PORTABLE_SKILL_RELATIVE, refs)
        self.assertIn(ADAPTER_RELATIVE, refs)

    def test_docs_and_adr_are_frozen(self):
        self.assertEqual(documentation_issues(self.root), [])
        self.assertEqual(adr_issues(self.root), [])

    def test_generated_project_without_source_docs_skips_source_doc_index(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.assertEqual(documentation_issues(Path(temporary.name)), [])

    def _temporary_repo(self, source_root=None):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        source_root = source_root or self.root
        for relative in (
            ".agent", "skills/evidence-claim-management",
            "harness/governance_contract", "docs/adr", "docs/contracts",
            "docs/design/ai-engineering-os",
        ):
            source = source_root / relative
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(source, target)
        source_audit = source_root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        if source_audit.is_file():
            shutil.copy2(source_audit,
                         root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
        return root

    def test_temporary_repo_accepts_generated_tree_without_source_audit(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        generated = Path(temporary.name)
        for relative in (
            ".agent", "skills/evidence-claim-management",
            "harness/governance_contract", "docs/adr", "docs/contracts",
            "docs/design/ai-engineering-os",
        ):
            (generated / relative).mkdir(parents=True)
        copied = self._temporary_repo(generated)
        self.assertFalse((copied / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").exists())

    def test_missing_activation_and_discipline_assets_reject(self):
        root = self._temporary_repo()
        activation = root / ".agent/engineering/activation.yml"
        text = activation.read_text(encoding="utf-8")
        activation.write_text(text.replace(
            "  evidence_claim_portable_skill: skills/evidence-claim-management/SKILL.md\n",
            ""), encoding="utf-8")
        issues = wiring_issues(root / ".agent")
        self.assertTrue(any("activation" in issue for issue in issues), issues)
        disciplines = root / ".agent/engineering/disciplines.yml"
        text = disciplines.read_text(encoding="utf-8")
        disciplines.write_text(text.replace(
            ", skills/evidence-claim-management/SKILL.md", ""), encoding="utf-8")
        issues = wiring_issues(root / ".agent")
        self.assertTrue(any("contract asset" in issue for issue in issues), issues)

    def test_adapter_and_vendored_module_drift_reject(self):
        root = self._temporary_repo()
        adapter = root / ADAPTER_RELATIVE
        adapter.write_text(adapter.read_text(encoding="utf-8").replace(
            "explicit EOF", "implicit input end", 1), encoding="utf-8")
        self.assertTrue(any("missing portable marker" in issue
                            for issue in skill_issues(root)))
        vendor = (root / "skills/evidence-claim-management/scripts/_vendor/"
                  "governance_contract/shape.py")
        vendor.write_bytes(vendor.read_bytes() + b"\n")
        self.assertTrue(any("vendored ADR-0045 module drifted" in issue
                            for issue in vendor_issues(root)))

    def test_manifest_and_adr_physical_drift_reject(self):
        root = self._temporary_repo()
        manifest = root / "skills/evidence-claim-management/references/package-manifest.json"
        manifest.write_bytes(manifest.read_bytes() + b"\n")
        self.assertTrue(any("manifest pin drifted" in issue
                            for issue in package_issues(root)))
        decision = root / DELIVERY_DECISION_RELATIVE
        decision.write_bytes(decision.read_bytes() + b"\n")
        self.assertTrue(adr_issues(root))


if __name__ == "__main__":
    unittest.main()
