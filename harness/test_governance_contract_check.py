#!/usr/bin/env python3
"""Golden and adversarial tests for the Governance Evidence/Claim v1 checker."""

from __future__ import annotations

import copy
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import governance_contract_check as governance  # noqa: E402


class GovernanceContractCheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / "docs/contracts/fixtures/governance-evidence-claim-v1.json"
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def records(self):
        return copy.deepcopy([entry["record"] for entry in self.golden["records"]])

    def resign(self, records):
        for record in records:
            record["integrity"]["canonical_sha256"] = governance.compute_record_digest(record)
        return records

    def issues_after(self, mutate, *, resign=True):
        records = self.records()
        mutate(records)
        if resign:
            self.resign(records)
        return governance.validate_record_set(records)

    def assert_issue(self, expected, mutate, *, resign=True):
        issues = self.issues_after(mutate, resign=resign)
        self.assertTrue(any(expected in issue for issue in issues), issues)

    def test_golden_record_set_is_structurally_valid(self):
        records = self.records()
        raw = governance.canonical_json(records)
        self.assertEqual(governance.check_record_set_bytes(raw), [])
        self.assertEqual(governance.validate_golden_fixture(ROOT), [])

    def test_golden_payload_full_bytes_and_digests_match(self):
        for entry in self.golden["records"]:
            record, expected = entry["record"], entry["expected"]
            self.assertEqual(governance.canonical_record_payload(record).decode(),
                             expected["canonical_payload_json"])
            self.assertEqual(governance.canonical_json(record).decode(),
                             expected["canonical_record_json"])
            self.assertEqual(governance.compute_record_digest(record),
                             expected["canonical_sha256"])

    def test_authoritative_state_is_rejected(self):
        def mutate(records):
            records[1]["status"]["state"] = "confirmed"

        self.assert_issue("authoritative state is unavailable", mutate)

    def test_duplicate_keys_whitespace_float_and_large_integer_fail_closed(self):
        cases = {
            "duplicate JSON key": b'[{"a":1,"a":2}]',
            "not exact compact canonical": b'[ {"a":1} ]',
            "floating JSON number": b'[{"a":1.5}]',
            "outside signed int64": b'[{"a":9223372036854775808}]',
        }
        for expected, raw in cases.items():
            with self.subTest(expected=expected):
                self.assertTrue(any(expected in issue for issue in
                                    governance.check_record_set_bytes(raw)))

    def test_array_string_and_record_set_limits_fail_closed(self):
        records = self.records()
        records[0]["spec"]["subjects"] = [f"subject:{index:03d}" for index in range(257)]
        issues = governance.validate_record_set(records)
        self.assertTrue(any("array exceeds 256 items" in issue for issue in issues), issues)
        records = self.records()
        records[1]["spec"]["object_value"] = "😀" * 4097
        issues = governance.validate_record_set(records)
        self.assertTrue(any("string exceeds 16384 UTF-8 bytes" in issue for issue in issues), issues)
        oversized_set = [copy.deepcopy(self.records()[0]) for _ in range(257)]
        issues = governance.validate_record_set(oversized_set)
        self.assertTrue(any("1..256 records" in issue for issue in issues), issues)

    def test_record_digest_helpers_reject_oversized_record(self):
        record = self.records()[1]
        for field, prefix in (
                ("supporting_evidence_record_ids", "e"),
                ("contradicting_evidence_record_ids", "x"),
                ("derived_from_claim_record_ids", "c")):
            record["spec"][field] = [
                f"{prefix}:{index:03d}:" + "x" * 140 for index in range(256)
            ]
        record["spec"]["object_value"] = "x" * 16_384
        with self.assertRaisesRegex(ValueError, "record exceeds 131072 bytes"):
            governance.canonical_record_payload(record)
        with self.assertRaisesRegex(ValueError, "record exceeds 131072 bytes"):
            governance.compute_record_digest(record)
        issues = governance.validate_record_set([record])
        self.assertTrue(any("record exceeds 131072 bytes" in issue for issue in issues), issues)

    def test_record_digest_helpers_control_memory_exhaustion(self):
        record = self.records()[0]
        with patch("governance_contract.codec.canonical_json", side_effect=MemoryError):
            with self.assertRaisesRegex(ValueError, "record canonicalization exhausted memory"):
                governance.canonical_record_payload(record)
        with patch("governance_contract.codec.canonical_record_payload",
                   side_effect=MemoryError):
            with self.assertRaisesRegex(ValueError, "record digest processing exhausted memory"):
                governance.compute_record_digest(record)

    def test_forbidden_unicode_bool_integer_unknown_field_and_digest_mismatch(self):
        issues = governance.check_record_set_bytes(r'[{"a":"\u202e"}]'.encode("ascii"))
        self.assertTrue(any("forbidden Unicode" in issue for issue in issues), issues)
        self.assert_issue("expected integer", lambda records:
                          records[0]["metadata"].update(sequence=True))
        self.assert_issue("unknown fields", lambda records:
                          records[0].update(unexpected_member="x"))
        self.assert_issue("digest mismatch", lambda records:
                          records[0]["integrity"].update(canonical_sha256="0" * 64), resign=False)

    def test_arbitrary_shape_invalid_records_return_issues_without_crashing(self):
        mutators = [
            lambda records: records[0].update(metadata=[]),
            lambda records: records[0].update(spec=None),
            lambda records: records[1]["spec"].update(claim_type=[]),
            lambda records: records[1]["spec"].update(object_type=[]),
            lambda records: records[1]["spec"].update(supporting_evidence_record_ids=[{}]),
            lambda records: records[0]["spec"].update(source_trust=[]),
            lambda records: records[0]["metadata"].update(supersedes_record_ids=[{}]),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                records = self.records()
                mutate(records)
                issues = governance.validate_record_set(records)
                self.assertTrue(issues)

    def test_missing_nested_fields_and_unhashable_enums_fail_closed(self):
        mutators = [
            lambda records: records[0].update(metadata={}),
            lambda records: records[0]["spec"]["collector"].update(collector_type=[]),
            lambda records: records[0]["status"].update(state=[]),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                records = self.records()
                mutate(records)
                self.assertTrue(governance.validate_record_set(records))

    def test_extreme_depth_and_integer_lexemes_fail_closed(self):
        cases = [
            b"[" * 100_000 + b"0" + b"]" * 100_000,
            b"[" + b"9" * 5_000 + b"]",
        ]
        for raw in cases:
            with self.subTest(size=len(raw)):
                issues = governance.check_record_set_bytes(raw)
                self.assertTrue(issues)

    def test_cli_reports_extreme_input_without_traceback(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "extreme.json"
            path.write_bytes(b"[" * 100_000 + b"0" + b"]" * 100_000)
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = governance.main([str(ROOT), str(path)])
        self.assertEqual(result, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("ERROR: JSON depth exceeds 16", stderr.getvalue())

    def test_cli_bounded_read_rejects_large_file_and_memory_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "oversized.json"
            with path.open("wb") as stream:
                stream.truncate(1_048_577)
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = governance.main([str(ROOT), str(path)])
            self.assertEqual(result, 1)
            self.assertIn("ERROR: record set exceeds 1048576 bytes", stderr.getvalue())
            with patch.object(Path, "open", side_effect=MemoryError):
                stdout, stderr = io.StringIO(), io.StringIO()
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    result = governance.main([str(ROOT), str(path)])
            self.assertEqual(result, 2)
            self.assertIn("bounded read exhausted memory", stderr.getvalue())

    def test_golden_cli_bounded_read_is_controlled(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs/contracts/fixtures/governance-evidence-claim-v1.json"
            path.parent.mkdir(parents=True)
            with path.open("wb") as stream:
                stream.truncate(1_048_577)
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = governance.main(["--golden", str(root)])
            self.assertEqual(result, 1)
            self.assertIn("golden fixture exceeds 1048576 bytes", stderr.getvalue())
            with patch.object(Path, "open", side_effect=MemoryError):
                stdout, stderr = io.StringIO(), io.StringIO()
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    result = governance.main(["--golden", str(root)])
            self.assertEqual(result, 1)
            self.assertIn("bounded read exhausted memory", stderr.getvalue())

    def test_cli_json_processing_memory_failures_are_controlled(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "records.json"
            path.write_bytes(governance.canonical_json(self.records()))
            cases = (
                ("governance_contract.codec.json.loads", "JSON decoding exhausted memory"),
                ("governance_contract.codec.json.dumps",
                 "canonical JSON processing exhausted memory"),
                ("governance_contract_check.check_record_set_bytes",
                 "governance contract processing exhausted memory"),
            )
            for target, expected in cases:
                with self.subTest(target=target), patch(target, side_effect=MemoryError):
                    stdout, stderr = io.StringIO(), io.StringIO()
                    with redirect_stdout(stdout), redirect_stderr(stderr):
                        result = governance.main([str(ROOT), str(path)])
                    self.assertEqual(result, 1)
                    self.assertEqual(stdout.getvalue(), "")
                    self.assertIn(f"ERROR: {expected}", stderr.getvalue())
                    self.assertNotIn("Traceback", stderr.getvalue())

    def test_golden_json_processing_memory_failures_are_controlled(self):
        cases = (
            "governance_contract.fixture.decode_json",
            "governance_contract.fixture.canonical_json",
            "governance_contract.fixture.validate_record_set",
        )
        for target in cases:
            with self.subTest(target=target), patch(target, side_effect=MemoryError):
                stdout, stderr = io.StringIO(), io.StringIO()
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    result = governance.main(["--golden", str(ROOT)])
                self.assertEqual(result, 1)
                self.assertEqual(stdout.getvalue(), "")
                self.assertIn("golden fixture processing exhausted memory", stderr.getvalue())
                self.assertNotIn("Traceback", stderr.getvalue())

    def test_evidence_locator_trust_and_content_role_are_restricted(self):
        self.assert_issue("type does not match", lambda records:
                          records[0]["spec"]["locator"].update(locator_type="repo"))
        self.assert_issue("controlled/authoritative", lambda records:
                          records[0]["spec"].update(source_trust="controlled"))
        self.assert_issue("trusted control is unavailable", lambda records:
                          records[0]["spec"].update(content_role="trusted_control"))

    def test_evidence_state_artifact_reason_and_time_rules(self):
        self.assert_issue("valid evidence requires", lambda records:
                          records[0]["spec"].update(artifact_sha256=None))
        self.assert_issue("unavailable evidence requires", lambda records:
                          records[0]["status"].update(state="unavailable"))
        self.assert_issue("invalid/expired evidence requires", lambda records:
                          records[0]["status"].update(state="invalid"))
        self.assert_issue("cannot exceed created", lambda records:
                          records[0]["spec"].update(observed_at_unix_ms=1700000000001))

    def test_expired_unavailable_and_repository_evidence_can_be_structural(self):
        records = self.records()
        evidence = records[0]
        evidence["spec"]["observed_at_unix_ms"] = 1699999998000
        evidence["status"].update(state="expired", reason_codes=["expired"],
                                  valid_from_unix_ms=1699999998000,
                                  valid_until_unix_ms=1699999999000)
        evidence["spec"].update(evidence_type="repo_locator", directness="direct")
        evidence["spec"]["locator"].update(locator_type="repo", locator_ref="src/a.py",
                                              exit_code=None, line_start=1, line_end=2)
        self.resign(records)
        self.assertEqual(governance.validate_record_set(records), [])

    def test_evidence_origin_and_locator_combinations_are_enforced(self):
        def human_direct(records):
            spec = records[0]["spec"]
            spec.update(evidence_type="human_attestation", directness="direct")
            spec["locator"].update(locator_type="attestation", exit_code=None)

        self.assert_issue("human attestation must be attested", human_direct)
        self.assert_issue("external source must be derived", lambda records:
                          records[0]["spec"].update(evidence_type="external_source"))
        self.assert_issue("unsafe repository path", lambda records:
                          self._make_repo_locator(records[0], "src/../secret"))
        for locator in ("C:/Windows/system.ini", "C:system.ini"):
            with self.subTest(locator=locator):
                self.assert_issue("unsafe repository path", lambda records:
                                  self._make_repo_locator(records[0], locator))
        self.assert_issue("command requires exit_code", lambda records:
                          records[0]["spec"]["locator"].update(exit_code=None))

    @staticmethod
    def _make_repo_locator(record, path):
        record["spec"].update(evidence_type="repo_locator")
        record["spec"]["locator"].update(locator_type="repo", locator_ref=path,
                                            exit_code=None, line_start=None, line_end=None)

    def test_claim_type_object_confidence_plan_and_queue_rules(self):
        self.assert_issue("invalid for claim_type", lambda records:
                          records[1]["status"].update(state="open"))
        self.assert_issue("does not match object_type", lambda records:
                          records[1]["spec"].update(object_type="integer"))
        self.assert_issue("must be null for this claim type", lambda records:
                          records[1]["spec"].update(confidence_micros=1))
        self.assert_issue("validation_plan: required", lambda records:
                          self._make_assumption(records[1], plan=None))
        self.assert_issue("required bounded identifier", lambda records:
                          self._make_unknown(records[1], queue=None))

    @staticmethod
    def _make_assumption(record, plan):
        record["spec"].update(claim_type="assumption", confidence_micros=500000,
                              validation_plan=plan)
        record["status"]["state"] = "open"

    @staticmethod
    def _make_unknown(record, queue):
        record["spec"].update(claim_type="unknown", queue_ref=queue,
                              supporting_evidence_record_ids=[])
        record["status"]["state"] = "open"

    def test_assumption_and_unknown_valid_forms_are_accepted(self):
        records = self.records()
        plan = {"due_at_unix_ms": 1700000002000, "impact_if_false": "revise",
                "method": "rerun", "owner_id": "owner-1",
                "required_evidence_types": ["test_run"]}
        self._make_assumption(records[1], plan)
        self.resign(records)
        self.assertEqual(governance.validate_record_set(records), [])
        records = self.records()
        self._make_unknown(records[1], "queue:governance")
        self.resign(records)
        self.assertEqual(governance.validate_record_set(records), [])

    def test_claim_support_and_authority_rules_are_enforced(self):
        self.assert_issue("fact requires supporting evidence", lambda records:
                          records[1]["spec"].update(supporting_evidence_record_ids=[]))
        self.assert_issue("must be disjoint", lambda records:
                          records[1]["spec"].update(contradicting_evidence_record_ids=["evr-0001"]))
        authority = {"adr_ref": "adr:45", "approval_ref": "approval:1"}
        self.assert_issue("authority is unavailable", lambda records:
                          records[1]["spec"].update(decision_authority=authority))

    def test_cross_record_references_must_exist_and_cover_subject(self):
        self.assert_issue("unknown EvidenceRecord", lambda records:
                          records[1]["spec"].update(supporting_evidence_record_ids=["evr-missing"]))
        self.assert_issue("does not cover claim subject", lambda records:
                          records[0]["spec"].update(subjects=["module:other"]))
        self.assert_issue("unknown KnowledgeClaim", lambda records:
                          records[1]["spec"].update(derived_from_claim_record_ids=["kcr-missing"]))

    def test_identity_sort_and_supersession_rules_are_enforced(self):
        self.assert_issue("sorted by record_id", lambda records: records.reverse())
        self.assert_issue("duplicate record_id", lambda records:
                          records[1]["metadata"].update(record_id="evr-0001"))
        self.assert_issue("sequence > 1", lambda records:
                          records[0]["metadata"].update(sequence=2))

    def test_valid_supersession_requires_immediately_prior_sequence(self):
        records = self.records()
        successor = copy.deepcopy(records[0])
        successor["metadata"].update(record_id="evr-0002", sequence=2,
                                     supersedes_record_ids=["evr-0001"],
                                     created_at_unix_ms=1700000000001)
        records.insert(1, successor)
        self.resign(records)
        self.assertEqual(governance.validate_record_set(records), [])

    def test_claim_derivation_cycle_is_rejected(self):
        records = self.records()
        second = copy.deepcopy(records[1])
        second["metadata"].update(record_id="kcr-0002", aggregate_id="claim-second")
        records[1]["spec"]["derived_from_claim_record_ids"] = ["kcr-0002"]
        second["spec"]["derived_from_claim_record_ids"] = ["kcr-0001"]
        records.append(second)
        self.resign(records)
        issues = governance.validate_record_set(records)
        self.assertTrue(any("claim derivation cycle" in issue for issue in issues), issues)

    def test_cross_subject_derivation_is_allowed_but_self_reference_is_rejected(self):
        records = self.records()
        derived = copy.deepcopy(records[1])
        derived["metadata"].update(record_id="kcr-0002", aggregate_id="claim-second")
        derived["spec"].update(claim_type="proposal", subject="module:other",
                               supporting_evidence_record_ids=[],
                               derived_from_claim_record_ids=["kcr-0001"])
        derived["status"]["state"] = "draft"
        records.append(derived)
        self.resign(records)
        self.assertEqual(governance.validate_record_set(records), [])
        records = self.records()
        records[1]["spec"]["derived_from_claim_record_ids"] = ["kcr-0001"]
        self.resign(records)
        issues = governance.validate_record_set(records)
        self.assertTrue(any("claim derivation cycle" in issue for issue in issues), issues)

    def test_cli_emits_only_the_narrow_positive_attestation(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "records.json"
            path.write_bytes(governance.canonical_json(self.records()))
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = governance.main([str(ROOT), str(path)])
        self.assertEqual(result, 0)
        self.assertEqual(stdout.getvalue(), governance.SUCCESS + "\n")
        self.assertEqual(stderr.getvalue(), "")

    def test_cli_validates_the_canonical_golden_wrapper_explicitly(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = governance.main(["--golden", str(ROOT)])
        self.assertEqual(result, 0)
        self.assertEqual(stdout.getvalue(), governance.SUCCESS + "\n")
        self.assertEqual(stderr.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
