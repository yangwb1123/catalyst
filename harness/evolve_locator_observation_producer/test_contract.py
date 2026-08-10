#!/usr/bin/env python3
"""Golden and positive-boundary tests for ADR-0052."""

from __future__ import annotations

import copy
import hashlib
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

HARNESS = Path(__file__).resolve().parents[1]
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import evolve_locator_observation_producer as producer  # noqa: E402
from evolve_locator_observation_producer import check  # noqa: E402
from evolve_locator_observation_producer.codec import decode_json  # noqa: E402
from evolve_locator_observation_producer.constants import (  # noqa: E402
    MARKER_PREFIX, MAX_PRODUCTION_BYTES,
)


class EvolveLocatorObservationProducerContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = (ROOT / "docs/contracts/fixtures" /
                "local-evolve-repo-locator-observation-producer-v1.json")
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def production(self):
        return copy.deepcopy(self.golden["production"])

    def report(self, production=None):
        production = self.production() if production is None else production
        raw = production["report_manifest"]["canonical_report"]
        return decode_json(raw[len(MARKER_PREFIX):].encode("utf-8"))

    def bind_report(self, production, report):
        raw = producer.canonical_report(report)
        manifest = production["report_manifest"]
        manifest["bytes"] = len(raw.encode("utf-8"))
        manifest["canonical_report"] = raw
        manifest["sha256"] = hashlib.sha256(raw.encode("utf-8")).hexdigest()
        production["parameters_manifest"]["expected_depth"] = report["depth"]

    def rederive(self, production):
        current = production["observations"]
        seed = current[0] if current else {
            "producer": {"run_id": "zero-observation-capture"},
            "observed_at_unix_ms": 1786428000999,
        }
        production["observations"] = [seed]
        production["observations"] = producer.expected_observations(production)

    def test_golden_fixture_and_wrapper_cli(self):
        self.assertEqual(producer.validate_golden_fixture(ROOT), [])
        self.assertEqual(self.golden["expected"]["result"], producer.RESULT)
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = check.main(["--golden", str(ROOT)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, producer.CHECKED + "\n", ""))

    def test_mapping_preserves_canonical_order_and_cross_relation_duplicate(self):
        production = self.production()
        observations = production["observations"]
        self.assertEqual(producer.validate_production(production), [])
        self.assertEqual([item["scan_context"]["relation"] for item in observations],
                         ["finding", "clear", "opportunity"])
        self.assertEqual([item["locator"]["path"] for item in observations],
                         ["src/analyzer.go", "docs/security.md", "src/analyzer.go"])
        self.assertNotEqual(observations[0]["locator"]["detail"],
                            observations[2]["locator"]["detail"])

    def test_all_observations_share_capture_and_profile_bindings(self):
        production = self.production()
        observations = production["observations"]
        parameters_sha = producer.parameters_digest(production["parameters_manifest"])
        source_sha = producer.source_digest(production["source_manifest"])
        self.assertEqual({item["observed_at_unix_ms"] for item in observations},
                         {1786428000123})
        self.assertEqual({item["producer"]["run_id"] for item in observations},
                         {"run-evolve-observation-0052"})
        self.assertEqual({item["producer"]["parameters_sha256"] for item in observations},
                         {parameters_sha})
        self.assertEqual({item["source"]["source_tree_sha256"] for item in observations},
                         {source_sha})

    def test_zero_locator_report_produces_valid_empty_set(self):
        production = self.production()
        report = {
            "version": "evolve_scan_v1", "depth": "standard",
            "dimensions": [{"name": "code", "status": "unavailable",
                            "evidence": [],
                            "unavailable_reason": "source language is absent"}],
            "opportunities": [],
        }
        self.bind_report(production, report)
        production["observations"] = []
        self.assertEqual(producer.validate_production(production), [])
        self.assertEqual(producer.expected_observations(production), [])

    def test_production_file_cli_requires_exact_canonical_bytes(self):
        production = self.production()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.json"
            path.write_bytes(producer.canonical_json(production))
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = check.main([str(path)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, producer.CHECKED + "\n", ""))

    def test_production_file_cli_rejects_oversized_input_before_decode(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "oversized-production.json"
            with path.open("wb") as stream:
                stream.truncate(MAX_PRODUCTION_BYTES + 1)
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = check.main([str(path)])
        self.assertEqual(result, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn(
            f"local Evolve locator production exceeds {MAX_PRODUCTION_BYTES} bytes",
            stderr.getvalue(),
        )

    def test_fixture_preimages_are_a_complete_regular_source_set(self):
        files = self.golden["preimages"]["source_regular_files"]
        regular = [entry for entry in self.production()["source_manifest"]["entries"]
                   if entry["kind"] == "regular"]
        self.assertEqual([item["path"] for item in files],
                         [item["path"] for item in regular])
        self.assertEqual(len(files), 2)

    def test_positive_package_contains_no_binding_evidence_or_result(self):
        production = self.production()
        self.assertEqual(set(production), {
            "api_version", "canonicalization", "observations",
            "parameters_manifest", "report_manifest", "source_manifest",
        })
        for forbidden in ("binding", "evidence", "result", "receipt"):
            self.assertNotIn(forbidden, production)

    def test_unexpected_checker_exception_is_not_relabelled_as_contract_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.json"
            path.write_text("{}", encoding="utf-8")
            with patch.object(check, "decode_production",
                              side_effect=RuntimeError("programming defect")):
                with self.assertRaisesRegex(RuntimeError, "programming defect"):
                    check.main([str(path)])


if __name__ == "__main__":
    unittest.main()
