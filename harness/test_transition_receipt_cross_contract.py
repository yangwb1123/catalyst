"""ADR-0060 pure compatibility with ADR-0056 and ADR-0059 contracts."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

import approval_record_contract
import capability_grant_contract
import transition_receipt_contract as transition_contract
from approval_record_contract.record import seal_record
from capability_grant_contract.constants import EFFECTS

ROOT = Path(__file__).resolve().parents[1]


class TransitionReceiptCrossContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.transition = transition_contract.load_golden(ROOT)
        cls.grant = capability_grant_contract.load_golden(ROOT)["grant"]
        cls.approval = approval_record_contract.load_golden(ROOT)["approval_record"]

    def receipt(self):
        return copy.deepcopy(self.transition["transition_receipt"])

    def reseal_receipt(self, receipt):
        receipt["receipt_id"] = ""
        receipt["receipt_sha256"] = ""
        return transition_contract.seal_receipt(receipt)

    def matching_approval(self):
        record = copy.deepcopy(self.approval)
        task = self.receipt()["task_binding"]
        record["scope"].update({
            "change_id": task["change_id"],
            "environment_class": task["environment_class"],
            "environment_id": task["environment_id"],
            "project_id": task["project_id"],
        })
        record["approval_id"] = ""
        record["approval_sha256"] = ""
        return seal_record(record)

    def test_grant_projection_is_exact_and_never_adds_transition_effect(self):
        projected = transition_contract.project_capability_grant_ref(self.grant)
        self.assertEqual(projected, self.transition["expected_capability_grant_ref"])
        self.assertEqual(set(projected), {"authority_domain", "grant_id", "grant_sha256"})
        self.assertNotIn("lifecycle.transition", EFFECTS)
        self.assertNotIn("transition", EFFECTS)

    def test_matching_grant_declarations_remain_authority_neutral(self):
        receipt = self.receipt()
        receipt["approval_refs"] = []
        receipt = self.reseal_receipt(receipt)
        result = transition_contract.assess_declared_grant_compatibility(self.grant, receipt)
        self.assertEqual(result["reason_codes"], [])
        self.assertTrue(all(value.startswith("same_declared_")
                            for value in result["relations"].values()))
        self.assertEqual(result["result"],
                         "ASSESSED_GRANT_TRANSITION_DECLARATIONS_ONLY "
                         "(no permission or transition authority)")
        self.assertNotIn("authorized", result["result"].lower())

    def test_each_grant_relation_mismatch_is_deterministic(self):
        mutations = {
            "actor": lambda value: value["actor"].update({"principal_id": "other"}),
            "approval_refs": lambda value: None,
            "bindings": lambda value: value["bindings"].update({"context_sha256": "d" * 64}),
            "declared_time": lambda value: value["transition"].update(
                {"declared_at_unix_ms": self.grant["validity"]["expires_at_unix_ms"]}),
            "grant_ref": lambda value: value["capability_grant_ref"].update({
                "grant_id": "capability-grant-" + "d" * 64, "grant_sha256": "d" * 64}),
            "task_binding": lambda value: value["task_binding"].update({"task_id": "other"}),
        }
        for field, mutate in mutations.items():
            with self.subTest(field=field):
                receipt = self.receipt()
                if field != "approval_refs":
                    receipt["approval_refs"] = []
                mutate(receipt)
                receipt = self.reseal_receipt(receipt)
                result = transition_contract.assess_declared_grant_compatibility(
                    self.grant, receipt)
                self.assertTrue(result["relations"][field].endswith("_mismatch"))
                self.assertIn(result["relations"][field], result["reason_codes"])

    def test_approval_projection_and_matching_scope_are_declarations_only(self):
        record = self.matching_approval()
        refs = transition_contract.project_approval_refs([record])
        receipt = self.receipt()
        receipt["approval_refs"] = refs
        receipt = self.reseal_receipt(receipt)
        result = transition_contract.assess_declared_approval_compatibility([record], receipt)
        self.assertEqual(result["reason_codes"], [])
        self.assertEqual(result["relations"], {
            "ref_set": "same_declared_ref_set", "scope": "same_declared_scope"})
        self.assertEqual(result["result"],
                         "ASSESSED_APPROVAL_TRANSITION_DECLARATIONS_ONLY "
                         "(no effective approval or transition authority)")
        self.assertNotIn("authorized", result["result"].lower())

    def test_approval_ref_or_scope_mismatch_cannot_activate_authorized(self):
        result = transition_contract.assess_declared_approval_compatibility(
            [self.approval], self.receipt())
        self.assertEqual(result["relations"]["ref_set"], "same_declared_ref_set")
        self.assertEqual(result["relations"]["scope"], "scope_mismatch")
        self.assertIn("scope_mismatch", result["reason_codes"])
        self.assertNotIn("effective", result["relations"])

    def test_projection_rejects_duplicate_or_unordered_records(self):
        with self.assertRaises(transition_contract.ContractError):
            transition_contract.project_approval_refs([self.approval, self.approval])


if __name__ == "__main__":
    unittest.main()

