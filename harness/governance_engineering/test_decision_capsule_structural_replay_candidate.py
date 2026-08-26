#!/usr/bin/env python3
"""Regressions for ADR-0092/0093 structural replay governance."""

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

from engineering_check_support import read_regular_file

from governance_engineering.decision_capsule_structural_replay_candidate import (
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
    GOVERNANCE_MODULE,
    NON_CAPABILITY,
    PINS,
    PYTHON_MANIFEST_SHA256,
    PYTHON_PACKAGE,
    REFERENCE_IMPLEMENTATIONS,
    RUNTIME_REFERENCE_ALLOWED_COUNTS,
    RUST_MODULE_MANIFEST_SHA256,
    RUST_PACKAGE,
    RUST_REGISTRATION,
    RUST_SHA256,
    SCAFFOLD_OWNERS,
    SCOPE_SHA256,
    adr_issues,
    core_artifact_issues,
    detector_issues,
    go_parity_issues,
    golden_issues,
    integration_issues,
    registry_issues,
    repository_flavor_issues,
    roadmap_issues,
    runtime_reference_issues,
    rust_parity_issues,
    wiring_issues,
)


class DecisionCapsuleStructuralReplayCandidateTest(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[2]
        self.agent = self.repo / ".agent"
        self.policy_path = self.agent / "engineering/governance-contracts.yml"
        self.policy = yaml.safe_load(self.policy_path.read_text(encoding="utf-8"))

    def test_registry_is_v39_scope_neutral_structural_candidate(self):
        self.assertEqual(39, self.policy["version"])
        self.assertEqual(CONTRACT, self.policy[
            "decision_capsule_structural_replay_core_v1_candidate_contract"])
        self.assertEqual([], registry_issues(self.policy, self.policy_path))
        scope = json.dumps(self.policy["scope"], sort_keys=True,
                           separators=(",", ":")).encode()
        self.assertEqual(SCOPE_SHA256, hashlib.sha256(scope).hexdigest())
        self.assertEqual(32, len(ATTESTATIONS))
        self.assertTrue(all(value is False for value in ATTESTATIONS.values()))
        narrow_completion = (
            "decision_capsule_structural_replay_repository_slice_complete")
        self.assertTrue(CONTRACT["completion"][narrow_completion])
        self.assertTrue(all(value is False for key, value in
                            CONTRACT["completion"].items()
                            if key != narrow_completion))
        self.assertIn(NON_CAPABILITY, self.policy["non_capabilities"])

    def test_refs_pins_and_three_language_projection_split_are_exact(self):
        for owner, expected in (("canonical_refs", CANONICAL_REFS),
                                ("contract_pins", PINS),
                                ("reference_implementations", REFERENCE_IMPLEMENTATIONS)):
            self.assertEqual(expected, {key: self.policy[owner][key]
                                        for key in expected})
        for digest in (PYTHON_MANIFEST_SHA256, CORE_MANIFEST_SHA256,
                       GO_MANIFEST_SHA256, RUST_MODULE_MANIFEST_SHA256):
            self.assertEqual(64, len(digest))
        self.assertEqual(14, len(RUST_SHA256))
        self.assertIn("source_distributed", REFERENCE_IMPLEMENTATIONS[
            "decision_capsule_structural_replay_core_v1_python"]["projection"])
        for language in ("go", "rust"):
            self.assertIn("not_scaffolded", REFERENCE_IMPLEMENTATIONS[
                f"decision_capsule_structural_replay_core_v1_{language}"]["projection"])

    def test_exact16_python_core_and_golden_are_frozen(self):
        self.assertEqual(16, len(CORE_SHA256))
        self.assertEqual(13, len(CORE_SHA256) - 3)
        self.assertEqual([], core_artifact_issues(self.repo))
        self.assertEqual([], golden_issues(self.repo))

    def test_catalyst_exact15_go_parity_is_frozen(self):
        if not (self.repo / GO_PACKAGE).is_dir():
            self.skipTest("Catalyst-only Go parity package is not source-distributed")
        self.assertEqual([], go_parity_issues(self.repo))

    def test_catalyst_exact14_rust_and_registration_are_frozen(self):
        if not (self.repo / RUST_PACKAGE).is_dir():
            self.skipTest("Catalyst-only Rust parity package is not source-distributed")
        self.assertEqual([], rust_parity_issues(self.repo))

    def test_strict_proposed_decisions_and_completed_roadmap_are_pinned(self):
        self.assertEqual([], adr_issues(self.repo))
        self.assertEqual([], roadmap_issues(self.repo))
        facts = self.repo / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        if not facts.is_file():
            return
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        project = root / ".agent/project.yml"
        project.parent.mkdir(parents=True, exist_ok=True)
        project.write_text("repository_flavor: catalyst_source\n", encoding="utf-8")
        project.chmod(0o644)
        for relative in ("docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md",
                         "docs/design/ai-engineering-os/implementation-roadmap.md"):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.repo / relative, target)
        roadmap = root / "docs/design/ai-engineering-os/implementation-roadmap.md"
        roadmap.write_text(roadmap.read_text(encoding="utf-8").replace(
            "- [x] 交付 Decision Capsule structural replay repository slice",
            "- [ ] 交付 Decision Capsule structural replay repository slice"),
            encoding="utf-8")
        self.assertTrue(any("must remain one exact completed item" in issue
                            for issue in roadmap_issues(root)))

    def test_pinned_golden_checker_detector_is_honest_and_unique(self):
        self.assertEqual([], detector_issues(self.agent))
        expected = ["python3", CHECKER, "--golden", "."]
        self.assertEqual(expected, DETECTOR["argv"])
        environment = dict(os.environ)
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        result = subprocess.run(
            [sys.executable, "-B", str(self.repo / CHECKER), "--golden", "."],
            cwd=self.repo, env=environment, capture_output=True, check=False,
        )
        self.assertEqual(0, result.returncode, result.stderr.decode())
        self.assertEqual((CONTRACT["positive_result"] + "\n").encode(), result.stdout)
        rejected = subprocess.run(
            [sys.executable, "-B", str(self.repo / CHECKER)],
            cwd=self.repo, env=environment, capture_output=True, check=False,
        )
        self.assertEqual(2, rejected.returncode)
        self.assertEqual(b"", rejected.stdout)

    def test_scope_argv_authority_completion_and_distribution_drift_fail_closed(self):
        mutated = copy.deepcopy(self.policy)
        mutated["scope"]["shipped_evaluators"].append(
            "decision_capsule_structural_replay_core_v1")
        block = mutated["decision_capsule_structural_replay_core_v1_candidate_contract"]
        block["attestations"]["authority_attestation"] = True
        block["completion"][
            "decision_capsule_structural_replay_repository_slice_complete"] = False
        block["source_distribution"]["copies_go_rust_or_runtime_registration"] = True
        mutated["canonical_refs"][next(iter(CANONICAL_REFS))] = "drift"
        mutated["contract_pins"][next(iter(PINS))] = "0" * 64
        mutated["reference_implementations"][
            "decision_capsule_structural_replay_core_v1_go"]["projection"] = "universal"
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

    def test_activation_discipline_route_and_skill_drift_fail_closed(self):
        root = self._agent_tree()
        activation = root / "engineering/activation.yml"
        data = yaml.safe_load(activation.read_text(encoding="utf-8"))
        data["canonical_extension_refs"].pop(next(iter(CANONICAL_REFS)))
        activation.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(wiring_issues(root))
        root = self._agent_tree()
        discipline = root / "engineering/disciplines.yml"
        data = yaml.safe_load(discipline.read_text(encoding="utf-8"))
        contract = next(item for item in data["disciplines"] if item["id"] == "contract")
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
        for relative in (".agent/skills/capsule-replay.md",
                         "skills/capsule-replay/SKILL.md"):
            with self.subTest(relative=relative):
                root = self._agent_tree()
                path = root.parent / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# Capsule replay\nUse Decision Capsule structural replay.\n",
                                encoding="utf-8")
                path.chmod(0o644)
                self.assertTrue(any("cannot install a Skill" in issue
                                    for issue in wiring_issues(root)))
        root = self._agent_tree()
        routes = root / "engineering/context-routes.yml"
        data = yaml.safe_load(routes.read_text(encoding="utf-8"))
        data["routes"][0].setdefault("include", []).append(
            {"ref": f"{PYTHON_PACKAGE}/manifest.py", "lane": "instruction",
             "required": False, "max_bytes": 65_536})
        routes.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(any("route" in issue for issue in wiring_issues(root)))

    def test_generated_shape_needs_no_go_or_rust_and_integration_is_exact(self):
        root = self._generated_tree()
        policy_path = root / ".agent/engineering/governance-contracts.yml"
        policy = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
        self.assertFalse((root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md").exists())
        self.assertEqual([], roadmap_issues(root))
        self.assertFalse((root / GO_PACKAGE).exists())
        self.assertFalse((root / RUST_PACKAGE).exists())
        audit = root / "docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md"
        audit.parent.mkdir(parents=True, exist_ok=True)
        audit.write_text(
            "# Ordinary project audit\n\nThis file is not a repository identity credential.\n",
            encoding="utf-8",
        )
        state = root / ".agent/scaffold-state.json"
        for mode in (0o600, 0o640, 0o644):
            state.chmod(mode)
            self.assertEqual([], repository_flavor_issues(root))
        for mode in (0o664, 0o646, 0o400):
            state.chmod(mode)
            self.assertTrue(repository_flavor_issues(root))
        alias = root / ".agent/scaffold-state-alias.json"
        state.rename(alias)
        os.link(alias, state)
        self.assertTrue(repository_flavor_issues(root))
        state.unlink()
        alias.rename(state)
        state.chmod(0o644)
        self.assertEqual([], go_parity_issues(root))
        self.assertEqual([], rust_parity_issues(root))
        for package, checker in ((GO_PACKAGE, go_parity_issues),
                                 (RUST_PACKAGE, rust_parity_issues)):
            parity_root, _ = self._parity_tree(package)
            self.assertTrue(any("scaffold cannot" in issue
                                for issue in checker(parity_root)))
        registration_root = self._generated_tree()
        registration = registration_root / RUST_REGISTRATION
        registration.parent.mkdir(parents=True, exist_ok=True)
        registration.write_text("pub mod decision_capsule_contract;\n", encoding="utf-8")
        registration.chmod(0o644)
        self.assertTrue(any("scaffold cannot register" in issue
                            for issue in rust_parity_issues(registration_root)))
        self.assertEqual([], integration_issues(
            policy, policy_path, root, root / ".agent"))
        self.assertEqual([], integration_issues(
            self.policy, self.policy_path, self.repo, self.agent))

    def test_repository_flavor_authorities_are_explicit_and_physical(self):
        root = self._generated_tree()
        project = root / ".agent/project.yml"
        data = yaml.safe_load(project.read_text(encoding="utf-8"))
        data.pop("repository_flavor")
        project.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        self.assertTrue(repository_flavor_issues(root))
        for kind in ("fifo", "symlink", "hardlink", "directory", "dangling"):
            with self.subTest(kind=kind):
                root = self._generated_tree()
                project = root / ".agent/project.yml"
                if kind in ("symlink", "hardlink"):
                    target = project.with_name(f"project-{kind}.yml")
                    project.rename(target)
                    (project.symlink_to(target.name) if kind == "symlink"
                     else os.link(target, project))
                else:
                    project.unlink()
                    if kind == "fifo": os.mkfifo(project, 0o644)
                    elif kind == "directory": project.mkdir()
                    else: project.symlink_to("missing-project.yml")
                self.assertTrue(repository_flavor_issues(root))
        root = self._generated_tree()
        project = root / ".agent/project.yml"
        data = yaml.safe_load(project.read_text(encoding="utf-8"))
        data["repository_flavor"] = "catalyst_source"
        project.write_text(yaml.safe_dump(data, sort_keys=False), encoding="utf-8")
        state = root / ".agent/scaffold-state.json"
        state.unlink(); state.symlink_to("missing-state.json")
        self.assertTrue(repository_flavor_issues(root))

    def test_descriptor_reader_rejects_leaf_and_parent_replacement_races(self):
        for parent_swap in (False, True):
            with self.subTest(parent_swap=parent_swap), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); parent = root / "live"; parent.mkdir()
                path = parent / "consumer.py"; path.write_bytes(b"safe\n")
                outside = root / "outside"; outside.mkdir()
                (outside / path.name).write_bytes(b"outside\n")
                def swap():
                    if parent_swap:
                        parent.rename(root / "held"); parent.symlink_to(outside, True)
                    else:
                        path.rename(parent / "held.py"); path.write_bytes(b"replacement\n")
                with self.assertRaises(OSError):
                    read_regular_file(path, str(path), modes=(0o644,), after_open=swap)

    def test_unowned_runtime_consumers_in_every_scanned_language_fail_closed(self):
        cases = (
            ("harness/unowned_consumer.py",
             "from decision_capsule_contract import load_golden\n"),
            ("forge-core/internal/unowned/consumer.go",
             "package unowned\n\nimport \"forgeos/forge-core/internal/decisioncapsulecontract\"\n"),
            ("forge-runtime/crates/domain/src/unowned.rs",
             "use crate::decision_capsule_contract::DecisionCapsule;\n"),
            *((f"runtime/unowned_consumer.{suffix}",
               "import contract from 'decision_capsule_contract';\n")
              for suffix in ("cjs", "cts", "js", "jsx", "mjs", "mts", "ts", "tsx")),
        )
        for relative, source in cases:
            with self.subTest(relative=relative):
                root = self._generated_tree()
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(source, encoding="utf-8")
                path.chmod(0o644)
                issues = runtime_reference_issues(root)
                self.assertTrue(any(relative in issue and "unauthorized" in issue
                                    for issue in issues), issues)

    def test_nested_output_directory_names_cannot_bypass_runtime_scan(self):
        names = (
            ".forge", ".git", ".hg", ".mypy_cache", ".pytest_cache",
            ".ruff_cache", ".svn", ".tox", ".venv", "__pycache__", "build",
            "coverage", "dist", "node_modules", "target", "venv",
        )
        for name in names:
            with self.subTest(name=name):
                root = self._generated_tree()
                relative = f"runtime/{name}/unowned_consumer.py"
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(
                    "from decision_capsule_contract import load_golden\n",
                    encoding="utf-8")
                path.chmod(0o644)
                issues = runtime_reference_issues(root)
                self.assertTrue(any(relative in issue and "unauthorized" in issue
                                    for issue in issues), issues)

    def test_required_runtime_allowance_paths_cannot_disappear(self):
        root = self._generated_tree()
        missing = GOVERNANCE_MODULE
        (root / missing).unlink()
        issues = runtime_reference_issues(root)
        self.assertTrue(any(missing in issue and "required" in issue
                            and "missing" in issue for issue in issues), issues)

    def test_source_suffixed_fifo_is_rejected_without_blocking(self):
        root = self._generated_tree()
        relative = "runtime/blocked.py"
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        os.mkfifo(path, 0o644)
        issues = runtime_reference_issues(root)
        self.assertTrue(any(relative in issue and "unreadable" in issue
                            for issue in issues), issues)

    def test_scaffold_identity_requires_each_exact19_owner_once(self):
        root = self._generated_tree()
        state = root / ".agent/scaffold-state.json"
        owners = sorted(SCAFFOLD_OWNERS)
        cases = (
            {"version": 1, "copied": []},
            {"version": 1, "copied": owners[:-1]},
            {"version": 1, "copied": [*owners, owners[0]]},
            {"version": 1, "copied": [*owners, "harness\\noncanonical.py"]},
        )
        for payload in cases:
            state.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
            self.assertTrue(repository_flavor_issues(root), payload)

    def test_physical_package_and_parity_drift_fail_closed(self):
        root = self._generated_tree()
        package = root / PYTHON_PACKAGE
        shutil.rmtree(package)
        package.symlink_to(self.repo / PYTHON_PACKAGE, target_is_directory=True)
        self.assertTrue(any("package" in issue for issue in core_artifact_issues(root)))
        for package_name, checker, filename in (
                (GO_PACKAGE, go_parity_issues, "model.go"),
                (RUST_PACKAGE, rust_parity_issues, "model.rs")):
            if not (self.repo / package_name).is_dir():
                continue
            with self.subTest(package=package_name):
                parity_root, target = self._parity_tree(package_name, catalyst=True)
                path = target / filename
                path.write_bytes(path.read_bytes() + b" ")
                self.assertTrue(any("physical pin" in issue
                                    for issue in checker(parity_root)))
        if (self.repo / RUST_PACKAGE).is_dir():
            root, _ = self._parity_tree(RUST_PACKAGE, catalyst=True)
            registration = root / RUST_REGISTRATION
            registration.write_text(registration.read_text(encoding="utf-8") +
                                    "pub mod decision_capsule_contract;\n",
                                    encoding="utf-8")
            self.assertTrue(any("registration" in issue
                                for issue in rust_parity_issues(root)))

            root, _ = self._parity_tree(RUST_PACKAGE, catalyst=True)
            registration = root / RUST_REGISTRATION
            registration.write_text(registration.read_text(encoding="utf-8") +
                                    "pub use decision_capsule_contract::DecisionCapsule;\n",
                                    encoding="utf-8")
            self.assertEqual([], rust_parity_issues(root))
            self.assertTrue(any("allowed reference count drifted" in issue
                                for issue in runtime_reference_issues(root)))

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
        project = root / ".agent/project.yml"
        project_data = yaml.safe_load(project.read_text(encoding="utf-8"))
        project_data["repository_flavor"] = "scaffolded_project"
        project.write_text(yaml.safe_dump(project_data, sort_keys=False), encoding="utf-8")
        (root / ".agent/scaffold-state.json").write_text(
            json.dumps({"version": 1, "copied": sorted(SCAFFOLD_OWNERS)}, indent=2) + "\n",
            encoding="utf-8")
        (root / ".agent/scaffold-state.json").chmod(0o644)
        excluded = (f"{GO_PACKAGE}/", f"{RUST_PACKAGE}/", "harness/scaffold/")
        scaffold_allowed = (relative for relative in RUNTIME_REFERENCE_ALLOWED_COUNTS
                            if relative != RUST_REGISTRATION
                            and not relative.startswith(excluded))
        for relative in sorted({*CORE_SHA256, GOVERNANCE_DECISION, *scaffold_allowed}):
            source, target = self.repo / relative, root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        return root

    def _parity_tree(self, package_name, *, catalyst=False):
        root = self._generated_tree()
        if catalyst:
            project = root / ".agent/project.yml"
            project_data = yaml.safe_load(project.read_text(encoding="utf-8"))
            project_data["repository_flavor"] = "catalyst_source"
            project.write_text(yaml.safe_dump(project_data, sort_keys=False), encoding="utf-8")
            (root / ".agent/scaffold-state.json").unlink()
        target = root / package_name
        target.parent.mkdir(parents=True, exist_ok=True)
        source = self.repo / package_name
        if source.is_dir():
            shutil.copytree(source, target)
        else:
            target.mkdir()
        if package_name == RUST_PACKAGE and (self.repo / RUST_REGISTRATION).is_file():
            registration = root / RUST_REGISTRATION
            registration.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.repo / RUST_REGISTRATION, registration)
        return root, target


if __name__ == "__main__":
    unittest.main()
