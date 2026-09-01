"""Shared evidence/state profiles and bounded programmatic-input tests."""

from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "harness"
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from platform_core_contract import (  # noqa: E402
    ContractError, load_golden, load_receipt_golden, validate_action_transition,
    validate_attempt_transition, validate_verification_receipt,
    validate_command_envelope, validate_work_item_transition)
from platform_core_contract.codec import (  # noqa: E402
    canonical_json, validate_document_shape, validate_text)
from platform_core_contract.constants import (  # noqa: E402
    DOCUMENT_INVALID, MAX_ARRAY_ITEMS, MAX_RECEIPT_BYTES, MAX_STRING_BYTES,
    REJECTION_CORPUS_PATH, STATE_INVALID, TRANSITION_INVALID,
    VERIFICATION_RECEIPT_TYPE_PROFILE)
from platform_core_contract.states import (  # noqa: E402
    ACTION_EDGES, ATTEMPT_EDGES, WORK_ITEM_EDGES)


class _IterationBomb(list):
    """Fail if an inexact built-in subclass is inspected."""

    def __len__(self) -> int:
        raise AssertionError("list subclass length was trusted")

    def __iter__(self):
        raise AssertionError("oversized list was traversed")


class _EncodeBomb(str):
    """Fail if an inexact text subclass is inspected."""

    def __len__(self) -> int:
        raise AssertionError("text subclass length was trusted")

    def encode(self, *args, **kwargs):
        raise AssertionError("text subclass was encoded")


class _MappingBomb(dict):
    """Fail if an inexact mapping subclass is inspected."""

    def __len__(self) -> int:
        raise AssertionError("mapping subclass length was trusted")

    def __iter__(self):
        raise AssertionError("mapping subclass was traversed")

    def items(self):
        raise AssertionError("mapping subclass items were traversed")


class _KeyBomb:
    """Allow dictionary construction, then fail if hashing is repeated."""

    def __init__(self) -> None:
        self.hashes = 0

    def __hash__(self) -> int:
        self.hashes += 1
        if self.hashes > 1:
            raise AssertionError("inexact object key was re-hashed")
        return 1


class _ReprBomb(str):
    """Fail if a rejected state is rendered into a diagnostic."""

    def __repr__(self) -> str:
        raise AssertionError("rejected state repr hook was invoked")

    def __str__(self) -> str:
        raise AssertionError("rejected state str hook was invoked")


