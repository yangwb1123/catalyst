#!/usr/bin/env python3
"""Regressions for ADR-0086/0087 legacy read-import candidate governance."""

import copy
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

HARNESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS_ROOT))

from governance_engineering.legacy_governance_read_import_candidate import (
    ATTESTATIONS,
    CANONICAL_REFS,
    CHECKER,
    CONTRACT,
    CORE_MANIFEST_SHA256,
    CORE_SHA256,
    DETECTOR,
    DETECTOR_ID,
    GO_MANIFEST_SHA256,
    GO_PACKAGE,
    GOVERNANCE_DECISION,
    LEGACY_POLICY,
    NON_CAPABILITY,
    PINS,
    PYTHON_PACKAGE,
    REFERENCE_IMPLEMENTATIONS,
    SCOPE_SHA256,
    adr_issues,
    core_artifact_issues,
    detector_issues,
    documentation_issues,
    go_parity_issues,
    golden_issues,
    integration_issues,
    registry_issues,
    wiring_issues,
)


class LegacyGovernanceReadImportCandidateTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v39_scope_neutral_unverified_candidate(self):
        self.assertEqual(39, self.policy["version"])
        self.assertEqual(LEGACY_POLICY, self.policy["legacy"])
        self.assertEqual(
            CONTRACT,
            self.policy["legacy_governance_read_import_v1_candidate_contract"],
        )
        self.assertEqual([], registry_issues(self.policy, self.policy_path))
        scope = json.dumps(self.policy["scope"], sort_keys=True,
                           separators=(",", ":")).encode()
        self.assertEqual(SCOPE_SHA256, hashlib.sha256(scope).hexdigest())
        self.assertEqual(ATTESTATIONS, CONTRACT["attestations"])
        self.assertEqual(13, len(ATTESTATIONS))
        self.assertTrue(all(value is False for value in ATTESTATIONS.values()))
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_refs_pins_and_python_go_projection_split_are_exact(self):
        for owner, expected in (("canonical_refs", CANONICAL_REFS),
                                ("contract_pins", PINS),
                                ("reference_implementations", REFERENCE_IMPLEMENTATIONS)):
            self.assertEqual(expected, {key: self.policy[owner][key]
                                        for key in expected})
        self.assertEqual(
            "source_distributed_dependency_free_strict_pure_core",
            REFERENCE_IMPLEMENTATIONS["legacy_governance_read_import_v1_python"]
            ["projection"],
        )
        self.assertEqual(
            "catalyst_repository_only_cross_language_parity_not_scaffolded",
            REFERENCE_IMPLEMENTATIONS["legacy_governance_read_import_v1_go"]
            ["projection"],
        )
        self.assertEqual(64, len(CORE_MANIFEST_SHA256))
        self.assertEqual(64, len(GO_MANIFEST_SHA256))

    def test_exact15_python_core_and_golden_are_frozen(self):
        self.assertEqual(15, len(CORE_SHA256))
        self.assertEqual([], core_artifact_issues(self.repo))
        self.assertEqual([], golden_issues(self.repo))

    def test_shared_integration_rejects_python_package_directory_symlink(self):
        root = self._generated_tree()
        package = root / PYTHON_PACKAGE
        shutil.rmtree(package)
        package.symlink_to(self.repo / PYTHON_PACKAGE, target_is_directory=True)
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        issues = integration_issues(policy, policy_path, root, root / ".agent")
        self.assertTrue(any("exact package must be a real directory" in issue
                            for issue in issues), issues)

    def test_catalyst_exact10_go_parity_is_frozen(self):
        if not (self.repo / GO_PACKAGE).is_dir():
            self.skipTest("Catalyst-only Go parity package is not source-distributed")
        self.assertEqual([], go_parity_issues(self.repo))
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        target = root / GO_PACKAGE
        shutil.copytree(self.repo / GO_PACKAGE, target)
        request = target / "request.go"
        text = request.read_text(encoding="utf-8")
        marker = 'selfDigest(requestDomain, request, "request_sha256",'
        self.assertIn(marker, text)
        request.write_text(text.replace(marker, "selfDigest(requestDomain, request, "
                                        '"request_digest",'), encoding="utf-8")
        issues = go_parity_issues(root)
        self.assertTrue(any("semantic marker" in issue for issue in issues), issues)

    def test_strict_proposed_decisions_and_documentation_are_pinned(self):
        self.assertEqual([], adr_issues(self.repo))
        self.assertEqual([], documentation_issues(self.repo))

    def test_zero_argument_stdin_checker_detector_is_honest_and_unique(self):
        self.assertEqual([], detector_issues(self.agent))
        self.assertEqual(["python3", CHECKER], DETECTOR["argv"])
        self.assertNotIn("--golden", DETECTOR["argv"])
        self.assertNotIn("repo_root", DETECTOR["argv"])
        request = (self.repo / CANONICAL_REFS[
            "legacy_governance_read_import_v1_request_fixture"]).read_bytes()
        result = subprocess.run(
            [sys.executable, str(self.repo / CHECKER)], input=request,
            capture_output=True, check=False,
        )
        self.assertEqual(0, result.returncode, result.stderr.decode())
        rejected = subprocess.run(
            [sys.executable, str(self.repo / CHECKER), "repo_root"],
            input=request, capture_output=True, check=False,
        )
        self.assertEqual(2, rejected.returncode)
        self.assertEqual(b"", rejected.stdout)

    def test_scope_argv_authority_and_distribution_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "legacy_governance_read_import_v1")
        block = mutated["legacy_governance_read_import_v1_candidate_contract"]
        block["attestations"]["truth"] = True
        block["source_distribution"]["copies_go_parity_package_or_runtime"] = True
        mutated["legacy"]["legacy_status_is_authority"] = True
        mutated["canonical_refs"][next(iter(CANONICAL_REFS))] = "drift"
        mutated["contract_pins"][next(iter(PINS))] = "0" * 64
        mutated["reference_implementations"][
            "legacy_governance_read_import_v1_go"]["projection"] = "universal"
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        for marker in ("scope", "contract", "legacy import", "canonical_refs",
                       "contract_pins", "reference_implementations", "non-capability"):
            self.assertTrue(any(marker in issue for issue in issues), (marker, issues))

    def test_detector_pseudo_argv_promotion_and_duplicate_use_fail_closed(self):
        root = self._agent_tree()
        path = root / "engineering/detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == DETECTOR_ID)
        detector["implementation"]["argv"].extend(["--golden", "repo_root"])
        detector["invocation"]["load_bearing"] = True
        duplicate = copy.deepcopy(detector)
        duplicate["id"] = f"{DETECTOR_ID}.duplicate"
        data["detectors"].append(duplicate)
        path.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        issues = detector_issues(root)
        self.assertTrue(any("argv" in issue for issue in issues), issues)
        self.assertTrue(any("shadow" in issue for issue in issues), issues)
        self.assertTrue(any("exactly one" in issue for issue in issues), issues)

    def test_activation_discipline_and_route_drift_fail_closed(self):
        root = self._agent_tree()
        activation = root / "engineering/activation.yml"
        data = yaml.safe_load(activation.read_text(encoding="utf-8"))
        data["canonical_extension_refs"].pop(next(iter(CANONICAL_REFS)))
        activation.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(wiring_issues(root))
        root = self._agent_tree()
        discipline = root / "engineering/disciplines.yml"
        data = yaml.safe_load(discipline.read_text(encoding="utf-8"))
        contract = next(item for item in data["disciplines"]
                        if item["id"] == "contract")
        contract["assets"].remove(next(iter(CANONICAL_REFS.values())))
        discipline.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(wiring_issues(root))
        root = self._agent_tree()
        routes = root / "engineering/context-routes.yml"
        data = yaml.safe_load(routes.read_text(encoding="utf-8"))
        data["routes"][0].setdefault("include", []).append(
            {"ref": next(iter(CANONICAL_REFS.values())), "required": False})
        routes.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("route" in issue for issue in wiring_issues(root)))

    def test_generated_shape_needs_no_go_and_shared_integration_is_exact(self):
        root = self._generated_tree()
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertFalse((root / GO_PACKAGE).exists())
        self.assertEqual([], integration_issues(
            policy, policy_path, root, root / ".agent"))
        self.assertEqual([], integration_issues(
            self.policy, self.policy_path, self.repo, self.agent))

    def test_shared_integration_rejects_catalyst_go_physical_drift(self):
        if not (self.repo / GO_PACKAGE).exists():
            self.assertFalse((self.repo / GO_PACKAGE).is_symlink())
            return
        expected = {
            "byte": "request.go: physical pin drifted",
            "extra": "exact ten-file Go closure drifted",
            "mode": "must be regular 0644 with link count one",
            "symlink": "must be a real directory",
        }
        for mutation, marker in expected.items():
            with self.subTest(mutation=mutation):
                root = self._generated_tree()
                package = root / GO_PACKAGE
                package.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(self.repo / GO_PACKAGE, package)
                if mutation == "byte":
                    path = package / "request.go"
                    path.write_bytes(path.read_bytes() + b" ")
                elif mutation == "extra":
                    (package / "extra.go").write_text("package drift\n", encoding="utf-8")
                elif mutation == "mode":
                    (package / "constants.go").chmod(0o600)
                else:
                    shutil.rmtree(package)
                    package.symlink_to(self.repo / GO_PACKAGE, target_is_directory=True)
                policy_path = root / ".agent/engineering/governance-contracts.yml"
                policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
                issues = integration_issues(
                    policy, policy_path, root, root / ".agent")
                self.assertTrue(any(marker in issue for issue in issues), issues)

    def _agent_tree(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name) / ".agent"
        shutil.copytree(self.agent, root)
        return root

    def _generated_tree(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        shutil.copytree(self.agent, root / ".agent")
        for relative in (*CORE_SHA256, GOVERNANCE_DECISION):
            source, target = self.repo / relative, root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        return root


if __name__ == "__main__":
    unittest.main()
