#!/usr/bin/env python3
"""Governance regressions for ADR-0079/0080 structural prerequisite wiring."""

import copy
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
from authenticated_adr_approval_contract import SUCCESS_MARKER
from governance_engineering.authenticated_adr_approval_candidate import (
    AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE_CONTRACT,
    CANONICAL_REFS,
    CHECKER,
    DETECTOR,
    GOVERNANCE_DECISION_BODY_SHA256,
    GOVERNANCE_DECISION_SELF_SHA256,
    GOVERNANCE_DECISION_SHA256,
    NON_CAPABILITY,
    REFERENCE_IMPLEMENTATIONS,
    SCHEMA,
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


class AuthenticatedADRApprovalCandidateGovernanceTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v39_scope_neutral_structural_candidate_only(self):
        self.assertEqual(self.policy["version"], 39)
        self.assertEqual(
            self.policy["authenticated_adr_approval_v1_candidate_contract"],
            AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE_CONTRACT,
        )
        self.assertEqual(registry_issues(self.policy, self.policy_path), [])
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])
        flattened = [item for value in self.policy["scope"].values()
                     if isinstance(value, list) for item in value]
        self.assertFalse(any("authenticated_adr" in str(item).lower() or
                             "architecture_decision_approval" in str(item).lower()
                             for item in flattened))

    def test_scope_route_authority_and_pin_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append("authenticated_adr_approval_v1")
        mutated["authenticated_adr_approval_v1_candidate_contract"][
            "authority_semantics"]["authorization"] = True
        mutated["contract_pins"][
            "authenticated_adr_approval_v1_schema_sha256"] = "0" * 64
        mutated["reference_implementations"][
            "authenticated_adr_approval_v1_go_authority"]["projection"] = (
                "source_distributed"
            )
        issues = registry_issues(mutated, self.policy_path)
        self.assertTrue(any("runtime scope" in issue for issue in issues), issues)
        self.assertTrue(any("candidate contract" in issue for issue in issues), issues)
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
            routes["routes"][0].setdefault("include", []).append({"ref": SCHEMA})
            route_path.write_text(yaml.safe_dump(routes), encoding="utf-8")
            self.assertTrue(any("cannot enter context route" in issue
                                for issue in wiring_issues(agent)))

    def test_schema_golden_proposal_and_cli_marker_are_frozen(self):
        self.assertEqual(artifact_issues(self.repo), [])
        result = self._golden_result(self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, SUCCESS_MARKER + "\n")
        self.assertEqual(result.stderr, "")

    def test_detector_is_one_checker_only_shadow(self):
        self.assertEqual(detector_issues(self.agent), [])
        detectors = yaml.safe_load(
            (self.agent / "engineering/detectors.yml").read_text(encoding="utf-8")
        )["detectors"]
        item = next(value for value in detectors if value["id"] ==
                    "governance.authenticated_adr_approval_v1_candidate")
        self.assertEqual(item["implementation"]["argv"], DETECTOR["argv"])
        self.assertFalse(item["invocation"]["load_bearing"])

    def test_activation_contract_discipline_and_route_absence(self):
        self.assertEqual(wiring_issues(self.agent), [])
        self.assertEqual({key: EXTENSION_REFS.get(key) for key in CANONICAL_REFS},
                         CANONICAL_REFS)
        routes = yaml.safe_load(
            (self.agent / "engineering/context-routes.yml").read_text(encoding="utf-8")
        )
        routed = {item["ref"] for route in routes["routes"]
                  for item in route.get("include", [])}
        self.assertTrue(routed.isdisjoint(CANONICAL_REFS.values()))

    def test_strict_proposed_decisions_and_pins_are_frozen(self):
        self.assertEqual(adr_issues(self.repo), [])
        self.assertEqual(GOVERNANCE_DECISION_BODY_SHA256,
                         "b7c87f0291cb58dc27098e7a918abfaffa4bd23b1b6290e82b2a5477c4e39b13")
        self.assertEqual(GOVERNANCE_DECISION_SELF_SHA256,
                         "8db896364d84ccd0f813d81bb816f120802b30d142f582755bac5b105fe292e3")
        self.assertEqual(GOVERNANCE_DECISION_SHA256,
                         "d91548ab698db4137a7f96bf78070c45303e2201e017a2094c7b6ee48aac6563")

    def test_docs_close_only_wave_zero_structural_prerequisite_evidence(self):
        self.assertEqual(documentation_issues(self.repo), [])
        roadmap = _source_roadmap(self.repo)
        if roadmap is None:
            self.skipTest("Catalyst-only roadmap is absent from generated trees")
        self.assertIn(
            "- [x] Authenticated ADR approval v1 Proposed structural prerequisite evidence",
            roadmap,
        )
        for marker in ("authority-bearing lifecycle promotion",
                       "authenticated Approval/revocation/usage/reservation",
                       "Accepted ADR immutable + supersede",
                       "按 `implementation_wave` 逐 package"):
            self.assertTrue(any(line.lstrip().startswith("- [ ]") and marker in line
                                for line in roadmap.splitlines()), marker)
        self.assertNotRegex(roadmap, r"(?m)^\s*- \[x\].*change-intake-orchestration")

    def test_generated_tree_without_source_docs_keeps_core_checks(self):
        root = self._temporary_repo()
        (root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").unlink(missing_ok=True)
        (root / "docs/design/ai-engineering-os/implementation-roadmap.md").unlink(
            missing_ok=True)
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertIsNone(_source_roadmap(root))
        self.assertEqual(documentation_issues(root), [])
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
        self.assertEqual(REFERENCE_IMPLEMENTATIONS,
                         {key: self.policy["reference_implementations"][key]
                          for key in REFERENCE_IMPLEMENTATIONS})
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
            "docs/adr", "docs/contracts", "docs/design/ai-engineering-os",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(self.repo / relative, target)
        for relative in (
            CHECKER,
            "harness/governance_engineering/authenticated_adr_approval_candidate.py",
            "harness/governance_engineering/test_authenticated_adr_approval_candidate.py",
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
