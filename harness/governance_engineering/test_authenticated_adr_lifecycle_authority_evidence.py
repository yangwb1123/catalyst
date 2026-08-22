#!/usr/bin/env python3
"""Regressions for ADR-0084/0085 lifecycle authority evidence governance."""

import copy
import hashlib
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from agent_engineering.contract import EXTENSION_REFS
from governance_engineering.authenticated_adr_lifecycle_authority_evidence import (
    AUTHORITY_DECISION_SHA256,
    AUTHORITY_EVIDENCE,
    AUTHORITY_FILES,
    AUTHORITY_MANIFEST_SHA256,
    AUTHORITY_PACKAGE,
    CANONICAL_REFS,
    DETECTOR_ID,
    GOVERNANCE_DECISION_BODY_SHA256,
    GOVERNANCE_DECISION_SELF_SHA256,
    GOVERNANCE_DECISION_SHA256,
    NON_CAPABILITY,
    PINS,
    REFERENCE_IMPLEMENTATIONS,
    SCOPE_SHA256,
    adr_issues,
    authority_implementation_issues,
    detector_issues,
    documentation_issues,
    integration_issues,
    registry_issues,
    wiring_issues,
)


class AuthenticatedADRLifecycleAuthorityEvidenceGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_v36_records_catalyst_only_authority_without_scope(self):
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(
            self.policy["authenticated_adr_lifecycle_v1_go_authority_evidence"],
            AUTHORITY_EVIDENCE,
        )
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        canonical = json.dumps(self.policy["scope"], sort_keys=True,
                               separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), SCOPE_SHA256)
        self.assertEqual(PINS, {key: self.policy["contract_pins"][key]
                                for key in PINS})
        self.assertEqual(REFERENCE_IMPLEMENTATIONS, {
            key: self.policy["reference_implementations"][key]
            for key in REFERENCE_IMPLEMENTATIONS
        })
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_catalyst_exact44_authority_bytes_and_semantics_are_frozen(self):
        package = self.repo / AUTHORITY_PACKAGE
        if not package.is_dir():
            self.skipTest("Catalyst-only Go lifecycle authority is not distributed")
        self.assertEqual(len(AUTHORITY_FILES), 44)
        self.assertEqual(authority_implementation_issues(self.repo), [])
        self.assertEqual(AUTHORITY_MANIFEST_SHA256,
                         "1a85aa0aa90414039815e90c7be53d56d0222c8a742e37f33ef9681586a00778")

    def test_authority_inventory_mode_link_symlink_and_bytes_fail_closed(self):
        source = self.repo / AUTHORITY_PACKAGE
        if not source.is_dir():
            self.assertFalse(source.exists())
            return
        root = self._authority_tree()
        package = root / AUTHORITY_PACKAGE
        self.assertEqual(authority_implementation_issues(root), [])
        extra = package / "extra.go"
        extra.write_text("package authenticatedadrlifecycleauthority\n", encoding="utf-8")
        self.assertTrue(authority_implementation_issues(root))
        extra.unlink()
        target = package / "types.go"
        saved = package / "types.saved"
        target.rename(saved)
        target.symlink_to("service.go")
        self.assertTrue(authority_implementation_issues(root))
        target.unlink()
        saved.rename(target)
        alias = package / "types.alias"
        os.link(target, alias)
        self.assertTrue(authority_implementation_issues(root))
        alias.unlink()
        target.chmod(0o600)
        self.assertTrue(authority_implementation_issues(root))
        target.chmod(0o644)
        raw = target.read_bytes()
        target.write_bytes(raw + b"\n")
        self.assertTrue(authority_implementation_issues(root))

    def test_strict_proposed_authority_and_governance_decisions_are_pinned(self):
        self.assertEqual(adr_issues(self.repo), [])
        self.assertEqual(AUTHORITY_DECISION_SHA256,
                         "5792739e70a6bdb6672ab5edbf9abe75a4c5ff16c4be770ac61e26a27e86dc48")
        self.assertEqual(GOVERNANCE_DECISION_BODY_SHA256,
                         "445f258a82446bb6aa436d15a7c93dbbaab40b142e102c8f8b287f922e396c56")
        self.assertEqual(GOVERNANCE_DECISION_SELF_SHA256,
                         "dbda4e571bb92f0bce4e6c1b7bae358dc56ccc0d793a82366c900c8e5f3cbdce")
        self.assertEqual(GOVERNANCE_DECISION_SHA256,
                         "481cb05ec6b1b0a729d0bf928bdd9a31df039fb14e1f50741cc9efc7b773f728")

    def test_checker_only_detector_activation_discipline_route_and_no_skill(self):
        self.assertEqual(detector_issues(self.agent), [])
        self.assertEqual(wiring_issues(self.agent), [])
        self.assertEqual({key: EXTENSION_REFS.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)
        detectors = yaml.safe_load(
            (self.agent / "engineering/detectors.yml").read_text(encoding="utf-8")
        )["detectors"]
        lifecycle = [item for item in detectors if
                     "authenticated_adr_lifecycle" in item["id"]]
        self.assertEqual([item["id"] for item in lifecycle], [DETECTOR_ID])
        self.assertFalse(lifecycle[0]["invocation"]["load_bearing"])

    def test_registry_authority_distribution_and_pin_mutations_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_runtime_profiles"].append(
            "authenticated_adr_lifecycle_v1")
        mutated["authenticated_adr_lifecycle_v1_go_authority_evidence"][
            "boundary"]["copies_go_contract_or_authority"] = True
        mutated["contract_pins"][
            "authenticated_adr_lifecycle_v1_go_authority_manifest_sha256"] = "0" * 64
        mutated["reference_implementations"][
            "authenticated_adr_lifecycle_v1_go_authority"]["projection"] = (
                "source_distributed"
            )
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("expand scope" in issue for issue in issues), issues)
        self.assertTrue(any("authority evidence" in issue for issue in issues), issues)
        self.assertTrue(any("contract_pins" in issue for issue in issues), issues)
        self.assertTrue(any("reference_implementations" in issue
                            for issue in issues), issues)

    def test_go_authority_route_and_service_detector_mutations_fail_closed(self):
        with tempfile.TemporaryDirectory() as raw:
            agent = Path(raw) / ".agent"
            shutil.copytree(self.agent, agent)
            route_path = agent / "engineering/context-routes.yml"
            routes = yaml.safe_load(route_path.read_text(encoding="utf-8"))
            routes["routes"][0].setdefault("include", []).append(
                {"ref": AUTHORITY_PACKAGE})
            route_path.write_text(yaml.safe_dump(routes), encoding="utf-8")
            self.assertTrue(any("context route" in issue
                                for issue in wiring_issues(agent)))
        with tempfile.TemporaryDirectory() as raw:
            agent = Path(raw) / ".agent"
            shutil.copytree(self.agent, agent)
            detector_path = agent / "engineering/detectors.yml"
            data = yaml.safe_load(detector_path.read_text(encoding="utf-8"))
            duplicate = copy.deepcopy(next(
                item for item in data["detectors"] if item["id"] == DETECTOR_ID))
            duplicate["id"] = "governance.authenticated_adr_lifecycle_v1_service"
            duplicate["implementation"]["argv"] = ["python3", AUTHORITY_PACKAGE]
            data["detectors"].append(duplicate)
            detector_path.write_text(yaml.safe_dump(data), encoding="utf-8")
            self.assertTrue(detector_issues(agent))

    def test_generated_shape_without_go_runs_every_nonimplementation_check(self):
        root = self._generated_tree()
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertFalse((root / "forge-core").exists())
        forbidden = ("private-key", "signing-seed", "authority-state",
                     "lifecycle-state", "lifecycle-ledger")
        names = {path.name for path in root.rglob("*")}
        self.assertTrue(set(forbidden).isdisjoint(names))
        self.assertEqual(registry_issues(policy, policy_path), [])
        self.assertEqual(adr_issues(root), [])
        self.assertEqual(detector_issues(root / ".agent"), [])
        self.assertEqual(wiring_issues(root / ".agent"), [])
        self.assertEqual(documentation_issues(root), [])
        self.assertEqual(integration_issues(
            policy, policy_path, root, root / ".agent"), [])

    def test_shared_integration_and_source_documentation_are_exact(self):
        self.assertEqual(documentation_issues(self.repo), [])
        self.assertEqual(integration_issues(
            self.policy, self.policy_path, self.repo, self.agent), [])

    def _authority_tree(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        package = root / AUTHORITY_PACKAGE
        package.parent.mkdir(parents=True)
        shutil.copytree(self.repo / AUTHORITY_PACKAGE, package)
        return root

    def _generated_tree(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        shutil.copytree(self.agent, root / ".agent")
        for relative in (
            "harness/architecture_decision_record_v2",
            "harness/governance_contract",
            "harness/agent_engineering",
            "docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-"
            "authority-service-v1.md",
            "docs/adr/ADR-0085-authenticated-architecture-decision-lifecycle-"
            "authority-evidence-and-source-distribution.md",
        ):
            source, target = self.repo / relative, root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            if source.is_dir():
                shutil.copytree(source, target)
            else:
                shutil.copy2(source, target)
        return root


if __name__ == "__main__":
    unittest.main()
