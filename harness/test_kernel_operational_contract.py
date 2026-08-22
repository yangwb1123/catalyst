"""Focused and adversarial tests for ADR-0088 operational core v1."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from kernel_operational_contract import (ContractError, SUCCESS_MARKER,
                                         canonical_json, decode_artifact_receipt,
                                         decode_artifact_ref,
                                         decode_capability_invocation,
                                         decode_closure, decode_execution_receipt,
                                         decode_interaction_event,
                                         empty_profile_closure, golden_bytes,
                                         golden_closure, load_golden,
                                         seal_artifact_receipt,
                                         seal_capability_invocation,
                                         seal_execution_receipt,
                                         seal_interaction_event)
from kernel_operational_contract.codec import decode_canonical_json
from kernel_operational_contract.constants import *  # noqa: F403
from kernel_operational_contract.fixture import GOLDEN_SHA256, fixture_candidate

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / FIXTURE_PATH  # noqa: F405
SCHEMA = ROOT / "docs/contracts/kernel-operational-reference-core-v1.schema.json"
CHECKER = ROOT / "harness/kernel_operational_contract_check.py"


def _blank(record: dict, identity: str, digest: str) -> dict:
    candidate = copy.deepcopy(record)
    candidate[identity], candidate[digest] = "", ""
    return candidate


def _fake_ref(prefix: str, id_field: str, hash_field: str, number: int) -> dict:
    digest = f"{number:064x}"
    return {id_field: f"{prefix}{digest}", hash_field: digest}


class KernelOperationalGoldenTests(unittest.TestCase):
    def test_full_dag_golden_is_exact_canonical_plus_lf(self) -> None:
        physical = FIXTURE.read_bytes()
        self.assertEqual(physical, golden_bytes())
        self.assertEqual(hashlib.sha256(physical).hexdigest(), GOLDEN_SHA256)
        self.assertEqual(physical[-1:], b"\n")
        self.assertEqual(load_golden(ROOT), golden_closure())
        self.assertEqual(golden_closure()["closure_sha256"],
                         "1db702583b8dae850413b75b80d620a6031ad452071908e33ea551a4f5feae0e")

    def test_all_record_identities_and_domains_are_pinned(self) -> None:
        closure = golden_closure()
        expected = {
            "artifact_receipts": [
                "3e30b65111a01e1aba29990afea3669760b15f020c63db79bec7e5e525126e85",
                "825f27d024f4761b84fa5cefdf5cc49f8ddacbdd7954200d8f7023c555439a9f",
                "c50db199a55b97e5bd98c1cfc9505e054e03a4441752cb87498d963239bc160c",
            ],
            "capability_invocations": [
                "fe5bd2c4747ce65cb1ae0697a70b19cb1bf595f687ecba44ac8ef693b62b5943",
                "63c191ccd48615a1f94f53993306d77701cb2523cfda99e0d5f0f03cfa373fa8",
            ],
            "interaction_events": [
                "4d6ed6a2b214cb8863cf6376c600c838889a2fbc81bf992872eab58e30277296",
                "ea1cc2d354ff7b2b29f369dfb32b7dd2286d759952579463face4e52f9ef1fcc",
                "63345e48cb28b19915d0d3116b9b90f904e86dfd394e7797a3ea4c2edd7b0b84",
                "3b5c9413733de9529603d013c1d87d29288de9fbfe639e96bcd3bfe16df28d8b",
            ],
            "execution_receipts": [
                "2cd7b48d3a9e521798dbdba954dff64f97988d78f9e739bc168e1bda3d75b20f",
                "f3fd09410ec21a74fdb8484cbf50f13f8fcba0a5027940af8aa46d7ab7789928",
            ],
        }
        digest_fields = {"artifact_receipts": "artifact_receipt_sha256",
                         "capability_invocations": "invocation_sha256",
                         "interaction_events": "event_sha256",
                         "execution_receipts": "execution_receipt_sha256"}
        for collection, digests in expected.items():
            self.assertEqual([item[digest_fields[collection]]
                              for item in closure[collection]], digests)

    def test_empty_io_event_profile_is_valid(self) -> None:
        closure = empty_profile_closure()
        self.assertEqual(closure["artifacts"], [])
        self.assertEqual(closure["artifact_receipts"], [])
        self.assertEqual(closure["interaction_events"], [])
        self.assertEqual(closure["capability_invocations"][0]["declared_output_slots"], [])
        self.assertEqual(closure["execution_receipts"][0]["event_refs"], [])
        self.assertEqual(decode_closure(canonical_json(closure)), closure)

    def test_principals_are_independent_exact_shapes(self) -> None:
        closure = golden_closure()
        input_producer = closure["artifact_receipts"][0]["producer"]
        subject = closure["capability_invocations"][0]["subject"]
        actor = closure["interaction_events"][0]["actor"]
        target = closure["interaction_events"][0]["target"]
        executor = closure["execution_receipts"][0]["executor"]
        self.assertEqual(len({canonical_json(item) for item in
                              (input_producer, subject, actor, target, executor)}), 5)

    def test_schema_and_authority_metadata_accept_both_profiles(self) -> None:
        try:
            import jsonschema
        except ImportError:
            self.skipTest("jsonschema is unavailable")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        for closure in (golden_closure(), empty_profile_closure()):
            jsonschema.validate(closure, schema)
        metadata = schema["x-forgeos-authority-semantics"]
        self.assertEqual(metadata["positive_result"], SUCCESS_MARKER)
        self.assertEqual(set(metadata["attestations"]), ATTESTATION_FIELDS)  # noqa: F405
        self.assertTrue(all(value is False for value in metadata["attestations"].values()))


class KernelOperationalRecordTests(unittest.TestCase):
    def setUp(self) -> None:
        closure = golden_closure()
        self.artifact_receipt = closure["artifact_receipts"][0]
        self.invocation = closure["capability_invocations"][0]
        self.event = closure["interaction_events"][0]
        self.receipt = closure["execution_receipts"][0]

    def test_standalone_decoders_accept_exact_records_and_artifact_ref(self) -> None:
        calls = ((decode_artifact_receipt, self.artifact_receipt),
                 (decode_capability_invocation, self.invocation),
                 (decode_interaction_event, self.event),
                 (decode_execution_receipt, self.receipt))
        for decode, record in calls:
            self.assertEqual(decode(canonical_json(record)), record)
        artifact = self.artifact_receipt["artifact"]
        self.assertEqual(decode_artifact_ref(canonical_json(artifact)), artifact)

    def test_every_common_attestation_is_exact_false(self) -> None:
        for field in sorted(ATTESTATION_FIELDS):  # noqa: F405
            candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
            candidate["attestations"][field] = True
            with self.assertRaises(ContractError):
                seal_capability_invocation(candidate)
        candidate = _blank(self.event, "event_id", "event_sha256")
        candidate["attestations"]["truth_attestation"] = False
        with self.assertRaises(ContractError):
            seal_interaction_event(candidate)

    def test_attempt_prior_pair_is_intrinsic(self) -> None:
        candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
        candidate["attempt"] = 2
        with self.assertRaisesRegex(ContractError, "retry requires"):
            seal_capability_invocation(candidate)
        receipt = _blank(self.receipt, "execution_receipt_id", "execution_receipt_sha256")
        receipt["prior_execution_receipt_ref"] = _fake_ref(
            EXECUTION_RECEIPT_PREFIX, "execution_receipt_id",  # noqa: F405
            "execution_receipt_sha256", 9)
        with self.assertRaisesRegex(ContractError, "attempt one"):
            seal_execution_receipt(receipt)

    def test_empty_record_sets_are_valid(self) -> None:
        invocation = _blank(self.invocation, "invocation_id", "invocation_sha256")
        invocation["declared_output_slots"] = []
        invocation["input_artifact_receipt_refs"] = []
        seal_capability_invocation(invocation)
        event = _blank(self.event, "event_id", "event_sha256")
        event["artifact_refs"] = []
        seal_interaction_event(event)
        receipt = _blank(self.receipt, "execution_receipt_id", "execution_receipt_sha256")
        receipt["event_refs"], receipt["input_artifacts"] = [], []
        receipt["output_artifact_receipt_refs"] = []
        seal_execution_receipt(receipt)

    def test_collection_n_and_n_plus_one_bounds(self) -> None:
        invocation = _blank(self.invocation, "invocation_id", "invocation_sha256")
        invocation["declared_output_slots"] = [f"slot-{index:02d}" for index in range(32)]
        seal_capability_invocation(invocation)
        invocation["declared_output_slots"].append("slot-32")
        with self.assertRaises(ContractError):
            seal_capability_invocation(invocation)
        event = _blank(self.event, "event_id", "event_sha256")
        event["artifact_refs"] = sorted([
            {"artifact_kind": "fixture", "artifact_ref": f"fixture/{index:02d}",
             "artifact_sha256": f"{index:064x}"} for index in range(32)
        ], key=canonical_json)
        seal_interaction_event(event)
        event["artifact_refs"].append(
            {"artifact_kind": "fixture", "artifact_ref": "fixture/32",
             "artifact_sha256": f"{32:064x}"})
        event["artifact_refs"].sort(key=canonical_json)
        with self.assertRaises(ContractError):
            seal_interaction_event(event)

    def test_elapsed_wall_interval_n_n_plus_one_and_mismatch(self) -> None:
        receipt = _blank(self.receipt, "execution_receipt_id", "execution_receipt_sha256")
        start = receipt["started_at_unix_ms"]
        receipt["ended_at_unix_ms"] = start + MAX_ELAPSED_MS  # noqa: F405
        receipt["observed_usage"]["elapsed_ms"] = MAX_ELAPSED_MS  # noqa: F405
        seal_execution_receipt(receipt)
        too_long = copy.deepcopy(receipt)
        too_long["ended_at_unix_ms"] += 1
        too_long["observed_usage"]["elapsed_ms"] += 1
        with self.assertRaisesRegex(ContractError, "wall interval"):
            seal_execution_receipt(too_long)
        mismatch = copy.deepcopy(receipt)
        mismatch["observed_usage"]["elapsed_ms"] -= 1
        with self.assertRaisesRegex(ContractError, "must equal"):
            seal_execution_receipt(mismatch)

    def test_observed_usage_accepts_nonzero_bounded_values(self) -> None:
        usage = self.receipt["observed_usage"]
        self.assertGreater(usage["cost_usd_micros"], 0)
        self.assertGreater(usage["network_bytes"], 0)
        receipt = _blank(self.receipt, "execution_receipt_id", "execution_receipt_sha256")
        for field, maximum in (("call_count", MAX_CALL_COUNT),  # noqa: F405
                               ("cost_usd_micros", MAX_COST_USD_MICROS),  # noqa: F405
                               ("input_tokens", MAX_TOKEN_COUNT),  # noqa: F405
                               ("network_bytes", MAX_NETWORK_BYTES),  # noqa: F405
                               ("output_bytes", MAX_OUTPUT_BYTES),  # noqa: F405
                               ("output_tokens", MAX_TOKEN_COUNT)):  # noqa: F405
            candidate = copy.deepcopy(receipt)
            candidate["observed_usage"][field] = maximum
            seal_execution_receipt(candidate)
            candidate["observed_usage"][field] = maximum + 1
            with self.assertRaises(ContractError):
                seal_execution_receipt(candidate)

    def test_bad_principal_target_and_kind_types_fail_closed(self) -> None:
        for field in ("actor", "target"):
            candidate = _blank(self.event, "event_id", "event_sha256")
            candidate[field] = []
            with self.assertRaises(ContractError):
                seal_interaction_event(candidate)
        candidate = _blank(self.artifact_receipt, "artifact_receipt_id",
                           "artifact_receipt_sha256")
        candidate["receipt_role"] = []
        with self.assertRaises(ContractError):
            seal_artifact_receipt(candidate)

    def test_reused_value_objects_reject_missing_extra_enum_hash_and_type(self) -> None:
        cases = []
        for field, value in (("authority_domain", None), ("extra", "x"),
                             ("principal_type", "tool"), ("principal_type", [])):
            candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
            if value is None:
                candidate["subject"].pop(field)
            else:
                candidate["subject"][field] = value
            cases.append((seal_capability_invocation, candidate))
        for field, value in (("task_id", None), ("extra", "x"),
                             ("environment_class", "prod"), ("attempt_id", 1)):
            candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
            if value is None:
                candidate["task_binding"].pop(field)
            else:
                candidate["task_binding"][field] = value
            cases.append((seal_capability_invocation, candidate))
        for field, value in (("capability_id", None), ("extra", "x"),
                             ("capability_contract_sha256", "A" * 64),
                             ("capability_version", [])):
            candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
            if value is None:
                candidate["capability"].pop(field)
            else:
                candidate["capability"][field] = value
            cases.append((seal_capability_invocation, candidate))
        for field, value in (("artifact_ref", None), ("extra", "x"),
                             ("artifact_sha256", "A" * 64), ("artifact_kind", [])):
            candidate = _blank(self.artifact_receipt, "artifact_receipt_id",
                               "artifact_receipt_sha256")
            if value is None:
                candidate["artifact"].pop(field)
            else:
                candidate["artifact"][field] = value
            cases.append((seal_artifact_receipt, candidate))
        for seal, candidate in cases:
            with self.assertRaises(ContractError):
                seal(candidate)

    def test_grant_ref_and_usage_runtime_shape_vectors(self) -> None:
        cases = []
        for field, value in (("authority_domain", None), ("extra", "x"),
                             ("authority_domain", ""),
                             ("grant_sha256", "A" * 64), ("grant_id", [])):
            candidate = _blank(self.invocation, "invocation_id", "invocation_sha256")
            if value is None:
                candidate["capability_grant_ref"].pop(field)
            else:
                candidate["capability_grant_ref"][field] = value
            cases.append((seal_capability_invocation, candidate))
        for field, value in (("call_count", None), ("extra", 0),
                             ("call_count", True), ("call_count", -1)):
            candidate = _blank(self.receipt, "execution_receipt_id",
                               "execution_receipt_sha256")
            if value is None:
                candidate["observed_usage"].pop(field)
            else:
                candidate["observed_usage"][field] = value
            cases.append((seal_execution_receipt, candidate))
        for seal, candidate in cases:
            with self.assertRaises(ContractError):
                seal(candidate)


class KernelOperationalCodecTests(unittest.TestCase):
    def test_exact_canonical_inputs_reject_lf_whitespace_and_unknown(self) -> None:
        raw = canonical_json(golden_closure())
        self.assertEqual(decode_closure(raw), golden_closure())
        for changed in (raw + b"\n", b" " + raw, raw + b" "):
            with self.assertRaises(ContractError):
                decode_closure(changed)
        candidate = fixture_candidate()
        candidate["authority"] = False
        from kernel_operational_contract import seal_closure
        with self.assertRaises(ContractError):
            seal_closure(candidate)

    def test_duplicate_float_nonfinite_utf8_and_key_reject(self) -> None:
        raw = canonical_json(golden_closure())
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"x","api_version":', 1)
        malformed = (duplicate, b'{"value":1.0}', b'{"value":NaN}',
                     b'"\xff"', b'{"Bad":1}')
        for changed in malformed:
            with self.assertRaises(ContractError):
                decode_canonical_json(changed, MAX_CLOSURE_BYTES)  # noqa: F405

    def test_int64_depth_array_and_forbidden_scalars_reject(self) -> None:
        self.assertEqual(decode_canonical_json(b"9223372036854775807", 64), 2**63 - 1)
        for raw in (b"9223372036854775808", b"-9223372036854775809",
                    b"[[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]]", b'"\\u202e"'):
            with self.assertRaises(ContractError):
                decode_canonical_json(raw, 256)
        with self.assertRaises(ContractError):
            canonical_json(list(range(257)))

    def test_per_document_decoder_n_plus_one_bounds(self) -> None:
        cases = ((decode_artifact_receipt, MAX_ARTIFACT_RECEIPT_BYTES),  # noqa: F405
                 (decode_capability_invocation, MAX_INVOCATION_BYTES),  # noqa: F405
                 (decode_interaction_event, MAX_EVENT_BYTES),  # noqa: F405
                 (decode_execution_receipt, MAX_EXECUTION_RECEIPT_BYTES),  # noqa: F405
                 (decode_closure, MAX_CLOSURE_BYTES))  # noqa: F405
        for decode, maximum in cases:
            with self.assertRaisesRegex(ContractError, "exceeds"):
                decode(b" " * (maximum + 1))


class KernelOperationalCheckerTests(unittest.TestCase):
    def test_golden_and_exact_file_emit_only_marker(self) -> None:
        commands = ([sys.executable, "-B", str(CHECKER), "--golden", str(ROOT)],)
        for command in commands:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual(result.stdout, (SUCCESS_MARKER + "\n").encode())
            self.assertEqual(result.stderr, b"")
        with tempfile.NamedTemporaryFile() as stream:
            stream.write(canonical_json(golden_closure()))
            stream.flush()
            result = subprocess.run(
                [sys.executable, "-B", str(CHECKER), "--file", stream.name],
                cwd=ROOT, capture_output=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(result.stdout, (SUCCESS_MARKER + "\n").encode())

    def test_rejection_has_no_stdout_or_traceback(self) -> None:
        with tempfile.NamedTemporaryFile() as stream:
            stream.write(b'{"bad":true}\n')
            stream.flush()
            result = subprocess.run(
                [sys.executable, "-B", str(CHECKER), "--file", stream.name],
                cwd=ROOT, capture_output=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"Traceback", result.stderr)

    def test_missing_directory_symlink_and_hardlink_inputs_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            regular = directory / "closure.json"
            regular.write_bytes(canonical_json(golden_closure()))
            symlink = directory / "closure-link.json"
            symlink.symlink_to(regular)
            hardlink = directory / "closure-hardlink.json"
            hardlink.hardlink_to(regular)
            candidates = (directory / "missing.json", directory, symlink,
                          regular, hardlink)
            for candidate in candidates:
                result = subprocess.run(
                    [sys.executable, "-B", str(CHECKER), "--file", str(candidate)],
                    cwd=ROOT, capture_output=True, check=False)
                self.assertEqual(result.returncode, 2, candidate)
                self.assertEqual(result.stdout, b"")
                self.assertNotIn(b"Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
