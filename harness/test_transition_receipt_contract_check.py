"""ADR-0060 strict TransitionReceipt wire and pure evaluator tests."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import transition_receipt_contract as contract
from transition_receipt_contract.assessment import assessment_sha256
from transition_receipt_contract.constants import EDGES, RESULT, STATES, TERMINAL_STATES
from transition_receipt_contract.receipt import declared_target_sha256, validate_receipt

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "docs/contracts/fixtures/transition-receipt-v1.json"
SCHEMA = ROOT / "docs/contracts/transition-receipt-v1.schema.json"
HASHES = {
    "vocabulary": "cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d",
    "receipt": "3d80d9578051338e447f674eedbb856455cd1e672247d88fbba8c51dab9bcb5d",
    "target": "8be69d5504d243bdb7fedc418c48559055d6639a33edb9aa9b4cb08c3f948d9a",
    "request": "20e3378571ef708b211ae145dbd285356a1ac05f6dae68784b71562fd95eed7f",
    "assessment": "5e4d62eedecaf2abd9c7f2030466ebc158cefbaa6f01ec21cfebd33db129eb6a",
}


class TransitionReceiptContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)

    def receipt(self):
        return copy.deepcopy(self.golden["transition_receipt"])

    def request(self):
        return copy.deepcopy(self.golden["assessment_request"])

    def assessment(self):
        return copy.deepcopy(self.golden["expected_assessment"])

    def reseal(self, receipt):
        receipt["receipt_id"] = ""
        receipt["receipt_sha256"] = ""
        return contract.seal_receipt(receipt)

    def prior_entering(self, target):
        receipt = self.receipt()
        transition = receipt["transition"]
        transition.update({"from_state": "DRAFT", "to_state": target,
                           "resume_state": None, "rework_target": None})
        if target in ("NEEDS_INFO", "BLOCKED"):
            transition["resume_state"] = "DRAFT"
        if target == "CHANGES_REQUESTED":
            transition["rework_target"] = "VERIFYING"
        receipt["applicability"]["stage_id"] = target
        return self.reseal(receipt)

    def successor(self, previous, target):
        receipt = self.receipt()
        source = previous["transition"]["to_state"]
        receipt["sequence"] = previous["sequence"] + 1
        receipt["previous_receipt_id"] = previous["receipt_id"]
        receipt["previous_receipt_sha256"] = previous["receipt_sha256"]
        transition = receipt["transition"]
        transition.update({"declared_at_unix_ms": previous["transition"][
            "declared_at_unix_ms"] + 1, "from_state": source, "to_state": target,
                           "resume_state": None, "rework_target": None})
        if target in ("NEEDS_INFO", "BLOCKED"):
            transition["resume_state"] = (previous["transition"]["resume_state"]
                                           if source == "NEEDS_INFO" and target == "BLOCKED"
                                           else source)
        if target == "CHANGES_REQUESTED":
            transition["rework_target"] = "VERIFYING"
        receipt["applicability"]["stage_id"] = target
        return self.reseal(receipt)

    def evaluated(self, receipt, previous=None, target=None, evaluated_at=None):
        expected = target if target is not None else contract.declared_target(receipt)
        instant = (evaluated_at if evaluated_at is not None else
                   receipt["transition"]["declared_at_unix_ms"])
        request = contract.seal_request(receipt, expected, previous, instant)
        return contract.evaluate_declared_assessment(request)

    def test_frozen_golden_and_all_five_hashes(self):
        self.assertEqual(self.golden["transition_vocabulary"]["vocabulary_sha256"],
                         HASHES["vocabulary"])
        self.assertEqual(self.receipt()["receipt_sha256"], HASHES["receipt"])
        self.assertEqual(self.request()["expected_target_sha256"], HASHES["target"])
        self.assertEqual(self.request()["request_sha256"], HASHES["request"])
        self.assertEqual(self.assessment()["assessment_sha256"], HASHES["assessment"])
        self.assertEqual(len(STATES), 23)
        self.assertEqual(len(EDGES), 20)

    def test_frozen_golden_schema_and_all_five_hashes(self):
        try:
            import jsonschema
        except ImportError:
            self.skipTest("jsonschema is unavailable; runtime contract checks still run")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        jsonschema.validate(self.golden, schema)
        limits = schema["x-forgeos-limits"]
        self.assertEqual(limits["runtime_string_length_unit"], "utf8_bytes")
        self.assertEqual(limits["schema_max_length_unit"], "unicode_code_points")
        self.assertTrue(limits["schema_length_keywords_are_non_authoritative_approximations"])
        malformed = copy.deepcopy(self.golden)
        malformed["assessment_request"]["expected_target"]["previous_receipt_id"] = ""
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(malformed, schema)

    def test_fixture_builder_is_byte_deterministic(self):
        from transition_receipt_contract.fixture import golden_fixture
        expected = contract.canonical_json(golden_fixture(ROOT)) + b"\n"
        self.assertEqual(FIXTURE.read_bytes(), expected)

    def test_every_authored_edge_and_every_other_edge_relation(self):
        for source in STATES:
            previous = self.prior_entering(source)
            for target in STATES:
                with self.subTest(source=source, target=target):
                    current = self.successor(previous, target)
                    relation = self.evaluated(current, previous)["relations"]["edge"]
                    dynamic = (source in ("NEEDS_INFO", "BLOCKED") and
                               target == previous["transition"]["resume_state"])
                    expected = ("listed_declared_edge" if
                                target in EDGES.get(source, ()) or dynamic
                                else "unlisted_declared_edge")
                    self.assertEqual(relation, expected)

    def test_terminal_states_have_no_outgoing_declared_edges(self):
        for source in TERMINAL_STATES:
            previous = self.prior_entering(source)
            current = self.successor(previous, "DRAFT")
            assessment = self.evaluated(current, previous)
            self.assertEqual(assessment["relations"]["edge"], "unlisted_declared_edge")
            self.assertIn("unlisted_declared_edge", assessment["reason_codes"])

    def test_initial_and_predecessor_chain_relations(self):
        self.assertEqual(self.assessment()["relations"]["chain"], "initial_declared_chain")
        previous = self.prior_entering("NEEDS_EVIDENCE")
        current = self.successor(previous, "BASELINED")
        self.assertEqual(self.evaluated(current, previous)["relations"]["chain"],
                         "same_declared_predecessor")
        current["previous_receipt_sha256"] = "a" * 64
        current["previous_receipt_id"] = "transition-receipt-" + "a" * 64
        current = self.reseal(current)
        assessment = self.evaluated(current, previous)
        self.assertEqual(assessment["relations"]["chain"], "predecessor_mismatch")

    def test_state_continuity_and_time_rollback_are_mismatches(self):
        previous = self.prior_entering("NEEDS_EVIDENCE")
        current = self.successor(previous, "BASELINED")
        current["transition"]["from_state"] = "BASELINED"
        current = self.reseal(current)
        assessment = self.evaluated(current, previous)
        self.assertEqual(assessment["relations"]["continuity"],
                         "state_continuity_mismatch")
        current = self.successor(previous, "BASELINED")
        assessment = self.evaluated(current, previous,
                                    evaluated_at=current["transition"][
                                        "declared_at_unix_ms"] - 1)
        self.assertEqual(assessment["relations"]["temporal"],
                         "temporal_declaration_mismatch")

    def test_dynamic_resume_and_rework_relations(self):
        info = self.prior_entering("NEEDS_INFO")
        blocked = self.successor(info, "BLOCKED")
        self.assertEqual(blocked["transition"]["resume_state"], "DRAFT")
        self.assertEqual(self.evaluated(blocked, info)["relations"]["recovery"],
                         "internally_consistent_declared_recovery")
        blocked["transition"]["resume_state"] = "BASELINED"
        blocked = self.reseal(blocked)
        self.assertEqual(self.evaluated(blocked, info)["relations"]["recovery"],
                         "rework_or_resume_mismatch")
        changes = self.prior_entering("CHANGES_REQUESTED")
        wrong = self.successor(changes, "DESIGNED")
        self.assertEqual(self.evaluated(wrong, changes)["relations"]["recovery"],
                         "rework_or_resume_mismatch")

    def test_fail_unknown_unlisted_and_target_mismatch_are_explanations(self):
        receipt = self.receipt()
        receipt["preconditions"][0]["result"] = "UNKNOWN"
        receipt = self.reseal(receipt)
        target = contract.declared_target(receipt)
        target["actor"]["principal_id"] = "different-agent"
        assessment = self.evaluated(receipt, target=target)
        self.assertEqual(assessment["relations"]["preconditions"],
                         "declared_fail_or_unknown_present")
        self.assertEqual(assessment["relations"]["target"], "target_mismatch")
        self.assertEqual(assessment["authorization_decision"], "none")
        self.assertFalse(assessment["transition_attestation"])

    def test_intrinsic_sequence_applicability_and_recovery_fail_closed(self):
        cases = (
            lambda value: value.update({"previous_receipt_id": "transition-receipt-" +
                                        "a" * 64, "previous_receipt_sha256": "a" * 64}),
            lambda value: value["transition"].update({"from_state": "BASELINED"}),
            lambda value: value["applicability"].update({"stage_id": "BASELINED"}),
            lambda value: value["applicability"].update({"reason_codes": ["unexpected"]}),
            lambda value: value["transition"].update({"resume_state": "DRAFT"}),
            lambda value: value["transition"].update({"rework_target": "VERIFYING"}),
        )
        for mutate in cases:
            with self.subTest(mutate=mutate):
                receipt = self.receipt()
                mutate(receipt)
                with self.assertRaises(contract.ContractError):
                    self.reseal(receipt)

    def test_not_applicable_requires_reason_and_evidence(self):
        receipt = self.receipt()
        receipt["applicability"].update({"decision": "not_applicable",
                                         "reason_codes": ["stage_not_required"]})
        with self.assertRaises(contract.ContractError):
            self.reseal(receipt)
        receipt["applicability"]["evidence_refs"] = [{
            "canonical_sha256": "b" * 64, "record_id": "evidence-na-fixture"}]
        receipt = self.reseal(receipt)
        self.assertEqual(self.evaluated(receipt)["relations"]["applicability"],
                         "internally_consistent_declared_applicability")

    def test_order_duplicates_unknown_alias_and_digest_attacks_fail_closed(self):
        receipt = self.receipt()
        receipt["approval_refs"] *= 2
        with self.assertRaises(contract.ContractError):
            self.reseal(receipt)
        request = self.request()
        request["transition_receipt"]["kind"] = "WorkflowReceipt"
        with self.assertRaisesRegex(contract.ContractError, "alias|unsupported"):
            contract.decode_request(contract.canonical_json(request))
        request = self.request()
        request["approved"] = True
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            contract.decode_request(contract.canonical_json(request))
        receipt = self.receipt()
        receipt["bindings"]["context_sha256"] = "c" * 64
        with self.assertRaisesRegex(contract.ContractError, "digest"):
            validate_receipt(receipt)

    def test_duplicate_noncanonical_numbers_and_unicode_fail_closed(self):
        raw = contract.canonical_json(self.request())
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(contract.ContractError, "duplicate"):
            contract.decode_request(duplicate)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            contract.decode_request(json.dumps(self.request()).encode())
        floating = raw.replace(b'"evaluated_at_unix_ms":1700000001000',
                               b'"evaluated_at_unix_ms":1.0')
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            contract.decode_request(floating)
        bidi = raw.replace(b'fixture-revision-1', b'fixture-\\u202erevision-1')
        with self.assertRaisesRegex(contract.ContractError, "bidi"):
            contract.decode_request(bidi)

    def test_programmatic_byte_ceiling_precedes_encoding(self):
        target = contract.declared_target(self.receipt())
        target["bindings"] = {"x": ["\\" * 16_384] * 256}
        with mock.patch("transition_receipt_contract.canonical.json.dumps",
                        side_effect=AssertionError("oversized value reached encoder")):
            with self.assertRaisesRegex(contract.ContractError, "byte ceiling"):
                declared_target_sha256(target)

    def test_closed_shape_rejects_oversized_key_sets_without_iterating_them(self):
        class IterationTrap(dict):
            def __iter__(self):
                raise AssertionError("oversized caller key set was copied")

        receipt = IterationTrap(self.receipt())
        receipt["unexpected"] = None
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            validate_receipt(receipt)
        golden = IterationTrap(copy.deepcopy(self.golden))
        golden["unexpected"] = None
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            contract.validate_golden(golden)

    def test_sealing_validates_deep_programmatic_input_before_copying(self):
        nested = {}
        for _ in range(700):
            nested = {"child": nested}
        receipt = self.receipt()
        receipt["bindings"] = nested
        with self.assertRaisesRegex(contract.ContractError, "depth|fields"):
            contract.seal_receipt(receipt)
        target = contract.declared_target(self.receipt())
        target["bindings"] = nested
        with self.assertRaisesRegex(contract.ContractError, "depth|fields"):
            contract.seal_request(self.receipt(), target, None, 1_700_000_001_000)

    def test_authority_escalation_and_assessment_drift_fail_closed(self):
        cases = (("authorization_decision", "authorized"),
                 ("policy_decision", "allow"), ("grant_state", "active"),
                 ("transition_attestation", True), ("completion_attestation", True),
                 ("result", "TRANSITIONED"))
        for field, value in cases:
            assessment = self.assessment()
            assessment[field] = value
            assessment["assessment_sha256"] = assessment_sha256(assessment)
            with self.assertRaises(contract.ContractError):
                contract.validate_assessment(self.request(), assessment)
        assessment = self.assessment()
        assessment["relations"]["edge"] = "unlisted_declared_edge"
        assessment["reason_codes"] = ["unlisted_declared_edge"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "reassembly"):
            contract.validate_assessment(self.request(), assessment)
        assessment = self.assessment()
        assessment["relations"]["applicability"] = "applicability_declaration_mismatch"
        assessment["reason_codes"] = ["applicability_declaration_mismatch"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "relations"):
            contract.decode_assessment(contract.canonical_json(assessment))

    def test_local_markers_are_not_transition_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "docs/contracts/fixtures/transition-receipt-v1.json"
            target.parent.mkdir(parents=True)
            target.write_bytes(FIXTURE.read_bytes())
            marker = root / ".forge/change.transitioned"
            marker.parent.mkdir()
            marker.write_text("CLOSED by caller\n")
            loaded = contract.load_golden(root)
        self.assertEqual(loaded["expected_assessment"]["result"], RESULT)
        self.assertFalse(loaded["expected_assessment"]["transition_attestation"])

    def test_cli_golden_and_instance_modes(self):
        golden = subprocess.run([sys.executable, "-B", "harness/transition_receipt_contract_check.py",
                                 "--golden", str(ROOT)], cwd=ROOT,
                                capture_output=True, text=True)
        self.assertEqual(golden.returncode, 0, golden.stderr)
        with tempfile.TemporaryDirectory() as directory:
            request_path = Path(directory) / "request.json"
            assessment_path = Path(directory) / "assessment.json"
            request_path.write_bytes(contract.canonical_json(self.request()))
            assessment_path.write_bytes(contract.canonical_json(self.assessment()))
            command = [sys.executable, "-B", "harness/transition_receipt_contract_check.py",
                       str(ROOT), str(request_path), str(assessment_path)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
