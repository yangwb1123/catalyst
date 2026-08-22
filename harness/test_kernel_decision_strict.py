from __future__ import annotations

import copy
import hashlib
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from kernel_decision_contract import (
    ContractError, canonical_json, decode_closure, load_golden, seal_closure,
)
from kernel_decision_contract.atoms import seal_cognitive_atom
from kernel_decision_contract.constants import ATOM_DOMAIN, ATOM_PREFIX
from kernel_decision_contract.transaction import seal_decision_transaction


ROOT = Path(__file__).resolve().parents[1]


def set_path(value: dict, path: tuple[object, ...], replacement: object) -> None:
    current: object = value
    for member in path[:-1]:
        current = current[member]  # type: ignore[index]
    current[path[-1]] = replacement  # type: ignore[index]


class KernelDecisionStrictTest(unittest.TestCase):
    def setUp(self) -> None:
        self.closure = load_golden(ROOT)

    def test_every_enum_dispatch_bad_type_is_contract_error(self) -> None:
        atom_paths = (
            (("atom_type",), []),
            (("source", "source_kind"), {}),
            (("declared_authority", "authority_kind"), []),
            (("proposition", "object_type"), {}),
            (("declared_hardness",), []),
            (("epistemic_state",), {}),
        )
        for path, replacement in atom_paths:
            atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
            atom["atom_id"] = atom["atom_sha256"] = ""
            set_path(atom, path, replacement)
            with self.subTest(path=path), self.assertRaises(ContractError):
                seal_cognitive_atom(atom)
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        transaction["compensation"]["applicability"] = []
        with self.assertRaises(ContractError):
            seal_decision_transaction(transaction)

    def test_top_level_and_nested_bad_type_canonical_corpus(self) -> None:
        paths = (
            (("kind",), []),
            (("cognitive_atoms", 0, "atom_type"), []),
            (("cognitive_atoms", 0, "source", "source_kind"), {}),
            (("cognitive_atoms", 0, "declared_authority", "authority_kind"), []),
            (("cognitive_atoms", 0, "proposition", "object_type"), {}),
            (("decision_transaction", "compensation", "applicability"), []),
            (("decision_transaction", "selected_option_id"), {}),
        )
        for path, replacement in paths:
            changed = copy.deepcopy(self.closure)
            set_path(changed, path, replacement)
            with self.subTest(path=path), self.assertRaises(ContractError):
                decode_closure(canonical_json(changed))

    def test_wire_drift_is_rejected(self) -> None:
        path = ROOT / "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
        physical = path.read_bytes()
        raw = physical[:-1]
        cases = (physical, b" " + raw, raw + b" ", raw.replace(
            b'{"api_version":', b'{"api_version":"x","api_version":', 1))
        for index, changed in enumerate(cases):
            with self.subTest(index=index), self.assertRaises(ContractError):
                decode_closure(changed)

    def test_unknown_duplicate_utf8_control_surrogate_depth_and_size(self) -> None:
        path = ROOT / "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
        raw = path.read_bytes()[:-1]
        invalid_utf8 = bytearray(raw)
        invalid_utf8[20] = 0xff
        cases = (
            bytes(invalid_utf8),
            raw.replace(b'{"api_version":', b'{"unknown":0,"api_version":', 1),
            raw.replace(b'{"api_version":', b'{"api_version":"x","api_version":', 1),
            raw.replace(b'"value-03"', b'"\\ud800"', 1),
            raw.replace(b'"value-03"', b'"\\u202e"', 1),
            raw.replace(b'"atom_type":"constraint"',
                        b'"atom_type":[[[[[[[[[[[[[[[[["x"]]]]]]]]]]]]]]]]]', 1),
            b" " * (20_971_521),
        )
        for index, changed in enumerate(cases):
            with self.subTest(index=index), self.assertRaises(ContractError):
                decode_closure(changed)

    def test_every_public_seal_rejects_forbidden_in_memory_scalar(self) -> None:
        atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["proposition"]["object_value"] = "x\ud800"
        with self.assertRaises(ContractError):
            seal_cognitive_atom(atom)
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        transaction["idempotency_key"] = "x\ud800"
        with self.assertRaises(ContractError):
            seal_decision_transaction(transaction)
        closure = copy.deepcopy(self.closure)
        closure["closure_id"] = closure["closure_sha256"] = ""
        closure["result"] = "x\ud800"
        with self.assertRaises(ContractError):
            seal_closure(closure)

    def test_public_canonical_json_rejects_c1_controls(self) -> None:
        for scalar in ("\u0080", "\u009f"):
            with self.subTest(scalar=ord(scalar)), self.assertRaises(ContractError):
                canonical_json({"value": scalar})

    def test_numeric_instruction_flag_rejected_before_and_after_reseal(self) -> None:
        atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["instruction_allowed"] = 0
        with self.assertRaisesRegex(ContractError, "instruction_allowed"):
            seal_cognitive_atom(atom)

        candidate = copy.deepcopy(self.closure)
        candidate["closure_id"] = candidate["closure_sha256"] = ""
        index = next(index for index, item in enumerate(candidate["cognitive_atoms"])
                     if item["source"]["source_phase"] == "postdecision")
        atom = candidate["cognitive_atoms"][index]
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["instruction_allowed"] = 0
        digest = hashlib.sha256(ATOM_DOMAIN + canonical_json(atom)).hexdigest()
        atom["atom_id"], atom["atom_sha256"] = f"{ATOM_PREFIX}{digest}", digest
        candidate["cognitive_atoms"].sort(key=lambda item: item["atom_id"].encode())
        with self.assertRaisesRegex(ContractError, "instruction_allowed"):
            seal_closure(candidate)

    def test_checker_failure_is_exit_two_silent_stdout_no_traceback(self) -> None:
        with tempfile.NamedTemporaryFile(dir=ROOT, suffix=".json") as stream:
            stream.write(b"{}")
            stream.flush()
            environment = dict(os.environ)
            environment["PYTHONDONTWRITEBYTECODE"] = "1"
            result = subprocess.run(
                [sys.executable, "-B", "harness/kernel_decision_contract_check.py",
                 "--file", stream.name], cwd=ROOT, env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"Traceback", result.stderr)
        self.assertIn(b"ERROR", result.stderr)


if __name__ == "__main__":
    unittest.main()
