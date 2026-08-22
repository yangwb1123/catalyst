"""ADR-0061 strict KnowledgeUpdateProposal wire and pure evaluator tests."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import knowledge_update_proposal_contract as contract
from governance_contract import compute_record_digest
from knowledge_update_proposal_contract.assessment import assessment_sha256
from knowledge_update_proposal_contract.constants import RESULT

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "docs/contracts/fixtures/knowledge-update-proposal-v1.json"
SCHEMA = ROOT / "docs/contracts/knowledge-update-proposal-v1.schema.json"
HASHES = {
    "record_set": "c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f",
    "proposal": "a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1",
    "target": "34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85",
    "request": "d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413",
    "assessment": "e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037",
}


class KnowledgeUpdateProposalContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)

    def proposal(self):
        return copy.deepcopy(self.golden["knowledge_update_proposal"])

    def request(self):
        return copy.deepcopy(self.golden["assessment_request"])

    def assessment(self):
        return copy.deepcopy(self.golden["expected_assessment"])

    def reseal_record(self, record):
        record["integrity"]["canonical_sha256"] = ""
        record["integrity"]["canonical_sha256"] = compute_record_digest(record)

    def reseal_proposal(self, proposal):
        proposal["proposal_id"] = ""
        proposal["proposal_sha256"] = ""
        proposal["record_set_sha256"] = contract.record_set_sha256(proposal["records"])
        return contract.seal_proposal(proposal)

    def test_all_five_domain_separated_hashes_are_frozen(self):
        proposal, request, assessment = self.proposal(), self.request(), self.assessment()
        self.assertEqual(proposal["record_set_sha256"], HASHES["record_set"])
        self.assertEqual(proposal["proposal_sha256"], HASHES["proposal"])
        self.assertEqual(request["expected_target_sha256"], HASHES["target"])
        self.assertEqual(request["request_sha256"], HASHES["request"])
        self.assertEqual(assessment["assessment_sha256"], HASHES["assessment"])
        self.assertEqual(len(proposal["proposal_id"]), 90)

    def test_schema_fixture_and_authority_metadata_are_frozen(self):
        try:
            import jsonschema
        except ImportError:
            self.skipTest("jsonschema unavailable; runtime semantic checks still run")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        jsonschema.validate(json.loads(FIXTURE.read_text(encoding="utf-8")), schema)
        limits = schema["x-forgeos-limits"]
        self.assertEqual(limits["runtime_string_length_unit"], "utf8_bytes")
        self.assertTrue(limits[
            "schema_length_keywords_are_non_authoritative_approximations"])
        semantics = schema["x-forgeos-authority-semantics"]
        self.assertEqual(semantics["attestations"], [])
        self.assertEqual(semantics["positive_result"], RESULT)

    def test_fixture_builder_is_byte_deterministic(self):
        from knowledge_update_proposal_contract.fixture import golden_fixture
        expected = contract.canonical_json(golden_fixture(ROOT)) + b"\n"
        self.assertEqual(FIXTURE.read_bytes(), expected)
        self.assertEqual(hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
                         "2808e44b27df5f7b183ae7da3847d5780a3f66887d6b49e5fb4544a069a7ad5f")

    def test_golden_covers_create_supersede_evidence_artifact_and_exact_closure(self):
        proposal = self.proposal()
        self.assertEqual([item["target_aggregate_id"] for item in proposal["mutations"]],
                         ["claim-kup-create", "claim-kup-revise"])
        self.assertEqual([item["operation"] for item in proposal["mutations"]],
                         ["create", "supersede"])
        self.assertEqual({record["kind"] for record in proposal["records"]},
                         {"EvidenceRecord", "KnowledgeClaim"})
        self.assertEqual(len(proposal["records"]), 4)
        self.assertEqual(len(proposal["bindings"]["artifacts"]), 1)

    def test_orphan_and_missing_reachable_records_fail_closed(self):
        proposal = self.proposal()
        orphan = copy.deepcopy(proposal["records"][-1])
        orphan["metadata"].update({"aggregate_id": "orphan-evidence",
                                   "record_id": "orphan-evidence"})
        self.reseal_record(orphan)
        proposal["records"].append(orphan)
        proposal["records"].sort(key=lambda item: item["metadata"]["record_id"])
        with self.assertRaisesRegex(contract.ContractError, "exact reachable closure"):
            self.reseal_proposal(proposal)
        proposal = self.proposal()
        proposal["records"] = [record for record in proposal["records"]
                               if record["kind"] != "EvidenceRecord"]
        with self.assertRaisesRegex(contract.ContractError, "record set invalid"):
            self.reseal_proposal(proposal)

    def test_create_and_supersede_lifecycle_rules_fail_closed(self):
        proposal = self.proposal()
        proposal["mutations"][0]["before_claim_ref"] = proposal[
            "mutations"][1]["before_claim_ref"]
        with self.assertRaisesRegex(contract.ContractError,
                                    "create requires null|operation and before_claim_ref"):
            self.reseal_proposal(proposal)
        proposal = self.proposal()
        proposal["mutations"][1]["before_claim_ref"]["canonical_sha256"] = "f" * 64
        with self.assertRaisesRegex(contract.ContractError, "exact referenced"):
            self.reseal_proposal(proposal)
        proposal = self.proposal()
        after = next(record for record in proposal["records"]
                     if record["metadata"]["record_id"].endswith("after"))
        after["spec"]["object_value"] = "identity drift"
        self.reseal_record(after)
        proposal["mutations"][1]["after_claim_ref"]["canonical_sha256"] = after[
            "integrity"]["canonical_sha256"]
        with self.assertRaisesRegex(contract.ContractError, "stable semantic identity"):
            self.reseal_proposal(proposal)

    def test_supersede_may_retain_older_declared_ancestors(self):
        proposal = self.proposal()
        mutation = proposal["mutations"][1]
        middle = next(record for record in proposal["records"]
                      if record["metadata"]["record_id"].endswith("after"))
        oldest = next(record for record in proposal["records"]
                      if record["metadata"]["record_id"].endswith("before"))
        successor = copy.deepcopy(middle)
        successor["metadata"].update({
            "created_at_unix_ms": 1_700_000_001_750,
            "record_id": "claim-knowledge-update-final",
            "sequence": 3,
            "supersedes_record_ids": [middle["metadata"]["record_id"],
                                        oldest["metadata"]["record_id"]],
        })
        successor["status"].update({
            "state": "candidate", "valid_from_unix_ms": 1_700_000_001_750,
        })
        self.reseal_record(successor)
        proposal["records"].append(successor)
        proposal["records"].sort(key=lambda item: item["metadata"]["record_id"])
        mutation["before_claim_ref"] = {
            "canonical_sha256": middle["integrity"]["canonical_sha256"],
            "record_id": middle["metadata"]["record_id"],
        }
        mutation["after_claim_ref"] = {
            "canonical_sha256": successor["integrity"]["canonical_sha256"],
            "record_id": successor["metadata"]["record_id"],
        }
        self.reseal_proposal(proposal)
        successor["metadata"]["supersedes_record_ids"].remove(
            middle["metadata"]["record_id"])
        self.reseal_record(successor)
        mutation["after_claim_ref"]["canonical_sha256"] = successor[
            "integrity"]["canonical_sha256"]
        with self.assertRaisesRegex(contract.ContractError,
                                    "sequence-1|immediate predecessor"):
            self.reseal_proposal(proposal)

    def test_mutation_sort_uniqueness_and_record_provenance_fail_closed(self):
        proposal = self.proposal()
        proposal["mutations"].reverse()
        with self.assertRaisesRegex(contract.ContractError, "target_aggregate_id"):
            self.reseal_proposal(proposal)
        proposal = self.proposal()
        proposal["mutations"][1]["target_aggregate_id"] = "claim-kup-create"
        with self.assertRaisesRegex(contract.ContractError, "target_aggregate_id"):
            self.reseal_proposal(proposal)
        proposal = self.proposal()
        after = next(record for record in proposal["records"]
                     if record["metadata"]["record_id"].endswith("after"))
        after["metadata"]["context_sha256"] = "f" * 64
        self.reseal_record(after)
        proposal["mutations"][1]["after_claim_ref"]["canonical_sha256"] = after[
            "integrity"]["canonical_sha256"]
        with self.assertRaisesRegex(contract.ContractError, "provenance"):
            self.reseal_proposal(proposal)

    def test_mutation_reasons_reuse_full_adr0045_identifier_grammar(self):
        proposal = self.proposal()
        proposal["mutations"][0]["reason_codes"] = ["1:declared/reason"]
        self.reseal_proposal(proposal)

    def test_declared_target_rejects_intrinsic_mutation_contradictions(self):
        target = copy.deepcopy(self.request()["expected_target"])
        target["mutations"][0]["before_claim_ref"] = copy.deepcopy(
            target["mutations"][1]["before_claim_ref"])
        with self.assertRaisesRegex(contract.ContractError,
                                    "operation and before_claim_ref"):
            contract.seal_request(self.proposal(), target,
                                  self.proposal()["submitted_at_unix_ms"])
        target = copy.deepcopy(self.request()["expected_target"])
        target["mutations"][1]["after_claim_ref"] = copy.deepcopy(
            target["mutations"][0]["after_claim_ref"])
        with self.assertRaisesRegex(contract.ContractError, "after_claim_ref is reused"):
            contract.seal_request(self.proposal(), target,
                                  self.proposal()["submitted_at_unix_ms"])
        target = copy.deepcopy(self.request()["expected_target"])
        target["mutations"][1]["before_claim_ref"] = copy.deepcopy(
            target["mutations"][1]["after_claim_ref"])
        with self.assertRaisesRegex(contract.ContractError,
                                    "before_claim_ref cannot equal after_claim_ref"):
            contract.seal_request(self.proposal(), target,
                                  self.proposal()["submitted_at_unix_ms"])
        target = copy.deepcopy(self.request()["expected_target"])
        target["mutations"][1]["before_claim_ref"] = copy.deepcopy(
            target["mutations"][0]["after_claim_ref"])
        with self.assertRaisesRegex(contract.ContractError, "sets must be disjoint"):
            contract.seal_request(self.proposal(), target,
                                  self.proposal()["submitted_at_unix_ms"])

    def test_scope_project_and_submission_time_are_intrinsic(self):
        for field, value, message in (
                ("scope", "knowledge:other", "knowledge object_ref"),
                ("project_id", "other-project", "task project"),
                ("created_at_unix_ms", 1_700_000_002_001, "provenance and time")):
            with self.subTest(field=field):
                proposal = self.proposal()
                after = next(record for record in proposal["records"]
                             if record["metadata"]["record_id"].endswith("create"))
                after["metadata"][field] = value
                if field == "created_at_unix_ms":
                    after["status"]["valid_from_unix_ms"] = value
                self.reseal_record(after)
                proposal["mutations"][0]["after_claim_ref"]["canonical_sha256"] = after[
                    "integrity"]["canonical_sha256"]
                with self.assertRaisesRegex(contract.ContractError, message):
                    self.reseal_proposal(proposal)

    def test_each_declared_target_mismatch_is_deterministic(self):
        mutations = {
            "binding": lambda value: value["bindings"].update({"context_sha256": "f" * 64}),
            "grant_ref": lambda value: value["capability_grant_ref"].update(
                {"grant_id": "capability-grant-" + "f" * 64,
                 "grant_sha256": "f" * 64}),
            "mutations": lambda value: value["mutations"][0].update(
                {"rationale": "different declared target"}),
            "proposer": lambda value: value["proposer"].update({"principal_id": "other"}),
            "record_set": lambda value: value.update({"record_set_sha256": "f" * 64}),
            "scope": lambda value: value["knowledge_scope"].update(
                {"object_scope_sha256": "f" * 64}),
            "task_binding": lambda value: value["task_binding"].update({"task_id": "other"}),
        }
        for relation, mutate in mutations.items():
            with self.subTest(relation=relation):
                target = contract.declared_target(self.proposal())
                mutate(target)
                request = contract.seal_request(
                    self.proposal(), target, self.proposal()["submitted_at_unix_ms"])
                assessment = contract.evaluate_declared_assessment(request)
                self.assertEqual(assessment["relations"][relation], f"{relation}_mismatch")
                self.assertIn(f"{relation}_mismatch", assessment["reason_codes"])

    def test_future_submission_is_explained_without_authority(self):
        proposal = self.proposal()
        request = contract.seal_request(
            proposal, contract.declared_target(proposal),
            proposal["submitted_at_unix_ms"] - 1)
        assessment = contract.evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["temporal"],
                         "future_declared_submission")
        self.assertEqual(assessment["reason_codes"], ["temporal_declaration_mismatch"])
        self.assertEqual(assessment["authorization_decision"], "none")
        self.assertFalse(assessment["knowledge_adoption_attestation"])

    def test_authority_escalation_and_reassembly_drift_fail_closed(self):
        for field, value in (("authorization_decision", "authorized"),
                             ("policy_decision", "allow"),
                             ("grant_state", "active"),
                             ("truth_attestation", True),
                             ("knowledge_adoption_attestation", True),
                             ("result", "KNOWLEDGE_APPLIED")):
            with self.subTest(field=field):
                assessment = self.assessment()
                assessment[field] = value
                assessment["assessment_sha256"] = assessment_sha256(assessment)
                with self.assertRaises(contract.ContractError):
                    contract.validate_assessment(self.request(), assessment)
        assessment = self.assessment()
        assessment["relations"]["scope"] = "scope_mismatch"
        assessment["reason_codes"] = ["scope_mismatch"]
        assessment["assessment_sha256"] = assessment_sha256(assessment)
        with self.assertRaisesRegex(contract.ContractError, "reassembly"):
            contract.validate_assessment(self.request(), assessment)

    def test_strict_json_duplicate_noncanonical_numbers_and_unicode_fail(self):
        raw = contract.canonical_json(self.request())
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(contract.ContractError, "duplicate"):
            contract.decode_request(duplicate)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            contract.decode_request(json.dumps(self.request()).encode())
        floating = raw.replace(b'"evaluated_at_unix_ms":1700000002000',
                               b'"evaluated_at_unix_ms":1.0')
        with self.assertRaisesRegex(contract.ContractError, "non-integer"):
            contract.decode_request(floating)
        bidi = raw.replace(b'fixture-revision-1', b'fixture-\\u202erevision-1')
        with self.assertRaisesRegex(contract.ContractError, "bidi"):
            contract.decode_request(bidi)

    def test_programmatic_size_depth_and_cycle_fail_before_encoding(self):
        target = contract.declared_target(self.proposal())
        target["bindings"] = {"x": ["\\" * 16_384] * 256}
        with mock.patch("knowledge_update_proposal_contract.canonical.json.dumps",
                        side_effect=AssertionError("oversized value reached encoder")):
            with self.assertRaisesRegex(contract.ContractError, "byte ceiling"):
                contract.declared_target_sha256(target)
        nested = {}
        for _ in range(40):
            nested = {"child": nested}
        proposal = self.proposal()
        proposal["bindings"] = nested
        with self.assertRaisesRegex(contract.ContractError, "depth"):
            contract.seal_proposal(proposal)
        cycle = {}
        cycle["child"] = cycle
        proposal = self.proposal()
        proposal["bindings"] = cycle
        with self.assertRaisesRegex(contract.ContractError, "depth"):
            contract.seal_proposal(proposal)

    def test_cli_golden_instance_and_no_site_packages_modes(self):
        commands = ([sys.executable, "-B"], [sys.executable, "-B", "-S"])
        for prefix in commands:
            result = subprocess.run(
                [*prefix, "harness/knowledge_update_proposal_contract_check.py",
                 "--golden", str(ROOT)], cwd=ROOT, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
        with tempfile.TemporaryDirectory() as directory:
            request_path, assessment_path = (Path(directory) / "request.json",
                                             Path(directory) / "assessment.json")
            request_path.write_bytes(contract.canonical_json(self.request()))
            assessment_path.write_bytes(contract.canonical_json(self.assessment()))
            result = subprocess.run(
                [sys.executable, "-B", "harness/knowledge_update_proposal_contract_check.py",
                 str(ROOT), str(request_path), str(assessment_path)], cwd=ROOT,
                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
