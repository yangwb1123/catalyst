"""Adversarial tests for the ADR-0056 authority-neutral reference contract."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

try:
    import jsonschema
except ModuleNotFoundError:  # Universal scaffold has no third-party Python dependency.
    jsonschema = None

import capability_grant_contract as contract
from capability_grant_contract.assessment import (assessment_sha256,
                                                   evaluate_declared_assessment,
                                                   request_sha256)
from capability_grant_contract.canonical import decode_canonical, digest
from capability_grant_contract.constants import (MAX_REQUEST_BYTES, RESULT, VOCABULARY_DOMAIN,
                                                  VOCABULARY_SHA256)
from capability_grant_contract.contract import decode_request
from capability_grant_contract.grant import grant_sha256
from capability_grant_contract.vocabulary import validate_vocabulary

ROOT = Path(__file__).resolve().parents[1]


class CapabilityGrantContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)

    def request(self):
        return copy.deepcopy(self.golden["assessment_request"])

    def assessment(self):
        return copy.deepcopy(self.golden["expected_assessment"])

    def resign_request(self, request):
        request["request_sha256"] = request_sha256(request)

    def resign_grant(self, grant):
        value = grant_sha256(grant)
        grant["grant_sha256"] = value
        grant["grant_id"] = f"capability-grant-{value}"

    def test_golden_fixture_is_assessed_declarations_only(self):
        request = self.request()
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment, self.assessment())
        self.assertEqual(assessment["result"], RESULT)
        self.assertEqual(assessment["authorization_decision"], "none")
        self.assertFalse(assessment["permission_attestation"])
        self.assertFalse(assessment["effect_attestation"])
        self.assertEqual(len(self.golden["effect_vocabulary"]["effects"]), 21)

    def test_authority_escalation_is_rejected(self):
        for field, value in (("authorization_decision", "authorized"),
                             ("permission_attestation", True), ("effect_attestation", True),
                             ("authority_proof_state", "verified"),
                             ("result", "AUTHORIZED")):
            with self.subTest(field=field):
                assessment = self.assessment()
                assessment[field] = value
                assessment["assessment_sha256"] = assessment_sha256(assessment)
                with self.assertRaises(contract.ContractError):
                    contract.validate_assessment(self.request(), assessment)

    @unittest.skipIf(jsonschema is None, "jsonschema is unavailable in this scaffold")
    def test_schema_accepts_the_authoritative_fixture(self):
        schema = json.loads((ROOT / "docs/contracts/capability-grant-v1.schema.json").read_text())
        jsonschema.Draft202012Validator.check_schema(schema)
        jsonschema.Draft202012Validator(schema).validate(self.golden)

    def test_frozen_vocabulary_rejects_self_consistent_replacement(self):
        vocabulary = copy.deepcopy(self.golden["effect_vocabulary"])
        vocabulary["effects"][0]["scope_profile"] = "policy_object"
        payload = copy.deepcopy(vocabulary)
        payload["vocabulary_sha256"] = ""
        vocabulary["vocabulary_sha256"] = digest(VOCABULARY_DOMAIN, payload)
        self.assertNotEqual(vocabulary["vocabulary_sha256"], VOCABULARY_SHA256)
        with self.assertRaisesRegex(contract.ContractError, "drifted|frozen"):
            validate_vocabulary(vocabulary)

    def test_alias_unknown_duplicate_and_noncanonical_are_rejected(self):
        request = self.request()
        request["grant"]["kind"] = "AuthorityGrant"
        raw = contract.canonical_json(request)
        with self.assertRaisesRegex(contract.ContractError, "CapabilityGrant|alias"):
            decode_request(raw)
        request = self.request()
        request["authority"] = "none"
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            decode_request(contract.canonical_json(request))
        raw = contract.canonical_json(self.request())
        duplicate = raw.replace(b'{"api_version":', b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(contract.ContractError, "duplicate"):
            decode_request(duplicate)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            decode_request(json.dumps(self.request()).encode())

    def test_number_unicode_and_size_boundaries_are_rejected(self):
        raw = contract.canonical_json(self.request())
        floating = raw.replace(b'"evaluated_at_unix_ms":1700000001000',
                               b'"evaluated_at_unix_ms":1.0')
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            decode_request(floating)
        oversized_int = raw.replace(b'1700000001000', b'9223372036854775808', 1)
        with self.assertRaisesRegex(contract.ContractError, "int64"):
            decode_request(oversized_int)
        bidi = raw.replace(b'agent-context-1', b'agent-\\u202econtext-1', 1)
        with self.assertRaisesRegex(contract.ContractError, "bidi"):
            decode_request(bidi)
        with self.assertRaisesRegex(contract.ContractError, "byte length"):
            decode_canonical(b" " * (MAX_REQUEST_BYTES + 1), MAX_REQUEST_BYTES, "request")

    def test_surrogate_values_and_keys_are_wrapped_contract_errors(self):
        for raw in (b'{"x":"\\ud800"}', b'{"\\ud800":"x"}'):
            with self.subTest(raw=raw):
                with self.assertRaises(contract.ContractError):
                    decode_canonical(raw, 1024, "surrogate fixture")

    def test_digests_bind_request_and_exclude_only_proof_bytes_from_grant(self):
        request = self.request()
        original_grant_hash = request["grant"]["grant_sha256"]
        request["grant"]["authority_proof"]["proof_base64url"] = "Zm9yZ2Vvcy1vdGhlci1wcm9vZg"
        self.assertEqual(grant_sha256(request["grant"]), original_grant_hash)
        old_request_hash = request["request_sha256"]
        self.resign_request(request)
        self.assertNotEqual(request["request_sha256"], old_request_hash)
        contract.evaluate_declared_assessment(request)
        request["grant"]["capability"]["capability_version"] = "2"
        with self.assertRaisesRegex(contract.ContractError, "digest"):
            contract.evaluate_declared_assessment(request)

    def test_deny_precedence_and_uncovered_scope_are_reported_only(self):
        request = self.request()
        request["requested_action"]["resources"][0]["path"] = "src/secrets/key.txt"
        self.resign_request(request)
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["scope"], "denied_by_declaration")
        self.assertIn("deny_matched", assessment["reason_codes"])
        self.assertEqual(assessment["authorization_decision"], "none")
        request = self.request()
        request["requested_action"]["resources"][0]["path"] = "docs/README.md"
        self.resign_request(request)
        self.assertEqual(evaluate_declared_assessment(request)["relations"]["scope"],
                         "outside_declared_scope")

    def test_binding_budget_task_and_time_relations_are_deterministic(self):
        mutations = (("binding", lambda request: request["expected"]["bindings"].update(
            {"context_sha256": "a" * 64})),
            ("budget", lambda request: request["requested_action"]["usage"].update(
                {"output_bytes": 131073})),
            ("task", lambda request: request["expected"]["task_binding"].update(
                {"task_id": "other-task"})),
            ("temporal", lambda request: request.update(
                {"evaluated_at_unix_ms": 1700003600000})))
        expected = {"binding": "binding_mismatch", "budget": "exceeds_declared_ceiling",
                    "task": "task_mismatch", "temporal": "outside_declared_window"}
        for relation, mutate in mutations:
            with self.subTest(relation=relation):
                request = self.request()
                mutate(request)
                self.resign_request(request)
                assessment = evaluate_declared_assessment(request)
                self.assertEqual(assessment["relations"][relation], expected[relation])
                self.assertEqual(assessment["authorization_decision"], "none")

    def test_typed_scope_order_and_repo_paths_fail_closed(self):
        request = self.request()
        request["requested_action"]["resources"] = [{
            "host": "example.com", "host_kind": "dns", "port": 443,
            "scheme": "https", "scope_kind": "network_origin"}]
        self.resign_request(request)
        with self.assertRaisesRegex(contract.ContractError, "scope kind"):
            evaluate_declared_assessment(request)
        request = self.request()
        resource = copy.deepcopy(request["requested_action"]["resources"][0])
        request["requested_action"]["resources"] = [resource, resource]
        self.resign_request(request)
        with self.assertRaisesRegex(contract.ContractError, "sorted"):
            evaluate_declared_assessment(request)
        request = self.request()
        request["requested_action"]["resources"][0]["path"] = "../escape"
        self.resign_request(request)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            evaluate_declared_assessment(request)

    def test_grant_ttl_transferability_usage_and_sod_fail_closed(self):
        cases = (("ttl", lambda grant: grant["validity"].update(
            {"expires_at_unix_ms": 1700086400001})),
            ("transfer", lambda grant: grant["validity"].update({"transferable": True})),
            ("usage", lambda grant: grant["usage_policy"].update(
                {"usage_ledger_required": False})),
            ("sod", lambda grant: grant["authority_proof"]["issuer"].update({
                "authority_domain": "forgeos.local", "principal_id": "agent-context-1",
                "principal_type": "agent"})))
        for label, mutate in cases:
            with self.subTest(label=label):
                request = self.request()
                mutate(request["grant"])
                with self.assertRaises(contract.ContractError):
                    evaluate_declared_assessment(request)

    def test_assessment_mutation_fails_exact_reassembly(self):
        assessment = self.assessment()
        assessment["relations"]["scope"] = "outside_declared_scope"
        assessment["reason_codes"] = ["scope_not_covered"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "reassembly"):
            contract.validate_assessment(self.request(), assessment)

    def test_standalone_assessment_rejects_impossible_effect_scope_pair(self):
        assessment = self.assessment()
        assessment["relations"]["effect"] = "effect_mismatch"
        assessment["relations"]["scope"] = "denied_by_declaration"
        assessment["reason_codes"] = ["effect_mismatch"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "outside_declared_scope"):
            contract.decode_assessment(contract.canonical_json(assessment))

    def test_extreme_json_nesting_is_a_contract_error(self):
        raw = b"[" * 2000 + b"0" + b"]" * 2000
        with self.assertRaisesRegex(contract.ContractError, "strict UTF-8 JSON|depth exceeds"):
            decode_canonical(raw, MAX_REQUEST_BYTES, "deep fixture")

    def test_cli_golden_and_instance_modes(self):
        golden = subprocess.run([sys.executable, "-B", "harness/capability_grant_contract_check.py",
                                 "--golden", str(ROOT)], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(golden.returncode, 0, golden.stderr)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request_path, assessment_path = root / "request.json", root / "assessment.json"
            request_path.write_bytes(contract.canonical_json(self.request()))
            assessment_path.write_bytes(contract.canonical_json(self.assessment()))
            command = [sys.executable, "-B", "harness/capability_grant_contract_check.py",
                       str(ROOT), str(request_path), str(assessment_path)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
