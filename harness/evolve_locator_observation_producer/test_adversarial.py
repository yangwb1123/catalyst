#!/usr/bin/env python3
"""Fail-closed adversarial tests for ADR-0052 production checking."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import evolve_locator_observation_producer as producer  # noqa: E402
from evolve_locator_observation_producer.codec import decode_json  # noqa: E402
from evolve_locator_observation_producer.constants import MARKER_PREFIX  # noqa: E402
from governance_contract import ContractError  # noqa: E402


class EvolveLocatorObservationProducerAdversarialTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = (ROOT / "docs/contracts/fixtures" /
                "local-evolve-repo-locator-observation-producer-v1.json")
        cls.golden = json.loads(path.read_text(encoding="utf-8"))

    def production(self):
        return copy.deepcopy(self.golden["production"])

    def assert_rejected(self, mutate):
        production = self.production()
        mutate(production)
        self.assertTrue(producer.validate_production(production),
                        getattr(mutate, "__name__", repr(mutate)))

    def report(self, production):
        raw = production["report_manifest"]["canonical_report"]
        return decode_json(raw[len(MARKER_PREFIX):].encode("utf-8"))

    def bind_raw_report(self, production, raw):
        manifest = production["report_manifest"]
        manifest["canonical_report"] = raw
        manifest["bytes"] = len(raw.encode("utf-8"))
        manifest["sha256"] = hashlib.sha256(raw.encode("utf-8")).hexdigest()

    def bind_report(self, production, report):
        self.bind_raw_report(production, producer.canonical_report(report))

    def test_duplicate_unknown_missing_and_noncanonical_production_are_rejected(self):
        canonical = producer.canonical_json(self.production())
        duplicate = canonical.replace(
            b'{"api_version":', b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(ContractError, "duplicate JSON key"):
            producer.decode_production(duplicate)
        pretty = json.dumps(self.production(), indent=2).encode("utf-8")
        with self.assertRaisesRegex(ContractError, "not exact compact canonical"):
            producer.decode_production(pretty)
        for mutate in (
            lambda p: p.update(extra=True),
            lambda p: p.pop("report_manifest"),
            lambda p: p["parameters_manifest"].update(extra=True),
            lambda p: p["report_manifest"].pop("profile_id"),
            lambda p: p["source_manifest"]["entries"][0].update(extra=True),
            lambda p: p["observations"][0]["locator"].update(extra=True),
        ):
            with self.subTest(mutate=mutate):
                self.assert_rejected(mutate)

    def test_float_nonfinite_overflow_and_invalid_utf8_are_rejected(self):
        canonical = producer.canonical_json(self.production())
        replacements = {
            "floating producer JSON": (b'"observed_at_unix_ms":1786428000123',
                                       b'"observed_at_unix_ms":1.5'),
            "non-finite producer JSON": (b'"observed_at_unix_ms":1786428000123',
                                         b'"observed_at_unix_ms":NaN'),
            "outside signed int64": (b'"observed_at_unix_ms":1786428000123',
                                     b'"observed_at_unix_ms":9223372036854775808'),
        }
        for expected, (old, new) in replacements.items():
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(ContractError, expected):
                    producer.decode_production(canonical.replace(old, new, 1))
        with self.assertRaisesRegex(ContractError, "invalid producer UTF-8"):
            producer.decode_production(canonical[:-1] + b"\xff}")

    def test_bool_is_not_an_integer_for_any_bounded_numeric_fact(self):
        mutations = (
            lambda p: p["report_manifest"].update(bytes=True),
            lambda p: p["source_manifest"]["entries"][1].update(bytes=True),
            lambda p: p["observations"][0].update(observed_at_unix_ms=True),
            lambda p: p["observations"][0]["content"].update(bytes=True),
            lambda p: p["observations"][0]["locator"].update(line=True),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.assert_rejected(mutate)

    def test_report_nested_duplicate_float_unknown_and_explicit_zero_are_rejected(self):
        production = self.production()
        raw = production["report_manifest"]["canonical_report"]
        cases = [
            raw.replace('{"version":', '{"version":"bad","version":', 1),
            raw.replace('"line":3', '"line":3.5', 1),
            raw.replace('"status":"finding"', '"status":"finding","extra":true', 1),
            raw.replace('"path":"docs/security.md"',
                        '"path":"docs/security.md","line":0', 1),
        ]
        for changed in cases:
            with self.subTest(changed=changed[:80]):
                candidate = self.production()
                self.bind_raw_report(candidate, changed)
                self.assertTrue(producer.validate_production(candidate))

    def test_report_dimension_and_opportunity_order_are_canonical(self):
        production = self.production()
        report = self.report(production)
        report["dimensions"].reverse()
        raw = MARKER_PREFIX + json.dumps(report, ensure_ascii=False,
                                         separators=(",", ":"))
        self.bind_raw_report(production, raw)
        self.assertTrue(any("canonical dimension order" in issue or
                            "canonical marker bytes" in issue
                            for issue in producer.validate_production(production)))

    def test_observation_order_multiplicity_and_mapping_drift_fail_closed(self):
        mutations = (
            lambda p: p["observations"].pop(),
            lambda p: p["observations"].reverse(),
            lambda p: p["observations"].append(copy.deepcopy(p["observations"][0])),
            lambda p: p["observations"][0]["content"].update(sha256="0" * 64),
            lambda p: p["observations"][0]["scan_context"].update(relation="clear"),
            lambda p: p["observations"][1]["producer"].update(run_id="other-run"),
            lambda p: p["observations"][2].update(observed_at_unix_ms=1),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.assert_rejected(mutate)

    def test_parameters_report_and_source_digest_drift_fail_closed(self):
        mutations = (
            lambda p: p["parameters_manifest"].update(expected_depth="advisory"),
            lambda p: p["report_manifest"].update(sha256="0" * 64),
            lambda p: p["report_manifest"].update(bytes=1),
            lambda p: p["source_manifest"].update(source_revision="git-sha1:" + "3" * 40),
            lambda p: p["source_manifest"]["entries"][1].update(content_sha256="0" * 64),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.assert_rejected(mutate)

    def test_report_locator_requires_matching_nonempty_bounded_regular_source(self):
        mutations = (
            lambda p: p["source_manifest"]["entries"][3].update(kind="symlink"),
            lambda p: p["source_manifest"]["entries"][3].update(bytes=0),
            lambda p: p["source_manifest"]["entries"][3].update(bytes=1048577),
            lambda p: p["source_manifest"]["entries"][3].update(path="other.go"),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                self.assert_rejected(mutate)

    def test_report_relation_matrix_and_unicode_fail_closed(self):
        def mutate_status(production):
            report = self.report(production)
            report["dimensions"][0]["status"] = "unavailable"
            self.bind_report(production, report)

        def mutate_opportunity(production):
            report = self.report(production)
            report["opportunities"][0]["dimension"] = "security"
            self.bind_report(production, report)

        def mutate_unicode(production):
            report = self.report(production)
            report["dimensions"][0]["evidence"][0]["detail"] = "bad\u202etext"
            self.bind_report(production, report)

        for mutate in (mutate_status, mutate_opportunity, mutate_unicode):
            with self.subTest(mutate=mutate.__name__):
                self.assert_rejected(mutate)

    def test_locator_bearing_report_cannot_claim_an_empty_observation_set(self):
        self.assert_rejected(lambda p: p.update(observations=[]))


if __name__ == "__main__":
    unittest.main()
