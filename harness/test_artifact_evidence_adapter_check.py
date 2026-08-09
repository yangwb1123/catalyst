#!/usr/bin/env python3
"""Golden and adversarial tests for Artifact Evidence adapter v1."""

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

import artifact_evidence_adapter_check as adapter  # noqa: E402
from governance_contract import ContractError, canonical_json as governance_json  # noqa: E402
from governance_contract import compute_record_digest  # noqa: E402


class ArtifactEvidenceAdapterCheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / "docs/contracts/fixtures/artifact-evidence-adapter-v1.json"
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def request(self):
        return copy.deepcopy(self.golden["request"])

    def record(self):
        return json.loads(self.golden["expected"]["canonical_evidence_record_json"])

    def request_bytes(self, request=None):
        return adapter.canonical_json(self.request() if request is None else request)

    def evidence_bytes(self, record=None):
        return governance_json(self.record() if record is None else record)

    def test_golden_fixture_is_adapted_shadow(self):
        self.assertEqual(adapter.validate_golden_fixture(ROOT), [])
        self.assertEqual(adapter.SUCCESS, self.golden["expected"]["result"])
        self.assertIn("ADAPTED_SHADOW", adapter.SUCCESS)
        self.assertIn("(no truth", adapter.SUCCESS)
        for denied in ("truth", "authority", "claim", "atom", "persistence", "effect"):
            self.assertIn(denied, adapter.SUCCESS)

    def test_golden_bytes_digests_time_and_existing_validator_match(self):
        request, expected = self.request(), self.golden["expected"]
        record = adapter.adapt_request(request)
        self.assertEqual(adapter.canonical_artifact(request["artifact"]).decode(),
                         expected["canonical_source_json"])
        self.assertEqual(adapter.canonical_json(request).decode(),
                         expected["canonical_request_json"])
        self.assertEqual(governance_json(record).decode(),
                         expected["canonical_evidence_record_json"])
        self.assertEqual(adapter.compute_source_digest(request),
                         expected["source_snapshot_sha256"])
        self.assertEqual(adapter.compute_request_digest(request), expected["request_sha256"])
        self.assertEqual(record["integrity"]["canonical_sha256"],
                         expected["evidence_record_sha256"])
        self.assertEqual(record["metadata"]["created_at_unix_ms"], 1_786_341_896_789)
        self.assertEqual(adapter.validate_evidence_record(record), [])

    def test_mapping_is_fixed_shadow_artifact_evidence(self):
        request, record = self.request(), adapter.adapt_request(self.request())
        artifact, binding = request["artifact"], request["binding"]
        principal, collector = record["metadata"]["created_by"], record["spec"]["collector"]
        self.assertEqual(principal, {"authority_domain": "shadow",
                                     "principal_id": "forgeos.artifact-evidence-adapter",
                                     "principal_type": "tool", "role": "evidence-adapter",
                                     "run_id": artifact["run_id"]})
        self.assertEqual(collector["collector_id"], principal["principal_id"])
        self.assertEqual(collector["collector_version"], "v1")
        self.assertEqual(collector["parameters_sha256"],
                         adapter.compute_request_digest(request))
        self.assertEqual((record["spec"]["evidence_type"], record["spec"]["directness"],
                          record["spec"]["source_trust"], record["spec"]["content_role"]),
                         ("artifact", "direct", "observed", "untrusted_data"))
        self.assertEqual(record["spec"]["subjects"], binding["subjects"])
        self.assertEqual(record["status"], {"reason_codes": [], "state": "valid",
                                             "valid_from_unix_ms": 1_786_341_896_789,
                                             "valid_until_unix_ms": None})

    def test_source_identity_excludes_binding_while_request_identity_includes_it(self):
        original, changed = self.request(), self.request()
        changed["binding"]["scope"] = "module:harness"
        self.assertEqual(adapter.compute_source_digest(original),
                         adapter.compute_source_digest(changed))
        self.assertNotEqual(adapter.compute_request_digest(original),
                            adapter.compute_request_digest(changed))
        self.assertNotEqual(adapter.adapt_request(original)["metadata"]["record_id"],
                            adapter.adapt_request(changed)["metadata"]["record_id"])

    def test_exported_digest_helpers_reject_non_strict_requests(self):
        mutators = [
            lambda request: request["artifact"].update(_format=""),
            lambda request: request["artifact"].update(path="../outside"),
            lambda request: request.update(unknown="field"),
            lambda request: request["binding"].update(subjects=["z", "a"]),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                request = self.request()
                mutate(request)
                with self.assertRaises(ContractError):
                    adapter.compute_source_digest(request)
                with self.assertRaises(ContractError):
                    adapter.compute_request_digest(request)

    def test_each_artifact_provenance_field_changes_source_identity(self):
        original = self.request()
        changes = {
            "agent": "reviewer", "created_at": "2026-08-10T06:04:56.789124Z",
            "model": "model-two", "path": "docs/release/other.json", "phase": "review",
            "prompt_sha256": "1" * 64, "run_id": "run-artifact-0048-b",
            "sha256": "2" * 64, "size": 17, "workflow": "review",
        }
        baseline = adapter.compute_source_digest(original)
        for field, value in changes.items():
            with self.subTest(field=field):
                changed = self.request()
                changed["artifact"][field] = value
                self.assertNotEqual(adapter.compute_source_digest(changed), baseline)
                self.assertNotEqual(adapter.adapt_request(changed)["metadata"]["record_id"],
                                    adapter.adapt_request(original)["metadata"]["record_id"])

    def test_each_binding_field_changes_request_and_output_identity(self):
        changes = {
            "aggregate_id": "artifact-run-other", "context_sha256": "1" * 64,
            "policy_sha256": "2" * 64, "project_id": "project-other",
            "scope": "module:harness", "sequence": 2, "sensitivity": "restricted",
            "source_revision": "c17e671", "source_tree_sha256": "3" * 64,
            "subjects": ["artifact:other"], "supersedes_record_ids": ["prior-record"],
        }
        original = self.request()
        baseline_source = adapter.compute_source_digest(original)
        baseline_request = adapter.compute_request_digest(original)
        for field, value in changes.items():
            with self.subTest(field=field):
                changed = self.request()
                changed["binding"][field] = value
                self.assertEqual(adapter.compute_source_digest(changed), baseline_source)
                self.assertNotEqual(adapter.compute_request_digest(changed), baseline_request)
                self.assertNotEqual(adapter.adapt_request(changed)["integrity"]["canonical_sha256"],
                                    adapter.adapt_request(original)["integrity"]["canonical_sha256"])

    def test_timestamp_floors_nanoseconds_and_preserves_source_spelling(self):
        request = self.request()
        offset = self.request()
        offset["artifact"]["created_at"] = "2026-08-10T12:34:56.789999999+06:30"
        self.assertEqual(adapter.timestamp_unix_ms(request["artifact"]["created_at"]),
                         adapter.timestamp_unix_ms(offset["artifact"]["created_at"]))
        self.assertEqual(adapter.adapt_request(offset)["metadata"]["created_at_unix_ms"],
                         1_786_341_896_789)
        self.assertNotEqual(adapter.compute_source_digest(request),
                            adapter.compute_source_digest(offset))

    def test_unicode_is_not_normalized_for_identity(self):
        composed, decomposed = self.request(), self.request()
        composed["artifact"]["model"] = "modèle"
        decomposed["artifact"]["model"] = "mode\u0300le"
        self.assertNotEqual(adapter.canonical_artifact(composed["artifact"]),
                            adapter.canonical_artifact(decomposed["artifact"]))
        self.assertNotEqual(adapter.compute_source_digest(composed),
                            adapter.compute_source_digest(decomposed))

    def test_duplicate_keys_are_rejected(self):
        raw = self.request_bytes()
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"duplicate","api_version":', 1)
        issues = adapter.check_projection_bytes(duplicate, self.evidence_bytes())
        self.assertTrue(any("duplicate JSON key" in issue for issue in issues), issues)

    def test_unknown_and_missing_request_fields_are_rejected(self):
        cases = []
        unknown = self.request()
        unknown["extra"] = "value"
        cases.append(unknown)
        missing = self.request()
        del missing["binding"]
        cases.append(missing)
        artifact_unknown = self.request()
        artifact_unknown["artifact"]["other"] = "value"
        cases.append(artifact_unknown)
        for request in cases:
            with self.subTest(keys=request.keys()):
                self.assertTrue(adapter.validate_request(request))

    def test_legacy_key_exception_is_only_for_artifact_format(self):
        self.assertTrue(self.request_bytes())
        request = self.request()
        request["artifact"]["_other"] = "forbidden"
        with self.assertRaisesRegex(ContractError, "ASCII snake_case"):
            adapter.canonical_json(request)
        request = self.request()
        request["_format"] = "forbidden-at-request-root"
        with self.assertRaisesRegex(ContractError, "ASCII snake_case"):
            adapter.canonical_json(request)

    def test_noncanonical_whitespace_and_evidence_bytes_are_rejected(self):
        pretty_request = json.dumps(self.request(), ensure_ascii=False, indent=2).encode()
        issues = adapter.check_projection_bytes(pretty_request, self.evidence_bytes())
        self.assertTrue(any("not exact compact canonical" in issue for issue in issues), issues)
        pretty_record = json.dumps(self.record(), ensure_ascii=False, indent=2).encode()
        issues = adapter.check_projection_bytes(self.request_bytes(), pretty_record)
        self.assertTrue(any("not exact compact canonical" in issue for issue in issues), issues)

    def test_float_nonfinite_int64_overflow_and_invalid_utf8_are_rejected(self):
        canonical = self.request_bytes()
        cases = {
            "floating JSON number": canonical.replace(b'"size":16', b'"size":1.5'),
            "non-finite JSON number": canonical.replace(b'"size":16', b'"size":NaN'),
            "outside signed int64": canonical.replace(
                b'"size":16', b'"size":9223372036854775808'),
            "invalid UTF-8 JSON": canonical[:-1] + b"\xff}",
        }
        for expected, raw in cases.items():
            with self.subTest(expected=expected):
                issues = adapter.check_projection_bytes(raw, self.evidence_bytes())
                self.assertTrue(any(expected in issue for issue in issues), issues)

    def test_forbidden_unicode_scalars_and_oversized_strings_are_rejected(self):
        for value in ("bad\ntext", "bad\u2028text", "bad\u202etext", "bad\x7ftext"):
            with self.subTest(value=repr(value)):
                request = self.request()
                request["artifact"]["agent"] = value
                self.assertTrue(adapter.validate_request(request))
        request = self.request()
        request["artifact"]["agent"] = "x" * 4097
        self.assertTrue(any("4096" in issue for issue in adapter.validate_request(request)))
        request["artifact"]["agent"] = "x" * 16_385
        with self.assertRaisesRegex(ContractError, "16384"):
            adapter.canonical_json(request)

    def test_unsafe_or_non_normalized_paths_are_rejected(self):
        paths = ["   ", "/absolute/file", "C:/drive/file", "a\\b", "a/../b", "a/./b",
                 "a//b", "a/b/"]
        for path in paths:
            with self.subTest(path=path):
                request = self.request()
                request["artifact"]["path"] = path
                self.assertTrue(any("normalized repo-relative" in issue
                                    for issue in adapter.validate_request(request)))

    def test_invalid_times_are_rejected(self):
        values = ["2026-08-10", "2026-02-30T00:00:00Z", "2026-08-10t00:00:00Z",
                  "2026-08-10T00:00:60Z", "2026-08-10T00:00:00+24:00",
                  "1969-12-31T23:59:59.999Z", "2026-08-10T00:00:00.1234567890Z"]
        for value in values:
            with self.subTest(value=value):
                request = self.request()
                request["artifact"]["created_at"] = value
                self.assertTrue(adapter.validate_request(request))

    def test_size_sequence_hash_identifier_sensitivity_and_lists_fail_closed(self):
        mutators = [
            lambda r: r["artifact"].update(size=0),
            lambda r: r["artifact"].update(size=True),
            lambda r: r["binding"].update(sequence=0),
            lambda r: r["artifact"].update(sha256="A" * 64),
            lambda r: r["artifact"].update(run_id="INVALID ID"),
            lambda r: r["binding"].update(sensitivity="secret"),
            lambda r: r["binding"].update(sensitivity=[]),
            lambda r: r["binding"].update(sensitivity={}),
            lambda r: r["binding"].update(subjects=[]),
            lambda r: r["binding"].update(subjects=["z", "a"]),
            lambda r: r["binding"].update(supersedes_record_ids=["same", "same"]),
        ]
        for mutate in mutators:
            request = self.request()
            mutate(request)
            self.assertTrue(adapter.validate_request(request), mutate)

    def test_more_than_256_items_and_request_byte_limit_fail_closed(self):
        request = self.request()
        request["binding"]["subjects"] = [f"subject:{index:03d}" for index in range(257)]
        with self.assertRaisesRegex(ContractError, "256 items"):
            adapter.canonical_json(request)
        oversized = b"{" + b" " * 131_072 + b"}"
        issues = adapter.check_projection_bytes(oversized, self.evidence_bytes())
        self.assertTrue(any("131072" in issue for issue in issues), issues)

        request = self.request()
        wide_text = "界" * 4096
        for field in ("agent", "model", "phase", "workflow"):
            request["artifact"][field] = wide_text
        request["artifact"]["path"] = wide_text
        request["binding"]["subjects"] = [
            f"subject:{index:03d}:" + "a" * 148 for index in range(256)
        ]
        request["binding"]["supersedes_record_ids"] = [
            f"record:{index:03d}:" + "b" * 149 for index in range(256)
        ]
        with self.assertRaisesRegex(ContractError, "exceeds 131072 bytes"):
            adapter.canonical_json(request)
        self.assertTrue(any("exceeds 131072 bytes" in issue
                            for issue in adapter.validate_request(request)))
        with self.assertRaisesRegex(ContractError, "exceeds 131072 bytes"):
            adapter.adapt_request(request)

    def test_output_does_not_alias_mutable_request_arrays(self):
        request = self.request()
        record = adapter.adapt_request(request)
        expected_subjects = list(record["spec"]["subjects"])
        expected_supersedes = list(record["metadata"]["supersedes_record_ids"])
        expected_digest = record["integrity"]["canonical_sha256"]

        request["binding"]["subjects"].append("artifact:mutated")
        request["binding"]["supersedes_record_ids"].append("mutated-record")

        self.assertEqual(record["spec"]["subjects"], expected_subjects)
        self.assertEqual(record["metadata"]["supersedes_record_ids"], expected_supersedes)
        self.assertEqual(record["integrity"]["canonical_sha256"], expected_digest)
        self.assertEqual(adapter.validate_evidence_record(record), [])

    def test_projection_drift_is_rejected(self):
        mutators = [
            lambda r: r["metadata"]["created_by"].update(authority_domain="declared-shadow"),
            lambda r: r["spec"].update(source_trust="untrusted"),
            lambda r: r["spec"]["collector"].update(collector_version="v2"),
            lambda r: r["metadata"].update(record_id="artifact-evidence-" + "0" * 64),
            lambda r: r["spec"]["source_snapshot"].update(snapshot_sha256="0" * 64),
        ]
        for mutate in mutators:
            record = self.record()
            mutate(record)
            record["integrity"]["canonical_sha256"] = compute_record_digest(record)
            issues = adapter.validate_projection(self.request(), record)
            self.assertTrue(issues, mutate)

    def test_output_digest_unknown_kind_and_noncanonical_input_fail_closed(self):
        digest_drift = self.record()
        digest_drift["integrity"]["canonical_sha256"] = "0" * 64
        self.assertTrue(any("digest mismatch" in issue
                            for issue in adapter.validate_evidence_record(digest_drift)))
        claim = self.record()
        claim["kind"] = "KnowledgeClaim"
        self.assertTrue(any("must be EvidenceRecord" in issue
                            for issue in adapter.validate_evidence_record(claim)))
        extra = self.record()
        extra["extra"] = "field"
        issues = adapter.check_projection_bytes(self.request_bytes(), governance_json(extra))
        self.assertTrue(any("unknown fields" in issue for issue in issues), issues)

    def test_cli_exact_projection_is_read_only_and_golden_mode_matches(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request_path, evidence_path = root / "request.json", root / "evidence.json"
            request_path.write_bytes(self.request_bytes())
            evidence_path.write_bytes(self.evidence_bytes())
            before = sorted(path.name for path in root.iterdir())
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = adapter.main([str(ROOT), str(request_path), str(evidence_path)])
            after = sorted(path.name for path in root.iterdir())
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, adapter.SUCCESS + "\n", ""))
        self.assertEqual(before, after)
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = adapter.main(["--golden", str(ROOT)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, adapter.SUCCESS + "\n", ""))

    def test_cli_usage_missing_files_and_bounded_reads_have_distinct_failures(self):
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(adapter.main([]), 2)
        self.assertIn("usage:", stderr.getvalue())
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(adapter.main([str(ROOT), "missing", "missing"]), 2)
        self.assertIn("cannot read adapter input", stderr.getvalue())
        with tempfile.TemporaryDirectory() as directory:
            huge, output = Path(directory) / "huge.json", Path(directory) / "output.json"
            huge.write_bytes(b"x" * 131_073)
            output.write_bytes(self.evidence_bytes())
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = adapter.main([str(ROOT), str(huge), str(output)])
        self.assertEqual(result, 1)
        self.assertIn("exceeds 131072 bytes", stderr.getvalue())

    def test_golden_drift_and_memory_exhaustion_are_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture_path = root / "docs/contracts/fixtures/artifact-evidence-adapter-v1.json"
            fixture_path.parent.mkdir(parents=True)
            fixture = copy.deepcopy(self.golden)
            fixture["expected"]["request_sha256"] = "0" * 64
            fixture_path.write_text(json.dumps(fixture), encoding="utf-8")
            issues = adapter.validate_golden_fixture(root)
        self.assertTrue(any("request_sha256" in issue for issue in issues), issues)
        with patch("artifact_evidence_adapter.fixture.read_bounded_file",
                   side_effect=MemoryError):
            issues = adapter.validate_golden_fixture(ROOT)
        self.assertTrue(any("exhausted memory" in issue for issue in issues), issues)
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch("artifact_evidence_adapter_check.read_bounded_file",
                   side_effect=MemoryError), redirect_stdout(stdout), redirect_stderr(stderr):
            result = adapter.main([str(ROOT), "request", "evidence"])
        self.assertEqual(result, 1)
        self.assertIn("exhausted memory", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
