from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from decision_capsule_contract import (
    ContractError, SUCCESS_MARKER, canonical_json, decision_capsule_digest,
    decode_decision_capsule, decode_evaluation_branch,
    decode_structural_replay_closure, decode_structural_replay_manifest,
    derive_evaluation_branch, derive_structural_replay_closure,
    evaluation_branch_digest, load_golden, seal_decision_capsule,
    seal_evaluation_branch, seal_structural_replay_closure,
    seal_structural_replay_manifest, structural_replay_closure_digest,
    structural_replay_manifest_digest, validate_decision_capsule,
    validate_evaluation_branch, validate_structural_replay_closure,
    validate_structural_replay_manifest,
)
from decision_capsule_contract.constants import (
    ATTESTATION_FIELDS, BRANCH_FIELDS, CAPSULE_FIELDS, CAPSULE_RESULT,
    CLOSURE_FIELDS, COMPARISON_RESULT, FIXTURE_PATH, MANIFEST_FIELDS,
)
from decision_capsule_contract.fixture import GOLDEN_SHA256, golden_bytes
from kernel_decision_contract.constants import ATTESTATION_FIELDS as DECISION_ATTESTATIONS


ROOT = Path(__file__).resolve().parents[1]


def blank(record: dict, id_field: str, hash_field: str) -> dict:
    candidate = copy.deepcopy(record)
    candidate[id_field] = candidate[hash_field] = ""
    return candidate


class DecisionCapsuleContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.closure = load_golden(ROOT)
        cls.capsule = cls.closure["decision_capsule"]
        cls.manifest = cls.capsule["replay_manifest"]
        cls.branch = cls.closure["evaluation_branch"]
        cls.decision = cls.capsule["decision_closure"]

    def test_golden_is_exact_deterministic_single_lf_file(self) -> None:
        path = ROOT / FIXTURE_PATH
        physical = path.read_bytes()
        self.assertEqual(hashlib.sha256(physical).hexdigest(), GOLDEN_SHA256)
        self.assertEqual(physical, golden_bytes())
        self.assertTrue(physical.endswith(b"\n"))
        self.assertFalse(physical.endswith(b"\n\n"))
        self.assertEqual(canonical_json(self.closure), physical[:-1])

    def test_four_new_objects_have_exact_frozen_fields(self) -> None:
        cases = (
            (self.manifest, MANIFEST_FIELDS, 19),
            (self.capsule, CAPSULE_FIELDS, 10),
            (self.branch, BRANCH_FIELDS, 13),
            (self.closure, CLOSURE_FIELDS, 10),
        )
        for record, fields, count in cases:
            with self.subTest(kind=record["kind"]):
                self.assertEqual(set(record), fields)
                self.assertEqual(len(fields), count)

    def test_every_new_object_has_exact_thirty_two_false_attestations(self) -> None:
        records = (self.manifest, self.capsule, self.branch, self.closure)
        self.assertEqual(len(ATTESTATION_FIELDS), 32)
        for record in records:
            with self.subTest(kind=record["kind"]):
                self.assertEqual(set(record["attestations"]), ATTESTATION_FIELDS)
                self.assertTrue(all(value is False for value in record["attestations"].values()))

    def _assert_manifest_local_preflight(self) -> None:
        import decision_capsule_contract.manifest as manifest_module

        invalid = copy.deepcopy(self.manifest)
        invalid["replay_mode"] += "_drift"
        blanked = blank(invalid, "manifest_id", "manifest_sha256")
        with mock.patch.object(manifest_module, "validate_decision_closure") as dependency:
            for operation in (
                lambda: structural_replay_manifest_digest(invalid, self.decision),
                lambda: validate_structural_replay_manifest(invalid, self.decision),
                lambda: seal_structural_replay_manifest(blanked, self.decision),
            ):
                with self.assertRaises(ContractError):
                    operation()
            dependency.assert_not_called()

    def _assert_capsule_and_branch_local_preflight(self) -> dict[str, object]:
        import decision_capsule_contract.branch as branch_module
        import decision_capsule_contract.capsule as capsule_module

        invalid_capsule = copy.deepcopy(self.capsule)
        invalid_capsule["replay_manifest"]["replay_mode"] += "_drift"
        blank_capsule = blank(invalid_capsule, "capsule_id", "capsule_sha256")
        with (mock.patch.object(capsule_module, "validate_decision_closure") as dependency,
              mock.patch.object(capsule_module, "bounded_deepcopy") as copier):
            for operation in (
                lambda: decision_capsule_digest(invalid_capsule),
                lambda: validate_decision_capsule(invalid_capsule),
                lambda: seal_decision_capsule(blank_capsule),
            ):
                with self.assertRaises(ContractError):
                    operation()
            dependency.assert_not_called()
            copier.assert_not_called()

        blank_branch = blank(self.branch, "branch_id", "branch_sha256")
        with (mock.patch.object(branch_module, "validate_decision_capsule") as dependency,
              mock.patch.object(branch_module, "bounded_deepcopy") as copier):
            for operation in (
                lambda: evaluation_branch_digest(self.branch, invalid_capsule),
                lambda: validate_evaluation_branch(self.branch, invalid_capsule),
                lambda: seal_evaluation_branch(blank_branch, invalid_capsule),
                lambda: derive_evaluation_branch(invalid_capsule),
            ):
                with self.assertRaises(ContractError):
                    operation()
            dependency.assert_not_called()
            copier.assert_not_called()
        return invalid_capsule

    def _assert_outer_local_preflight(self, invalid_capsule: dict[str, object]) -> None:
        import decision_capsule_contract.closure as closure_module

        invalid_outer = copy.deepcopy(self.closure)
        invalid_outer["evaluation_branch"]["branch_mode"] += "_drift"
        invalid_outer["decision_capsule"]["replay_manifest"]["replay_mode"] += "_drift"
        blank_outer = blank(invalid_outer, "closure_id", "closure_sha256")
        with (mock.patch.object(closure_module, "validate_decision_capsule") as dependency,
              mock.patch.object(closure_module, "bounded_deepcopy") as copier):
            for operation in (
                lambda: structural_replay_closure_digest(invalid_outer),
                lambda: validate_structural_replay_closure(invalid_outer),
                lambda: seal_structural_replay_closure(blank_outer),
            ):
                with self.assertRaisesRegex(ContractError, "branch_mode"):
                    operation()
            dependency.assert_not_called()
            copier.assert_not_called()

        with (mock.patch.object(closure_module, "validate_decision_capsule") as dependency,
              mock.patch.object(closure_module, "bounded_deepcopy") as copier):
            with self.assertRaises(ContractError):
                derive_structural_replay_closure(invalid_capsule, [])
            dependency.assert_not_called()
            copier.assert_not_called()

    def test_nested_local_invalidity_preempts_every_composite_public_route(self) -> None:
        self._assert_manifest_local_preflight()
        invalid_capsule = self._assert_capsule_and_branch_local_preflight()
        self._assert_outer_local_preflight(invalid_capsule)

    def test_relational_failures_preempt_dependency_clones_on_all_public_routes(self) -> None:
        import decision_capsule_contract.branch as branch_module
        import decision_capsule_contract.capsule as capsule_module
        import decision_capsule_contract.closure as closure_module
        import decision_capsule_contract.manifest as manifest_module

        digest = "0" * 64
        manifest = copy.deepcopy(self.manifest)
        manifest["decision_closure_ref"] = {
            "closure_id": f"kernel-decision-reference-closure-{digest}",
            "closure_sha256": digest}
        capsule = copy.deepcopy(self.capsule)
        capsule["replay_manifest"] = manifest
        branch = copy.deepcopy(self.branch)
        branch["capsule_ref"] = {
            "capsule_id": f"decision-capsule-{digest}", "capsule_sha256": digest}
        outer = copy.deepcopy(self.closure)
        outer["evaluation_branch"] = branch
        cases = (
            (manifest_module, "validate_decision_closure", (
                lambda: structural_replay_manifest_digest(manifest, self.decision),
                lambda: validate_structural_replay_manifest(manifest, self.decision),
                lambda: seal_structural_replay_manifest(
                    blank(manifest, "manifest_id", "manifest_sha256"), self.decision))),
            (capsule_module, "_reseal_decision_closure", (
                lambda: decision_capsule_digest(capsule),
                lambda: validate_decision_capsule(capsule),
                lambda: seal_decision_capsule(
                    blank(capsule, "capsule_id", "capsule_sha256")))),
            (branch_module, "validate_decision_capsule", (
                lambda: evaluation_branch_digest(branch, self.capsule),
                lambda: validate_evaluation_branch(branch, self.capsule),
                lambda: seal_evaluation_branch(
                    blank(branch, "branch_id", "branch_sha256"), self.capsule))),
            (closure_module, "validate_decision_capsule", (
                lambda: structural_replay_closure_digest(outer),
                lambda: validate_structural_replay_closure(outer),
                lambda: seal_structural_replay_closure(
                    blank(outer, "closure_id", "closure_sha256")))),
        )
        for module, dependency_name, operations in cases:
            with (self.subTest(module=module.__name__),
                  mock.patch.object(module, dependency_name) as dependency,
                  mock.patch.object(module, "bounded_deepcopy") as copier):
                for operation in operations:
                    with self.assertRaises(ContractError):
                        operation()
                dependency.assert_not_called()
                copier.assert_not_called()

    def test_embedded_adr0090_keeps_its_exact_twenty_two_attestations(self) -> None:
        self.assertEqual(len(DECISION_ATTESTATIONS), 22)
        decision_records = [self.decision, self.decision["decision_transaction"]]
        decision_records.extend(self.decision["cognitive_atoms"])
        for record in decision_records:
            self.assertEqual(set(record["attestations"]), DECISION_ATTESTATIONS)
        self.assertNotEqual(ATTESTATION_FIELDS, DECISION_ATTESTATIONS)

    def test_manifest_roundtrip_digest_and_reseal(self) -> None:
        raw = canonical_json(self.manifest)
        self.assertEqual(decode_structural_replay_manifest(raw, self.decision), self.manifest)
        self.assertEqual(structural_replay_manifest_digest(self.manifest, self.decision),
                         self.manifest["manifest_sha256"])
        candidate = blank(self.manifest, "manifest_id", "manifest_sha256")
        self.assertEqual(seal_structural_replay_manifest(candidate, self.decision),
                         self.manifest)
        self.assertEqual(validate_structural_replay_manifest(
            self.manifest, self.decision), self.manifest)

    def test_capsule_roundtrip_digest_and_reseal(self) -> None:
        raw = canonical_json(self.capsule)
        self.assertEqual(decode_decision_capsule(raw), self.capsule)
        self.assertEqual(decision_capsule_digest(self.capsule),
                         self.capsule["capsule_sha256"])
        candidate = blank(self.capsule, "capsule_id", "capsule_sha256")
        self.assertEqual(seal_decision_capsule(candidate), self.capsule)
        self.assertEqual(validate_decision_capsule(self.capsule), self.capsule)

    def test_branch_roundtrip_digest_and_reseal(self) -> None:
        raw = canonical_json(self.branch)
        self.assertEqual(decode_evaluation_branch(raw, self.capsule), self.branch)
        self.assertEqual(evaluation_branch_digest(self.branch, self.capsule),
                         self.branch["branch_sha256"])
        candidate = blank(self.branch, "branch_id", "branch_sha256")
        self.assertEqual(seal_evaluation_branch(candidate, self.capsule), self.branch)
        self.assertEqual(validate_evaluation_branch(self.branch, self.capsule), self.branch)

    def test_outer_roundtrip_digest_and_reseal(self) -> None:
        raw = canonical_json(self.closure)
        self.assertEqual(decode_structural_replay_closure(raw), self.closure)
        self.assertEqual(structural_replay_closure_digest(self.closure),
                         self.closure["closure_sha256"])
        candidate = blank(self.closure, "closure_id", "closure_sha256")
        self.assertEqual(seal_structural_replay_closure(candidate), self.closure)
        self.assertEqual(validate_structural_replay_closure(self.closure), self.closure)

    def test_constants_and_negative_replay_controls_are_exact(self) -> None:
        self.assertEqual(self.capsule["result"], CAPSULE_RESULT)
        self.assertEqual(self.closure["result"], SUCCESS_MARKER)
        self.assertEqual(self.branch["comparison_result"], COMPARISON_RESULT)
        self.assertEqual(self.manifest["replay_mode"],
                         "structural_validate_reseal_compare_only")
        self.assertEqual(self.branch["branch_mode"],
                         "structural_validate_reseal_compare_only")
        self.assertEqual(self.capsule["capsule_mode"],
                         "structural_replay_manifest_only")
        for record in (self.manifest, self.branch):
            self.assertIs(record["effect_replay_allowed"], False)
            self.assertIs(record["history_rewrite_allowed"], False)

    def test_missing_unknown_and_bad_kind_fail_for_each_object(self) -> None:
        cases = (
            (self.manifest, lambda value: validate_structural_replay_manifest(
                value, self.decision)),
            (self.capsule, validate_decision_capsule),
            (self.branch, lambda value: validate_evaluation_branch(value, self.capsule)),
            (self.closure, validate_structural_replay_closure),
        )
        for record, validator in cases:
            first = sorted(record)[0]
            mutations = []
            missing = copy.deepcopy(record)
            del missing[first]
            mutations.append(missing)
            unknown = copy.deepcopy(record)
            unknown["unknown"] = None
            mutations.append(unknown)
            bad_kind = copy.deepcopy(record)
            bad_kind["kind"] = []
            mutations.append(bad_kind)
            for index, changed in enumerate(mutations):
                with self.subTest(kind=record["kind"], index=index), self.assertRaises(ValueError):
                    validator(changed)

    def test_draft_2020_schema_validates_golden(self) -> None:
        try:
            from jsonschema import Draft202012Validator
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema/referencing unavailable: {error}")
        paths = (
            "kernel-operational-reference-core-v1.schema.json",
            "kernel-decision-reference-core-v1.schema.json",
            "decision-capsule-structural-replay-core-v1.schema.json",
        )
        schemas = [json.loads((ROOT / "docs/contracts" / path).read_text())
                   for path in paths]
        for schema in schemas:
            Draft202012Validator.check_schema(schema)
        registry = Registry()
        for schema in schemas[:2]:
            registry = registry.with_resource(schema["$id"], Resource.from_contents(schema))
        Draft202012Validator(schemas[-1], registry=registry).validate(self.closure)

    def test_checker_golden_and_explicit_file_stdout_are_exact(self) -> None:
        environment = dict(os.environ)
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        commands = [["--golden", "."]]
        with tempfile.NamedTemporaryFile(suffix=".json") as stream:
            self.assertNotIn(ROOT, Path(stream.name).resolve().parents)
            stream.write(canonical_json(self.closure))
            stream.flush()
            commands.append(["--file", stream.name])
            for args in commands:
                result = subprocess.run(
                    [sys.executable, "-B", "harness/decision_capsule_contract_check.py",
                     *args], cwd=ROOT, env=environment, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, check=False,
                )
                with self.subTest(args=args):
                    self.assertEqual(result.returncode, 0, result.stderr.decode())
                    self.assertEqual(result.stdout, (SUCCESS_MARKER + "\n").encode())
                    self.assertEqual(result.stderr, b"")

    def test_malformed_nested_closure_is_a_controlled_contract_error(self) -> None:
        changed = copy.deepcopy(self.closure)
        changed["decision_capsule"]["decision_closure"] = {}
        raw = canonical_json(changed)
        with self.assertRaises(ContractError):
            decode_structural_replay_closure(raw)
        environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
        with tempfile.NamedTemporaryFile(suffix=".json") as stream:
            self.assertNotIn(ROOT, Path(stream.name).resolve().parents)
            stream.write(raw)
            stream.flush()
            result = subprocess.run(
                [sys.executable, "-B", "harness/decision_capsule_contract_check.py",
                 "--file", stream.name], cwd=ROOT, env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"Traceback", result.stderr)
        self.assertIn(b"ERROR:", result.stderr)


if __name__ == "__main__":
    unittest.main()
