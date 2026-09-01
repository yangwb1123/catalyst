"""Independent Platform Core receipt/state conformance tests."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "harness"
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from platform_core_contract import (  # noqa: E402
    RECEIPT_SUCCESS_MARKER, ContractError, canonical_execution_receipt,
    canonical_verification_receipt, canonical_verification_request,
    decode_artifact_ref, decode_command_envelope, decode_event_envelope,
    decode_execution_receipt, decode_verification_receipt,
    decode_verification_request, execution_receipt_sha256,
    load_golden, load_receipt_golden, validate_action_transition,
    validate_attempt_transition, validate_execution_receipt,
    validate_verification_exchange, validate_verification_receipt,
    validate_verification_request, validate_work_item_transition,
    verification_receipt_sha256, verification_request_sha256)
from platform_core_contract.codec import canonical_json  # noqa: E402
from platform_core_contract.constants import *  # noqa: E402,F403

CHECKER = HARNESS / "platform_core_contract/check.py"


class _FlipOnRead(dict):
    """Expose one different field value after a configured number of reads."""

    def __init__(self, source: dict, field: str, replacement: Any,
                 flip_after: int) -> None:
        super().__init__(source)
        self.field = field
        self.replacement = replacement
        self.flip_after = flip_after
        self.reads = 0

    def __getitem__(self, key):
        value = super().__getitem__(key)
        if key == self.field:
            self.reads += 1
            if self.reads >= self.flip_after:
                return self.replacement
        return value


class PlatformCoreReceiptGoldenTests(unittest.TestCase):
    """Shared golden and independent checker parity."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = load_receipt_golden(ROOT)

    def test_digests_and_round_trips_match(self) -> None:
        expected = self.fixture["expected"]
        execution = self.fixture["execution_receipt"]
        request = self.fixture["verification_request"]
        receipt = self.fixture["verification_receipt"]
        self.assertEqual(execution_receipt_sha256(execution),
                         expected["execution_receipt_sha256"])
        self.assertEqual(verification_request_sha256(request),
                         expected["verification_request_sha256"])
        self.assertEqual(verification_receipt_sha256(receipt),
                         expected["verification_receipt_sha256"])
        self.assertEqual(decode_execution_receipt(
            canonical_execution_receipt(execution)), execution)
        self.assertEqual(decode_verification_request(
            canonical_verification_request(request)), request)
        self.assertEqual(decode_verification_receipt(
            canonical_verification_receipt(receipt)), receipt)
        validate_verification_exchange(request, receipt)

    def test_receipt_checker_has_exact_non_authority_marker(self) -> None:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER),
             "--receipt-golden", str(ROOT)],
            cwd=ROOT, text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, RECEIPT_SUCCESS_MARKER + "\n")

    def test_schema_shadow_accepts_golden_and_rejects_shape_drift(self) -> None:
        try:
            import jsonschema
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"jsonschema unavailable: {error}")
        envelope = json.loads((ROOT / SCHEMA_PATH).read_text())  # noqa: F405
        schema = json.loads((ROOT / RECEIPT_SCHEMA_PATH).read_text())  # noqa: F405
        registry = Registry().with_resource(
            envelope["$id"], Resource.from_contents(envelope))
        validator = jsonschema.Draft202012Validator(schema, registry=registry)
        for field in ("execution_receipt", "verification_request",
                      "verification_receipt"):
            validator.validate(self.fixture[field])
        invalid = copy.deepcopy(self.fixture["execution_receipt"])
        invalid["terminal_state"] = "running"
        with self.assertRaises(jsonschema.ValidationError):
            validator.validate(invalid)
        invalid = copy.deepcopy(self.fixture["verification_receipt"])
        invalid["produced_by"]["actor_type"] = "service"
        with self.assertRaises(jsonschema.ValidationError):
            validator.validate(invalid)


