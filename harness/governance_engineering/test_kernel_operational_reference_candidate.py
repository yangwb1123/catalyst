#!/usr/bin/env python3
"""Regressions for ADR-0088/0089 operational reference governance."""

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

from governance_engineering.kernel_operational_reference_candidate import (
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
    NON_CAPABILITY,
    PINS,
    PYTHON_MANIFEST_SHA256,
    PYTHON_PACKAGE,
    REFERENCE_IMPLEMENTATIONS,
    RUST_MODULE_MANIFEST_SHA256,
    RUST_PACKAGE,
    RUST_REGISTRATION,
    RUST_SHA256,
    SCOPE_SHA256,
    adr_issues,
    core_artifact_issues,
    detector_issues,
    go_parity_issues,
    golden_issues,
    integration_issues,
    registry_issues,
    roadmap_issues,
    rust_parity_issues,
    wiring_issues,
)


class KernelOperationalReferenceCandidateTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v39_scope_neutral_structural_candidate(self):
        self.assertEqual(39, self.policy["version"])
        self.assertEqual(
            CONTRACT,
            self.policy["kernel_operational_reference_core_v1_candidate_contract"],
        )
        self.assertEqual([], registry_issues(self.policy, self.policy_path))
        scope = json.dumps(self.policy["scope"], sort_keys=True,
                           separators=(",", ":")).encode()
        self.assertEqual(SCOPE_SHA256, hashlib.sha256(scope).hexdigest())
        self.assertEqual(14, len(ATTESTATIONS))
        self.assertTrue(all(value is False for value in ATTESTATIONS.values()))
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_refs_pins_and_three_language_projection_split_are_exact(self):
        for owner, expected in (("canonical_refs", CANONICAL_REFS),
                                ("contract_pins", PINS),
                                ("reference_implementations", REFERENCE_IMPLEMENTATIONS)):
            self.assertEqual(expected, {key: self.policy[owner][key]
                                        for key in expected})
        self.assertEqual(64, len(PYTHON_MANIFEST_SHA256))
        self.assertEqual(64, len(CORE_MANIFEST_SHA256))
        self.assertEqual(64, len(GO_MANIFEST_SHA256))
        self.assertEqual(64, len(RUST_MODULE_MANIFEST_SHA256))
        self.assertEqual(13, len(RUST_SHA256))
        self.assertIn("source_distributed", REFERENCE_IMPLEMENTATIONS[
            "kernel_operational_reference_core_v1_python"]["projection"])
        for language in ("go", "rust"):
            self.assertIn("not_scaffolded", REFERENCE_IMPLEMENTATIONS[
                f"kernel_operational_reference_core_v1_{language}"]["projection"])

    def test_exact15_python_core_and_golden_are_frozen(self):
        self.assertEqual(15, len(CORE_SHA256))
        self.assertEqual([], core_artifact_issues(self.repo))
        self.assertEqual([], golden_issues(self.repo))

    def test_catalyst_exact11_go_parity_is_frozen(self):
        if not (self.repo / GO_PACKAGE).is_dir():
            self.skipTest("Catalyst-only Go parity package is not source-distributed")
        self.assertEqual([], go_parity_issues(self.repo))

    def test_catalyst_exact13_rust_module_and_registration_are_frozen(self):
        if not (self.repo / RUST_PACKAGE).is_dir():
            self.skipTest("Catalyst-only Rust parity package is not source-distributed")
        self.assertEqual([], rust_parity_issues(self.repo))

    def test_strict_proposed_decisions_are_pinned(self):
        self.assertEqual([], adr_issues(self.repo))
        self.assertEqual([], roadmap_issues(self.repo))
        if not (self.repo / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").is_file():
            return
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        for relative in (
                "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md",
                "docs/design/ai-engineering-os/implementation-roadmap.md"):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.repo / relative, target)
        roadmap = root / "docs/design/ai-engineering-os/implementation-roadmap.md"
        roadmap.write_text(roadmap.read_text(encoding="utf-8").replace(
            "- [x] 冻结 Kernel structural reference-family ABI",
            "- [ ] 冻结 Kernel structural reference-family ABI"), encoding="utf-8")
        self.assertTrue(any("remain one exact completed item" in issue
                            for issue in roadmap_issues(root)))

    def test_pinned_golden_checker_detector_is_honest_and_unique(self):
        self.assertEqual([], detector_issues(self.agent))
        expected = ["python3", CHECKER, "--golden", "."]
        self.assertEqual(expected, DETECTOR["argv"])
        self.assertNotIn("repo_root", DETECTOR["argv"])
        result = subprocess.run(
            [sys.executable, str(self.repo / CHECKER), "--golden", "."],
            cwd=self.repo, capture_output=True, check=False,
        )
        self.assertEqual(0, result.returncode, result.stderr.decode())
        self.assertEqual((CONTRACT["positive_result"] + "\n").encode(), result.stdout)
        rejected = subprocess.run(
            [sys.executable, str(self.repo / CHECKER)],
            cwd=self.repo, capture_output=True, check=False,
        )
        self.assertEqual(2, rejected.returncode)
        self.assertEqual(b"", rejected.stdout)

    def test_scope_argv_authority_and_distribution_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "kernel_operational_reference_core_v1")
        block = mutated["kernel_operational_reference_core_v1_candidate_contract"]
        block["attestations"]["execution_attestation"] = True
        block["source_distribution"]["copies_go_rust_or_runtime_registration"] = True
        mutated["canonical_refs"][next(iter(CANONICAL_REFS))] = "drift"
        mutated["contract_pins"][next(iter(PINS))] = "0" * 64
        mutated["reference_implementations"][
            "kernel_operational_reference_core_v1_go"]["projection"] = "universal"
        mutated["non_capabilities"].remove(NON_CAPABILITY)
        issues = registry_issues(mutated, self.policy_path)
        for marker in ("scope", "contract", "canonical_refs", "contract_pins",
                       "reference_implementations", "non-capability"):
            self.assertTrue(any(marker in issue for issue in issues), (marker, issues))

    def test_detector_file_mode_promotion_and_duplicate_use_fail_closed(self):
        root = self._agent_tree()
        path = root / "engineering/detectors.yml"
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
        detector = next(item for item in data["detectors"]
                        if item["id"] == DETECTOR_ID)
        detector["implementation"]["argv"] = ["python3", CHECKER, "--file", "request"]
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

    def test_generated_shape_needs_no_go_or_rust_and_integration_is_exact(self):
        root = self._generated_tree()
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertFalse((root / GO_PACKAGE).exists())
        self.assertFalse((root / RUST_PACKAGE).exists())
        self.assertEqual([], integration_issues(
            policy, policy_path, root, root / ".agent"))
        self.assertEqual([], integration_issues(
            self.policy, self.policy_path, self.repo, self.agent))

    def test_physical_package_and_parity_drift_fail_closed(self):
        root = self._generated_tree()
        package = root / PYTHON_PACKAGE
        shutil.rmtree(package)
        package.symlink_to(self.repo / PYTHON_PACKAGE, target_is_directory=True)
        self.assertTrue(any("package" in issue for issue in core_artifact_issues(root)))
        for package_name, checker, filename in (
                (GO_PACKAGE, go_parity_issues, "records.go"),
                (RUST_PACKAGE, rust_parity_issues, "records.rs")):
            if not (self.repo / package_name).is_dir():
                continue
            with self.subTest(package=package_name):
                root, target = self._parity_tree(package_name)
                path = target / filename
                path.write_bytes(path.read_bytes() + b" ")
                self.assertTrue(any("physical pin" in issue for issue in checker(root)))
            for residue in ("extra.file", "extra-dir"):
                with self.subTest(package=package_name, residue=residue):
                    root, target = self._parity_tree(package_name)
                    extra = target / residue
                    extra.mkdir() if residue.endswith("dir") else extra.write_text("x")
                    self.assertTrue(any("lexical closure" in issue
                                        for issue in checker(root)))
            with self.subTest(package=package_name, mutation="package-symlink"):
                root, target = self._parity_tree(package_name)
                shutil.rmtree(target)
                target.symlink_to(self.repo / package_name, target_is_directory=True)
                self.assertTrue(any("real directory" in issue for issue in checker(root)))
            with self.subTest(package=package_name, mutation="required-missing"):
                root = self._generated_tree()
                self._add_catalyst_sentinel(root)
                self.assertTrue(any("required" in issue for issue in checker(root)))
            with self.subTest(package=package_name, mutation="dangling-symlink"):
                root = self._generated_tree()
                self._add_catalyst_sentinel(root)
                target = root / package_name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.symlink_to(root / "missing-package", target_is_directory=True)
                self.assertTrue(any("real directory" in issue for issue in checker(root)))
        if (self.repo / RUST_PACKAGE).is_dir():
            for mutation in ("missing", "duplicate"):
                with self.subTest(package=RUST_PACKAGE, registration=mutation):
                    root, _ = self._parity_tree(RUST_PACKAGE)
                    registration, line = root / RUST_REGISTRATION, "pub mod kernel_operational_contract;"
                    text = registration.read_text(encoding="utf-8")
                    changed = text.replace(line, "", 1) if mutation == "missing" else (
                        text + line + "\n")
                    registration.write_text(changed, encoding="utf-8")
                    self.assertTrue(any("registration" in issue
                                        for issue in rust_parity_issues(root)))

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

    def _parity_tree(self, package_name):
        root = self._generated_tree()
        target = root / package_name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(self.repo / package_name, target)
        if package_name == RUST_PACKAGE:
            relative = Path("forge-runtime/crates/domain/src/lib.rs")
            registration = root / relative
            registration.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.repo / relative, registration)
        return root, target

    def _add_catalyst_sentinel(self, root):
        relative = Path("docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md")
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.repo / relative, target)


if __name__ == "__main__":
    unittest.main()
