#!/usr/bin/env python3
"""Governance regressions for ADR-0081/0082/0083 lifecycle candidate wiring."""

import copy
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from agent_engineering.contract import EXTENSION_REFS
from authenticated_adr_lifecycle_contract import SUCCESS_MARKER
from governance_engineering.authenticated_adr_lifecycle_candidate import (
    APPROVAL_AUTHORITY_DECISION_SHA256,
    CANONICAL_REFS,
    CHECKER,
    CORE_SHA256,
    DETECTOR,
    GO_AUTHORITY_EVIDENCE,
    GO_NON_CAPABILITY,
    GOVERNANCE_DECISION_BODY_SHA256,
    GOVERNANCE_DECISION_SELF_SHA256,
    GOVERNANCE_DECISION_SHA256,
    LIFECYCLE_CANDIDATE_CONTRACT,
    LIFECYCLE_NON_CAPABILITY,
    PINS,
    REFERENCE_IMPLEMENTATIONS,
    SCOPE_SHA256,
    adr_issues,
    artifact_issues,
    detector_issues,
    documentation_issues,
    integration_issues,
    registry_issues,
    wiring_issues,
)


def _source_roadmap(root):
    audit = root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
    roadmap = root / "docs/design/ai-engineering-os/implementation-roadmap.md"
    if not audit.is_file() or not roadmap.is_file():
        return None
    return roadmap.read_text(encoding="utf-8")


class AuthenticatedADRLifecycleCandidateGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v39_scope_neutral_lifecycle_candidate_and_go_evidence(self):
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(
            self.policy["authenticated_adr_approval_v1_go_authority_evidence"],
            GO_AUTHORITY_EVIDENCE,
        )
        self.assertEqual(
            self.policy["authenticated_adr_lifecycle_v1_candidate_contract"],
            LIFECYCLE_CANDIDATE_CONTRACT,
        )
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        canonical = json.dumps(self.policy["scope"], sort_keys=True,
                               separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), SCOPE_SHA256)
        flattened = [item for value in self.policy["scope"].values()
                     if isinstance(value, list) for item in value]
        self.assertFalse(any("authenticated_adr" in str(item).lower() or
                             "architecture_decision_lifecycle" in str(item).lower()
                             for item in flattened))

    def test_scope_route_authority_go_distribution_and_pin_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "authenticated_adr_lifecycle_v1")
        mutated["authenticated_adr_lifecycle_v1_candidate_contract"][
            "authority_semantics"]["adr_acceptance_rejection_or_supersession_execution"] = True
        mutated["authenticated_adr_approval_v1_go_authority_evidence"][
            "boundary"]["copies_go_contract_or_authority"] = True
        mutated["contract_pins"][
            "authenticated_adr_lifecycle_v1_schema_sha256"] = "0" * 64
        mutated["reference_implementations"][
            "authenticated_adr_approval_v1_go_authority"]["projection"] = (
                "source_distributed"
            )
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("expanded scope" in issue for issue in issues), issues)
        self.assertTrue(any("go_authority_evidence" in issue for issue in issues), issues)
        self.assertTrue(any("candidate_contract" in issue for issue in issues), issues)
        self.assertTrue(any("contract_pins" in issue for issue in issues), issues)
        self.assertTrue(any("reference_implementations" in issue for issue in issues),
                        issues)
        with tempfile.TemporaryDirectory() as raw:
            agent = Path(raw) / ".agent"
            (agent / "engineering").mkdir(parents=True)
            for name in ("activation.yml", "context-routes.yml", "disciplines.yml"):
                shutil.copy2(self.agent / "engineering" / name,
                             agent / "engineering" / name)
            route_path = agent / "engineering/context-routes.yml"
            routes = yaml.safe_load(route_path.read_text(encoding="utf-8"))
            routes["routes"][0].setdefault("include", []).append(
                {"ref": CANONICAL_REFS["authenticated_adr_lifecycle_v1_schema"]})
            route_path.write_text(yaml.safe_dump(routes), encoding="utf-8")
            self.assertTrue(any("cannot enter route" in issue
                                for issue in wiring_issues(agent)))

    def test_route_and_detector_reject_go_and_lifecycle_service_identifiers(self):
        for token in (
            "forge-core/internal/authenticatedadrapprovalcontract",
            "forge-core/internal/authenticatedadrapprovalauthority",
            "authenticated_adr_approval_v1_go_contract",
            "authenticated_adr_approval_v1_go_authority",
            "authenticated_adr_lifecycle_v1_candidate_contract",
        ):
            with self.subTest(route_token=token), tempfile.TemporaryDirectory() as raw:
                agent = Path(raw) / ".agent"
                shutil.copytree(self.agent, agent)
                path = agent / "engineering/context-routes.yml"
                data = yaml.safe_load(path.read_text(encoding="utf-8"))
                data["routes"][0].setdefault("include", []).append({"ref": token})
                path.write_text(yaml.safe_dump(data), encoding="utf-8")
                self.assertTrue(any("identifier cannot enter a route" in issue
                                    for issue in wiring_issues(agent)))
        with tempfile.TemporaryDirectory() as raw:
            agent = Path(raw) / ".agent"
            shutil.copytree(self.agent, agent)
            path = agent / "engineering/detectors.yml"
            data = yaml.safe_load(path.read_text(encoding="utf-8"))
            data["detectors"][0]["implementation"]["argv"].append(
                "forge-core/internal/authenticatedadrapprovalauthority")
            duplicate = copy.deepcopy(next(item for item in data["detectors"] if
                                      item["id"] ==
                                      "governance.authenticated_adr_lifecycle_v1_candidate"))
            duplicate["id"] = "governance.authenticated_adr_lifecycle_v1_service"
            duplicate["implementation"]["argv"] = ["python3", "lifecycle-service"]
            data["detectors"].append(duplicate)
            path.write_text(yaml.safe_dump(data), encoding="utf-8")
            issues = detector_issues(agent)
            self.assertTrue(any("Go approval authority" in issue for issue in issues),
                            issues)
            self.assertTrue(any("exactly one checker-only" in issue for issue in issues),
                            issues)

    def test_exact20_core_modes_pins_and_cli_marker_are_frozen(self):
        self.assertEqual(len(CORE_SHA256), 20)
        self.assertEqual(artifact_issues(self.repo), [])
        result = self._golden_result(self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, SUCCESS_MARKER + "\n")
        self.assertEqual(result.stderr, "")

    def test_package_physical_closure_rejects_extra_and_symlink_entries(self):
        root = self._temporary_repo()
        package = root / "harness/authenticated_adr_lifecycle_contract"
        for name in ("extra.py", "extra.txt"):
            with self.subTest(extra=name):
                extra = package / name
                extra.write_text("extra\n", encoding="utf-8")
                self.assertTrue(any("physical closure drifted" in issue
                                    for issue in artifact_issues(root)))
                extra.unlink()
        link = package / "extra-link"
        link.symlink_to("canonical.py")
        self.assertTrue(any("physical closure drifted" in issue
                            for issue in artifact_issues(root)))
        link.unlink()
        expected = package / "authority.py"
        saved = package / "authority.py.saved"
        expected.rename(saved)
        expected.symlink_to("canonical.py")
        issues = artifact_issues(root)
        self.assertTrue(any("regular 0644" in issue for issue in issues), issues)
        expected.unlink()
        saved.rename(expected)
        self.assertEqual(artifact_issues(root), [])

    def test_detector_is_one_lifecycle_checker_only_shadow_and_no_go_detector(self):
        self.assertEqual(detector_issues(self.agent), [])
        detectors = yaml.safe_load(
            (self.agent / "engineering/detectors.yml").read_text(encoding="utf-8")
        )["detectors"]
        item = next(value for value in detectors if value["id"] ==
                    "governance.authenticated_adr_lifecycle_v1_candidate")
        self.assertEqual(item["implementation"]["argv"], DETECTOR["argv"])
        self.assertFalse(item["invocation"]["load_bearing"])
        commands = "\n".join(" ".join(value["implementation"]["argv"])
                             for value in detectors)
        self.assertNotIn("authenticatedadrapprovalauthority", commands)
        self.assertNotIn("authenticatedadrapprovalcontract", commands)

    def test_activation_discipline_route_and_no_skill_boundaries_are_exact(self):
        self.assertEqual(wiring_issues(self.agent), [])
        self.assertEqual({key: EXTENSION_REFS.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)
        routes = yaml.safe_load(
            (self.agent / "engineering/context-routes.yml").read_text(encoding="utf-8")
        )
        routed = {item["ref"] for route in routes["routes"]
                  for item in route.get("include", [])}
        self.assertTrue(routed.isdisjoint(CANONICAL_REFS.values()))
        self.assertNotIn("authenticated_adr_lifecycle", " ".join(
            key for key in EXTENSION_REFS if key.endswith("_skill")))

    def test_strict_proposed_decisions_and_pins_are_frozen(self):
        self.assertEqual(adr_issues(self.repo), [])
        self.assertEqual(APPROVAL_AUTHORITY_DECISION_SHA256,
                         "e5a8742a3f49757151ade8df8637ed7fdb9f8d5af1cbbe236e18f474982336bd")
        self.assertEqual(GOVERNANCE_DECISION_BODY_SHA256,
                         "ed0b0c467118595719654928f963f18d4d740f41a369fd5fa23f61d5279ec533")
        self.assertEqual(GOVERNANCE_DECISION_SELF_SHA256,
                         "205765efff8bada13dbb28fd0fbe9f73c7ef088713b14a14598b1db56ba9ab1f")
        self.assertEqual(GOVERNANCE_DECISION_SHA256,
                         "bb79f21073d3d972f2b4493173d64056f915327dc03b9ca8f7497c2bc98e598e")

    def test_docs_close_only_proposed_candidate_evidence(self):
        self.assertEqual(documentation_issues(self.repo), [])
        roadmap = _source_roadmap(self.repo)
        if roadmap is None:
            self.skipTest("Catalyst-only roadmap is absent from generated trees")
        self.assertIn("- [x] Authenticated ADR lifecycle v1 Proposed candidate evidence",
                      roadmap)
        for marker in ("authority-bearing lifecycle promotion",
                       "authenticated Approval/revocation/usage/reservation",
                       "Accepted ADR immutable + supersede",
                       "按 `implementation_wave` 逐 package"):
            self.assertTrue(any(line.lstrip().startswith("- [ ]") and marker in line
                                for line in roadmap.splitlines()), marker)
        self.assertNotRegex(roadmap,
                            r"(?m)^\s*- \[x\].*change-intake-orchestration$")

    def test_generated_tree_without_go_authority_keys_or_state_passes(self):
        root = self._temporary_repo()
        (root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").unlink(missing_ok=True)
        (root / "docs/design/ai-engineering-os/implementation-roadmap.md").unlink(
            missing_ok=True)
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertIsNone(_source_roadmap(root))
        self.assertFalse((root / "forge-core").exists())
        forbidden = {"authority-state", "private-key", "signing-seed"}
        self.assertTrue(forbidden.isdisjoint(path.name for path in root.rglob("*")))
        self.assertEqual(registry_issues(policy, policy_path), [])
        self.assertEqual(artifact_issues(root), [])
        self.assertEqual(wiring_issues(root / ".agent"), [])
        self.assertEqual(detector_issues(root / ".agent"), [])
        self.assertEqual(adr_issues(root), [])
        result = self._golden_result(root)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, SUCCESS_MARKER + "\n")
        self.assertEqual(integration_issues(
            policy, policy_path, root, root / ".agent"), [])

    def test_shared_integration_is_exact(self):
        self.assertEqual(PINS, {key: self.policy["contract_pins"][key]
                                for key in PINS})
        self.assertEqual(REFERENCE_IMPLEMENTATIONS,
                         {key: self.policy["reference_implementations"][key]
                          for key in REFERENCE_IMPLEMENTATIONS})
        self.assertIn(GO_NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertIn(LIFECYCLE_NON_CAPABILITY, self.policy["non_capabilities"])
        self.assertEqual(integration_issues(
            self.policy, self.policy_path, self.repo, self.agent), [])

    def _golden_result(self, root):
        return subprocess.run(
            [sys.executable, "-B", str(root / CHECKER), "--golden", str(root)],
            cwd=root, env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
            text=True, capture_output=True, check=False,
        )

    def _temporary_repo(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in (
            ".agent", "harness/governance_contract",
            "harness/architecture_decision_record_v2",
            "harness/approval_record_contract",
            "harness/authenticated_adr_approval_contract",
            "harness/authenticated_adr_lifecycle_contract",
            "docs/adr", "docs/contracts", "docs/design/ai-engineering-os",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(self.repo / relative, target)
        for relative in (
            CHECKER, "harness/test_authenticated_adr_lifecycle_contract.py",
            "harness/governance_engineering/authenticated_adr_lifecycle_candidate.py",
            "harness/governance_engineering/test_authenticated_adr_lifecycle_candidate.py",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.repo / relative, target)
        audit = self.repo / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        if audit.is_file():
            shutil.copy2(audit, root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
        return root


if __name__ == "__main__":
    unittest.main()
