from __future__ import annotations

import copy
import unittest
from pathlib import Path
from unittest import mock

from decision_capsule_contract import (
    ContractError, canonical_json, derive_decision_capsule,
    derive_evaluation_branch, derive_structural_replay_closure,
    derive_structural_replay_manifest, load_golden, seal_evaluation_branch,
    seal_structural_replay_closure,
    seal_structural_replay_manifest, validate_evaluation_branch,
    validate_structural_replay_closure, validate_structural_replay_manifest,
)
from decision_capsule_contract.constants import ATTESTATION_FIELDS
from kernel_decision_contract import (seal_closure as seal_decision_closure,
                                      seal_cognitive_atom,
                                      seal_decision_transaction)
from kernel_operational_contract import seal_closure as seal_operational_closure
from kernel_operational_contract.records import (
    seal_artifact_receipt, seal_capability_invocation, seal_execution_receipt,
    seal_interaction_event,
)


ROOT = Path(__file__).resolve().parents[1]


def ref(record: dict, id_field: str, hash_field: str) -> dict[str, object]:
    return {id_field: record[id_field], hash_field: record[hash_field]}


def blank(record: dict, id_field: str, hash_field: str) -> dict:
    candidate = copy.deepcopy(record)
    candidate[id_field] = candidate[hash_field] = ""
    return candidate


def _reseal_retry_records(op: dict, new_first: dict) -> tuple[dict, dict[str, dict]]:
    old_invocation = op["capability_invocations"][1]
    invocation = blank(old_invocation, "invocation_id", "invocation_sha256")
    invocation["prior_execution_receipt_ref"] = ref(
        new_first, "execution_receipt_id", "execution_receipt_sha256")
    invocation = seal_capability_invocation(invocation)
    mappings: dict[str, dict] = {old_invocation["invocation_id"]: ref(
        invocation, "invocation_id", "invocation_sha256")}
    old_invocation_ref = ref(old_invocation, "invocation_id", "invocation_sha256")
    for index, old in enumerate(op["artifact_receipts"]):
        if old["producer_invocation_ref"] != old_invocation_ref:
            continue
        changed = blank(old, "artifact_receipt_id", "artifact_receipt_sha256")
        changed["producer_invocation_ref"] = mappings[old_invocation["invocation_id"]]
        changed = seal_artifact_receipt(changed)
        mappings[old["artifact_receipt_id"]] = ref(
            changed, "artifact_receipt_id", "artifact_receipt_sha256")
        op["artifact_receipts"][index] = changed
    return invocation, mappings


def _reseal_retry_events(op: dict, old_invocation: dict, invocation: dict,
                         mappings: dict[str, dict]) -> list[dict]:
    new_events, prior = [], None
    old_ref = ref(old_invocation, "invocation_id", "invocation_sha256")
    for old in op["interaction_events"]:
        if old["invocation_ref"] != old_ref:
            new_events.append(old)
            continue
        changed = blank(old, "event_id", "event_sha256")
        changed["invocation_ref"] = ref(invocation, "invocation_id", "invocation_sha256")
        changed["causation_event_ref"] = prior
        changed = seal_interaction_event(changed)
        prior = ref(changed, "event_id", "event_sha256")
        mappings[old["event_id"]] = prior
        new_events.append(changed)
    return new_events


def _lost_operational_closure(decision: dict) -> tuple[dict, dict[str, dict]]:
    op = copy.deepcopy(decision["operational_closure"])
    old_first, old_second = op["execution_receipts"]
    first = blank(old_first, "execution_receipt_id", "execution_receipt_sha256")
    first["outcome"], first["reason_codes"] = "lost", ["fixture_lost"]
    first = seal_execution_receipt(first)
    old_invocation = op["capability_invocations"][1]
    invocation, mappings = _reseal_retry_records(op, first)
    op["interaction_events"] = _reseal_retry_events(
        op, old_invocation, invocation, mappings)
    second = blank(old_second, "execution_receipt_id", "execution_receipt_sha256")
    second["invocation_ref"] = ref(invocation, "invocation_id", "invocation_sha256")
    second["prior_execution_receipt_ref"] = ref(
        first, "execution_receipt_id", "execution_receipt_sha256")
    second["event_refs"] = [mappings.get(item["event_id"], item)
                            for item in second["event_refs"]]
    second["output_artifact_receipt_refs"] = [
        mappings.get(item["artifact_receipt_id"], item)
        for item in second["output_artifact_receipt_refs"]]
    second = seal_execution_receipt(second)
    mappings[old_second["execution_receipt_id"]] = ref(
        second, "execution_receipt_id", "execution_receipt_sha256")
    op["capability_invocations"][1] = invocation
    op["execution_receipts"] = [first, second]
    op["artifact_receipts"].sort(key=lambda item: item["artifact_receipt_id"].encode())
    return seal_operational_closure(blank(op, "closure_id", "closure_sha256")), mappings


