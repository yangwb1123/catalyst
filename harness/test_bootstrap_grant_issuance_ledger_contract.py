"""Ledger, replay, and full-Grant-binding tests for ADR-0057."""

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bootstrap_grant_issuance_contract as contract
from bootstrap_grant_issuance_contract.documents import (receipt_sha256,
                                                         record_key_sha256)
from bootstrap_grant_issuance_contract.ledger import ledger_sha256

ROOT = Path(__file__).resolve().parents[1]


class BootstrapGrantIssuanceLedgerContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_fixture(ROOT)

    def document(self) -> dict[str, object]:
        return copy.deepcopy(self.golden)

    def test_result_stored_and_exact_replay_bind_same_signed_artifacts(self):
        document = self.document()
        self.assertEqual(contract.validate_document(document)["result"][
            "delivery_disposition"], "stored")
        document["result"]["delivery_disposition"] = "exact_replay"
        self.assertEqual(contract.validate_document(document)["result"][
            "delivery_disposition"], "exact_replay")
        document["result"]["receipt"]["stored_at_unix_ms"] += 1
        with self.assertRaisesRegex(contract.ContractError, "self digest"):
            contract.validate_document(document)

    def test_receipt_binds_complete_grant_including_proof_bytes(self):
        document = self.document()
        proof = document["grant"]["authority_proof"]["proof_base64url"]
        replacement = "A" if proof[-1] != "A" else "Q"
        document["grant"]["authority_proof"]["proof_base64url"] = proof[:-1] + replacement
        document["result"]["grant"] = copy.deepcopy(document["grant"])
        document["ledger"]["entries"][0]["grant"] = copy.deepcopy(document["grant"])
        document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
        self.assertEqual(document["grant"]["grant_sha256"], self.golden["grant"]["grant_sha256"])
        with self.assertRaisesRegex(contract.ContractError, "complete issued Grant"):
            contract.validate_document(document)

    def test_record_key_is_domain_bound_to_exact_idempotency_key(self):
        request = self.golden["request"]
        self.assertEqual(self.golden["receipt"]["record_key_sha256"],
                         record_key_sha256(request["idempotency_key"]))
        document = self.document()
        document["receipt"]["record_key_sha256"] = "0" * 64
        _replace_receipt_everywhere(document)
        with self.assertRaisesRegex(contract.ContractError, "record key"):
            contract.validate_document(document)

    def test_sequence_prior_chain_and_duplicate_record_key_fail_closed(self):
        for mutation, expected in (("sequence", "contiguous"),
                                   ("prior", "prior digest chain"),
                                   ("duplicate", "duplicate idempotency")):
            with self.subTest(mutation=mutation):
                document = self.document()
                second = _second_entry(document)
                if mutation == "sequence":
                    second["sequence"] = 3
                elif mutation == "prior":
                    second["receipt"]["prior_receipt_sha256"] = "0" * 64
                    second["receipt"]["receipt_sha256"] = receipt_sha256(second["receipt"])
                document["ledger"]["entries"].append(second)
                document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
                with self.assertRaisesRegex(contract.ContractError, expected):
                    contract.validate_document(document)

    def test_clock_high_water_and_ledger_signing_usage_fail_closed(self):
        document = self.document()
        document["ledger"]["clock_high_water_unix_ms"] = 0
        document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
        with self.assertRaisesRegex(contract.ContractError, "high-water"):
            contract.validate_document(document)
        document = self.document()
        document["ledger"]["signature"]["key_id"] = "fixture-policy-sign-key-v1"
        document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
        with self.assertRaisesRegex(contract.ContractError, "grant_issue"):
            contract.validate_document(document)

    def test_ledger_cardinality_and_full_snapshot_byte_ceiling(self):
        document = self.document()
        entry = document["ledger"]["entries"][0]
        document["ledger"]["entries"] = [entry] * 257
        with self.assertRaisesRegex(contract.ContractError, "array item count exceeds 256"):
            contract.validate_document(document)
        ledger = copy.deepcopy(self.golden["ledger"])
        huge = copy.deepcopy(ledger["entries"][0])
        resources = _huge_resources()
        for artifact in (huge["policy"], huge["request"], huge["grant"]):
            artifact["scope"]["allow"][0]["resources"] = resources
        ledger["entries"] = [huge] * 256
        with self.assertRaisesRegex(contract.ContractError, "canonical byte length"):
            ledger_sha256(ledger)

    def test_grant_validity_may_outlive_request_but_not_policy(self):
        request_expiry = self.golden["request"]["expires_at_unix_ms"]
        grant_expiry = self.golden["grant"]["validity"]["expires_at_unix_ms"]
        self.assertGreater(grant_expiry, request_expiry)
        document = self.document()
        document["policy"]["validity"]["expires_at_unix_ms"] = grant_expiry - 1
        from bootstrap_grant_issuance_contract.documents import policy_sha256
        document["policy"]["policy_sha256"] = policy_sha256(document["policy"])
        document["request"]["policy_sha256"] = document["policy"]["policy_sha256"]
        from bootstrap_grant_issuance_contract.documents import request_sha256
        document["request"]["request_sha256"] = request_sha256(document["request"])
        grant = document["grant"]
        grant["bindings"]["policy_sha256"] = document["policy"]["policy_sha256"]
        grant["bindings"]["grant_request_sha256"] = document["request"]["request_sha256"]
        from capability_grant_contract.grant import grant_sha256
        grant["grant_sha256"] = grant_sha256(grant)
        grant["grant_id"] = "capability-grant-" + grant["grant_sha256"]
        document["receipt"]["policy_sha256"] = document["policy"]["policy_sha256"]
        document["receipt"]["request_sha256"] = document["request"]["request_sha256"]
        document["receipt"]["grant_id"] = grant["grant_id"]
        document["receipt"]["grant_sha256"] = grant["grant_sha256"]
        document["receipt"]["grant_envelope_sha256"] = contract.grant_envelope_sha256(grant)
        document["result"]["grant"] = copy.deepcopy(grant)
        document["ledger"]["entries"][0]["grant"] = copy.deepcopy(grant)
        _replace_receipt_everywhere(document)
        document["ledger"]["entries"][0]["policy"] = copy.deepcopy(document["policy"])
        document["ledger"]["entries"][0]["request"] = copy.deepcopy(document["request"])
        document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])
        with self.assertRaisesRegex(contract.ContractError, "Policy validity"):
            contract.validate_document(document)


def _replace_receipt_everywhere(document: dict[str, object]) -> None:
    document["receipt"]["receipt_sha256"] = receipt_sha256(document["receipt"])
    document["result"]["receipt"] = copy.deepcopy(document["receipt"])
    document["ledger"]["entries"][0]["receipt"] = copy.deepcopy(document["receipt"])
    document["ledger"]["ledger_sha256"] = ledger_sha256(document["ledger"])


def _second_entry(document: dict[str, object]) -> dict[str, object]:
    second = copy.deepcopy(document["ledger"]["entries"][0])
    second["sequence"] = 2
    second["receipt"]["ledger_sequence"] = 2
    second["receipt"]["prior_receipt_sha256"] = document["receipt"]["receipt_sha256"]
    second["receipt"]["receipt_sha256"] = receipt_sha256(second["receipt"])
    return second


def _huge_resources() -> list[dict[str, str]]:
    resources = []
    for index in range(16):
        prefix = f"path-{index:02d}-"
        resources.append({"match": "exact", "path": prefix + "a" * (4090 - len(prefix)),
                          "scope_kind": "repo_path"})
    return resources


if __name__ == "__main__":
    unittest.main()
