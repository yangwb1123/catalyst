from __future__ import annotations

import copy
import hashlib
import inspect
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
    evaluation_branch_digest, load_golden, seal_decision_capsule,
    seal_evaluation_branch,
    seal_structural_replay_closure, seal_structural_replay_manifest,
    structural_replay_closure_digest, structural_replay_manifest_digest,
    validate_decision_capsule, validate_evaluation_branch,
    validate_structural_replay_closure, validate_structural_replay_manifest,
)
from decision_capsule_contract.codec import read_bounded_file
from decision_capsule_contract.constants import (
    BRANCH_DOMAIN, CAPSULE_DOMAIN, CLOSURE_DOMAIN, FIXTURE_PATH,
    MANIFEST_DOMAIN, MAX_BRANCH_BYTES, MAX_CAPSULE_BYTES, MAX_CLOSURE_BYTES,
    MAX_MANIFEST_BYTES,
)


ROOT = Path(__file__).resolve().parents[1]


def blank(record: dict, id_field: str, hash_field: str) -> dict:
    candidate = copy.deepcopy(record)
    candidate[id_field] = candidate[hash_field] = ""
    return candidate


def checker(args: list[str]) -> subprocess.CompletedProcess:
    environment = dict(os.environ)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    return subprocess.run(
        [sys.executable, "-B", "harness/decision_capsule_contract_check.py", *args],
        cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=False,
    )


def reflection_refs(count: int) -> list[dict[str, object]]:
    values = [{"artifact_kind": "reflection_report",
               "artifact_ref": f"fixture/reflection/{index:02d}",
               "artifact_sha256": f"{index + 1:064x}"}
              for index in range(count)]
    return sorted(values, key=canonical_json)


def ceiling_cases(test: "DecisionCapsuleStrictTest") -> tuple:
    return (
        ("decision_capsule_contract.manifest.MAX_MANIFEST_BYTES", test.manifest,
         "manifest_id", "manifest_sha256",
         lambda: structural_replay_manifest_digest(
             blank(test.manifest, "manifest_id", "manifest_sha256"), test.decision),
         lambda value: seal_structural_replay_manifest(value, test.decision),
         lambda raw: decode_structural_replay_manifest(raw, test.decision),
         lambda: validate_structural_replay_manifest(test.manifest, test.decision)),
        ("decision_capsule_contract.capsule.MAX_CAPSULE_BYTES", test.capsule,
         "capsule_id", "capsule_sha256", lambda: decision_capsule_digest(
             blank(test.capsule, "capsule_id", "capsule_sha256")),
         seal_decision_capsule, decode_decision_capsule,
         lambda: validate_decision_capsule(test.capsule)),
        ("decision_capsule_contract.branch.MAX_BRANCH_BYTES", test.branch,
         "branch_id", "branch_sha256",
         lambda: evaluation_branch_digest(
             blank(test.branch, "branch_id", "branch_sha256"), test.capsule),
         lambda value: seal_evaluation_branch(value, test.capsule),
         lambda raw: decode_evaluation_branch(raw, test.capsule),
         lambda: validate_evaluation_branch(test.branch, test.capsule)),
        ("decision_capsule_contract.closure.MAX_CLOSURE_BYTES", test.outer,
         "closure_id", "closure_sha256",
         lambda: structural_replay_closure_digest(
             blank(test.outer, "closure_id", "closure_sha256")),
         seal_structural_replay_closure, decode_structural_replay_closure,
         lambda: validate_structural_replay_closure(test.outer)),
    )


def deep_value() -> dict[str, object]:
    value: dict[str, object] = {}
    for _ in range(2_000):
        value = {"x": value}
    return value


class IterationTrap(dict):
    def __iter__(self):
        raise AssertionError("oversized object must fail before key iteration")


class EncodingTrap(str):
    def encode(self, *_args, **_kwargs):
        raise AssertionError("string subclass encode must never run")


class ItemsTrap(dict):
    def items(self):
        raise AssertionError("dict subclass items must never run")


class LengthTrap(list):
    def __len__(self):
        raise RuntimeError("list subclass length must never run")

class ReprTrap:
    def __repr__(self):
        raise RuntimeError("unsupported key repr must never run")


class NameTrap(type):
    @property
    def __name__(cls):
        raise RuntimeError("unsupported type name must never run")

class UnsupportedTrap(metaclass=NameTrap):
    pass


