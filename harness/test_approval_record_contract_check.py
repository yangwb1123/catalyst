"""Adversarial tests for the ADR-0059 authority-neutral ApprovalRecord contract."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

try:
    import jsonschema
except ModuleNotFoundError:  # Universal scaffold has no third-party Python dependency.
    jsonschema = None

import approval_record_contract as contract
from approval_record_contract.assessment import (assessment_sha256,
                                                   request_sha256,
                                                   validate_request)
from approval_record_contract.canonical import decode_canonical
from approval_record_contract.constants import MAX_REQUEST_BYTES, RESULT
from approval_record_contract.contract import FIXTURE
from approval_record_contract.fixture import golden_fixture
from approval_record_contract.record import (declared_target_sha256,
                                               validate_record)

ROOT = Path(__file__).resolve().parents[1]


class ApprovalRecordContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)

    def request(self):
        return copy.deepcopy(self.golden["assessment_request"])

    def record(self):
        return copy.deepcopy(self.golden["approval_record"])

    def assessment(self):
        return copy.deepcopy(self.golden["expected_assessment"])

    def resign_request(self, request):
        request["request_sha256"] = request_sha256(request)

    def reseal_record(self, record):
        record["approval_id"] = ""
        record["approval_sha256"] = ""
        return contract.seal_record(record)

    def test_golden_digests_and_result_are_frozen(self):
        request = self.request()
        assessment = contract.evaluate_declared_assessment(request)
        self.assertEqual(golden_fixture(), self.golden)
        self.assertEqual(assessment, self.assessment())
        self.assertEqual(request["approval_record"]["approval_sha256"],
                         "a2c47ec0c9242d9088532ce58140643a11b3a28f43836134ed36c2c9e2ca09d4")
        self.assertEqual(request["expected_target_sha256"],
                         "8402062537970279a1a2cff83913131656e9da341c593918281742850c646f6c")
        self.assertEqual(request["request_sha256"],
                         "c90f6108ade8e9066e907bb09a4d5b7ace848e0b9da3be9ee718ccfbc39d9f33")
        self.assertEqual(assessment["assessment_sha256"],
                         "1719084506446d2979d4294e53f3a4541200b35d6ac103660b2861df75f786d4")
        self.assertEqual(assessment["result"], RESULT)

    def test_positive_assessment_never_claims_authority(self):
        assessment = self.assessment()
        states = ("approver_identity_state", "authority_proof_state",
                  "condition_satisfaction_state", "effective_approval_state",
                  "revocation_registry_state", "risk_acceptance_state",
                  "separation_of_duty_proof_state")
        self.assertTrue(all(assessment[field] == "not_evaluated" for field in states))
        self.assertEqual(assessment["authorization_decision"], "none")
        self.assertEqual(assessment["policy_decision"], "none")
        self.assertFalse(assessment["permission_attestation"])
        self.assertFalse(assessment["effect_attestation"])
        self.assertFalse(assessment["persistence_attestation"])
        self.assertFalse(assessment["transition_attestation"])

    @unittest.skipIf(jsonschema is None, "jsonschema is unavailable in this scaffold")
    def test_schema_accepts_the_authoritative_fixture(self):
        schema = json.loads((ROOT / "docs/contracts/approval-record-v1.schema.json").read_text())
        jsonschema.Draft202012Validator.check_schema(schema)
        jsonschema.Draft202012Validator(schema).validate(self.golden)

    @unittest.skipIf(jsonschema is None, "jsonschema is unavailable in this scaffold")
    def test_schema_rejects_terminal_lf_in_proof_and_reason(self):
        schema = json.loads((ROOT / "docs/contracts/approval-record-v1.schema.json").read_text())
        validator = jsonschema.Draft202012Validator(schema)
        for path in ("proof", "reason"):
            with self.subTest(path=path):
                fixture = copy.deepcopy(self.golden)
                records = (fixture["approval_record"],
                           fixture["assessment_request"]["approval_record"])
                for record in records:
                    if path == "proof":
                        record["authority_proof"]["proof_base64url"] += "\n"
                    else:
                        record["decision_basis"]["reason_codes"][0] += "\n"
                self.assertTrue(list(validator.iter_errors(fixture)))

    def test_proof_bytes_do_not_mint_identity_but_request_binds_them(self):
        record = self.record()
        original_record_digest = record["approval_sha256"]
        record["authority_proof"]["proof_base64url"] = "ZGlmZmVyZW50LWF1dGhvcml0eS1wcm9vZg"
        record["separation_of_duty"]["proof_base64url"] = "ZGlmZmVyZW50LXNvZC1wcm9vZg"
        validate_record(record)
        self.assertEqual(contract.approval_sha256(record), original_record_digest)
        request = self.request()
        original_request_digest = request["request_sha256"]
        request["approval_record"] = record
        self.resign_request(request)
        validate_request(request)
        self.assertNotEqual(request["request_sha256"], original_request_digest)

    def test_every_nonproof_record_change_breaks_record_identity(self):
        record = self.record()
        record["bindings"]["context_sha256"] = "d" * 64
        with self.assertRaisesRegex(contract.ContractError, "digest"):
            validate_record(record)
        resealed = self.reseal_record(record)
        self.assertNotEqual(resealed["approval_sha256"],
                            self.golden["approval_record"]["approval_sha256"])

    def test_all_declared_target_mismatches_are_deterministic(self):
        mutations = {
            "approver": lambda target: target["approver"].update({"principal_id": "other"}),
            "authority_binding": lambda target: target["authority_binding"].update(
                {"trust_epoch": 2}),
            "binding": lambda target: target["bindings"].update({"context_sha256": "d" * 64}),
            "conditions": lambda target: target["conditions"][0].update(
                {"condition_id": "other-condition"}),
            "decision": lambda target: target.update({"decision": "reject"}),
            "risk_acceptance": lambda target: target["risk_acceptance_refs"][0].update(
                {"risk_acceptance_id": "other-risk"}),
            "scope": lambda target: target["scope"].update({"change_id": "other-change"}),
            "separation_of_duty": lambda target: target[
                "separation_of_duty_declaration"]["requester"].update(
                    {"principal_id": "other-requester"}),
            "subject": lambda target: target["subject"].update({"principal_id": "other"}),
        }
        for relation, mutate in mutations.items():
            with self.subTest(relation=relation):
                request = self.request()
                mutate(request["expected_target"])
                request["expected_target_sha256"] = declared_target_sha256(
                    request["expected_target"])
                self.resign_request(request)
                assessment = contract.evaluate_declared_assessment(request)
                self.assertTrue(assessment["relations"][relation].endswith("mismatch"))
                self.assertIn(assessment["relations"][relation], assessment["reason_codes"])

    def test_temporal_and_declared_revocation_relations_are_not_validation(self):
        request = self.request()
        request["evaluated_at_unix_ms"] = request["approval_record"]["validity"][
            "expires_at_unix_ms"]
        self.resign_request(request)
        assessment = contract.evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["temporal"], "outside_declared_window")
        self.assertIn("temporal_window_mismatch", assessment["reason_codes"])
        request = self.request()
        record = request["approval_record"]
        record["validity"]["revoked_at_unix_ms"] = request["evaluated_at_unix_ms"] - 1
        request["approval_record"] = self.reseal_record(record)
        self.resign_request(request)
        assessment = contract.evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["revocation"],
                         "declared_revocation_time_reached")
        self.assertEqual(assessment["revocation_registry_state"], "not_evaluated")

    def test_authority_escalation_is_rejected(self):
        cases = (("authorization_decision", "authorized"), ("policy_decision", "allow"),
                 ("effective_approval_state", "active"),
                 ("authority_proof_state", "verified"),
                 ("permission_attestation", True), ("effect_attestation", True),
                 ("persistence_attestation", True), ("transition_attestation", True),
                 ("result", "APPROVED"))
        for field, value in cases:
            with self.subTest(field=field):
                assessment = self.assessment()
                assessment[field] = value
                assessment["assessment_sha256"] = assessment_sha256(assessment)
                with self.assertRaises(contract.ContractError):
                    contract.validate_assessment(self.request(), assessment)

    def test_record_semantic_bounds_fail_closed(self):
        def kernel_production(record):
            source = record["authority_proof"]["authority_source"]
            source.update({"authority_class": "forgeos_kernel",
                           "principal_id": "kernel", "principal_type": "service"})

        cases = (
            lambda record: record["validity"].update({"transferable": True}),
            lambda record: record["validity"].update(
                {"expires_at_unix_ms": record["validity"]["issued_at_unix_ms"] + 86_400_001}),
            lambda record: record["scope"].update({"scope_type": "gate", "gate_id": "G8"}),
            lambda record: record["separation_of_duty"].update({"implementers": []}),
            lambda record: record["separation_of_duty"].update(
                {"requester": copy.deepcopy(record["approver"])}),
            lambda record: record["bindings"].update(
                {"artifacts": record["bindings"]["artifacts"] * 2}),
            kernel_production,
        )
        for mutate in cases:
            with self.subTest(mutation=mutate):
                record = self.record()
                mutate(record)
                with self.assertRaises(contract.ContractError):
                    self.reseal_record(record)

    def test_expected_target_internal_contradictions_fail_closed(self):
        def same_requester(target):
            target["separation_of_duty_declaration"]["requester"] = copy.deepcopy(
                target["approver"])

        def missing_l3_implementer(target):
            target["separation_of_duty_declaration"]["implementers"] = []

        def missing_risk_distinction(target):
            target["scope"]["materiality_level"] = "L2"
            target["separation_of_duty_declaration"]["required_distinctions"] = [
                "approver_not_implementer", "approver_not_subject"]

        def kernel_production(target):
            source = target["authority_binding"]["authority_source"]
            source.update({"authority_class": "forgeos_kernel",
                           "principal_id": "kernel", "principal_type": "service"})

        for mutate in (same_requester, missing_l3_implementer,
                       missing_risk_distinction, kernel_production):
            with self.subTest(mutation=mutate):
                request = self.request()
                mutate(request["expected_target"])
                with self.assertRaises(contract.ContractError):
                    declared_target_sha256(request["expected_target"])
                with self.assertRaises(contract.ContractError):
                    validate_request(request)

    def test_alias_unknown_duplicate_and_noncanonical_bytes_are_rejected(self):
        request = self.request()
        request["approval_record"]["kind"] = "HumanApproval"
        with self.assertRaisesRegex(contract.ContractError, "alias|unsupported"):
            contract.decode_request(contract.canonical_json(request))
        request = self.request()
        request["approved"] = True
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            contract.decode_request(contract.canonical_json(request))
        raw = contract.canonical_json(self.request())
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(contract.ContractError, "duplicate"):
            contract.decode_request(duplicate)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            contract.decode_request(json.dumps(self.request()).encode())

    def test_number_unicode_depth_and_size_boundaries_are_rejected(self):
        raw = contract.canonical_json(self.request())
        floating = raw.replace(b'"evaluated_at_unix_ms":1700000001000',
                               b'"evaluated_at_unix_ms":1.0')
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            contract.decode_request(floating)
        bidi = raw.replace(b'fixture-revision-0059', b'fixture-\\u202erevision-0059')
        with self.assertRaisesRegex(contract.ContractError, "bidi"):
            contract.decode_request(bidi)
        deep = b"[" * 2000 + b"0" + b"]" * 2000
        with self.assertRaises(contract.ContractError):
            decode_canonical(deep, MAX_REQUEST_BYTES, "deep request")
        with self.assertRaisesRegex(contract.ContractError, "byte length"):
            decode_canonical(b" " * (MAX_REQUEST_BYTES + 1), MAX_REQUEST_BYTES, "request")

    def test_programmatic_document_ceiling_is_measured_before_encoding(self):
        target = contract.declared_target(self.record())
        target["bindings"] = {"x": ["\\" * 16_384] * 256}
        with mock.patch("approval_record_contract.canonical.json.dumps",
                        side_effect=AssertionError("oversized value reached encoder")):
            with self.assertRaisesRegex(contract.ContractError, "byte ceiling"):
                declared_target_sha256(target)

    def test_approval_ref_projection_and_relation_are_declarations_only(self):
        record = self.record()
        reference = contract.approval_ref(record)
        self.assertEqual(reference, self.golden["expected_approval_ref"])
        self.assertEqual(contract.approval_ref_relation(record, reference),
                         "same_declared_reference")
        mismatch = copy.deepcopy(reference)
        mismatch["approval_sha256"] = "d" * 64
        mismatch["approval_id"] = f"approval-record-{mismatch['approval_sha256']}"
        self.assertEqual(contract.approval_ref_relation(record, mismatch),
                         "reference_mismatch")
        self.assertNotIn("authorized", contract.approval_ref_relation(record, reference))

    def test_local_approved_marker_is_never_imported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = root / "docs/contracts/fixtures/approval-record-v1.json"
            fixture.parent.mkdir(parents=True)
            fixture.write_bytes((ROOT / FIXTURE).read_bytes())
            marker = root / ".forge/change.approved"
            marker.parent.mkdir()
            marker.write_text("APPROVED by caller\n")
            loaded = contract.load_golden(root)
        self.assertEqual(loaded, self.golden)
        self.assertEqual(loaded["expected_assessment"]["effective_approval_state"],
                         "not_evaluated")

    def test_assessment_mutation_fails_exact_reassembly(self):
        assessment = self.assessment()
        assessment["relations"]["scope"] = "scope_mismatch"
        assessment["reason_codes"] = ["scope_mismatch"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "reassembly"):
            contract.validate_assessment(self.request(), assessment)

    def test_cli_golden_and_instance_modes(self):
        golden = subprocess.run([sys.executable, "-B", "harness/approval_record_contract_check.py",
                                 "--golden", str(ROOT)], cwd=ROOT,
                                capture_output=True, text=True)
        self.assertEqual(golden.returncode, 0, golden.stderr)
        with tempfile.TemporaryDirectory() as directory:
            request_path = Path(directory) / "request.json"
            assessment_path = Path(directory) / "assessment.json"
            request_path.write_bytes(contract.canonical_json(self.request()))
            assessment_path.write_bytes(contract.canonical_json(self.assessment()))
            command = [sys.executable, "-B", "harness/approval_record_contract_check.py",
                       str(ROOT), str(request_path), str(assessment_path)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
