#!/usr/bin/env python3
"""Golden, projection, and adversarial tests for ADR-0050's pure adapter."""

from __future__ import annotations

import copy
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import evolve_repo_locator_evidence_adapter as adapter  # noqa: E402
import evolve_repo_locator_evidence_adapter_check as checker  # noqa: E402
from governance_contract import ContractError, canonical_json as governance_json  # noqa: E402
from governance_contract import compute_record_digest  # noqa: E402


class EvolveRepoLocatorEvidenceAdapterCheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = (ROOT / "docs/contracts/fixtures" /
                "evolve-repo-locator-evidence-adapter-v1.json")
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def request(self):
        return copy.deepcopy(self.golden["request"])

    def record(self, request=None):
        return adapter.adapt_request(self.request() if request is None else request)

    def request_bytes(self, request=None):
        return adapter.canonical_json(self.request() if request is None else request)

    def evidence_bytes(self, request=None, record=None):
        return governance_json(self.record(request) if record is None else record)

    def assert_request_rejected(self, mutate):
        request = self.request()
        mutate(request)
        issues = adapter.validate_request(request)
        self.assertTrue(issues, getattr(mutate, "__name__", repr(mutate)))
        with self.assertRaises(ContractError):
            adapter.adapt_request(request)

    def assert_raw_rejected(self, raw, expected):
        issues = adapter.check_projection_bytes(raw, self.evidence_bytes())
        self.assertTrue(any(expected in issue for issue in issues), issues)

    def test_golden_fixture_is_adapted_shadow(self):
        self.assertEqual(adapter.validate_golden_fixture(ROOT), [])
        self.assertEqual(adapter.SUCCESS, self.golden["expected"]["result"])
        for denied in ("file/report verification", "scan judgment", "completion",
                       "truth", "authority", "claim", "atom", "persistence", "effect"):
            self.assertIn(denied, adapter.SUCCESS)
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = checker.main(["--golden", str(ROOT)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, adapter.SUCCESS + "\n", ""))

    def test_normal_projection_matches_golden_and_fixed_mapping(self):
        request, expected = self.request(), self.golden["expected"]
        record = adapter.adapt_request(request)
        observation, binding = request["observation"], request["binding"]
        request_digest = adapter.compute_request_digest(request)
        source_digest = adapter.compute_source_digest(request)
        self.assertEqual(adapter.validate_request(request), [])
        self.assertEqual(adapter.validate_evidence_record(record), [])
        self.assertEqual(adapter.check_projection_bytes(
            self.request_bytes(request), governance_json(record)), [])
        self.assertEqual(adapter.canonical_locator(observation["locator"]).decode(),
                         expected["canonical_locator_json"])
        self.assertEqual(adapter.canonical_observation(observation).decode(),
                         expected["canonical_observation_json"])
        self.assertEqual(adapter.canonical_json(request).decode(),
                         expected["canonical_request_json"])
        self.assertEqual(governance_json(record).decode(),
                         expected["canonical_evidence_record_json"])
        self.assertEqual(record["metadata"]["record_id"],
                         "evolve-locator-evidence-" + request_digest)
        self.assertEqual(record["metadata"]["created_by"]["run_id"],
                         "evolve-locator-adaptation-" + request_digest)
        self.assertEqual(record["spec"]["source_snapshot"], {
            "snapshot_id": "evolve-locator-" + source_digest,
            "snapshot_sha256": source_digest, "snapshot_type": "repository",
        })
        self.assertEqual(record["spec"]["artifact_sha256"],
                         observation["content"]["sha256"])
        self.assertEqual(record["spec"]["subjects"], binding["subjects"])
        self.assertEqual(record["spec"]["locator"], {
            "content_sha256": observation["content"]["sha256"], "exit_code": None,
            "line_end": 114, "line_start": 114,
            "locator_ref": ".arch/rules.yaml", "locator_type": "repo",
        })

    def test_zero_line_maps_to_null_pair_and_positive_line_to_equal_pair(self):
        request = self.request()
        request["observation"]["locator"]["line"] = 0
        locator = adapter.adapt_request(request)["spec"]["locator"]
        self.assertIsNone(locator["line_start"])
        self.assertIsNone(locator["line_end"])
        request["observation"]["locator"]["line"] = 1
        locator = adapter.adapt_request(request)["spec"]["locator"]
        self.assertEqual((locator["line_start"], locator["line_end"]), (1, 1))

    def test_locator_observation_and_binding_identity_domains_are_separate(self):
        original = self.request()
        locator_changed = self.request()
        locator_changed["observation"]["locator"]["detail"] = "Different detail."
        self.assertNotEqual(adapter.compute_locator_digest(original),
                            adapter.compute_locator_digest(locator_changed))
        self.assertNotEqual(adapter.compute_source_digest(original),
                            adapter.compute_source_digest(locator_changed))

        content_changed = self.request()
        content_changed["observation"]["content"]["sha256"] = "1" * 64
        self.assertEqual(adapter.compute_locator_digest(original),
                         adapter.compute_locator_digest(content_changed))
        self.assertNotEqual(adapter.compute_source_digest(original),
                            adapter.compute_source_digest(content_changed))

        binding_changed = self.request()
        binding_changed["binding"]["scope"] = "module:harness"
        self.assertEqual(adapter.compute_source_digest(original),
                         adapter.compute_source_digest(binding_changed))
        self.assertNotEqual(adapter.compute_request_digest(original),
                            adapter.compute_request_digest(binding_changed))

    def test_duplicate_unknown_and_missing_fields_are_rejected(self):
        canonical = self.request_bytes()
        duplicate = canonical.replace(
            b'{"api_version":', b'{"api_version":"duplicate","api_version":', 1)
        self.assert_raw_rejected(duplicate, "duplicate JSON key")
        mutators = [
            lambda r: r.update(extra="value"),
            lambda r: r.pop("binding"),
            lambda r: r["binding"].update(extra="value"),
            lambda r: r["observation"].update(extra="value"),
            lambda r: r["observation"]["locator"].update(extra="value"),
            lambda r: r["observation"]["scan_context"].pop("depth"),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                self.assert_request_rejected(mutate)

    def test_noncanonical_request_and_evidence_bytes_are_rejected(self):
        pretty_request = json.dumps(self.request(), ensure_ascii=False, indent=2).encode()
        self.assert_raw_rejected(pretty_request, "not exact compact canonical")
        pretty_record = json.dumps(self.record(), ensure_ascii=False, indent=2).encode()
        issues = adapter.check_projection_bytes(self.request_bytes(), pretty_record)
        self.assertTrue(any("not exact compact canonical" in issue for issue in issues), issues)

    def test_float_nonfinite_overflow_and_invalid_utf8_are_rejected(self):
        canonical = self.request_bytes()
        cases = {
            "floating JSON number": canonical.replace(
                b'"sequence":1', b'"sequence":1.5'),
            "non-finite JSON number": canonical.replace(
                b'"sequence":1', b'"sequence":NaN'),
            "outside signed int64": canonical.replace(
                b'"sequence":1', b'"sequence":9223372036854775808'),
            "invalid UTF-8 JSON": canonical[:-1] + b"\xff}",
        }
        for expected, raw in cases.items():
            with self.subTest(expected=expected):
                self.assert_raw_rejected(raw, expected)

    def test_forbidden_unicode_surrogate_and_non_normalization(self):
        for value in ("bad\ntext", "bad\x85text", "bad\u2028text", "bad\u202etext", "bad\x7ftext"):
            with self.subTest(value=repr(value)):
                self.assert_request_rejected(
                    lambda r, value=value: r["observation"]["locator"].update(detail=value))
        escaped_surrogate = self.request_bytes().replace(
            b'"detail":"The architecture budget is a repository-local scan input."',
            b'"detail":"bad\\ud800text"')
        self.assert_raw_rejected(escaped_surrogate, "forbidden Unicode")

        composed, decomposed = self.request(), self.request()
        composed["observation"]["locator"]["detail"] = "caf\u00e9"
        decomposed["observation"]["locator"]["detail"] = "cafe\u0301"
        self.assertNotEqual(adapter.compute_locator_digest(composed),
                            adapter.compute_locator_digest(decomposed))

    def test_repository_path_rules_include_casefolded_control_roots_and_drives(self):
        paths = [
            "", "   ", ".", "/absolute", "\\unc\\share", "C:/drive/file",
            "z:relative", "a\\b", "a/../b", "a/./b", "a//b", "a/b/",
            ".GiT/config", ".GIT", ".FoRgE/state", ".FORGE", "a" * 4097,
            "dir/bad\x85path",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assert_request_rejected(
                    lambda r, path=path: r["observation"]["locator"].update(path=path))

    def test_symlink_locator_is_not_resolved_or_read(self):
        request = self.request()
        request["observation"]["locator"]["path"] = "evidence-link"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target, link = root / "outside.txt", root / "evidence-link"
            target.write_text("secret-current-bytes", encoding="utf-8")
            link.symlink_to(target)
            previous = Path.cwd()
            try:
                os.chdir(root)
                with patch("builtins.open", side_effect=AssertionError("path read")), \
                        patch.object(Path, "read_bytes", side_effect=AssertionError("path read")), \
                        patch.object(Path, "read_text", side_effect=AssertionError("path read")):
                    record = adapter.adapt_request(request)
            finally:
                os.chdir(previous)
            self.assertTrue(link.is_symlink())
            self.assertEqual(target.read_text(encoding="utf-8"), "secret-current-bytes")
        self.assertEqual(record["spec"]["locator"]["locator_ref"], "evidence-link")

    def test_opportunity_id_keeps_the_evolve_scan_v1_vocabulary(self):
        for value in ("x:y", "x/y", "a" * 65):
            with self.subTest(value=value):
                self.assert_request_rejected(
                    lambda r, value=value: r["observation"]["scan_context"].update(
                        opportunity_id=value
                    )
                )

    def test_line_and_content_bounds_and_types_fail_closed(self):
        mutators = [
            lambda r: r["observation"]["locator"].update(line=-1),
            lambda r: r["observation"]["locator"].update(line=True),
            lambda r: r["observation"]["locator"].update(line=9_223_372_036_854_775_808),
            lambda r: r["observation"]["content"].update(bytes=0),
            lambda r: r["observation"]["content"].update(bytes=True),
            lambda r: r["observation"]["content"].update(bytes=1_048_577),
            lambda r: r["observation"]["locator"].update(detail=" "),
            lambda r: r["observation"]["locator"].update(detail="界" * 171),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                self.assert_request_rejected(mutate)

    def test_relation_and_opportunity_matrix_fail_closed(self):
        for relation in ("finding", "clear"):
            request = self.request()
            request["observation"]["scan_context"].update(
                relation=relation, opportunity_id=None)
            self.assertEqual(adapter.validate_request(request), [])
        mutators = [
            lambda r: r["observation"]["scan_context"].update(relation="unavailable"),
            lambda r: r["observation"]["scan_context"].update(relation=[]),
            lambda r: r["observation"]["scan_context"].update(depth=[]),
            lambda r: r["observation"]["scan_context"].update(dimension={}),
            lambda r: r["observation"]["producer"].update(producer_type=[]),
            lambda r: r["binding"].update(sensitivity={}),
            lambda r: r["observation"]["scan_context"].update(opportunity_id=None),
            lambda r: r["observation"]["scan_context"].update(opportunity_id="INVALID ID"),
            lambda r: r["observation"]["scan_context"].update(
                relation="finding", opportunity_id="unexpected-id"),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                self.assert_request_rejected(mutate)

    def test_identifier_lists_must_be_bounded_sorted_unique_and_typed(self):
        mutators = [
            lambda r: r["binding"].update(subjects=[]),
            lambda r: r["binding"].update(subjects=["z:last", "a:first"]),
            lambda r: r["binding"].update(subjects=["same", "same"]),
            lambda r: r["binding"].update(subjects=["INVALID ID"]),
            lambda r: r["binding"].update(subjects="not-a-list"),
            lambda r: r["binding"].update(supersedes_record_ids=["z", "a"]),
            lambda r: r["binding"].update(supersedes_record_ids=["same", "same"]),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                self.assert_request_rejected(mutate)
        request = self.request()
        request["binding"]["subjects"] = [f"subject:{index:03d}" for index in range(257)]
        self.assertTrue(any("256 items" in issue for issue in adapter.validate_request(request)))

    def test_all_digest_declarations_fail_closed(self):
        paths = [
            ("binding", "context_sha256"), ("binding", "policy_sha256"),
            ("observation", "content", "sha256"),
            ("observation", "producer", "parameters_sha256"),
            ("observation", "scan_context", "report_sha256"),
            ("observation", "source", "source_tree_sha256"),
        ]
        for path in paths:
            for value in ("A" * 64, "0" * 63, ["0" * 64]):
                with self.subTest(path=path, value_type=type(value).__name__):
                    def mutate(request, path=path, value=value):
                        target = request
                        for key in path[:-1]:
                            target = target[key]
                        target[path[-1]] = value
                    self.assert_request_rejected(mutate)

    def test_projection_drift_is_rejected_even_with_recomputed_self_digest(self):
        mutators = [
            lambda r: r["metadata"]["created_by"].update(authority_domain="declared-shadow"),
            lambda r: r["metadata"].update(
                record_id="evolve-locator-evidence-" + "0" * 64),
            lambda r: r["spec"].update(source_trust="untrusted"),
            lambda r: r["spec"]["collector"].update(parameters_sha256="0" * 64),
            lambda r: r["spec"]["locator"].update(line_start=113),
            lambda r: r["spec"]["source_snapshot"].update(snapshot_sha256="0" * 64),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate):
                record = self.record()
                mutate(record)
                record["integrity"]["canonical_sha256"] = compute_record_digest(record)
                self.assertTrue(adapter.validate_projection(self.request(), record))
        record = self.record()
        record["integrity"]["canonical_sha256"] = "0" * 64
        self.assertTrue(adapter.validate_projection(self.request(), record))

    def test_output_does_not_alias_mutable_binding_lists(self):
        request = self.request()
        record = adapter.adapt_request(request)
        subjects = list(record["spec"]["subjects"])
        supersedes = list(record["metadata"]["supersedes_record_ids"])
        request["binding"]["subjects"].append("z:mutated")
        request["binding"]["supersedes_record_ids"].append("record:mutated")
        self.assertEqual(record["spec"]["subjects"], subjects)
        self.assertEqual(record["metadata"]["supersedes_record_ids"], supersedes)
        self.assertEqual(adapter.validate_evidence_record(record), [])

    def test_cli_projection_is_read_only_and_reports_exact_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request_path, evidence_path = root / "request.json", root / "evidence.json"
            request_path.write_bytes(self.request_bytes())
            evidence_path.write_bytes(self.evidence_bytes())
            before = {path.name: path.read_bytes() for path in root.iterdir()}
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = checker.main([str(root), "request.json", "evidence.json"])
            after = {path.name: path.read_bytes() for path in root.iterdir()}
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, adapter.SUCCESS + "\n", ""))
        self.assertEqual(before, after)

    def test_cold_cli_does_not_create_bytecode_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            isolated = Path(directory) / "harness"
            isolated.mkdir()
            shutil.copy2(HARNESS / "evolve_repo_locator_evidence_adapter_check.py",
                         isolated)
            shutil.copytree(HARNESS / "evolve_repo_locator_evidence_adapter",
                            isolated / "evolve_repo_locator_evidence_adapter",
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
            shutil.copytree(HARNESS / "governance_contract",
                            isolated / "governance_contract",
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
            result = subprocess.run(
                [sys.executable, "evolve_repo_locator_evidence_adapter_check.py",
                 "--golden", str(ROOT)],
                cwd=isolated, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(list(isolated.rglob("__pycache__")), [])


if __name__ == "__main__":
    unittest.main()
