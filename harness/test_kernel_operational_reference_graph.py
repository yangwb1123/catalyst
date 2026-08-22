"""Reference, retry, time, and projection tests for ADR-0088 closure semantics."""

from __future__ import annotations

import copy
import unittest

from kernel_operational_contract import ContractError, empty_profile_closure
from kernel_operational_contract.fixture import fixture_candidate
from kernel_operational_contract.graph import validate_reference_graph


def _must_reject(test: unittest.TestCase, mutator, pattern: str | None = None) -> None:
    closure = fixture_candidate()
    mutator(closure)
    manager = test.assertRaisesRegex(ContractError, pattern) if pattern else test.assertRaises(
        ContractError)
    with manager:
        validate_reference_graph(closure)


class KernelOperationalReferenceGraphTests(unittest.TestCase):
    def test_full_retry_and_empty_profile_graphs_are_valid(self) -> None:
        validate_reference_graph(fixture_candidate())
        validate_reference_graph(empty_profile_closure())

    def test_record_principals_are_not_forced_equal(self) -> None:
        closure = fixture_candidate()
        principals = [closure["artifact_receipts"][0]["producer"],
                      closure["capability_invocations"][0]["subject"],
                      closure["interaction_events"][0]["actor"],
                      closure["interaction_events"][0]["target"],
                      closure["execution_receipts"][0]["executor"]]
        self.assertEqual(len({str(item) for item in principals}), len(principals))
        validate_reference_graph(closure)

    def test_common_task_binding_and_opaque_bindings_are_exact(self) -> None:
        _must_reject(self, lambda item: item["interaction_events"][0][
            "bindings"].__setitem__("policy_sha256", "0" * 64), "opaque bindings")
        _must_reject(self, lambda item: item["artifact_receipts"][0][
            "task_binding"].__setitem__("role", "other"), "TaskBinding")

    def test_correlation_and_idempotency_are_single_chain(self) -> None:
        _must_reject(self, lambda item: item["interaction_events"][0].__setitem__(
            "correlation_id", "other-correlation"), "correlation_id")
        _must_reject(self, lambda item: item["execution_receipts"][1].__setitem__(
            "correlation_id", "other-correlation"), "correlation_id")
        _must_reject(self, lambda item: item["capability_invocations"][1].__setitem__(
            "idempotency_key", "other-key"), "static request")

    def test_retry_attempt_prior_and_terminal_success_relations(self) -> None:
        _must_reject(self, lambda item: item["capability_invocations"][1].__setitem__(
            "attempt", 3), "contiguous attempts")
        _must_reject(self, lambda item: item["capability_invocations"][1].__setitem__(
            "prior_execution_receipt_ref", None), "preceding attempt")
        _must_reject(self, lambda item: item["execution_receipts"][0].__setitem__(
            "outcome", "succeeded"), "cannot be retried")
        _must_reject(self, lambda item: item["capability_invocations"][1].__setitem__(
            "requested_at_unix_ms", item["execution_receipts"][0][
                "ended_at_unix_ms"] - 1), "precedes")

    def test_input_receipt_projection_role_and_time_are_exact(self) -> None:
        _must_reject(self, lambda item: item["execution_receipts"][0].__setitem__(
            "input_artifacts", []), "exactly project")
        def mutate_input(item: dict, field: str, value) -> None:
            receipt = next(record for record in item["artifact_receipts"]
                           if record["receipt_role"] == "declared_input")
            receipt[field] = value(item) if callable(value) else value
        _must_reject(self, lambda item: mutate_input(
            item, "receipt_role", "declared_output"), "declared_input")
        _must_reject(self, lambda item: mutate_input(
            item, "created_at_unix_ms", lambda closure: closure[
                "capability_invocations"][0]["requested_at_unix_ms"] + 1),
            "after the invocation")

    def test_output_slots_receipts_and_times_are_exact(self) -> None:
        _must_reject(self, lambda item: [invocation.__setitem__(
            "declared_output_slots", []) for invocation in item[
                "capability_invocations"]], "exactly cover")
        _must_reject(self, lambda item: item["execution_receipts"][0].__setitem__(
            "output_artifact_receipt_refs", []), "reference every")
        _must_reject(self, lambda item: item["artifact_receipts"][1].__setitem__(
            "created_at_unix_ms", item["execution_receipts"][0][
                "ended_at_unix_ms"] + 1), "outside its execution")

    def test_event_order_causation_coverage_and_time_are_exact(self) -> None:
        _must_reject(self, lambda item: item["interaction_events"].reverse(),
                     "ordered contiguous")
        _must_reject(self, lambda item: item["interaction_events"][1].__setitem__(
            "causation_event_ref", None), "immediately preceding")
        _must_reject(self, lambda item: item["execution_receipts"][0].__setitem__(
            "event_refs", []), "exactly cover")
        _must_reject(self, lambda item: item["interaction_events"][0].__setitem__(
            "occurred_at_unix_ms", item["execution_receipts"][0][
                "started_at_unix_ms"] - 1), "outside its execution")
        _must_reject(self, lambda item: item["interaction_events"][1].__setitem__(
            "occurred_at_unix_ms", item["interaction_events"][0][
                "occurred_at_unix_ms"] - 1), "nondecreasing")
        equal = fixture_candidate()
        equal["interaction_events"][0]["occurred_at_unix_ms"] = equal[
            "interaction_events"][1]["occurred_at_unix_ms"]
        validate_reference_graph(equal)

    def test_event_cannot_reference_a_future_artifact_receipt(self) -> None:
        def future(item: dict) -> None:
            final_artifact = item["execution_receipts"][1]["input_artifacts"][0]
            final_artifact = next(
                receipt["artifact"] for receipt in item["artifact_receipts"]
                if receipt["artifact"]["artifact_ref"] == "fixture/attempt-2")
            item["interaction_events"][2]["artifact_refs"] = [final_artifact]
        _must_reject(self, future, "non-future")

    def test_event_artifact_requires_some_included_receipt(self) -> None:
        unknown = {"artifact_kind": "fixture", "artifact_ref": "fixture/unknown",
                   "artifact_sha256": "d" * 64}
        _must_reject(self, lambda item: item["interaction_events"][0].__setitem__(
            "artifact_refs", [unknown]), "included ArtifactReceipt")

    def test_artifact_inventory_is_exact_and_receipts_have_no_orphans(self) -> None:
        _must_reject(self, lambda item: item.__setitem__(
            "artifacts", item["artifacts"][:-1]), "exactly equal")

        def orphan(item: dict) -> None:
            receipt = copy.deepcopy(next(record for record in item["artifact_receipts"]
                                         if record["receipt_role"] == "declared_input"))
            receipt["artifact_receipt_id"] = f"artifact-receipt-{'d' * 64}"
            receipt["artifact_receipt_sha256"] = "d" * 64
            item["artifact_receipts"].append(receipt)
        _must_reject(self, orphan, "orphan")


if __name__ == "__main__":
    unittest.main()
