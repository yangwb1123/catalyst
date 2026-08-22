from __future__ import annotations

import copy
import unittest
from pathlib import Path

from kernel_decision_contract import (
    ContractError, load_golden, seal_closure, seal_cognitive_atom,
    seal_decision_transaction,
)
from kernel_decision_contract.graph import (
    _validate_atom_roles, _validate_budget, _validate_selected_operation,
    _validate_times, validate_reference_graph,
)


ROOT = Path(__file__).resolve().parents[1]


def blank_outer(value: dict) -> dict:
    candidate = copy.deepcopy(value)
    candidate["closure_id"] = candidate["closure_sha256"] = ""
    return candidate


def reseal_atom(atom: dict) -> dict:
    atom = copy.deepcopy(atom)
    atom["atom_id"] = atom["atom_sha256"] = ""
    return seal_cognitive_atom(atom)


def atom_ref(atom: dict) -> dict[str, str]:
    return {"atom_id": atom["atom_id"], "atom_sha256": atom["atom_sha256"]}


class KernelDecisionReferenceGraphTest(unittest.TestCase):
    def setUp(self) -> None:
        self.closure = load_golden(ROOT)

    def replace_atom(self, changed: dict) -> dict:
        candidate = blank_outer(self.closure)
        old_type = changed["atom_type"]
        old_source = changed["source"]["source_kind"]
        index = next(index for index, atom in enumerate(candidate["cognitive_atoms"])
                     if atom["atom_type"] == old_type and
                     atom["source"]["source_kind"] == old_source)
        candidate["cognitive_atoms"][index] = reseal_atom(changed)
        candidate["cognitive_atoms"].sort(key=lambda item: item["atom_id"].encode())
        return candidate

    def resealed_role_transaction(self, mutate) -> dict:
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        mutate(transaction)
        return seal_decision_transaction(transaction)

    def test_goal_role_requires_exactly_one_goal_atom(self) -> None:
        def swap_goal_and_trigger(transaction: dict) -> None:
            transaction["goal_atom_ref"], transaction["trigger_atom_refs"][0] = (
                transaction["trigger_atom_refs"][0], transaction["goal_atom_ref"])

        wrong_goal = self.resealed_role_transaction(swap_goal_and_trigger)
        with self.assertRaisesRegex(ContractError, "only predecision goal"):
            _validate_atom_roles(self.closure["cognitive_atoms"], wrong_goal)

        atoms = copy.deepcopy(self.closure["cognitive_atoms"])
        index = next(index for index, atom in enumerate(atoms)
                     if atom["source"]["source_phase"] == "predecision" and
                     atom["atom_type"] == "preference")
        old_id = atoms[index]["atom_id"]
        changed = copy.deepcopy(atoms[index])
        changed["atom_type"] = "goal"
        atoms[index] = reseal_atom(changed)
        replacement = atom_ref(atoms[index])
        atoms.sort(key=lambda item: item["atom_id"].encode())

        def replace_extra_goal_ref(transaction: dict) -> None:
            for field in ("trigger_atom_refs", "guard_atom_refs"):
                transaction[field] = [replacement if item["atom_id"] == old_id else item
                                      for item in transaction[field]]
                transaction[field].sort(key=lambda item: item["atom_id"].encode())

        extra_goal = self.resealed_role_transaction(replace_extra_goal_ref)
        with self.assertRaisesRegex(ContractError, "only predecision goal"):
            _validate_atom_roles(atoms, extra_goal)

    def test_non_goal_trigger_and_guard_roles_remain_untyped(self) -> None:
        def swap_trigger_and_guard(transaction: dict) -> None:
            transaction["trigger_atom_refs"][0], transaction["guard_atom_refs"][0] = (
                transaction["guard_atom_refs"][0], transaction["trigger_atom_refs"][0])
            transaction["guard_atom_refs"].sort(key=lambda item: item["atom_id"].encode())

        transaction = self.resealed_role_transaction(swap_trigger_and_guard)
        _validate_atom_roles(self.closure["cognitive_atoms"], transaction)

    def test_fully_resealed_post_atom_task_and_binding_drift_rejected(self) -> None:
        post = next(atom for atom in self.closure["cognitive_atoms"]
                    if atom["source"]["source_phase"] == "postdecision")
        for path, value in (("task_binding", "drifted-task"),
                            ("bindings", "f" * 64)):
            changed = copy.deepcopy(post)
            if path == "task_binding":
                changed[path]["task_id"] = value
            else:
                changed[path]["context_sha256"] = value
            with self.subTest(path=path), self.assertRaises(ContractError):
                seal_closure(self.replace_atom(changed))

    def test_fully_resealed_declared_input_cannot_be_postdecision_source(self) -> None:
        changed = next(atom for atom in self.closure["cognitive_atoms"]
                       if atom["source"]["source_kind"] == "artifact_receipt")
        changed = copy.deepcopy(changed)
        input_receipt = next(receipt for receipt in self.closure["operational_closure"]
                             ["artifact_receipts"] if receipt["receipt_role"] == "declared_input")
        changed["source"]["source_ref"] = {
            "artifact_receipt_id": input_receipt["artifact_receipt_id"],
            "artifact_receipt_sha256": input_receipt["artifact_receipt_sha256"],
        }
        changed["validity"]["valid_from_unix_ms"] = self.closure[
            "decision_transaction"]["created_at_unix_ms"]
        with self.assertRaises(ContractError):
            seal_closure(self.replace_atom(changed))

    def test_fully_resealed_post_atom_cannot_predate_transaction(self) -> None:
        changed = next(atom for atom in self.closure["cognitive_atoms"]
                       if atom["source"]["source_kind"] == "capability_invocation")
        changed = copy.deepcopy(changed)
        changed["validity"]["valid_from_unix_ms"] = (
            self.closure["decision_transaction"]["created_at_unix_ms"] - 1)
        with self.assertRaises(ContractError):
            seal_closure(self.replace_atom(changed))

    def test_fully_resealed_post_atom_cannot_fall_between_transaction_and_source(self) -> None:
        changed = next(atom for atom in self.closure["cognitive_atoms"]
                       if atom["source"]["source_kind"] == "capability_invocation")
        changed = copy.deepcopy(changed)
        created = self.closure["decision_transaction"]["created_at_unix_ms"]
        requested = self.closure["operational_closure"]["capability_invocations"][0][
            "requested_at_unix_ms"]
        changed["validity"]["valid_from_unix_ms"] = (created + requested) // 2
        with self.assertRaisesRegex(ContractError, "predates its operational source"):
            seal_closure(self.replace_atom(changed))

    def test_transaction_read_and_predecision_temporal_edges(self) -> None:
        atoms = self.closure["cognitive_atoms"]
        transaction = self.closure["decision_transaction"]
        operational = self.closure["operational_closure"]

        late = copy.deepcopy(transaction)
        first_request = min(item["requested_at_unix_ms"]
                            for item in operational["capability_invocations"])
        late["created_at_unix_ms"] = first_request + 1
        with self.assertRaisesRegex(ContractError, "must not follow its first request"):
            _validate_times(atoms, late, operational)

        future_read = copy.deepcopy(operational)
        read_id = transaction["read_artifact_receipt_refs"][0]["artifact_receipt_id"]
        receipt = next(item for item in future_read["artifact_receipts"]
                       if item["artifact_receipt_id"] == read_id)
        receipt["created_at_unix_ms"] = transaction["created_at_unix_ms"] + 1
        with self.assertRaisesRegex(ContractError, "nonfuture declared-input"):
            _validate_times(atoms, transaction, future_read)

        for label, mutate in (
            ("future", lambda validity: validity.__setitem__(
                "valid_from_unix_ms", transaction["created_at_unix_ms"] + 1)),
            ("expired", lambda validity: validity.__setitem__(
                "valid_until_unix_ms", transaction["created_at_unix_ms"])),
        ):
            changed_atoms = copy.deepcopy(atoms)
            predecision = next(item for item in changed_atoms
                               if item["source"]["source_phase"] == "predecision")
            mutate(predecision["validity"])
            with self.subTest(label=label), self.assertRaisesRegex(
                    ContractError, "begins after|expired before"):
                _validate_times(changed_atoms, transaction, operational)

    def test_missing_exact_atom_reference_is_rejected(self) -> None:
        candidate = blank_outer(self.closure)
        candidate["decision_transaction"]["goal_atom_ref"]["atom_sha256"] = "0" * 64
        candidate["decision_transaction"]["goal_atom_ref"]["atom_id"] = "cognitive-atom-" + "0" * 64
        transaction = candidate["decision_transaction"]
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        candidate["decision_transaction"] = seal_decision_transaction(transaction)
        with self.assertRaises(ContractError):
            seal_closure(candidate)

    def test_fully_resealed_selected_projection_drift_rejected(self) -> None:
        mutations = (
            lambda value: value.__setitem__("idempotency_key", "drifted-idempotency"),
            lambda value: value.__setitem__("read_artifact_receipt_refs", []),
            lambda value: value.__setitem__("write_slots", []),
        )
        for index, mutation in enumerate(mutations):
            candidate = blank_outer(self.closure)
            transaction = candidate["decision_transaction"]
            transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
            mutation(transaction)
            candidate["decision_transaction"] = seal_decision_transaction(transaction)
            with self.subTest(index=index), self.assertRaises(ContractError):
                seal_closure(candidate)
        for field in ("capability", "requested_action_sha256"):
            candidate = blank_outer(self.closure)
            transaction = candidate["decision_transaction"]
            transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
            selected = next(option for option in transaction["options"]
                            if option["option_id"] == transaction["selected_option_id"])
            if field == "capability":
                selected[field]["capability_id"] = "drifted.capability"
            else:
                selected[field] = "0" + selected[field][1:]
            candidate["decision_transaction"] = seal_decision_transaction(transaction)
            with self.subTest(field=field), self.assertRaises(ContractError):
                seal_closure(candidate)

    def test_selected_projection_and_correlation_edges_are_independent(self) -> None:
        transaction = self.closure["decision_transaction"]
        operational = self.closure["operational_closure"]
        selected = next(option for option in transaction["options"]
                        if option["option_id"] == transaction["selected_option_id"])
        cases = (
            ("capability", lambda value: value["capability"].__setitem__(
                "capability_id", "drifted.capability")),
            ("action", lambda value: value.__setitem__("requested_action_sha256", "0" * 64)),
            ("idempotency", lambda value: value.__setitem__("idempotency_key", "drifted")),
            ("reads", lambda value: value.__setitem__("input_artifact_receipt_refs", [])),
            ("writes", lambda value: value.__setitem__("declared_output_slots", [])),
            ("invocation-correlation", lambda value: value.__setitem__("correlation_id", "drifted")),
        )
        for label, mutation in cases:
            changed = copy.deepcopy(operational)
            mutation(changed["capability_invocations"][0])
            with self.subTest(label=label), self.assertRaises(ContractError):
                _validate_selected_operation(transaction, changed)
        for array in ("interaction_events", "execution_receipts"):
            changed = copy.deepcopy(operational)
            changed[array][0]["correlation_id"] = "drifted"
            with self.subTest(array=array), self.assertRaises(ContractError):
                _validate_selected_operation(transaction, changed)

    def test_every_operational_record_class_binding_drift_rejected(self) -> None:
        arrays = ("artifact_receipts", "capability_invocations",
                  "interaction_events", "execution_receipts")
        for array in arrays:
            candidate = blank_outer(self.closure)
            candidate["operational_closure"][array][0]["bindings"]["context_sha256"] = "f" * 64
            with self.subTest(array=array), self.assertRaises(ContractError):
                seal_closure(candidate)

    def test_aggregate_all_receipts_exact_n_and_n_plus_one(self) -> None:
        atoms = self.closure["cognitive_atoms"]
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        operational = self.closure["operational_closure"]
        totals = {field: sum(receipt["observed_usage"][field]
                             for receipt in operational["execution_receipts"])
                  for field in operational["execution_receipts"][0]["observed_usage"]}
        transaction["budget"].update({
            "max_calls": max(len(operational["capability_invocations"]), totals["call_count"]),
            "max_cost_usd_micros": totals["cost_usd_micros"],
            "max_input_tokens": totals["input_tokens"],
            "max_network_bytes": totals["network_bytes"],
            "max_output_bytes": totals["output_bytes"],
            "max_output_tokens": totals["output_tokens"],
            "timeout_ms": totals["elapsed_ms"],
        })
        validate_reference_graph(atoms, transaction, operational)
        for usage_field in totals:
            changed = copy.deepcopy(operational)
            changed["execution_receipts"][0]["observed_usage"][usage_field] += 1
            with self.subTest(field=usage_field), self.assertRaisesRegex(
                    ContractError, "aggregate usage exceeds transaction budget"):
                _validate_budget(transaction, changed)
        changed = copy.deepcopy(operational)
        changed["capability_invocations"].append(changed["capability_invocations"][-1])
        with self.assertRaisesRegex(ContractError, "Invocation count"):
            _validate_budget(transaction, changed)

    def test_checked_aggregate_rejects_signed_i64_overflow(self) -> None:
        transaction = {"budget": {"max_calls": 2, "max_cost_usd_micros": 0,
                                   "timeout_ms": 0, "max_input_tokens": 0,
                                   "max_network_bytes": 0, "max_output_bytes": 0,
                                   "max_output_tokens": 0}}
        usage = {"call_count": 0, "cost_usd_micros": 0, "elapsed_ms": 0,
                 "input_tokens": 0, "network_bytes": 0, "output_bytes": 0,
                 "output_tokens": 0}
        for field in usage:
            first, second = copy.deepcopy(usage), copy.deepcopy(usage)
            first[field], second[field] = (2 ** 63) - 1, 1
            operational = {"capability_invocations": [{}, {}],
                           "execution_receipts": [{"observed_usage": first},
                                                  {"observed_usage": second}]}
            with self.subTest(field=field), self.assertRaisesRegex(
                    ContractError, "exceeds signed int64"):
                _validate_budget(transaction, operational)


if __name__ == "__main__":
    unittest.main()