def digest_seal_cases(test: "DecisionCapsuleStrictTest") -> tuple:
    return (
        (test.manifest, "manifest_id", "manifest_sha256",
         lambda value: structural_replay_manifest_digest(value, test.decision),
         lambda value: seal_structural_replay_manifest(value, test.decision)),
        (test.capsule, "capsule_id", "capsule_sha256", decision_capsule_digest,
         seal_decision_capsule),
        (test.branch, "branch_id", "branch_sha256",
         lambda value: evaluation_branch_digest(value, test.capsule),
         lambda value: seal_evaluation_branch(value, test.capsule)),
        (test.outer, "closure_id", "closure_sha256", structural_replay_closure_digest,
         seal_structural_replay_closure),
    )


class DecisionCapsuleStrictTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.outer = load_golden(ROOT)
        cls.capsule = cls.outer["decision_capsule"]
        cls.manifest = cls.capsule["replay_manifest"]
        cls.branch = cls.outer["evaluation_branch"]
        cls.decision = cls.capsule["decision_closure"]

    def test_domain_separated_own_identity_preimages_are_exact(self) -> None:
        cases = (
            (self.manifest, "manifest_id", "manifest_sha256", MANIFEST_DOMAIN),
            (self.capsule, "capsule_id", "capsule_sha256", CAPSULE_DOMAIN),
            (self.branch, "branch_id", "branch_sha256", BRANCH_DOMAIN),
            (self.outer, "closure_id", "closure_sha256", CLOSURE_DOMAIN),
        )
        for record, id_field, hash_field, domain in cases:
            candidate = blank(record, id_field, hash_field)
            expected = hashlib.sha256(domain + canonical_json(candidate)).hexdigest()
            with self.subTest(kind=record["kind"]):
                self.assertEqual(record[hash_field], expected)
                self.assertTrue(record[id_field].endswith(expected))

    def test_seals_require_both_own_identity_fields_blank_only(self) -> None:
        cases = (
            (self.manifest, "manifest_id", "manifest_sha256",
             lambda value: seal_structural_replay_manifest(value, self.decision)),
            (self.capsule, "capsule_id", "capsule_sha256", seal_decision_capsule),
            (self.branch, "branch_id", "branch_sha256",
             lambda value: seal_evaluation_branch(value, self.capsule)),
            (self.outer, "closure_id", "closure_sha256",
             seal_structural_replay_closure),
        )
        for record, id_field, hash_field, seal in cases:
            for blank_field in (id_field, hash_field):
                changed = copy.deepcopy(record)
                changed[blank_field] = ""
                with self.subTest(kind=record["kind"], blank=blank_field), self.assertRaises(ContractError):
                    seal(changed)

    def test_nested_blank_and_stale_nested_seals_are_rejected(self) -> None:
        capsule = blank(self.capsule, "capsule_id", "capsule_sha256")
        capsule["decision_closure"]["closure_id"] = ""
        with self.assertRaises(ContractError):
            seal_decision_capsule(capsule)
        outer = blank(self.outer, "closure_id", "closure_sha256")
        outer["decision_capsule"]["capsule_sha256"] = "0" * 64
        with self.assertRaises(ContractError):
            seal_structural_replay_closure(outer)
        stale = blank(self.capsule, "capsule_id", "capsule_sha256")
        stale["decision_closure"]["result"] += " drift"
        with self.assertRaises(ContractError):
            seal_decision_capsule(stale)

    def test_digest_and_seal_fail_closed_on_deep_and_cyclic_values(self) -> None:
        cyclic: dict[str, object] = {}
        cyclic["x"] = cyclic
        for payload in (deep_value(), cyclic):
            for record, id_field, hash_field, digest, seal in digest_seal_cases(self):
                changed = blank(record, id_field, hash_field)
                changed["attestations"] = payload
                with self.subTest(kind=record["kind"], api="digest"), \
                        self.assertRaises(ContractError):
                    digest(changed)
                with self.subTest(kind=record["kind"], api="seal"), \
                        self.assertRaises(ContractError):
                    seal(changed)

    def test_public_inputs_preflight_oversized_objects_without_iteration(self) -> None:
        oversized = IterationTrap()
        for index in range(65):
            dict.__setitem__(oversized, f"k{index}", None)
        for _, _, _, digest, seal in digest_seal_cases(self):
            with self.assertRaises(ContractError):
                digest(oversized)
            with self.assertRaises(ContractError):
                seal(oversized)
        from decision_capsule_contract import derive_structural_replay_closure
        with self.assertRaises(ContractError):
            derive_structural_replay_closure(self.capsule, oversized)

    def test_object_specific_size_preflight_runs_before_deepcopy(self) -> None:
        import decision_capsule_contract.codec as codec_module
        maximum = "a" * 16_384
        cases = (
            ([maximum] * 256,
             lambda value: seal_structural_replay_manifest(value, self.decision)),
            ([maximum] * 4,
             lambda value: seal_evaluation_branch(value, self.capsule)),
        )
        for value, seal in cases:
            with mock.patch.object(codec_module.copy, "deepcopy") as deep_copy:
                with self.assertRaises(ContractError):
                    seal(value)
                deep_copy.assert_not_called()

    def test_cross_partition_atom_duplicate_preempts_dependency_validator(self) -> None:
        import decision_capsule_contract.manifest as manifest_module
        invalid = copy.deepcopy(self.manifest)
        invalid["postdecision_atom_refs"][0] = copy.deepcopy(
            invalid["predecision_atom_refs"][0])
        with mock.patch.object(manifest_module, "validate_decision_closure") as dependency:
            with self.assertRaisesRegex(ContractError, "combined decision atom refs"):
                validate_structural_replay_manifest(invalid, self.decision)
            dependency.assert_not_called()
    def test_bounded_size_preempts_alias_expansion_and_bad_object_keys(self) -> None:
        import decision_capsule_contract.codec as codec_module
        shared: object = ["x" * 16_384]
        for _ in range(4):
            shared = [shared] * 256
        with mock.patch.object(codec_module, "_operational_canonical_json") as encoder:
            with self.assertRaisesRegex(ContractError, "canonical JSON exceeds"):
                codec_module.canonical_json(shared)
            encoder.assert_not_called()
        with self.assertRaisesRegex(ContractError, "object key"):
            codec_module.validate_json_tree({1: None}, MAX_CLOSURE_BYTES)

    def test_programmatic_subclasses_fail_as_contract_errors_without_callbacks(self) -> None:
        import decision_capsule_contract.codec as codec_module
        manifest = copy.deepcopy(self.manifest)
        manifest["artifact_refs"][0]["artifact_ref"] = EncodingTrap("fixture/input")
        with self.assertRaises(ContractError):
            validate_structural_replay_manifest(manifest, self.decision)
        capsule = copy.deepcopy(self.capsule)
        capsule["replay_manifest"] = ItemsTrap(capsule["replay_manifest"])
        with self.assertRaises(ContractError):
            validate_decision_capsule(capsule)
        from decision_capsule_contract import derive_structural_replay_closure
        for index, hostile in enumerate(
                (LengthTrap(), {ReprTrap(): None}, UnsupportedTrap())):
            with self.subTest(index=index), self.assertRaises(ContractError):
                if type(hostile) is LengthTrap:
                    derive_structural_replay_closure(self.capsule, hostile)
                else:
                    codec_module.canonical_json(hostile)

    def test_wire_rejects_lf_whitespace_duplicate_invalid_utf8_float_and_depth(self) -> None:
        physical = (ROOT / FIXTURE_PATH).read_bytes()
        raw = physical[:-1]
        invalid_utf8 = bytearray(raw)
        invalid_utf8[20] = 0xff
        cases = (
            physical,
            b" " + raw,
            raw + b" ",
            raw.replace(b'{"api_version":',
                        b'{"api_version":"x","api_version":', 1),
            bytes(invalid_utf8),
            raw.replace(b'"effect_replay_allowed":false',
                        b'"effect_replay_allowed":0.0', 1),
            raw.replace(b'"comparison_result":"EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY"',
                        b'"comparison_result":[[[[[[[[[[[[[[[[["x"]]]]]]]]]]]]]]]]]', 1),
        )
        for index, changed in enumerate(cases):
            with self.subTest(index=index), self.assertRaises(ContractError):
                decode_structural_replay_closure(changed)

    def test_public_canonical_encoder_has_exact_local_twenty_eight_mib_ceiling(self) -> None:
        self.assertEqual(MAX_CLOSURE_BYTES, 28 * 1024 * 1024)
        maximum = "x" * 16_384
        accepted = [[maximum] * 256 for _ in range(7)]
        accepted[-1][-1] = "x" * 10_993
        self.assertEqual(len(canonical_json(accepted)), MAX_CLOSURE_BYTES)
        accepted[-1][-1] += "x"
        with self.assertRaisesRegex(ContractError, "canonical JSON exceeds"):
            canonical_json(accepted)

    def test_manifest_internal_shape_preserves_256_escaped_ref_ceiling(self) -> None:
        from decision_capsule_contract.manifest import _shape
        candidate = blank(self.manifest, "manifest_id", "manifest_sha256")
        kind = '"' * 160
        candidate["artifact_refs"] = sorted([
            {"artifact_kind": kind, "artifact_ref": "\\" * 4096,
             "artifact_sha256": f"{index:064x}"}
            for index in range(256)
        ], key=canonical_json)
        _shape(candidate, allow_blank=True)
        size = len(canonical_json(candidate))
        self.assertEqual(size, 2_218_274)
        candidate["artifact_refs"].append(copy.deepcopy(candidate["artifact_refs"][-1]))
        with self.assertRaises(ContractError):
            _shape(candidate, allow_blank=True)

    def test_manifest_semantic_upper_envelope_fits_four_mib(self) -> None:
        from decision_capsule_contract.manifest import _shape
        candidate = blank(self.manifest, "manifest_id", "manifest_sha256")

        def refs(prefix: str, id_field: str, hash_field: str,
                 count: int) -> list[dict[str, str]]:
            return [{id_field: prefix + f"{index + 1:064x}",
                     hash_field: f"{index + 1:064x}"}
                    for index in range(count)]

        kind = '"' * 160
        candidate["artifact_refs"] = sorted([
            {"artifact_kind": kind, "artifact_ref": "\\" * 4096,
             "artifact_sha256": f"{index + 1:064x}"}
            for index in range(64)
        ], key=canonical_json)
        candidate["artifact_receipt_refs"] = refs(
            "artifact-receipt-", "artifact_receipt_id", "artifact_receipt_sha256", 64)
        candidate["capability_invocation_refs"] = refs(
            "capability-invocation-", "invocation_id", "invocation_sha256", 64)
        candidate["interaction_event_refs"] = refs(
            "interaction-event-", "event_id", "event_sha256", 256)
        candidate["execution_receipt_refs"] = refs(
            "execution-receipt-", "execution_receipt_id", "execution_receipt_sha256", 64)
        candidate["predecision_atom_refs"] = refs(
            "cognitive-atom-", "atom_id", "atom_sha256", 256)
        candidate["postdecision_atom_refs"] = []
        _shape(candidate, allow_blank=True)
        blank_size = len(canonical_json(candidate))
        self.assertEqual(blank_size, 684_285)
        candidate["manifest_id"] = self.manifest["manifest_id"]
        candidate["manifest_sha256"] = self.manifest["manifest_sha256"]
        self.assertEqual(len(canonical_json(candidate)), 684_440)

    def test_conservative_capsule_and_outer_byte_bounds_are_exact(self) -> None:
        capsule_size = len(canonical_json(self.capsule))
        decision_size = len(canonical_json(self.decision))
        manifest_size = len(canonical_json(self.manifest))
        capsule_overhead = capsule_size - decision_size - manifest_size
        self.assertEqual(capsule_overhead, 1_867)
        self.assertEqual(20 * 1024 * 1024 + 684_440 + capsule_overhead,
                         21_657_827)

        branch_size = len(canonical_json(self.branch))
        self.assertEqual(branch_size, 2_305)
        maximum_refs = sorted([
            {"artifact_kind": "reflection_report",
             "artifact_ref": "\\" * 4_096,
             "artifact_sha256": f"{index + 1:064x}"}
            for index in range(32)
        ], key=canonical_json)
        refs_size = len(canonical_json(maximum_refs))
        self.assertEqual(refs_size, 266_657)
        outer_overhead = (len(canonical_json(self.outer)) - capsule_size -
                          branch_size - len(canonical_json(
                              self.outer["reflection_report_artifact_refs"])))
        self.assertEqual(outer_overhead, 2_083)
        self.assertEqual(21_657_827 + branch_size + refs_size + outer_overhead,
                         21_928_872)

    def test_configured_object_ceilings_and_public_n_n_plus_one_seams(self) -> None:
        self.assertEqual((MAX_MANIFEST_BYTES, MAX_CAPSULE_BYTES, MAX_BRANCH_BYTES,
                          MAX_CLOSURE_BYTES),
                         (4 * 1024 * 1024, 26 * 1024 * 1024,
                          64 * 1024, 28 * 1024 * 1024))
        for target, record, id_field, hash_field, digest, _, _, _ in ceiling_cases(self):
            blank_record = blank(record, id_field, hash_field)
            blank_size = len(canonical_json(blank_record))
            with mock.patch(target, blank_size):
                digest()
            with mock.patch(target, blank_size - 1):
                with self.subTest(target=target, boundary="blank"), \
                        self.assertRaises(ContractError):
                    digest()

    def test_public_seal_decode_and_validate_use_exact_sealed_ceiling(self) -> None:
        for target, record, id_field, hash_field, _, seal, decode, validate \
                in ceiling_cases(self):
            blank_record = blank(record, id_field, hash_field)
            sealed_size = len(canonical_json(record))
            with mock.patch(target, sealed_size):
                seal(blank_record)
                decode(canonical_json(record))
                validate()
            with mock.patch(target, sealed_size - 1):
                with self.subTest(target=target, boundary="seal"), \
                        self.assertRaises(ContractError):
                    seal(blank_record)
                with self.subTest(target=target, boundary="decode"), \
                        self.assertRaises(ContractError):
                    decode(canonical_json(record))
                with self.subTest(target=target, boundary="validate"), \
                        self.assertRaises(ContractError):
                    validate()

    def test_reflection_report_refs_accept_0_and_32_reject_33_and_drift(self) -> None:
        from decision_capsule_contract import derive_structural_replay_closure
        for count in (0, 32):
            derive_structural_replay_closure(self.capsule, reflection_refs(count))
        with self.assertRaises(ContractError):
            derive_structural_replay_closure(self.capsule, reflection_refs(33))
        wrong = reflection_refs(1)
        wrong[0]["artifact_kind"] = "other"
        with self.assertRaises(ContractError):
            derive_structural_replay_closure(self.capsule, wrong)
        duplicate = reflection_refs(1) * 2
        with self.assertRaises(ContractError):
            derive_structural_replay_closure(self.capsule, duplicate)
        with self.assertRaises(ContractError):
            derive_structural_replay_closure(self.capsule,
                                             list(reversed(reflection_refs(2))))

    def test_public_manifest_and_branch_validation_require_upstream_objects(self) -> None:
        import decision_capsule_contract as api
        manifest_parameters = inspect.signature(
            api.validate_structural_replay_manifest).parameters
        branch_parameters = inspect.signature(api.validate_evaluation_branch).parameters
        self.assertEqual(tuple(manifest_parameters), ("value", "decision_closure"))
        self.assertEqual(tuple(branch_parameters), ("value", "capsule"))
        with self.assertRaises(TypeError):
            api.validate_structural_replay_manifest(self.manifest)
        with self.assertRaises(TypeError):
            api.validate_evaluation_branch(self.branch)

    def test_checker_rejects_missing_directory_symlink_hardlink_and_oversize(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            base = Path(directory)
            source = base / "source.json"
            source.write_bytes(canonical_json(self.outer))
            symlink = base / "link.json"
            symlink.symlink_to(source.name)
            hardlink = base / "hard.json"
            os.link(source, hardlink)
            oversize = base / "oversize.json"
            with oversize.open("wb") as stream:
                stream.seek(MAX_CLOSURE_BYTES)
                stream.write(b"x")
            paths = (base / "missing.json", base, symlink, hardlink, oversize)
            for path in paths:
                result = checker(["--file", str(path)])
                with self.subTest(path=path.name):
                    self.assertEqual(result.returncode, 2)
                    self.assertEqual(result.stdout, b"")
                    self.assertNotIn(b"Traceback", result.stderr)

    def test_no_follow_absence_fails_closed(self) -> None:
        path = ROOT / FIXTURE_PATH
        with mock.patch.object(os, "O_NOFOLLOW", None):
            with self.assertRaisesRegex(ContractError, "no-follow"):
                read_bounded_file(path, "fixture", MAX_CLOSURE_BYTES)

    def test_checker_file_mode_rejects_physical_golden_lf_and_failure_is_silent(self) -> None:
        result = checker(["--file", str(ROOT / FIXTURE_PATH)])
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"ERROR", result.stderr)
        self.assertNotIn(b"Traceback", result.stderr)
        success = checker(["--golden", "."])
        self.assertEqual(success.stdout, (SUCCESS_MARKER + "\n").encode())

    def test_core_imports_no_effect_runtime_or_mutable_state_client(self) -> None:
        package = ROOT / "harness/decision_capsule_contract"
        forbidden = ("import socket", "import sqlite3", "import subprocess",
                     "import requests", "from urllib", "import time")
        for path in package.glob("*.py"):
            source = path.read_text()
            for token in forbidden:
                with self.subTest(path=path.name, token=token):
                    self.assertNotIn(token, source)
if __name__ == "__main__":
    unittest.main()
