"""ADR-0059 ApprovalRef compatibility with the ADR-0056 Grant contract."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

import approval_record_contract as approval_contract
import capability_grant_contract as grant_contract
from capability_grant_contract.assessment import (evaluate_declared_assessment,
                                                   request_sha256)
from capability_grant_contract.grant import grant_sha256, validate_grant

ROOT = Path(__file__).resolve().parents[1]


class ApprovalRecordCapabilityGrantCompatibilityTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.approval = approval_contract.load_golden(ROOT)
        cls.grant = grant_contract.load_golden(ROOT)

    def grant_with_approval_ref(self):
        grant = copy.deepcopy(self.grant["grant"])
        grant["approval_refs"] = [approval_contract.approval_ref(
            self.approval["approval_record"])]
        grant["grant_id"] = ""
        grant["grant_sha256"] = ""
        digest = grant_sha256(grant)
        grant["grant_id"] = f"capability-grant-{digest}"
        grant["grant_sha256"] = digest
        return validate_grant(grant)

    def test_projection_is_an_exact_valid_capability_grant_reference(self):
        grant = self.grant_with_approval_ref()
        reference = grant["approval_refs"][0]
        self.assertEqual(reference, self.approval["expected_approval_ref"])
        self.assertEqual(approval_contract.approval_ref_relation(
            self.approval["approval_record"], reference), "same_declared_reference")
        self.assertEqual(set(reference),
                         {"approval_id", "approval_sha256", "authority_domain"})

    def test_reference_presence_does_not_upgrade_grant_approval_state(self):
        request = copy.deepcopy(self.grant["assessment_request"])
        request["grant"] = self.grant_with_approval_ref()
        request["request_sha256"] = request_sha256(request)
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment["approval_state"], "not_evaluated")
        self.assertEqual(assessment["authorization_decision"], "none")
        self.assertFalse(assessment["permission_attestation"])
        self.assertFalse(assessment["effect_attestation"])

    def test_reference_mismatch_is_only_a_declared_relation(self):
        reference = copy.deepcopy(self.approval["expected_approval_ref"])
        reference["authority_domain"] = "other.authority"
        relation = approval_contract.approval_ref_relation(
            self.approval["approval_record"], reference)
        self.assertEqual(relation, "reference_mismatch")
        self.assertNotIn("denied", relation)
        self.assertNotIn("authorized", relation)


if __name__ == "__main__":
    unittest.main()
