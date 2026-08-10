#!/usr/bin/env python3
"""Golden, semantic and adversarial tests for CognitiveAtom shadow projection."""

from __future__ import annotations

import copy
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import cognitive_atom_contract_check as cognitive  # noqa: E402
from governance_contract import compute_record_digest, validate_record_set  # noqa: E402


class CognitiveAtomContractCheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / "docs/contracts/fixtures/cognitive-atom-projection-v1.json"
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def records(self):
        return copy.deepcopy(self.golden["source_records"])

    @staticmethod
    def resign(records):
        for record in records:
            record["integrity"]["canonical_sha256"] = compute_record_digest(record)
        return records

    def atoms(self):
        return cognitive.project_record_set(self.records(), self.golden["task_id"])

    def claim_type_records(self, claim_types):
        evidence, template = self.records()
        states = {
            "assumption": "testing", "constraint": "candidate", "decision": "proposed",
            "fact": "contested", "hypothesis": "open", "inference": "candidate",
            "lesson": "candidate", "proposal": "submitted", "unknown": "investigating",
        }
        records = [evidence]
        for index, claim_type in enumerate(claim_types, 1):
            claim = copy.deepcopy(template)
            claim["metadata"].update(record_id=f"kcr-{index:04d}",
                                     aggregate_id=f"claim-{claim_type}-{index}")
            claim["spec"].update(claim_type=claim_type,
                                 confidence_micros=(300_000 + index if claim_type in
                                                    {"assumption", "hypothesis", "inference"}
                                                    else None),
                                 queue_ref=(f"queue:{index}" if claim_type == "unknown" else None),
                                 validation_plan=(self._plan(index) if claim_type in
                                                  {"assumption", "hypothesis"} else None))
            claim["status"]["state"] = states[claim_type]
            records.append(claim)
        return self.resign(records)

    @staticmethod
    def _plan(index):
        return {"due_at_unix_ms": 1700000003000 + index, "impact_if_false": "revise",
                "method": "recheck", "owner_id": f"owner-{index}",
                "required_evidence_types": ["test_run"]}

    def test_golden_projection_is_projected_shadow(self):
        self.assertEqual(cognitive.validate_golden_fixture(ROOT), [])
        atoms = self.atoms()
        expected = self.golden["expected"]
        self.assertEqual(cognitive.canonical_atom_payload(atoms[0]).decode(),
                         expected["canonical_atom_payload_json"])
        self.assertEqual(cognitive.canonical_json(atoms[0]).decode(),
                         expected["canonical_atom_json"])
        self.assertEqual(cognitive.compute_atom_digest(atoms[0]),
                         expected["canonical_atom_sha256"])
        self.assertEqual(cognitive.compute_atom_set_digest(atoms), expected["atom_set_sha256"])
        self.assertIn("PROJECTED_SHADOW", cognitive.SUCCESS)
        for forbidden_claim in ("truth", "authority", "instruction", "hard-guard",
                                "transition", "completion", "effect"):
            self.assertIn(f"no {forbidden_claim}", cognitive.SUCCESS)

    def test_golden_expected_identity_set_and_closure_bytes_match(self):
        atoms = self.atoms()
        expected = self.golden["expected"]
        claim = self.records()[1]
        _, closure_bytes, closure_digest = cognitive.source_closure(claim, self.records())
        self.assertEqual(atoms[0]["metadata"]["atom_id"], expected["atom_id"])
        self.assertEqual(cognitive.canonical_json(atoms).decode(),
                         expected["canonical_atom_set_json"])
        self.assertEqual(closure_bytes.decode(), expected["canonical_source_closure_json"])
        self.assertEqual(closure_digest, expected["source_closure_sha256"])
        self.assertEqual(atoms[0]["source"]["closure_byte_count"], len(closure_bytes))

    def test_seven_types_and_shadow_states_project_while_two_types_are_ignored(self):
        claim_types = ["assumption", "constraint", "decision", "fact", "hypothesis",
                       "inference", "lesson", "proposal", "unknown"]
        records = self.claim_type_records(claim_types)
        self.assertEqual(validate_record_set(records), [])
        atoms = cognitive.project_record_set(records, "task-nine-types")
        projected = {atom["spec"]["atom_type"]: atom for atom in atoms}
        self.assertEqual(set(projected), {"assumption", "constraint", "decision", "fact",
                                          "hypothesis", "inference", "unknown"})
        for claim in records[1:]:
            claim_type = claim["spec"]["claim_type"]
            if claim_type in projected:
                self.assertEqual(projected[claim_type]["spec"]["epistemic_state"],
                                 claim["status"]["state"])

    def test_confidence_is_copied_only_for_three_projectable_types(self):
        records = self.claim_type_records(["fact", "assumption", "hypothesis", "inference"])
        atoms = cognitive.project_record_set(records, "task-confidence")
        by_type = {atom["spec"]["atom_type"]: atom for atom in atoms}
        self.assertIsNone(by_type["fact"]["spec"]["projection_confidence_micros"])
        claims = {record["spec"]["claim_type"]: record for record in records[1:]}
        for claim_type in ("assumption", "hypothesis", "inference"):
            self.assertEqual(by_type[claim_type]["spec"]["projection_confidence_micros"],
                             claims[claim_type]["spec"]["confidence_micros"])

    def test_source_closure_follows_all_reference_classes_transitively(self):
        records = self.records()
        successor = copy.deepcopy(records[0])
        successor["metadata"].update(record_id="evr-0002", sequence=2,
                                     created_at_unix_ms=1700000000500,
                                     supersedes_record_ids=["evr-0001"])
        derived = copy.deepcopy(records[1])
        derived["metadata"].update(record_id="kcr-0002", aggregate_id="claim-inference")
        derived["spec"].update(claim_type="inference", confidence_micros=700000,
                               supporting_evidence_record_ids=["evr-0002"],
                               derived_from_claim_record_ids=["kcr-0001"])
        records = self.resign([records[0], successor, records[1], derived])
        closure, encoded, digest = cognitive.source_closure(records[3], records)
        self.assertEqual([item["metadata"]["record_id"] for item in closure],
                         ["evr-0001", "evr-0002", "kcr-0001", "kcr-0002"])
        atom = next(item for item in cognitive.project_record_set(records, "task-closure")
                    if item["source"]["claim_record_id"] == "kcr-0002")
        self.assertEqual(atom["source"]["closure_byte_count"], len(encoded))
        self.assertEqual(atom["source"]["closure_sha256"], digest)

    def test_projection_drift_is_rejected(self):
        records, atoms = self.records(), self.atoms()
        atoms[0]["spec"]["proposition"]["object_value"] = "structurally valid drift"
        atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
        source_raw = cognitive.canonical_json(records)
        atom_raw = cognitive.canonical_json(atoms)
        issues = cognitive.check_projection_bytes(self.golden["task_id"], source_raw, atom_raw)
        self.assertTrue(any("does not exactly match" in issue for issue in issues), issues)
        self.assertEqual(cognitive.validate_projection(records, self.golden["task_id"], atoms),
                         issues)

    def test_authority_instruction_hardness_state_and_confidence_fail_closed(self):
        mutators = [
            lambda atom: atom["spec"].update(authority_ref="approval:1"),
            lambda atom: atom["spec"].update(instruction_allowed=True),
            lambda atom: atom["spec"].update(hardness="required"),
            lambda atom: atom["spec"].update(epistemic_state="confirmed"),
            lambda atom: atom["spec"].update(projection_confidence_micros=1),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                atoms = self.atoms()
                mutate(atoms[0])
                atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
                self.assertTrue(cognitive.validate_atom_set(atoms))

    def test_unknown_fields_digest_and_atom_identifier_fail_closed(self):
        mutators = [
            lambda atom: atom.update(extra="x"),
            lambda atom: atom["integrity"].update(canonical_sha256="0" * 64),
            lambda atom: atom["metadata"].update(atom_id="atom-not-a-digest"),
        ]
        for index, mutate in enumerate(mutators):
            atoms = self.atoms()
            mutate(atoms[0])
            if index != 1:
                atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
            self.assertTrue(cognitive.validate_atom_set(atoms))

    def test_proposition_object_type_and_value_must_correspond(self):
        mismatches = [
            ("artifact_ref", "not an identifier"),
            ("boolean", 1),
            ("integer", True),
            ("null", "not-null"),
            ("string", None),
        ]
        for object_type, object_value in mismatches:
            with self.subTest(object_type=object_type):
                atoms = self.atoms()
                atoms[0]["spec"]["proposition"].update(object_type=object_type,
                                                        object_value=object_value)
                atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
                issues = cognitive.validate_atom_set(atoms)
                self.assertTrue(any("does not match object_type" in issue for issue in issues),
                                issues)

    def test_validity_order_and_evidence_disjointness_are_enforced(self):
        atoms = self.atoms()
        validity = atoms[0]["spec"]["validity"]
        validity["valid_until_unix_ms"] = validity["valid_from_unix_ms"]
        atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
        issues = cognitive.validate_atom_set(atoms)
        self.assertTrue(any("must be greater" in issue for issue in issues), issues)
        atoms = self.atoms()
        reference = atoms[0]["spec"]["supporting_evidence_record_ids"][0]
        atoms[0]["spec"]["contradicting_evidence_record_ids"] = [reference]
        atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
        issues = cognitive.validate_atom_set(atoms)
        self.assertTrue(any("must be disjoint" in issue for issue in issues), issues)

    def test_atom_id_is_recomputed_from_embedded_source_and_metadata(self):
        mutators = [
            lambda atom: atom["metadata"].update(task_id="fixture-task-002"),
            lambda atom: atom["source"].update(canonical_sha256="0" * 64),
            lambda atom: atom["metadata"].update(context_sha256="0" * 64),
            lambda atom: atom["metadata"].update(policy_sha256="0" * 64),
            lambda atom: atom["metadata"].update(source_tree_sha256="0" * 64),
            lambda atom: atom["metadata"].update(source_revision="revision-two"),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                atoms = self.atoms()
                mutate(atoms[0])
                atoms[0]["integrity"]["canonical_sha256"] = cognitive.compute_atom_digest(atoms[0])
                issues = cognitive.validate_atom_set(atoms)
                self.assertTrue(any("embedded source/metadata identity" in issue
                                    for issue in issues), issues)

    def test_duplicate_keys_noncanonical_float_and_oversized_set_are_rejected(self):
        cases = {
            "duplicate JSON key": b'[{"a":1,"a":2}]',
            "not exact compact canonical": b'[ {"a":1} ]',
            "floating JSON number": b'[{"a":1.5}]',
            "exceeds 1048576 bytes": b"[" + b" " * 1_048_576 + b"]",
        }
        for expected, raw in cases.items():
            with self.subTest(expected=expected):
                source = cognitive.canonical_json(self.records())
                issues = cognitive.check_projection_bytes(self.golden["task_id"], source, raw)
                self.assertTrue(any(expected in issue for issue in issues), issues)

    def test_atom_and_count_limits_are_fail_closed(self):
        atom = self.atoms()[0]
        long_ids = [f"a{index:03d}-" + "x" * 155 for index in range(256)]
        for field in ("supporting_evidence_record_ids", "contradicting_evidence_record_ids",
                      "derived_from_claim_record_ids"):
            atom["spec"][field] = long_ids
        atom["spec"]["proposition"]["object_value"] = "x" * 16_384
        with self.assertRaisesRegex(ValueError, "atom exceeds 131072 bytes"):
            cognitive.canonical_atom_payload(atom)
        issues = cognitive.validate_atom_set([self.atoms()[0]] * 257)
        self.assertTrue(any("1..256" in issue for issue in issues), issues)

    def test_invalid_source_task_and_no_projectable_claims_are_rejected(self):
        records = self.records()
        records[1]["status"]["state"] = "confirmed"
        self.resign(records)
        with self.assertRaisesRegex(ValueError, "authoritative state"):
            cognitive.project_record_set(records, "task-invalid-source")
        with self.assertRaisesRegex(ValueError, "bounded identifier"):
            cognitive.project_record_set(self.records(), "INVALID TASK")
        ignored = self.claim_type_records(["lesson", "proposal"])
        with self.assertRaisesRegex(ValueError, "no projectable"):
            cognitive.project_record_set(ignored, "task-no-projectable")

    def test_cli_exact_reprojection_and_narrow_success_message(self):
        with tempfile.TemporaryDirectory() as directory:
            source_path, atom_path = Path(directory) / "source.json", Path(directory) / "atoms.json"
            source_path.write_bytes(cognitive.canonical_json(self.records()))
            atom_path.write_bytes(cognitive.canonical_json(self.atoms()))
            stdout, stderr = io.StringIO(), io.StringIO()
            arguments = [str(ROOT), self.golden["task_id"], str(source_path), str(atom_path)]
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = cognitive.main(arguments)
        self.assertEqual(result, 0)
        self.assertEqual(stdout.getvalue(), cognitive.SUCCESS + "\n")
        self.assertEqual(stderr.getvalue(), "")

    def test_golden_cli_and_expected_drift_are_checked(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = cognitive.main(["--golden", str(ROOT)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, cognitive.SUCCESS + "\n", ""))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs/contracts/fixtures/cognitive-atom-projection-v1.json"
            path.parent.mkdir(parents=True)
            fixture = copy.deepcopy(self.golden)
            fixture["expected"]["atom_id"] = "atom-" + "0" * 64
            path.write_text(json.dumps(fixture), encoding="utf-8")
            issues = cognitive.validate_golden_fixture(root)
        self.assertTrue(any("atom_id: golden value mismatch" in issue for issue in issues), issues)


if __name__ == "__main__":
    unittest.main()