def lost_decision_closure(decision: dict) -> dict:
    candidate = copy.deepcopy(decision)
    operational, mappings = _lost_operational_closure(candidate)
    candidate["operational_closure"] = operational
    changed_atoms = []
    for atom in candidate["cognitive_atoms"]:
        source = atom["source"]
        reference = source["source_ref"]
        id_fields = [field for field in reference if field.endswith("_id")]
        if source["source_phase"] == "postdecision" and id_fields:
            replacement = mappings.get(reference[id_fields[0]])
            if replacement is not None:
                atom = blank(atom, "atom_id", "atom_sha256")
                atom["source"]["source_ref"] = replacement
                atom = seal_cognitive_atom(atom)
        changed_atoms.append(atom)
    candidate["cognitive_atoms"] = sorted(
        changed_atoms, key=lambda item: item["atom_id"].encode())
    return seal_decision_closure(blank(candidate, "closure_id", "closure_sha256"))


def _escaped_artifact(index: int) -> dict[str, object]:
    return {"artifact_kind": "reflection_report" if index == 0 else '"' * 160,
            "artifact_ref": f"{index:03d}" + "\\" * 4093,
            "artifact_sha256": f"{index + 1:064x}"}


def _max_receipts(decision: dict) -> tuple[list[dict], list[dict]]:
    op = decision["operational_closure"]
    input_base = next(item for item in op["artifact_receipts"]
                      if item["receipt_role"] == "declared_input")
    output_base = next(item for item in op["artifact_receipts"]
                       if item["receipt_role"] == "declared_output")
    inputs, outputs = [], []
    for index in range(32):
        member = blank(input_base, "artifact_receipt_id", "artifact_receipt_sha256")
        member.update({"artifact": _escaped_artifact(index), "slot": f"input-{index:02d}"})
        inputs.append(seal_artifact_receipt(member))
    for index in range(32, 64):
        member = blank(output_base, "artifact_receipt_id", "artifact_receipt_sha256")
        member.update({"artifact": _escaped_artifact(index), "slot": f"output-{index:02d}"})
        outputs.append(member)
    return inputs, outputs


def _max_transaction(decision: dict, inputs: list[dict]) -> dict:
    transaction = blank(decision["decision_transaction"],
                        "decision_transaction_id", "decision_transaction_sha256")
    transaction["read_artifact_receipt_refs"] = sorted([
        ref(item, "artifact_receipt_id", "artifact_receipt_sha256")
        for item in inputs], key=canonical_json)
    transaction["write_slots"] = [f"output-{index:02d}" for index in range(32, 64)]
    return seal_decision_transaction(transaction)


def worst_escaped_decision_closure(decision: dict) -> dict:
    inputs, pending_outputs = _max_receipts(decision)
    transaction = _max_transaction(decision, inputs)
    op = decision["operational_closure"]
    invocation = blank(op["capability_invocations"][0],
                       "invocation_id", "invocation_sha256")
    invocation.update({
        "correlation_id": transaction["decision_transaction_id"],
        "declared_output_slots": transaction["write_slots"],
        "input_artifact_receipt_refs": transaction["read_artifact_receipt_refs"],
    })
    invocation = seal_capability_invocation(invocation)
    invocation_reference = ref(invocation, "invocation_id", "invocation_sha256")
    outputs = []
    for member in pending_outputs:
        member["producer_invocation_ref"] = invocation_reference
        outputs.append(seal_artifact_receipt(member))
    receipt = blank(op["execution_receipts"][0],
                    "execution_receipt_id", "execution_receipt_sha256")
    receipt.update({"correlation_id": transaction["decision_transaction_id"],
                    "event_refs": [], "input_artifacts": sorted(
                        [item["artifact"] for item in inputs], key=canonical_json),
                    "invocation_ref": invocation_reference, "outcome": "succeeded",
                    "output_artifact_receipt_refs": sorted([
                        ref(item, "artifact_receipt_id", "artifact_receipt_sha256")
                        for item in outputs], key=canonical_json), "reason_codes": []})
    receipt = seal_execution_receipt(receipt)
    operational = blank(op, "closure_id", "closure_sha256")
    operational.update({"artifact_receipts": sorted(inputs + outputs,
                       key=lambda item: item["artifact_receipt_id"].encode()),
                       "artifacts": sorted([item["artifact"] for item in inputs + outputs],
                                           key=canonical_json),
                       "capability_invocations": [invocation],
                       "execution_receipts": [receipt], "interaction_events": []})
    candidate = blank(decision, "closure_id", "closure_sha256")
    candidate.update({"cognitive_atoms": [item for item in decision["cognitive_atoms"]
                                          if item["source"]["source_phase"] == "predecision"],
                      "decision_transaction": transaction,
                      "operational_closure": seal_operational_closure(operational)})
    return seal_decision_closure(candidate)


class DecisionCapsuleReplayGraphTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.outer = load_golden(ROOT)
        cls.capsule = cls.outer["decision_capsule"]
        cls.decision = cls.capsule["decision_closure"]
        cls.manifest = cls.capsule["replay_manifest"]
        cls.branch = cls.outer["evaluation_branch"]

    def test_manifest_is_every_exact_ordered_embedded_reference(self) -> None:
        expected = derive_structural_replay_manifest(self.decision)
        self.assertEqual(self.manifest, expected)
        op = self.decision["operational_closure"]
        counts = {
            "artifact_refs": len(op["artifacts"]),
            "artifact_receipt_refs": len(op["artifact_receipts"]),
            "capability_invocation_refs": len(op["capability_invocations"]),
            "interaction_event_refs": len(op["interaction_events"]),
            "execution_receipt_refs": len(op["execution_receipts"]),
        }
        for field, count in counts.items():
            self.assertEqual(len(self.manifest[field]), count)

    def test_each_manifest_inventory_rejects_omit_extra_duplicate_and_reorder(self) -> None:
        fields = (
            "artifact_refs", "artifact_receipt_refs", "capability_invocation_refs",
            "interaction_event_refs", "execution_receipt_refs",
            "predecision_atom_refs", "postdecision_atom_refs",
        )
        for field in fields:
            values = self.manifest[field]
            mutations = (values[:-1], values + [values[0]],
                         values + [self._foreign(field, values[0])], list(reversed(values)))
            for label, replacement in zip(("omit", "duplicate", "extra", "reorder"),
                                          mutations, strict=True):
                candidate = blank(self.manifest, "manifest_id", "manifest_sha256")
                candidate[field] = copy.deepcopy(replacement)
                with self.subTest(field=field, mutation=label), self.assertRaises(ContractError):
                    seal_structural_replay_manifest(candidate, self.decision)

    @staticmethod
    def _foreign(field: str, value: dict) -> dict:
        result = copy.deepcopy(value)
        if field == "artifact_refs":
            result["artifact_sha256"] = "0" * 64
            result["artifact_ref"] += "/foreign"
            return result
        hash_field = next(key for key in result if key.endswith("sha256"))
        id_field = next(key for key in result if key.endswith("_id"))
        result[hash_field] = "0" * 64
        prefix = result[id_field][:-64]
        result[id_field] = prefix + "0" * 64
        return result

    def test_pre_and_post_atom_partitions_are_exact_and_ordered(self) -> None:
        expected_pre = [item for item in self.decision["cognitive_atoms"]
                        if item["source"]["source_phase"] == "predecision"]
        expected_post = [item for item in self.decision["cognitive_atoms"]
                         if item["source"]["source_phase"] == "postdecision"]
        self.assertEqual(len(self.manifest["predecision_atom_refs"]), len(expected_pre))
        self.assertEqual(len(self.manifest["postdecision_atom_refs"]), len(expected_post))
        self.assertLessEqual(len(expected_pre) + len(expected_post), 256)
        candidate = blank(self.manifest, "manifest_id", "manifest_sha256")
        moved = candidate["predecision_atom_refs"].pop()
        candidate["postdecision_atom_refs"].append(moved)
        candidate["postdecision_atom_refs"].sort(key=lambda item: item["atom_id"].encode())
        with self.assertRaises(ContractError):
            seal_structural_replay_manifest(candidate, self.decision)

    def test_failed_lost_retry_and_success_attempts_are_never_trimmed(self) -> None:
        outcomes = [item["outcome"] for item in self.decision["operational_closure"]
                    ["execution_receipts"]]
        self.assertEqual(outcomes, ["failed", "succeeded"])
        lost = lost_decision_closure(self.decision)
        lost_outcomes = [item["outcome"] for item in lost["operational_closure"]
                         ["execution_receipts"]]
        self.assertEqual(lost_outcomes, ["lost", "succeeded"])
        manifest = derive_structural_replay_manifest(lost)
        self.assertEqual(len(manifest["capability_invocation_refs"]), 2)
        self.assertEqual(len(manifest["execution_receipt_refs"]), 2)
        self.assertIsNotNone(lost["operational_closure"]["capability_invocations"][1]
                             ["prior_execution_receipt_ref"])

    def test_full_semantic_projection_accepts_sixty_four_worst_escaped_artifacts(self) -> None:
        decision = worst_escaped_decision_closure(self.decision)
        self.assertEqual(len(decision["operational_closure"]["artifacts"]), 64)
        self.assertEqual(sum(item["artifact_kind"] == "reflection_report" for item in
                             decision["operational_closure"]["artifacts"]), 1)
        manifest = derive_structural_replay_manifest(decision)
        capsule = derive_decision_capsule(decision)
        branch = derive_evaluation_branch(capsule)
        outer = derive_structural_replay_closure(capsule, [])
        self.assertEqual(len(manifest["artifact_refs"]), 64)
        self.assertEqual(capsule["replay_manifest"], manifest)
        self.assertEqual(outer["evaluation_branch"], branch)
        validate_structural_replay_closure(outer)
        self.assertLessEqual(len(canonical_json(manifest)), 4 * 1024 * 1024)

    def test_branch_is_uniquely_derived_and_cross_capsule_substitution_fails(self) -> None:
        self.assertEqual(self.branch, derive_evaluation_branch(self.capsule))
        other = derive_decision_capsule(lost_decision_closure(self.decision))
        with self.assertRaises(ContractError):
            validate_evaluation_branch(self.branch, other)
        for field in ("capsule_ref", "decision_closure_ref", "manifest_ref"):
            candidate = blank(self.branch, "branch_id", "branch_sha256")
            candidate[field] = copy.deepcopy(derive_evaluation_branch(other)[field])
            with self.subTest(field=field), self.assertRaises(ContractError):
                seal_evaluation_branch(candidate, self.capsule)

    def test_branch_forbids_candidate_fields_and_result_drift(self) -> None:
        for mutation in ("candidate_capsule", "candidate_decision_closure"):
            changed = copy.deepcopy(self.branch)
            changed[mutation] = None
            with self.subTest(field=mutation), self.assertRaises(ContractError):
                validate_evaluation_branch(changed, self.capsule)
        changed = blank(self.branch, "branch_id", "branch_sha256")
        changed["comparison_result"] = "SEMANTICALLY_EQUIVALENT"
        with self.assertRaises(ContractError):
            seal_evaluation_branch(changed, self.capsule)

    def test_outer_binds_branch_and_dedicated_reflection_reports_stay_outer_only(self) -> None:
        self.assertNotIn("reflection_report_artifact_refs", self.capsule)
        changed = blank(self.outer, "closure_id", "closure_sha256")
        other = derive_decision_capsule(lost_decision_closure(self.decision))
        changed["evaluation_branch"] = derive_evaluation_branch(other)
        with self.assertRaises(ContractError):
            seal_structural_replay_closure(changed)
        empty = copy.deepcopy(self.outer)
        empty["closure_id"] = empty["closure_sha256"] = ""
        empty["reflection_report_artifact_refs"] = []
        seal_structural_replay_closure(empty)

    def test_nested_validation_calls_each_public_dependency_once(self) -> None:
        import decision_capsule_contract.capsule as capsule_module
        import decision_capsule_contract.closure as closure_module
        import kernel_decision_contract.closure as decision_closure_module
        with mock.patch.object(
                closure_module, "validate_decision_capsule",
                wraps=closure_module.validate_decision_capsule) as capsule_call:
            validate_structural_replay_closure(self.outer)
            self.assertEqual(capsule_call.call_count, 1)
        with mock.patch.object(
                capsule_module, "validate_decision_closure",
                wraps=capsule_module.validate_decision_closure) as decision_call:
            capsule_module.validate_decision_capsule(self.capsule)
            self.assertEqual(decision_call.call_count, 1)
        with mock.patch.object(
                decision_closure_module, "validate_closure_shape",
                wraps=decision_closure_module.validate_closure_shape) as shape_call:
            capsule_module.validate_decision_capsule(self.capsule)
            self.assertEqual(shape_call.call_count, 2)

    def test_all_replay_attestations_and_controls_fail_closed(self) -> None:
        records = (
            (self.manifest, lambda value: validate_structural_replay_manifest(
                value, self.decision)),
            (self.capsule, derive_evaluation_branch),
            (self.branch, lambda value: validate_evaluation_branch(value, self.capsule)),
            (self.outer, validate_structural_replay_closure),
        )
        for record, validator in records:
            for field in ATTESTATION_FIELDS:
                for replacement in (True, 0):
                    changed = copy.deepcopy(record)
                    changed["attestations"][field] = replacement
                    with self.subTest(kind=record["kind"], field=field,
                                      replacement=replacement), self.assertRaises(ContractError):
                        validator(changed)
        for record, validator in records[::2]:
            for field in ("effect_replay_allowed", "history_rewrite_allowed"):
                changed = copy.deepcopy(record)
                changed[field] = True
                with self.subTest(kind=record["kind"], field=field), self.assertRaises(ContractError):
                    validator(changed)


if __name__ == "__main__":
    unittest.main()
