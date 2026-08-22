from __future__ import annotations

import copy
import hashlib
import json
import unittest
from pathlib import Path

from kernel_decision_contract import (
    ContractError, canonical_json, decode_closure, decode_cognitive_atom,
    decode_decision_transaction, load_golden, seal_closure, seal_cognitive_atom,
    seal_decision_transaction,
)
from kernel_decision_contract.closure import _atoms
from kernel_decision_contract.constants import (
    ATTESTATION_FIELDS, MAX_ATOMS, MAX_ATOM_BYTES, MAX_ATOM_SET_BYTES,
    MAX_CLOSURE_BYTES, MAX_TRANSACTION_BYTES,
)
from kernel_operational_contract.constants import MAX_ARRAY_ITEMS, MAX_STRING_BYTES


ROOT = Path(__file__).resolve().parents[1]
PHYSICAL_SHA256 = "93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c"
CLOSURE_SHA256 = "cdadf0e5fddbbda429939be4e68dc77dd0b52c0bb7e4fe955f1d485183908e58"


def oversized_atom_set(closure: dict) -> list[dict]:
    template = next(atom for atom in closure["cognitive_atoms"]
                    if atom["source"]["source_kind"] == "artifact")
    result = []
    for index in range(MAX_ATOMS):
        atom = copy.deepcopy(template)
        atom["atom_id"] = atom["atom_sha256"] = ""
        subject = f"aggregate-boundary-{index:03d}"
        atom["proposition"]["subject"] = subject
        atom["scope"]["object"] = subject
        atom["source"]["source_selector"] = "/" + ("x" * 4_094)
        result.append(seal_cognitive_atom(atom))
    result.sort(key=lambda item: item["atom_id"].encode())
    return result


def wide_canonical_tree(groups: int) -> list[list[str]]:
    row = ["x" * MAX_STRING_BYTES] * MAX_ARRAY_ITEMS
    return [row] * groups


class KernelDecisionContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.closure = load_golden(ROOT)

    def test_golden_physical_semantic_and_canonical_pins(self) -> None:
        path = ROOT / "docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"
        physical = path.read_bytes()
        self.assertEqual(hashlib.sha256(physical).hexdigest(), PHYSICAL_SHA256)
        self.assertEqual(physical[-1:], b"\n")
        self.assertNotEqual(physical[-2:], b"\n\n")
        self.assertEqual(self.closure["closure_sha256"], CLOSURE_SHA256)
        self.assertEqual(canonical_json(self.closure), physical[:-1])

    def test_exact_nineteen_atom_fields_and_twenty_two_false_attestations(self) -> None:
        self.assertEqual(len(self.closure["cognitive_atoms"]), 17)
        for atom in self.closure["cognitive_atoms"]:
            self.assertEqual(len(atom), 19)
            self.assertEqual(set(atom["attestations"]), ATTESTATION_FIELDS)
            self.assertEqual(len(atom["attestations"]), 22)
            self.assertFalse(any(atom["attestations"].values()))
            self.assertEqual(atom["effective_hardness"], "none")
            self.assertIs(atom["instruction_allowed"], False)

    def test_golden_covers_every_atom_type_source_and_hardness(self) -> None:
        atoms = self.closure["cognitive_atoms"]
        self.assertEqual(len({atom["atom_type"] for atom in atoms}), 16)
        self.assertEqual(
            {atom["source"]["source_kind"] for atom in atoms},
            {"artifact", "artifact_receipt", "capability_invocation", "cognitive_atom_v1",
             "evidence_record", "execution_receipt", "interaction_event", "work_intent"},
        )
        self.assertEqual(
            {atom["declared_hardness"] for atom in atoms},
            {"advisory", "contract", "invariant", "none", "preferred", "required"},
        )
        self.assertEqual(
            {atom["declared_authority"]["authority_kind"] for atom in atoms},
            {"approval_record", "architecture_decision", "contract_artifact", "none"},
        )

    def test_restricted_source_type_matrix_rejects_cross_feed(self) -> None:
        invalid_types = {
            "artifact_receipt": "goal",
            "capability_invocation": "goal",
            "cognitive_atom_v1": "goal",
            "evidence_record": "goal",
            "execution_receipt": "goal",
            "interaction_event": "goal",
            "work_intent": "actor",
        }
        for source_kind, atom_type in invalid_types.items():
            atom = copy.deepcopy(next(item for item in self.closure["cognitive_atoms"]
                                      if item["source"]["source_kind"] == source_kind))
            atom["atom_id"] = atom["atom_sha256"] = ""
            atom["atom_type"] = atom_type
            with self.subTest(source_kind=source_kind), self.assertRaisesRegex(
                    ContractError, "source_kind does not admit atom_type"):
                seal_cognitive_atom(atom)

    def test_hardness_authority_negative_matrix(self) -> None:
        atoms = self.closure["cognitive_atoms"]
        none_authority = {"authority_kind": "none", "authority_ref": None}
        approval = copy.deepcopy(next(item for item in atoms
                                     if item["declared_authority"]["authority_kind"] ==
                                     "approval_record")["declared_authority"])
        cases = []

        legacy = copy.deepcopy(next(item for item in atoms
                                    if item["source"]["source_kind"] == "cognitive_atom_v1"))
        legacy["declared_hardness"] = "advisory"
        cases.append(("legacy", legacy))

        observation = copy.deepcopy(next(item for item in atoms
                                         if item["atom_type"] == "observation"))
        inadmitted = copy.deepcopy(observation)
        inadmitted["declared_hardness"] = "advisory"
        cases.append(("inadmitted", inadmitted))
        observation["declared_authority"] = approval
        cases.append(("none-with-authority", observation))

        constraint = copy.deepcopy(next(item for item in atoms
                                        if item["declared_hardness"] == "contract"))
        constraint["declared_authority"] = copy.deepcopy(none_authority)
        cases.append(("contract-without-artifact", constraint))

        goal = copy.deepcopy(next(item for item in atoms if item["atom_type"] == "goal"))
        goal["declared_authority"] = copy.deepcopy(none_authority)
        cases.append(("required-without-authority", goal))

        decision = copy.deepcopy(next(item for item in atoms
                                      if item["source"]["source_kind"] == "artifact" and
                                      item["declared_hardness"] == "invariant"))
        decision["atom_type"] = "decision"
        decision["declared_hardness"] = "required"
        cases.append(("required-decision-contract-authority", decision))

        for label, atom in cases:
            atom["atom_id"] = atom["atom_sha256"] = ""
            with self.subTest(label=label), self.assertRaises(ContractError):
                seal_cognitive_atom(atom)

    def test_every_ineffective_field_promotion_is_rejected(self) -> None:
        for field, value in (("effective_hardness", "advisory"),
                             ("instruction_allowed", True)):
            atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
            atom["atom_id"] = atom["atom_sha256"] = ""
            atom[field] = value
            with self.subTest(field=field), self.assertRaises(ContractError):
                seal_cognitive_atom(atom)

        for attestation in sorted(ATTESTATION_FIELDS):
            atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
            atom["atom_id"] = atom["atom_sha256"] = ""
            atom["attestations"][attestation] = True
            transaction = copy.deepcopy(self.closure["decision_transaction"])
            transaction["decision_transaction_id"] = ""
            transaction["decision_transaction_sha256"] = ""
            transaction["attestations"][attestation] = True
            closure = copy.deepcopy(self.closure)
            closure["closure_id"] = closure["closure_sha256"] = ""
            closure["attestations"][attestation] = True
            for label, seal, value in (
                ("atom", seal_cognitive_atom, atom),
                ("transaction", seal_decision_transaction, transaction),
                ("closure", seal_closure, closure),
            ):
                with self.subTest(attestation=attestation, record=label), \
                        self.assertRaises(ContractError):
                    seal(value)

    def test_every_atom_and_transaction_reseals_exactly(self) -> None:
        for expected in self.closure["cognitive_atoms"]:
            blank = copy.deepcopy(expected)
            blank["atom_id"] = blank["atom_sha256"] = ""
            self.assertEqual(seal_cognitive_atom(blank), expected)
        expected = self.closure["decision_transaction"]
        blank = copy.deepcopy(expected)
        blank["decision_transaction_id"] = blank["decision_transaction_sha256"] = ""
        self.assertEqual(seal_decision_transaction(blank), expected)

    def test_empty_io_and_zero_trigger_transaction_is_valid(self) -> None:
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        transaction["guard_atom_refs"] += transaction["trigger_atom_refs"]
        transaction["guard_atom_refs"].sort(key=lambda item: item["atom_id"].encode())
        transaction["trigger_atom_refs"] = []
        transaction["read_artifact_receipt_refs"] = []
        transaction["write_preconditions"] = []
        transaction["write_slots"] = []
        sealed = seal_decision_transaction(transaction)
        self.assertEqual(sealed["trigger_atom_refs"], [])
        self.assertEqual(sealed["read_artifact_receipt_refs"], [])

    def test_sixty_five_trigger_refs_are_rejected(self) -> None:
        transaction = copy.deepcopy(self.closure["decision_transaction"])
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        refs = []
        for index in range(65):
            digest = hashlib.sha256(str(index).encode()).hexdigest()
            refs.append({"atom_id": f"cognitive-atom-{digest}", "atom_sha256": digest})
        transaction["trigger_atom_refs"] = sorted(refs, key=lambda item: item["atom_id"].encode())
        with self.assertRaisesRegex(ContractError, "trigger_atom_refs cardinality"):
            seal_decision_transaction(transaction)

    def test_every_transaction_n_plus_one_bound_is_rejected(self) -> None:
        source = self.closure["decision_transaction"]
        cases = {
            "guard_atom_refs": ([source["guard_atom_refs"][0]] * 65, "cardinality"),
            "options": ([source["options"][0]] * 17, "options cardinality"),
            "proof_obligations": ([source["proof_obligations"][0]] * 33,
                                  "proof_obligations cardinality"),
            "read_artifact_receipt_refs": ([source["read_artifact_receipt_refs"][0]] * 33,
                                           "exceeds the frozen bound"),
            "write_preconditions": ([source["write_preconditions"][0]] * 33,
                                    "write_preconditions cardinality"),
            "write_slots": ([source["write_slots"][0]] * 33, "cardinality"),
        }
        for field, (values, error) in cases.items():
            transaction = copy.deepcopy(source)
            transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
            transaction[field] = copy.deepcopy(values)
            with self.subTest(field=field), self.assertRaisesRegex(ContractError, error):
                seal_decision_transaction(transaction)
        transaction = copy.deepcopy(source)
        transaction["decision_transaction_id"] = transaction["decision_transaction_sha256"] = ""
        transaction["proof_obligations"][0]["required_evidence_kinds"] = [
            f"evidence-{index:02d}" for index in range(17)]
        with self.assertRaisesRegex(ContractError, "cardinality"):
            seal_decision_transaction(transaction)

    def test_atom_count_selector_and_document_n_plus_one_bounds(self) -> None:
        closure = copy.deepcopy(self.closure)
        closure["closure_id"] = closure["closure_sha256"] = ""
        closure["cognitive_atoms"] = [closure["cognitive_atoms"][0]] * (MAX_ATOMS + 1)
        with self.assertRaisesRegex(ContractError, "cognitive_atoms cardinality"):
            seal_closure(closure)
        atom = copy.deepcopy(next(item for item in self.closure["cognitive_atoms"]
                                  if item["source"]["source_kind"] == "artifact"))
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["source"]["source_selector"] = "/" + ("x" * 4_096)
        with self.assertRaisesRegex(ContractError, "source_selector.*4096"):
            seal_cognitive_atom(atom)
        cases = ((decode_cognitive_atom, MAX_ATOM_BYTES),
                 (decode_decision_transaction, MAX_TRANSACTION_BYTES),
                 (decode_closure, MAX_CLOSURE_BYTES))
        for decoder, maximum in cases:
            with self.subTest(maximum=maximum), self.assertRaisesRegex(
                    ContractError, "input exceeds"):
                decoder(b" " * (maximum + 1))

    def test_atom_set_aggregate_n_plus_one_bound_is_rejected(self) -> None:
        atoms = oversized_atom_set(self.closure)
        self.assertGreater(len(canonical_json(atoms)), MAX_ATOM_SET_BYTES)
        with self.assertRaisesRegex(ContractError, "aggregate byte ceiling"):
            _atoms(atoms)

    def test_public_canonical_json_enforces_twenty_mib(self) -> None:
        accepted = canonical_json(wide_canonical_tree(4))
        self.assertGreater(len(accepted), 16 * 1_048_576)
        self.assertLessEqual(len(accepted), MAX_CLOSURE_BYTES)
        with self.assertRaisesRegex(ContractError, "canonical JSON exceeds"):
            canonical_json(wide_canonical_tree(5))

    def test_legacy_id_and_digest_are_independent_exact_upstream_values(self) -> None:
        atom = next(item for item in self.closure["cognitive_atoms"]
                    if item["source"]["source_kind"] == "cognitive_atom_v1")
        atom = copy.deepcopy(atom)
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["source"]["source_ref"] = {
            "atom_id": "atom-99045a525632c18aec6b1c783ba1925e4603b4378b389e5ce86621ab25b145ae",
            "canonical_sha256": "3905ee9fd8293924644dd5d9a1da522ffe944dc58db51a26ee6c584e1335ce20",
        }
        self.assertNotIn(atom["source"]["source_ref"]["canonical_sha256"],
                         atom["source"]["source_ref"]["atom_id"])
        seal_cognitive_atom(atom)

    def test_evidence_record_id_is_short_text_not_identifier(self) -> None:
        atom = next(item for item in self.closure["cognitive_atoms"]
                    if item["source"]["source_kind"] == "evidence_record")
        atom = copy.deepcopy(atom)
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["source"]["source_ref"]["record_id"] = "fixture evidence record / v1"
        seal_cognitive_atom(atom)

    def test_artifact_ref_proposition_uses_identifier_grammar(self) -> None:
        atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
        atom["atom_id"] = atom["atom_sha256"] = ""
        atom["proposition"]["object_type"] = "artifact_ref"
        for invalid in ("artifact with spaces", "artifact$illegal"):
            atom["proposition"]["object_value"] = invalid
            with self.assertRaises(ContractError):
                seal_cognitive_atom(atom)

    def test_scope_is_exact_and_caller_declared_consistent(self) -> None:
        atom = copy.deepcopy(self.closure["cognitive_atoms"][0])
        atom["atom_id"] = atom["atom_sha256"] = ""
        for field, value in (("project", "other-project"), ("object", "other.object")):
            changed = copy.deepcopy(atom)
            changed["scope"][field] = value
            with self.assertRaises(ContractError):
                seal_cognitive_atom(changed)
        atom["scope"]["module"] = None
        atom["scope"]["object"] = None
        seal_cognitive_atom(atom)

    def test_draft_2020_schema_validates_golden_and_forbidden_scalars(self) -> None:
        try:
            from jsonschema import Draft202012Validator
            from referencing import Registry, Resource
        except ImportError as error:
            self.skipTest(f"optional jsonschema/referencing unavailable: {error}")
        schema = json.loads((ROOT / "docs/contracts/kernel-decision-reference-core-v1.schema.json").read_text())
        operational = json.loads((ROOT / "docs/contracts/kernel-operational-reference-core-v1.schema.json").read_text())
        Draft202012Validator.check_schema(schema)
        registry = Registry().with_resource(operational["$id"], Resource.from_contents(operational))
        validator = Draft202012Validator(schema, registry=registry)
        validator.validate(self.closure)
        identifier_validator = validator.evolve(schema=schema["$defs"]["identifier"])
        wire_validator = validator.evolve(schema=schema["$defs"]["wire_text"])
        pointer_schema = schema["$defs"]["source"]["properties"]["source_selector"]["oneOf"][0]
        pointer_validator = validator.evolve(schema=pointer_schema)
        for invalid in ("valid\n", "valid\r"):
            self.assertTrue(list(identifier_validator.iter_errors(invalid)))
        for invalid in ("x\x01", "x\u0085", "x\u2028", "x\u2029", "x\u202e"):
            self.assertTrue(list(wire_validator.iter_errors(invalid)))
        for invalid in ("/valid\n", "/valid\r", "/x\u202e"):
            self.assertTrue(list(pointer_validator.iter_errors(invalid)))


if __name__ == "__main__":
    unittest.main()
