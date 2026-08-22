#!/usr/bin/env python3
"""Governance regressions for ADR-0076 portable lexical ImpactPreScan."""

import copy
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from governance_engineering.change_impact_cost_risk_portable import (
    ADAPTER, CANONICAL_REFS, DECISION, MANIFEST_SHA256, NON_CAPABILITY,
    PORTABLE_PROJECTION, PROJECTOR, SKILL, adr_issues, compatibility_issues,
    detector_issues, documentation_issues, package_issues, registry_issues,
    skill_issues, vendor_issues, wiring_issues,
)
from governance_engineering.evidence_claim_portable import EXPECTED_SCOPE
from governance_engineering.registry_contract import LOCAL_GO_PACKAGE_IMPACT_PRESCAN


def _source_roadmap(root):
    audit = root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    roadmap = root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    if not audit.is_file() or not roadmap.is_file():
        return None
    return roadmap.read_text(encoding="utf-8")


class ChangeImpactCostRiskPortableTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.policy_path = self.root / ".agent/engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v31_exact_scope_neutral_and_semantics_unchanged(self):
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(self.policy["scope"], EXPECTED_SCOPE)
        self.assertEqual(
            self.policy["change_impact_cost_risk_portable_projection"],
            PORTABLE_PROJECTION,
        )
        self.assertEqual(
            self.policy["local_go_package_impact_prescan"],
            LOCAL_GO_PACKAGE_IMPACT_PRESCAN,
        )
        self.assertEqual(
            self.policy["contract_pins"][
                "change_impact_cost_risk_package_manifest_sha256"
            ], MANIFEST_SHA256,
        )
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_registry_authority_ref_pin_and_scope_mutations_reject(self):
        mutated = copy.deepcopy(self.policy)
        block = mutated["change_impact_cost_risk_portable_projection"]
        block["authority"]["system_impact"] = "complete"
        mutated["canonical_refs"]["change_impact_cost_risk_portable_skill"] = "wrong"
        mutated["contract_pins"][
            "change_impact_cost_risk_package_manifest_sha256"] = "0" * 64
        mutated["reference_implementations"][
            "change_impact_cost_risk_portable_skill"]["ref"] = "runtime"
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        mutated["scope"]["shipped_evaluators"].append("portable_full_impact")
        issues = registry_issues(mutated, self.policy_path)
        for marker in ("contract drifted", "canonical_refs", "manifest pin",
                       "implementation drifted", "non-capability", "runtime scope"):
            self.assertTrue(any(marker in issue for issue in issues), (marker, issues))

    def test_wire_is_one_zero_argument_existing_request(self):
        operation = PORTABLE_PROJECTION["operation"]
        self.assertEqual(operation["adapter_argv"][-1], PROJECTOR)
        self.assertEqual(PORTABLE_PROJECTION["process"]["arguments"], "zero")
        self.assertEqual(PORTABLE_PROJECTION["input"]["exact_fields"], [
            "api_version", "canonicalization", "changed_paths",
            "graph_observation_base64url", "graph_observation_sha256",
            "request_sha256", "run_id",
        ])
        forbidden = PORTABLE_PROJECTION["input"][
            "raw_graph_fixture_envelope_wrapper_union_dispatch_or_mode"]
        self.assertEqual(forbidden, "forbidden")

    def test_package_semantic_artifacts_and_vendor_leaves_are_exact(self):
        self.assertEqual(package_issues(self.root), [])
        self.assertEqual(compatibility_issues(self.root), [])
        self.assertEqual(vendor_issues(self.root), [])

    def test_detector_is_checker_only_shadow_and_projector_is_not_detector(self):
        self.assertEqual(detector_issues(self.root / ".agent"), [])
        detectors = yaml.safe_load(
            (self.root / ".agent/engineering/detectors.yml").read_text())
        commands = [item["implementation"]["argv"]
                    for item in detectors["detectors"]]
        self.assertFalse(any(PROJECTOR in command for command in commands))

    def test_activation_routes_disciplines_and_adapter_are_frozen(self):
        self.assertEqual(wiring_issues(self.root / ".agent"), [])
        self.assertEqual(skill_issues(self.root), [])
        activation = yaml.safe_load(
            (self.root / ".agent/engineering/activation.yml").read_text())
        refs = activation["canonical_extension_refs"]
        self.assertEqual({key: refs.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)
        routes = yaml.safe_load(
            (self.root / ".agent/engineering/context-routes.yml").read_text())
        route_refs = {item["ref"] for route in routes["routes"]
                      for item in route.get("include", [])}
        self.assertNotIn(SKILL, route_refs)
        self.assertIn(ADAPTER, route_refs)

    def test_docs_adr_and_nested_roadmap_are_frozen(self):
        self.assertEqual(documentation_issues(self.root), [])
        self.assertEqual(adr_issues(self.root), [])
        roadmap = _source_roadmap(self.root)
        if roadmap is None:
            self.skipTest("Catalyst-only roadmap is absent from generated trees")
        self.assertIn("- [ ] 按 `implementation_wave` 逐 package 实现 Skill", roadmap)
        self.assertIn("  - [x] `change-impact-cost-risk` 窄切片", roadmap)
        self.assertIn("其余 31 个 package items 保持开放", roadmap)
        self.assertIn("- [ ] 从模块/import/call", roadmap)
        self.assertIn("- [ ] 记录 extractor coverage", roadmap)

    def test_generated_tree_without_source_docs_keeps_core_checks(self):
        root = self._temporary_repo()
        (root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").unlink(missing_ok=True)
        (root / "docs/design/ai-engineering-os/implementation-roadmap.md").unlink(
            missing_ok=True)
        self.assertEqual(documentation_issues(root), [])
        self.assertIsNone(_source_roadmap(root))
        self.assertEqual(package_issues(root), [])
        self.assertEqual(compatibility_issues(root), [])
        self.assertEqual(vendor_issues(root), [])
        self.assertEqual(skill_issues(root), [])
        self.assertEqual(wiring_issues(root / ".agent"), [])
        self.assertEqual(detector_issues(root / ".agent"), [])
        self.assertEqual(adr_issues(root), [])

    def test_temp_manifest_vendor_adapter_and_adr_drift_reject(self):
        root = self._temporary_repo()
        manifest = root / "skills/change-impact-cost-risk/references/package-manifest.json"
        manifest.write_bytes(manifest.read_bytes() + b"\n")
        self.assertTrue(package_issues(root))
        vendor = (root / "skills/change-impact-cost-risk/scripts/_vendor/"
                  "local_go_package_impact_prescan_contract/derive.py")
        vendor.write_bytes(vendor.read_bytes() + b"\n")
        self.assertTrue(vendor_issues(root))
        adapter = root / ADAPTER
        adapter.write_text(adapter.read_text().replace("explicit EOF", "implicit"))
        self.assertTrue(skill_issues(root))
        decision = root / DECISION
        decision.write_bytes(decision.read_bytes() + b"\n")
        self.assertTrue(adr_issues(root))

    def test_temp_activation_route_and_projector_detector_reject(self):
        root = self._temporary_repo()
        activation = root / ".agent/engineering/activation.yml"
        activation.write_text(activation.read_text().replace(
            "  change_impact_cost_risk_portable_skill: "
            "skills/change-impact-cost-risk/SKILL.md\n", ""))
        self.assertTrue(wiring_issues(root / ".agent"))
        detector = root / ".agent/engineering/detectors.yml"
        detector.write_text(detector.read_text().replace(
            "skills/change-impact-cost-risk/scripts/check_package.py", PROJECTOR, 1))
        self.assertTrue(detector_issues(root / ".agent"))

    def _temporary_repo(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in (
            ".agent", "skills/change-impact-cost-risk",
            "harness/governance_contract", "harness/architecture_decision_record_v2",
            "harness/local_command_observation_producer",
            "harness/go_package_dependency_graph_observation_producer",
            "harness/local_go_package_impact_prescan_contract",
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
