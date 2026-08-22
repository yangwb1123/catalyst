#!/usr/bin/env python3
"""Governance regressions for ADR-0073 portable declaration assessment."""

import copy
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from governance_engineering.policy_authority_portable import (
    ADAPTER, CANONICAL_REFS, DECISION, MANIFEST_SHA256, NON_CAPABILITY,
    PORTABLE_ASSESSMENT, SKILL, adr_issues, compatibility_issues,
    detector_issues, documentation_issues, package_issues, registry_issues,
    skill_issues, vendor_issues, wiring_issues,
)


class PolicyAuthorityPortableGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.policy_path = self.root / ".agent/engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_preserves_contract_at_v30(self):
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(
            self.policy["policy_authority_portable_declaration_assessment"],
            PORTABLE_ASSESSMENT,
        )
        self.assertEqual(
            self.policy["contract_pins"][
                "policy_authority_package_manifest_sha256"], MANIFEST_SHA256,
        )
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_registry_mutations_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        block = mutated["policy_authority_portable_declaration_assessment"]
        block["authority"]["authorization_decision"] = "allow"
        mutated["canonical_refs"]["policy_authority_portable_skill"] = "wrong"
        mutated["contract_pins"][
            "policy_authority_package_manifest_sha256"] = "0" * 64
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract drifted" in issue for issue in issues), issues)
        self.assertTrue(any("canonical_refs" in issue for issue in issues), issues)
        self.assertTrue(any("manifest pin" in issue for issue in issues), issues)
        self.assertTrue(any("non-capability" in issue for issue in issues), issues)

    def test_runtime_scope_promotion_rejects(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "policy_authority_portable_declaration_assessment")
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("must not expand runtime scope" in item
                            for item in issues), issues)

    def test_package_vendor_fixtures_and_semantic_pins_are_exact(self):
        self.assertEqual(package_issues(self.root), [])
        self.assertEqual(vendor_issues(self.root), [])
        self.assertEqual(compatibility_issues(self.root), [])

    def test_detector_is_shadow_and_adapters_are_not_detectors(self):
        self.assertEqual(detector_issues(self.root / ".agent"), [])

    def test_activation_route_disciplines_and_skills_are_exact(self):
        self.assertEqual(wiring_issues(self.root / ".agent"), [])
        self.assertEqual(skill_issues(self.root), [])
        activation = yaml.safe_load(
            (self.root / ".agent/engineering/activation.yml").read_text())
        refs = activation["canonical_extension_refs"]
        self.assertEqual({key: refs.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)

    def test_portable_skill_is_absent_from_every_route(self):
        routes = yaml.safe_load(
            (self.root / ".agent/engineering/context-routes.yml").read_text())
        refs = {item["ref"] for route in routes["routes"]
                for item in route.get("include", [])}
        self.assertNotIn(SKILL, refs)
        self.assertIn(ADAPTER, refs)

    def test_adapter_repo_root_argv_is_exact_and_short_paths_reject(self):
        root = self._temporary_repo()
        adapter = root / ADAPTER
        adapter.write_text(adapter.read_text().replace(
            "skills/policy-authority/scripts/", "scripts/"),
            encoding="utf-8",
        )
        issues = skill_issues(root)
        self.assertTrue(any(
            "skills/policy-authority/scripts/" in issue for issue in issues
        ), issues)

    def test_docs_and_adr_are_frozen(self):
        self.assertEqual(documentation_issues(self.root), [])
        self.assertEqual(adr_issues(self.root), [])

    def test_old_pending_scaffold_documentation_rejects(self):
        if not (self.root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
            self.skipTest("Catalyst source-documentation sentinel is absent")
        root = self._temporary_repo()
        readme = root / ".agent/engineering/README.md"
        text = readme.read_text(encoding="utf-8")
        readme.write_text(text.replace(
            "Source-only fresh and legacy scaffold now copies the sealed source "
            "package and\n  governance checks; it installs no host Skill or runtime "
            "and grants no authority.",
            "Source-only scaffold remains pending and no host Skill installation "
            "is claimed."), encoding="utf-8")
        issues = documentation_issues(root)
        self.assertTrue(any("stale pending scaffold claim" in issue
                            for issue in issues), issues)

    def test_generated_tree_without_source_docs_skips_doc_index(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        readme = root / ".agent/engineering/README.md"
        readme.parent.mkdir(parents=True)
        readme.write_text(
            "Source-only scaffold remains pending and no host Skill "
            "installation is claimed.\n",
            encoding="utf-8",
        )
        self.assertEqual(documentation_issues(root), [])

    def _temporary_repo(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in (
            ".agent", "skills/policy-authority", "harness/capability_grant_contract",
            "harness/approval_record_contract", "docs/adr", "docs/contracts",
            "docs/design/ai-engineering-os",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(self.root / relative, target)
        audit = self.root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        if audit.is_file():
            shutil.copy2(audit, root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
        return root

    def test_manifest_vendor_adapter_and_adr_drift_reject(self):
        root = self._temporary_repo()
        manifest = root / "skills/policy-authority/references/package-manifest.json"
        manifest.write_bytes(manifest.read_bytes() + b"\n")
        self.assertTrue(package_issues(root))
        vendor = (root / "skills/policy-authority/scripts/_vendor/"
                  "capability_grant_contract/shape.py")
        vendor.write_bytes(vendor.read_bytes() + b"\n")
        self.assertTrue(vendor_issues(root))
        adapter = root / ADAPTER
        adapter.write_text(adapter.read_text().replace("explicit EOF", "implicit end", 1))
        self.assertTrue(skill_issues(root))
        decision = root / DECISION
        decision.write_bytes(decision.read_bytes() + b"\n")
        self.assertTrue(adr_issues(root))

    def test_missing_activation_and_contract_asset_reject(self):
        root = self._temporary_repo()
        activation = root / ".agent/engineering/activation.yml"
        activation.write_text(activation.read_text().replace(
            "  policy_authority_portable_skill: skills/policy-authority/SKILL.md\n",
            ""))
        self.assertTrue(wiring_issues(root / ".agent"))
        disciplines = root / ".agent/engineering/disciplines.yml"
        disciplines.write_text(disciplines.read_text().replace(
            ", skills/policy-authority/SKILL.md", ""))
        self.assertTrue(wiring_issues(root / ".agent"))


if __name__ == "__main__":
    unittest.main()