class PlatformCoreStateAndRejectionTests(unittest.TestCase):
    """Frozen edges and shared machine-readable rejection classes."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = load_receipt_golden(ROOT)
        envelope = load_golden(ROOT)
        for field in ("artifact_ref", "command_envelope", "event_envelope"):
            cls.fixture[field] = envelope[field]
        cls.corpus = json.loads((ROOT / REJECTION_CORPUS_PATH).read_text())  # noqa: F405

    def test_declared_state_edges_accept(self) -> None:
        validate_work_item_transition("verifying", "completed")
        validate_attempt_transition("running", "completed")
        validate_action_transition("started", "finished")
        with self.assertRaises(ContractError) as captured:
            validate_attempt_transition("unknown", "running")
        self.assertEqual(captured.exception.code, STATE_INVALID)  # noqa: F405

    def test_shared_rejection_corpus(self) -> None:
        role_cases = self.corpus["executor_role_cases"]
        self.assertEqual(len(role_cases), 3)
        self.assertEqual(
            {case["actor_type"] for case in role_cases},
            {"agent", "service", "system"})
        for case in role_cases:
            receipt = copy.deepcopy(self.fixture["execution_receipt"])
            receipt["executor"]["actor_ref"]["actor_type"] = case["actor_type"]
            validate_execution_receipt(receipt)
        self.assertEqual(len(self.corpus["wire_cases"]), 62)
        self.assertEqual(len(self.corpus["transition_cases"]), 9)
        covered = {
            case["expected_code"]
            for group in ("wire_cases", "transition_cases")
            for case in self.corpus[group]
        }
        self.assertEqual(covered, {
            DOCUMENT_INVALID, IDENTIFIER_INVALID, VALUE_INVALID,  # noqa: F405
            REFERENCE_MISMATCH, STATE_INVALID, TRANSITION_INVALID,  # noqa: F405
            RELATION_MISMATCH,  # noqa: F405
        })
        for case in self.corpus["wire_cases"]:
            with self.subTest(case=case["case_id"]):
                target = ("verification_receipt" if case["target"] ==
                          "verification_exchange" else case["target"])
                value = copy.deepcopy(self.fixture[target])
                for mutation in case["mutations"]:
                    _apply_mutation(value, mutation)
                error = self._reject_wire(case["target"], value)
                self.assertEqual(error.code, case["expected_code"], str(error))
        for case in self.corpus["transition_cases"]:
            with self.subTest(case=case["case_id"]):
                operation = {
                    "work_item": validate_work_item_transition,
                    "attempt": validate_attempt_transition,
                    "action": validate_action_transition,
                }[case["machine"]]
                with self.assertRaises(ContractError) as captured:
                    operation(case["source"], case["target"])
                self.assertEqual(captured.exception.code, case["expected_code"])

    def _reject_wire(self, target: str, value: dict[str, Any]) -> ContractError:
        raw = canonical_json(value, MAX_RECEIPT_BYTES)  # noqa: F405
        decoder = {
            "execution_receipt": decode_execution_receipt,
            "verification_request": decode_verification_request,
            "verification_receipt": decode_verification_receipt,
            "verification_exchange": decode_verification_receipt,
            "event_envelope": decode_event_envelope,
            "artifact_ref": decode_artifact_ref,
            "command_envelope": decode_command_envelope,
        }[target]
        try:
            decoded = decoder(raw)
            if target == "verification_exchange":
                validate_verification_exchange(
                    self.fixture["verification_request"], decoded)
        except ContractError as error:
            return error
        self.fail(f"{target} unexpectedly accepted")


class PlatformCoreReceiptSemanticTests(unittest.TestCase):
    """Receipt timing, scope, outcome, and non-authority relations."""

    def setUp(self) -> None:
        self.fixture = load_receipt_golden(ROOT)

    def test_execution_relations_fail_closed(self) -> None:
        mutations = (
            lambda value: value["session_ref"].__setitem__(
                "entity_id", "ses_0000000000000000000000000m"),
            lambda value: value["observed_usage"].__setitem__("elapsed_ms", 3999),
            lambda value: value.__setitem__("terminal_state", "running"),
            lambda value: value["output_artifact_refs"][0].__setitem__(
                "producer_attempt_id", "atm_0000000000000000000000000m"),
            lambda value: value["event_range"].__setitem__("last_sequence", 8),
        )
        for mutate in mutations:
            value = copy.deepcopy(self.fixture["execution_receipt"])
            mutate(value)
            with self.assertRaises(ContractError):
                validate_execution_receipt(value)

    def test_programmatic_validation_uses_staged_rejections(self) -> None:
        value = copy.deepcopy(self.fixture["execution_receipt"])
        value["terminal_state"] = "running"
        value["attempt_ref"]["entity_id"] = "atm_0000000000000000000000000m"
        with self.assertRaises(ContractError) as captured:
            validate_execution_receipt(value)
        self.assertEqual(captured.exception.code, REFERENCE_MISMATCH)  # noqa: F405

        value = copy.deepcopy(self.fixture["execution_receipt"])
        value["observed_usage"]["input_tokens"] = -1
        value["observed_usage"]["elapsed_ms"] = 3999
        with self.assertRaises(ContractError) as captured:
            validate_execution_receipt(value)
        self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405

    def test_exchange_invalid_roots_are_document_rejections(self) -> None:
        request = self.fixture["verification_request"]
        receipt = self.fixture["verification_receipt"]
        cases = ((None, receipt), ([], receipt),
                 (request, None), (request, []))
        for request_value, receipt_value in cases:
            with self.subTest(request=type(request_value).__name__,
                              receipt=type(receipt_value).__name__), \
                    self.assertRaises(ContractError) as captured:
                validate_verification_exchange(request_value, receipt_value)
            self.assertEqual(captured.exception.code, DOCUMENT_INVALID)  # noqa: F405

    def test_verification_status_is_strictly_derived(self) -> None:
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        receipt["results"][1]["status"] = "fail"
        receipt["results"][1]["reason_codes"] = ["test_failed"]
        receipt["overall_status"] = "fail"
        validate_verification_receipt(receipt)
        receipt["overall_status"] = "pass"
        with self.assertRaises(ContractError) as captured:
            validate_verification_receipt(receipt)
        self.assertEqual(captured.exception.code, RELATION_MISMATCH)  # noqa: F405

    def test_all_not_applicable_derives_not_executed(self) -> None:
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        receipt["results"] = [receipt["results"][0]]
        receipt["overall_status"] = "not_executed"
        validate_verification_receipt(receipt)
        receipt["overall_status"] = "pass"
        with self.assertRaises(ContractError):
            validate_verification_receipt(receipt)

    def test_exchange_requires_exact_request_and_result_set(self) -> None:
        request = copy.deepcopy(self.fixture["verification_request"])
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        receipt["results"] = receipt["results"][:-1]
        receipt["overall_status"] = "not_executed"
        validate_verification_receipt(receipt)
        with self.assertRaises(ContractError) as captured:
            validate_verification_exchange(request, receipt)
        self.assertEqual(captured.exception.code, RELATION_MISMATCH)  # noqa: F405

    def test_exchange_uses_global_stage_order(self) -> None:
        request = copy.deepcopy(self.fixture["verification_request"])
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        request["input_artifact_ref"]["content_id"] = "sha256:" + "0" * 64
        receipt["overall_status"] = "unknown"
        with self.assertRaises(ContractError) as captured:
            validate_verification_exchange(request, receipt)
        self.assertEqual(captured.exception.code, STATE_INVALID)  # noqa: F405


class PlatformCoreReceiptBoundaryTests(unittest.TestCase):
    """Exact collection, framing, and writer snapshot boundaries."""

    def setUp(self) -> None:
        self.fixture = load_receipt_golden(ROOT)

    def test_check_count_boundary(self) -> None:
        request = copy.deepcopy(self.fixture["verification_request"])
        request["checks"] = [
            {"check_id": f"check_{index:02d}", "check_name": "forge.check.test",
             "declared_required": True}
            for index in range(MAX_VERIFICATION_CHECKS)  # noqa: F405
        ]
        validate_verification_request(request)
        request["checks"].append(
            {"check_id": "check_extra", "check_name": "forge.check.test",
             "declared_required": True})
        with self.assertRaises(ContractError):
            validate_verification_request(request)

    def test_artifact_count_boundary(self) -> None:
        receipt = copy.deepcopy(self.fixture["execution_receipt"])
        source = receipt["output_artifact_refs"][0]
        receipt["output_artifact_refs"] = [
            _numbered_artifact(source, index) for index in range(MAX_RECEIPT_ARTIFACTS)  # noqa: F405
        ]
        validate_execution_receipt(receipt)
        receipt["output_artifact_refs"].append(
            _numbered_artifact(source, MAX_RECEIPT_ARTIFACTS))  # noqa: F405
        with self.assertRaises(ContractError):
            validate_execution_receipt(receipt)

    def test_reason_count_boundary(self) -> None:
        receipt = copy.deepcopy(self.fixture["execution_receipt"])
        receipt["terminal_state"] = "failed"
        receipt["reason_codes"] = [
            f"reason_{index:02d}" for index in range(MAX_REASON_CODES)  # noqa: F405
        ]
        validate_execution_receipt(receipt)
        receipt["reason_codes"].append("reason_extra")
        with self.assertRaises(ContractError) as captured:
            validate_execution_receipt(receipt)
        self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405

    def test_verification_whole_document_bound_precedes_state(self) -> None:
        receipt = _oversized_verification_receipt(self.fixture)
        receipt["overall_status"] = "unknown"
        with self.assertRaises(ContractError) as captured:
            validate_verification_receipt(receipt)
        self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405

    def test_exchange_bounds_both_documents_before_values(self) -> None:
        request = copy.deepcopy(self.fixture["verification_request"])
        request["verification_id"] = "atm_0000000000000000000000000g"
        receipt = _oversized_verification_receipt(self.fixture)
        with self.assertRaises(ContractError) as captured:
            validate_verification_exchange(request, receipt)
        self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405

    def test_decoders_require_exact_immutable_bytes(self) -> None:
        cases = (
            (decode_execution_receipt, canonical_execution_receipt(
                self.fixture["execution_receipt"])),
            (decode_verification_request, canonical_verification_request(
                self.fixture["verification_request"])),
            (decode_verification_receipt, canonical_verification_receipt(
                self.fixture["verification_receipt"])),
        )
        for decoder, raw in cases:
            for malformed in (bytearray(raw), memoryview(raw), raw.decode(), True):
                with self.subTest(decoder=decoder.__name__, kind=type(malformed).__name__), \
                        self.assertRaises(ContractError):
                    decoder(malformed)

    def test_writers_round_trip_builtins_and_reject_mapping_subclasses(self) -> None:
        cases = (
            ("execution_receipt", canonical_execution_receipt,
             decode_execution_receipt, "execution_receipt_version", 2),
            ("verification_request", canonical_verification_request,
             decode_verification_request, "verification_request_version", 2),
            ("verification_receipt", canonical_verification_receipt,
             decode_verification_receipt, "verification_receipt_version", 2),
        )
        for field, writer, decoder, changing_field, replacement in cases:
            source = copy.deepcopy(self.fixture[field])
            expected = writer(source)
            with self.subTest(field=field):
                canonical = writer(source)
                self.assertEqual(canonical, expected)
                decoder(canonical)
                with self.assertRaises(ContractError) as captured:
                    writer(_FlipOnRead(
                        source, changing_field, replacement, 2))
                self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405

    def test_exchange_accepts_builtins_and_rejects_mapping_subclasses(self) -> None:
        source = copy.deepcopy(self.fixture["verification_request"])
        receipt = copy.deepcopy(self.fixture["verification_receipt"])
        replacement = source["input_artifact_ref"]["created_at_unix_ms"] - 1
        validate_verification_exchange(source, receipt)
        with self.assertRaises(ContractError) as captured:
            validate_verification_exchange(
                _FlipOnRead(source, "requested_at_unix_ms", replacement, 2),
                receipt)
        self.assertEqual(captured.exception.code, DOCUMENT_INVALID)  # noqa: F405


def _apply_mutation(root: Any, mutation: dict[str, Any]) -> None:
    parts = mutation["pointer"].removeprefix("/").split("/")
    current = root
    for part in parts[:-1]:
        current = current[int(part)] if isinstance(current, list) else current[part]
    last = parts[-1]
    if isinstance(current, list):
        if mutation["operation"] != "replace":
            raise AssertionError("array mutation must replace")
        current[int(last)] = copy.deepcopy(mutation["value"])
    elif mutation["operation"] == "remove":
        del current[last]
    elif mutation["operation"] == "replace":
        current[last] = copy.deepcopy(mutation["value"])
    else:
        raise AssertionError(f"unsupported mutation {mutation['operation']}")


def _numbered_artifact(source: dict[str, Any], number: int) -> dict[str, Any]:
    alphabet = "0123456789abcdefghjkmnpqrstvwxyz"
    high, low = divmod(number, len(alphabet))
    value = copy.deepcopy(source)
    value["logical_id"] = "art_" + "0" * 24 + alphabet[high] + alphabet[low]
    return value


def _oversized_verification_receipt(fixture: dict[str, Any]) -> dict[str, Any]:
    receipt = copy.deepcopy(fixture["verification_receipt"])
    record_type = ".".join(("a" * 31, "b" * 31, "c" * 31, "d" * 32))
    evidence = []
    for index in range(MAX_EVIDENCE_REFS):  # noqa: F405
        prefix = f"evidence_{index:04d}_"
        evidence.append({
            "record_id": prefix + "a" * (160 - len(prefix)),
            "record_sha256": "a" * 64, "record_type": record_type,
        })
    receipt["results"] = [{
        "applicability": "applicable", "applicability_reason": None,
        "check_id": f"check_{index:02d}", "evidence_refs": evidence,
        "reason_codes": [], "status": "pass",
    } for index in range(MAX_VERIFICATION_CHECKS)]  # noqa: F405
    return receipt


if __name__ == "__main__":
    unittest.main()
