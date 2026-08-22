"""ADR-0061 declared-only Grant, Context, and artifact compatibility tests."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

import capability_grant_contract
import context_package_contract
import knowledge_update_proposal_contract as knowledge_contract
from capability_grant_contract.grant import grant_sha256
from context_package_contract.fixture import load_fixture as load_context_fixture
from governance_contract import compute_record_digest
from knowledge_update_proposal_contract.constants import CONTEXT_RESULT, GRANT_RESULT

ROOT = Path(__file__).resolve().parents[1]


class KnowledgeUpdateProposalCrossContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.knowledge = knowledge_contract.load_golden(ROOT)
        cls.grant = capability_grant_contract.load_golden(ROOT)["grant"]
        cls.context = load_context_fixture(ROOT)

    def proposal(self):
        return copy.deepcopy(self.knowledge["knowledge_update_proposal"])

    def seal_grant(self, grant):
        grant["grant_id"] = ""
        grant["grant_sha256"] = ""
        digest = grant_sha256(grant)
        grant["grant_id"] = "capability-grant-" + digest
        grant["grant_sha256"] = digest
        return capability_grant_contract.validate_grant(grant)

    def seal_proposal(self, proposal):
        proposal["proposal_id"] = ""
        proposal["proposal_sha256"] = ""
        proposal["record_set_sha256"] = knowledge_contract.record_set_sha256(
            proposal["records"])
        return knowledge_contract.seal_proposal(proposal)

    def matching_grant_and_proposal(self):
        grant = copy.deepcopy(self.grant)
        scope = self.proposal()["knowledge_scope"]
        grant["scope"] = {"allow": [{"resources": [copy.deepcopy(scope)]}],
                          "deny": [], "effect_id": "knowledge.propose"}
        grant = self.seal_grant(grant)
        proposal = self.proposal()
        proposal["capability_grant_ref"] = (
            knowledge_contract.project_capability_grant_ref(grant))
        return grant, self.seal_proposal(proposal)

    def _align_after_provenance(self, proposal):
        refs = {mutation["after_claim_ref"]["record_id"]: mutation
                for mutation in proposal["mutations"]}
        for record in proposal["records"]:
            record_id = record["metadata"]["record_id"]
            if record_id not in refs:
                continue
            metadata = record["metadata"]
            bindings, task, proposer = (proposal["bindings"], proposal["task_binding"],
                                         proposal["proposer"])
            metadata.update({"context_sha256": bindings["context_sha256"],
                             "policy_sha256": bindings["policy_sha256"],
                             "source_revision": bindings["source_revision"],
                             "source_tree_sha256": bindings["source_tree_sha256"]})
            metadata["created_by"].update({**proposer, "role": task["role"],
                                           "run_id": task["run_id"]})
            record["integrity"]["canonical_sha256"] = ""
            record["integrity"]["canonical_sha256"] = compute_record_digest(record)
            refs[record_id]["after_claim_ref"]["canonical_sha256"] = record[
                "integrity"]["canonical_sha256"]
        return self.seal_proposal(proposal)

    def matching_context_and_proposal(self):
        proposal = self.proposal()
        request = copy.deepcopy(self.context["request"])
        shared = ("change_id", "node_id", "project_id", "role", "run_id", "task_id")
        for field in shared:
            request["task_binding"][field] = proposal["task_binding"][field]
        bindings = proposal["bindings"]
        request["source_binding"].update({
            "policy_sha256": bindings["policy_sha256"],
            "source_revision": bindings["source_revision"],
            "source_tree_sha256": bindings["source_tree_sha256"],
        })
        counter = context_package_contract.Utf8ByteTokenCounter()
        package = context_package_contract.assemble(request, counter)
        proposal["bindings"]["context_sha256"] = package["context_sha256"]
        return request, package, self._align_after_provenance(proposal), counter

    def test_matching_grant_relations_remain_declaration_only(self):
        grant, proposal = self.matching_grant_and_proposal()
        result = knowledge_contract.assess_declared_grant_compatibility(grant, proposal)
        self.assertEqual(result["reason_codes"], [])
        self.assertEqual(result["relations"], {
            "bindings": "same_declared_bindings",
            "declared_time": "same_declared_time",
            "effect": "same_declared_effect",
            "grant_ref": "same_declared_grant_ref",
            "proposer": "same_declared_proposer",
            "scope": "covered_by_declaration",
            "task_binding": "same_declared_task_binding",
        })
        self.assertEqual(result["result"], GRANT_RESULT)
        self.assertNotIn("authorized", result["result"].lower())

    def test_each_grant_mismatch_is_exact(self):
        cases = {
            "bindings": lambda grant, proposal: grant["bindings"].update(
                {"context_sha256": "f" * 64}),
            "declared_time": lambda grant, proposal: proposal.update(
                {"submitted_at_unix_ms": grant["validity"]["expires_at_unix_ms"]}),
            "proposer": lambda grant, proposal: grant["subject"].update(
                {"principal_id": "other-agent"}),
            "task_binding": lambda grant, proposal: grant["task_binding"].update(
                {"task_id": "other-task"}),
        }
        for field, mutate in cases.items():
            with self.subTest(field=field):
                grant, proposal = self.matching_grant_and_proposal()
                mutate(grant, proposal)
                grant = self.seal_grant(grant)
                proposal["capability_grant_ref"] = (
                    knowledge_contract.project_capability_grant_ref(grant))
                proposal = self.seal_proposal(proposal)
                result = knowledge_contract.assess_declared_grant_compatibility(
                    grant, proposal)
                self.assertEqual(result["relations"][field], f"{field}_mismatch")
                self.assertIn(f"{field}_mismatch", result["reason_codes"])

    def test_grant_ref_mismatch_is_not_authorization(self):
        grant, proposal = self.matching_grant_and_proposal()
        proposal["capability_grant_ref"].update({
            "grant_id": "capability-grant-" + "f" * 64, "grant_sha256": "f" * 64})
        proposal = self.seal_proposal(proposal)
        result = knowledge_contract.assess_declared_grant_compatibility(grant, proposal)
        self.assertEqual(result["relations"]["grant_ref"], "grant_ref_mismatch")
        self.assertEqual(result["reason_codes"], ["grant_ref_mismatch"])

    def test_effect_mismatch_suppresses_redundant_scope_reason(self):
        grant = copy.deepcopy(self.grant)
        proposal = self.proposal()
        proposal["capability_grant_ref"] = (
            knowledge_contract.project_capability_grant_ref(grant))
        proposal = self.seal_proposal(proposal)
        result = knowledge_contract.assess_declared_grant_compatibility(grant, proposal)
        self.assertEqual(result["relations"]["effect"], "effect_mismatch")
        self.assertEqual(result["relations"]["scope"], "outside_declared_scope")
        self.assertIn("effect_mismatch", result["reason_codes"])
        self.assertNotIn("scope_not_covered", result["reason_codes"])

    def test_grant_deny_precedes_allow_and_outside_scope_is_exact(self):
        grant, proposal = self.matching_grant_and_proposal()
        grant["scope"]["deny"] = [copy.deepcopy(proposal["knowledge_scope"])]
        grant = self.seal_grant(grant)
        proposal["capability_grant_ref"] = knowledge_contract.project_capability_grant_ref(grant)
        proposal = self.seal_proposal(proposal)
        result = knowledge_contract.assess_declared_grant_compatibility(grant, proposal)
        self.assertEqual(result["relations"]["scope"], "denied_by_declaration")
        self.assertIn("deny_matched", result["reason_codes"])
        grant, proposal = self.matching_grant_and_proposal()
        grant["scope"]["allow"][0]["resources"][0]["object_ref"] = "knowledge:other"
        grant = self.seal_grant(grant)
        proposal["capability_grant_ref"] = knowledge_contract.project_capability_grant_ref(grant)
        proposal = self.seal_proposal(proposal)
        result = knowledge_contract.assess_declared_grant_compatibility(grant, proposal)
        self.assertEqual(result["relations"]["scope"], "outside_declared_scope")
        self.assertIn("scope_not_covered", result["reason_codes"])

    def test_matching_reassembled_context_relations_are_declarations_only(self):
        request, package, proposal, counter = self.matching_context_and_proposal()
        result = knowledge_contract.assess_reassembled_context_compatibility(
            request, package, proposal, counter)
        self.assertEqual(result["reason_codes"], [])
        self.assertEqual(result["relations"], {
            "context": "same_declared_context",
            "freshness": "inside_declared_freshness",
            "policy": "same_declared_policy",
            "source": "same_declared_source",
            "task_binding": "same_declared_task_binding",
        })
        self.assertEqual(result["result"], CONTEXT_RESULT)

    def test_context_mismatches_and_freshness_reason_are_exact(self):
        cases = {
            "context": lambda proposal, package: proposal["bindings"].update(
                {"context_sha256": "f" * 64}),
            "policy": lambda proposal, package: proposal["bindings"].update(
                {"policy_sha256": "f" * 64}),
            "source": lambda proposal, package: proposal["bindings"].update(
                {"source_revision": "other-revision"}),
            "task_binding": lambda proposal, package: proposal["task_binding"].update(
                {"task_id": "other-task"}),
            "freshness": lambda proposal, package: proposal.update(
                {"submitted_at_unix_ms": package["freshness"]["expires_at_unix_ms"]}),
        }
        for field, mutate in cases.items():
            with self.subTest(field=field):
                request, package, proposal, counter = self.matching_context_and_proposal()
                mutate(proposal, package)
                if field in ("context", "policy", "source"):
                    proposal = self._align_after_provenance(proposal)
                else:
                    proposal = self.seal_proposal(proposal)
                result = knowledge_contract.assess_reassembled_context_compatibility(
                    request, package, proposal, counter)
                expected = ("outside_declared_freshness" if field == "freshness"
                            else f"{field}_mismatch")
                self.assertEqual(result["relations"][field], expected)
                reason = "freshness_mismatch" if field == "freshness" else expected
                self.assertIn(reason, result["reason_codes"])

    def test_context_helper_requires_exact_reassembly(self):
        request, package, proposal, counter = self.matching_context_and_proposal()
        package["context_sha256"] = "f" * 64
        with self.assertRaises(ValueError):
            knowledge_contract.assess_reassembled_context_compatibility(
                request, package, proposal, counter)

    def test_artifact_projection_is_typed_but_not_an_artifact_attestation(self):
        proposal = self.proposal()
        projected = knowledge_contract.project_artifact_resources(proposal)
        self.assertEqual(projected, self.knowledge["expected_artifact_resources"])
        self.assertTrue(all(item["scope_kind"] == "artifact" for item in projected))
        self.assertNotIn("validated", knowledge_contract.canonical_json(projected).decode())
        self.assertNotIn("exists", knowledge_contract.canonical_json(projected).decode())


if __name__ == "__main__":
    unittest.main()