class PlatformCoreReceiptProfileTests(unittest.TestCase):
    """Consume shared evidence and complete state-machine profiles."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = load_receipt_golden(ROOT)
        cls.corpus = json.loads((ROOT / REJECTION_CORPUS_PATH).read_text())

    def test_shared_evidence_reference_profile(self) -> None:
        cases = self.corpus["evidence_ref_cases"]
        self.assertEqual(len(cases), 4)
        result_index = next(
            index for index, result in enumerate(
                self.fixture["verification_receipt"]["results"])
            if result["evidence_refs"])
        template = self.fixture["verification_receipt"]["results"][
            result_index]["evidence_refs"][0]
        for case in cases:
            receipt = copy.deepcopy(self.fixture["verification_receipt"])
            receipt["results"][result_index]["evidence_refs"] = \
                _corpus_evidence_refs(template, case)
            try:
                validate_verification_receipt(receipt)
                actual = None
            except ContractError as error:
                actual = error.code
            self.assertEqual(actual, case["expected_code"], case["case_id"])

    def test_shared_state_profiles_exhaust_every_pair(self) -> None:
        cases = self.corpus["state_machines"]
        expected_counts = {"work_item": 12, "attempt": 8, "action": 9}
        self.assertEqual(len(cases), len(expected_counts))
        total_edges = 0
        for case in cases:
            states = case["states"]
            self.assertEqual(len(states), expected_counts[case["machine"]])
            self.assertEqual(len(states), len(set(states)))
            self.assertEqual(set(case["allowed_targets"]), set(states))
            allowed = _allowed_edges(case)
            total_edges += len(allowed)
            operation = _state_operation(case["machine"])
            for source in states:
                for target in states:
                    if (source, target) in allowed:
                        operation(source, target)
                    else:
                        with self.assertRaises(ContractError) as captured:
                            operation(source, target)
                        self.assertEqual(captured.exception.code,
                                         TRANSITION_INVALID)
        self.assertEqual(total_edges, 61)

    def test_state_graph_constants_are_immutable(self) -> None:
        for graph in (WORK_ITEM_EDGES, ATTEMPT_EDGES, ACTION_EDGES):
            with self.assertRaises(TypeError):
                graph[next(iter(graph))] = frozenset()  # type: ignore[index]
        with self.assertRaises(AttributeError):
            ATTEMPT_EDGES["completed"].add("running")  # type: ignore[attr-defined]

    def test_programmatic_bounds_precede_encoding_and_traversal(self) -> None:
        with self.assertRaises(ContractError):
            canonical_json("x" * (MAX_STRING_BYTES + 1), MAX_RECEIPT_BYTES)
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        receipt["results"] = [None] * (MAX_ARRAY_ITEMS + 1)
        with self.assertRaisesRegex(ContractError, "exceeds"):
            validate_document_shape(
                receipt, VERIFICATION_RECEIPT_TYPE_PROFILE,
                "VerificationReceipt")

    def test_programmatic_inputs_reject_builtin_subclasses_without_hooks(self) -> None:
        for value in (_EncodeBomb("x"), _IterationBomb(), _MappingBomb()):
            with self.subTest(kind=type(value).__name__), \
                    self.assertRaises(ContractError):
                canonical_json(value, MAX_RECEIPT_BYTES)
        with self.assertRaises(ContractError):
            validate_text(_EncodeBomb("x"), "fixture", MAX_STRING_BYTES)
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        receipt["results"] = _IterationBomb()
        with self.assertRaises(ContractError):
            validate_document_shape(
                receipt, VERIFICATION_RECEIPT_TYPE_PROFILE,
                "VerificationReceipt")
        with self.assertRaises(ContractError):
            validate_document_shape(
                _MappingBomb(), VERIFICATION_RECEIPT_TYPE_PROFILE,
                "VerificationReceipt")
        key = _KeyBomb()
        with self.assertRaises(ContractError):
            validate_document_shape(
                {key: None}, {"expected": "text"}, "fixture")
        self.assertEqual(key.hashes, 1)

    def test_state_and_extension_rejections_do_not_invoke_hooks(self) -> None:
        for source, target in (
                (_ReprBomb("requested"), "accepted"),
                ("requested", _ReprBomb("accepted"))):
            with self.assertRaises(ContractError) as captured:
                validate_attempt_transition(source, target)
            self.assertEqual(captured.exception.code, STATE_INVALID)
        command = copy.deepcopy(load_golden(ROOT)["command_envelope"])
        key = _KeyBomb()
        command["extensions"] = {key: None}
        with self.assertRaises(ContractError) as captured:
            validate_command_envelope(command)
        self.assertEqual(captured.exception.code, DOCUMENT_INVALID)
        self.assertEqual(key.hashes, 1)


def _corpus_evidence_refs(
        template: dict[str, Any], case: dict[str, Any]) -> list[dict[str, Any]]:
    values = []
    for index in range(case["count"]):
        value = copy.deepcopy(template)
        value["record_id"] = f"evidence-{index:02d}"
        values.append(value)
    if case["arrangement"] == "duplicate":
        for value in values:
            value["record_id"] = "evidence-00"
    elif case["arrangement"] == "descending":
        values.reverse()
    elif case["arrangement"] != "ascending":
        raise AssertionError(f"unknown arrangement {case['arrangement']}")
    return values


def _allowed_edges(case: dict[str, Any]) -> set[tuple[str, str]]:
    return {
        (source, target)
        for source, targets in case["allowed_targets"].items()
        for target in targets
    }


def _state_operation(machine: str):
    return {
        "work_item": validate_work_item_transition,
        "attempt": validate_attempt_transition,
        "action": validate_action_transition,
    }[machine]


if __name__ == "__main__":
    unittest.main()
