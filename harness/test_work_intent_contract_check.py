"""Focused and adversarial tests for the WorkIntent v1 Proposed candidate core."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from work_intent_contract import (ContractError, SUCCESS_MARKER, canonical_json,
                                  decode_canonical_json, decode_work_intent,
                                  golden_bytes, golden_fixture, load_golden,
                                  seal_work_intent, validate_work_intent,
                                  work_intent_digest)
from work_intent_contract.constants import (ATTESTATION_FIELDS, MAX_RECORD_BYTES,
                                            TOP_FIELDS)
from work_intent_contract.fixture import GOLDEN_SHA256, fixture_candidate

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "harness" / "work_intent_contract_check.py"
SCHEMA = ROOT / "docs" / "contracts" / "work-intent-v1.schema.json"
FIXTURE = ROOT / "docs" / "contracts" / "fixtures" / "work-intent-v1.json"


def _resign(mutator) -> dict[str, object]:
    candidate = fixture_candidate()
    mutator(candidate)
    return seal_work_intent(candidate)


def _must_reject(test: unittest.TestCase, mutator) -> None:
    candidate = fixture_candidate()
    mutator(candidate)
    with test.assertRaises(ContractError):
        seal_work_intent(candidate)


class WorkIntentGoldenTests(unittest.TestCase):
    def test_golden_is_exact_pinned_canonical_plus_lf(self) -> None:
        physical = FIXTURE.read_bytes()
        self.assertEqual(physical, golden_bytes())
        self.assertEqual(hashlib.sha256(physical).hexdigest(), GOLDEN_SHA256)
        self.assertEqual(physical[-1:], b"\n")
        self.assertNotEqual(physical, canonical_json(golden_fixture()))
        self.assertEqual(load_golden(ROOT), golden_fixture())

    def test_wire_and_self_identity_are_exact(self) -> None:
        record = golden_fixture()
        self.assertEqual(set(record), TOP_FIELDS)
        self.assertEqual(set(record["attestations"]), ATTESTATION_FIELDS)
        self.assertNotIn("ownership", record)
        digest = "2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225"
        self.assertEqual(record["work_intent_sha256"], digest)
        self.assertEqual(record["work_intent_id"], f"work-intent-{digest}")

    def test_schema_accepts_golden_structure(self) -> None:
        try:
            import jsonschema
        except ImportError:
            self.skipTest("jsonschema is unavailable")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        jsonschema.validate(golden_fixture(), schema)

    def test_schema_rejects_identifier_control_suffixes(self) -> None:
        try:
            import jsonschema
        except ImportError:
            self.skipTest("jsonschema is unavailable")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        record_ref = copy.deepcopy(golden_fixture())
        record_ref["references"]["claim_record_refs"][0]["record_id"] = "a\n"
        snapshot = copy.deepcopy(golden_fixture())
        snapshot["references"]["local_source_snapshot_declaration"]["snapshot_id"] = "a\n"
        for candidate in (record_ref, snapshot):
            with self.assertRaises(jsonschema.ValidationError):
                jsonschema.validate(candidate, schema)


class WorkIntentShapeTests(unittest.TestCase):
    def test_nullable_owner_run_and_early_deadline_are_valid(self) -> None:
        record = _resign(lambda item: (
            item.__setitem__("declared_owner", None),
            item["binding"].__setitem__("run_id", None),
            item["intent"].__setitem__("deadline_unix_ms", 0),
        ))
        self.assertIsNone(record["declared_owner"])

    def test_narrative_order_is_authored_not_sorted(self) -> None:
        record = _resign(lambda item: item["intent"].__setitem__(
            "scope", ["z-last-lexically", "a-first-lexically"]))
        self.assertEqual(record["intent"]["scope"],
                         ["z-last-lexically", "a-first-lexically"])

    def test_missing_unknown_and_constant_fields_reject(self) -> None:
        _must_reject(self, lambda item: item.pop("status"))
        _must_reject(self, lambda item: item.__setitem__("ownership", False))
        _must_reject(self, lambda item: item.__setitem__("api_version", "other"))

    def test_every_attestation_is_exact_false(self) -> None:
        for field in sorted(ATTESTATION_FIELDS):
            _must_reject(self, lambda item, name=field:
                         item["attestations"].__setitem__(name, True))
        _must_reject(self, lambda item: item["attestations"].__setitem__(
            "truth_attestation", 0))
        _must_reject(self, lambda item: item["attestations"].pop("scope_attestation"))

    def test_principal_is_exact_adr0056_shape(self) -> None:
        _must_reject(self, lambda item: item["requester"].pop("authority_domain"))
        _must_reject(self, lambda item: item["requester"].__setitem__(
            "principal_type", "tool"))
        _must_reject(self, lambda item: item["requester"].__setitem__("extra", "x"))
        _must_reject(self, lambda item: item["requester"].__setitem__(
            "principal_type", []))

    def test_intent_cardinalities_duplicates_and_total_reject(self) -> None:
        _must_reject(self, lambda item: item["intent"].__setitem__("scope", []))
        _must_reject(self, lambda item: item["intent"].__setitem__(
            "success_signals", ["same", "same"]))
        _must_reject(self, lambda item: item["intent"].__setitem__(
            "external_constraints", [str(index) for index in range(65)]))
        candidate = fixture_candidate()
        for field in ("external_constraints", "non_goals", "open_questions", "scope"):
            candidate["intent"][field] = [f"{field}-{index}" for index in range(64)]
        candidate["intent"]["success_signals"] = ["one-more"]
        with self.assertRaisesRegex(ContractError, "total items"):
            seal_work_intent(candidate)

    def test_enums_materiality_and_optional_origin_reject_or_accept(self) -> None:
        _must_reject(self, lambda item: item["intent"].__setitem__("work_type", "run"))
        _must_reject(self, lambda item: item["materiality"].__setitem__("basis", "verified"))
        _must_reject(self, lambda item: item["origin"].__setitem__("origin_kind", "alert"))
        record = _resign(lambda item: item["origin"].__setitem__("origin_ref", None))
        self.assertIsNone(record["origin"]["origin_ref"])
        for parent, field in (("intent", "work_type"), ("materiality", "level"),
                              ("origin", "origin_kind")):
            _must_reject(self, lambda item, node=parent, name=field:
                         item[node].__setitem__(name, []))

    def test_record_refs_order_uniqueness_and_disjointness(self) -> None:
        def reverse(item: dict[str, object]) -> None:
            item["references"]["claim_record_refs"] = [
                {"canonical_sha256": "5" * 64, "record_id": "z-record"},
                {"canonical_sha256": "6" * 64, "record_id": "a-record"},
            ]
        _must_reject(self, reverse)
        _must_reject(self, lambda item: item["references"].__setitem__(
            "evidence_record_refs", copy.deepcopy(item["references"]["claim_record_refs"])))
        _must_reject(self, lambda item: item["references"]["claim_record_refs"][0].__setitem__(
            "canonical_sha256", "A" * 64))
        _must_reject(self, lambda item: item["references"]["claim_record_refs"][0].__setitem__(
            "record_id", "Invalid Record"))

    def test_reference_cardinality_bounds_reject(self) -> None:
        def refs(prefix: str, count: int) -> list[dict[str, object]]:
            return [{"canonical_sha256": f"{index:064x}",
                     "record_id": f"{prefix}-{index:03d}"}
                    for index in range(count)]
        _resign(lambda item: item["references"].__setitem__(
            "claim_record_refs", refs("claim", 64)))
        _must_reject(self, lambda item: item["references"].__setitem__(
            "claim_record_refs", refs("claim", 65)))
        _must_reject(self, lambda item: (
            item["references"].__setitem__("claim_record_refs", refs("claim", 64)),
            item["references"].__setitem__("evidence_record_refs", refs("evidence", 65))))
        _must_reject(self, lambda item: item["references"].__setitem__(
            "local_artifact_declarations", [
                {"artifact_kind": "artifact", "artifact_ref": f"ref/{index:03d}",
                 "artifact_sha256": f"{index:064x}"} for index in range(33)]))

    def test_artifact_order_and_pair_identity_reject(self) -> None:
        def reverse(item: dict[str, object]) -> None:
            item["references"]["local_artifact_declarations"] = [
                {"artifact_kind": "z", "artifact_ref": "a", "artifact_sha256": "5" * 64},
                {"artifact_kind": "a", "artifact_ref": "z", "artifact_sha256": "6" * 64},
            ]
        _must_reject(self, reverse)
        def duplicate_pair(item: dict[str, object]) -> None:
            original = item["references"]["local_artifact_declarations"][0]
            other = {**original, "artifact_sha256": "9" * 64}
            item["references"]["local_artifact_declarations"] = sorted(
                [original, other], key=canonical_json)
        _must_reject(self, duplicate_pair)

    def test_source_snapshot_is_exact_adr0045_triple(self) -> None:
        _resign(lambda item: item["references"].__setitem__(
            "local_source_snapshot_declaration", None))
        _must_reject(self, lambda item: item["references"][
            "local_source_snapshot_declaration"].__setitem__("snapshot_type", "git"))
        _must_reject(self, lambda item: item["references"][
            "local_source_snapshot_declaration"].__setitem__("snapshot_id", "UPPER"))
        _must_reject(self, lambda item: item["references"][
            "local_source_snapshot_declaration"].__setitem__("snapshot_type", []))

    def test_short_and_reference_utf8_byte_limits(self) -> None:
        _resign(lambda item: item["binding"].__setitem__("change_id", "x" * 160))
        _must_reject(self, lambda item: item["binding"].__setitem__(
            "change_id", "x" * 161))
        _resign(lambda item: item["origin"].__setitem__("origin_ref", "r" * 4096))
        _must_reject(self, lambda item: item["origin"].__setitem__(
            "origin_ref", "r" * 4097))


class WorkIntentCodecTests(unittest.TestCase):
    def test_exact_canonical_instance_has_no_lf_or_whitespace(self) -> None:
        raw = canonical_json(golden_fixture())
        self.assertEqual(decode_work_intent(raw), golden_fixture())
        for changed in (raw + b"\n", b" " + raw, raw + b" "):
            with self.assertRaises(ContractError):
                decode_work_intent(changed)

    def test_duplicate_keys_floats_nonfinite_and_utf8_reject(self) -> None:
        raw = canonical_json(golden_fixture())
        duplicate = raw.replace(b'{"api_version":', b'{"api_version":"x","api_version":', 1)
        for changed in (duplicate, b'{"value":1.0}', b'{"value":NaN}', b'"\xff"'):
            with self.assertRaises(ContractError):
                decode_canonical_json(changed)

    def test_generic_depth_integer_and_key_bounds(self) -> None:
        self.assertEqual(decode_canonical_json(b"[[[[[[[0]]]]]]]")[0][0][0][0][0][0][0], 0)
        self.assertEqual(decode_canonical_json(b"-9223372036854775808"), -(2**63))
        for raw in (b"[[[[[[[[0]]]]]]]]", b"9223372036854775808",
                    b"-9223372036854775809", b'{"Bad":1}'):
            with self.assertRaises(ContractError):
                decode_canonical_json(raw)

    def test_generic_field_array_and_forbidden_scalar_bounds(self) -> None:
        self.assertEqual(len(decode_canonical_json(canonical_json(list(range(256))))), 256)
        self.assertEqual(len(decode_canonical_json(canonical_json(
            {f"k{index}": index for index in range(32)}))), 32)
        for value in (list(range(257)), {f"k{index}": index for index in range(33)},
                      "has\u0085control", "has\u202eoverride", "lone\ud800surrogate"):
            with self.assertRaises(ContractError):
                canonical_json(value)
        with self.assertRaises(ContractError):
            decode_canonical_json(b'"\\u0085"')
        self.assertEqual(decode_canonical_json(
            b'{"' + b"a" * 16_384 + b'":0}'), {"a" * 16_384: 0})
        with self.assertRaises(ContractError):
            decode_canonical_json(b'{"' + b"a" * 16_385 + b'":0}')

    def test_python_api_malformed_values_fail_with_contract_error(self) -> None:
        malformed = fixture_candidate()
        malformed[1] = "non-string-key"
        for call in (lambda: seal_work_intent(malformed),
                     lambda: work_intent_digest([]),
                     lambda: work_intent_digest(None)):
            with self.assertRaises(ContractError):
                call()
        for missing in ("work_intent_id", "work_intent_sha256"):
            incomplete = fixture_candidate()
            incomplete.pop(missing)
            with self.assertRaises(ContractError):
                work_intent_digest(incomplete)

    def test_utf8_string_byte_limit(self) -> None:
        _resign(lambda item: item["intent"].__setitem__("goal", "é" * 8192))
        _must_reject(self, lambda item: item["intent"].__setitem__("goal", "é" * 8193))

    def test_self_digest_and_id_mutations_reject(self) -> None:
        for field, value in (("work_intent_sha256", "0" * 64),
                             ("work_intent_id", "work-intent-" + "0" * 64)):
            record = copy.deepcopy(golden_fixture())
            record[field] = value
            with self.assertRaises(ContractError):
                validate_work_intent(record)

    def test_exact_n_and_n_plus_one_input_bound(self) -> None:
        candidate = fixture_candidate()
        prefix_items = [f"{index:02d}:" + "x" * (16_384 - 3) for index in range(15)]
        candidate["intent"]["external_constraints"] = prefix_items + ["p"]
        first = seal_work_intent(candidate)
        delta = MAX_RECORD_BYTES - len(canonical_json(first))
        candidate["intent"]["external_constraints"][-1] = "p" + "y" * delta
        record = seal_work_intent(candidate)
        raw = canonical_json(record)
        self.assertEqual(len(raw), MAX_RECORD_BYTES)
        self.assertEqual(decode_work_intent(raw), record)
        with self.assertRaisesRegex(ContractError, "exceeds"):
            decode_work_intent(raw + b" ")
        candidate["intent"]["external_constraints"][-1] = "p" + "y" * 16_383
        with self.assertRaisesRegex(ContractError, "blank identity preimage exceeds"):
            seal_work_intent(candidate)


class WorkIntentCliTests(unittest.TestCase):
    def _run(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run([sys.executable, "-B", str(CHECKER), *arguments],
                              cwd=ROOT, capture_output=True, check=False)

    def test_golden_and_file_modes_emit_the_exact_marker(self) -> None:
        golden = self._run("--golden", str(ROOT))
        self.assertEqual(golden.returncode, 0, golden.stderr)
        self.assertEqual(golden.stdout, (SUCCESS_MARKER + "\n").encode())
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "work-intent.json"
            path.write_bytes(canonical_json(golden_fixture()))
            instance = self._run("--file", str(path))
        self.assertEqual(instance.returncode, 0, instance.stderr)
        self.assertEqual(instance.stdout, golden.stdout)

    def test_failure_has_no_success_stdout_and_usage_is_two(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            path.write_bytes(golden_bytes())
            failed = self._run("--file", str(path))
        self.assertEqual(failed.returncode, 1)
        self.assertEqual(failed.stdout, b"")
        self.assertTrue(failed.stderr.startswith(b"WorkIntent v1: ERROR:"))
        usage = self._run()
        self.assertEqual(usage.returncode, 2)
        self.assertEqual(usage.stdout, b"")


if __name__ == "__main__":
    unittest.main()
