#!/usr/bin/env python3
"""Governance regressions for ADR-0074 portable Proposed validation."""

import copy
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from governance_engineering.adr_governance_portable import (
    ADAPTER, CANONICAL_REFS, DECISION, MANIFEST_SHA256, NON_CAPABILITY,
    PORTABLE_VALIDATION, SKILL, adr_issues, compatibility_issues,
    detector_issues, documentation_issues, package_issues, registry_issues,
    skill_issues, vendor_issues, wiring_issues,
)


def _source_roadmap(root):
    audit = root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    roadmap = root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    if not audit.is_file() or not roadmap.is_file():
        return None
    return roadmap.read_text(encoding="utf-8")


class ADRGovernancePortableTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.policy_path = self.root / ".agent/engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_preserves_v29_contract_at_v30(self):
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(
            self.policy["adr_governance_portable_proposed_document_validation"],
            PORTABLE_VALIDATION,
        )
        self.assertEqual(
            PORTABLE_VALIDATION["input"]["request_envelope"],
            "none_preserves_ADR_0067_document_wire",
        )
        self.assertEqual(
            self.policy["contract_pins"][
                "adr_governance_package_manifest_sha256"], MANIFEST_SHA256,
        )
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_registry_mutations_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        block = mutated["adr_governance_portable_proposed_document_validation"]
        block["authority"]["acceptance_compliance_or_lifecycle"] = "available"
        mutated["canonical_refs"]["adr_governance_portable_skill"] = "wrong"
        mutated["contract_pins"]["adr_governance_package_manifest_sha256"] = "0" * 64
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("contract drifted" in item for item in issues), issues)
        self.assertTrue(any("canonical_refs" in item for item in issues), issues)
        self.assertTrue(any("manifest pin" in item for item in issues), issues)
        self.assertTrue(any("non-capability" in item for item in issues), issues)

    def test_runtime_scope_and_zero_argument_promotion_reject(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "adr_governance_portable_proposed_document_validation")
        block = mutated["adr_governance_portable_proposed_document_validation"]
        block["input"]["basename_argument"] = "zero"
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("scope" in item for item in issues), issues)
        self.assertTrue(any("contract drifted" in item for item in issues), issues)

    def test_package_vendor_and_semantic_pins_are_exact(self):
        self.assertEqual(package_issues(self.root), [])
        self.assertEqual(vendor_issues(self.root), [])
        self.assertEqual(compatibility_issues(self.root), [])

    def test_detector_is_shadow_and_validator_is_not_detector(self):
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

    def test_adapter_repo_root_argv_is_exact_and_short_path_rejects(self):
        root = self._temporary_repo()
        adapter = root / ADAPTER
        adapter.write_text(adapter.read_text().replace(
            "skills/adr-governance/scripts/", "scripts/"), encoding="utf-8")
        issues = skill_issues(root)
        self.assertTrue(any("skills/adr-governance/scripts/" in item
                            for item in issues), issues)

    def test_docs_and_adr_are_frozen(self):
        self.assertEqual(documentation_issues(self.root), [])
        self.assertEqual(adr_issues(self.root), [])

    def test_parent_lifecycle_and_query_remain_open_while_legacy_is_closed(self):
        roadmap = _source_roadmap(self.root)
        if roadmap is None:
            self.skipTest("Catalyst-only roadmap is absent from generated trees")
        for marker in (
            "- [ ] 按 `implementation_wave` 逐 package 实现 Skill",
            "- [ ] 实现 Accepted ADR immutable + supersede 状态机",
            "- [ ] 合并 `.agent/DECISIONS` 与 ADR 的查询视图",
            "- [x] 设计旧 memory/ADR 的只读导入",
        ):
            self.assertIn(marker, roadmap)
        self.assertNotIn("- [ ] 设计旧 memory/ADR 的只读导入", roadmap)

    def test_generated_tree_without_source_docs_skips_doc_index(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            readme = root / ".agent/engineering/README.md"
            readme.parent.mkdir(parents=True)
            readme.write_text("Catalyst source documentation is absent.\n")
            self.assertEqual(documentation_issues(root), [])
            self.assertIsNone(_source_roadmap(root))

    def test_manifest_vendor_adapter_and_adr_drift_reject(self):
        root = self._temporary_repo()
        manifest = root / "skills/adr-governance/references/package-manifest.json"
        manifest.write_bytes(manifest.read_bytes() + b"\n")
        self.assertTrue(package_issues(root))
        vendor = (root / "skills/adr-governance/scripts/_vendor/"
                  "architecture_decision_record_v2/shape.py")
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
            "  adr_governance_portable_skill: skills/adr-governance/SKILL.md\n", ""))
        self.assertTrue(wiring_issues(root / ".agent"))
        disciplines = root / ".agent/engineering/disciplines.yml"
        disciplines.write_text(disciplines.read_text().replace(
            ", skills/adr-governance/SKILL.md", ""))
        self.assertTrue(wiring_issues(root / ".agent"))

    def _temporary_repo(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in (
            ".agent", "skills/adr-governance",
            "harness/architecture_decision_record_v2", "harness/governance_contract",
            "docs/adr", "docs/contracts", "docs/design/ai-engineering-os",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(self.root / relative, target)
        audit = self.root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        if audit.is_file():
            shutil.copy2(audit, root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
        return root


if __name__ == "__main__":
    unittest.main()
