"""Adversarial tests for the ContextPackage v1 reference contract."""

from __future__ import annotations

import copy
import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import context_package_contract as context
from context_package_contract.codec import digest
from context_package_contract.constants import (CACHE_DOMAIN, MAX_REQUEST_BYTES, RESULT,
                                                UTF8_COUNTER_ID, UTF8_COUNTER_SHA256)
from context_package_contract.fixture import load_fixture

ROOT = Path(__file__).resolve().parents[1]


class FailingCounter:
    tokenizer_id = UTF8_COUNTER_ID
    tokenizer_sha256 = UTF8_COUNTER_SHA256

    def count(self, projection_bytes: bytes) -> int:
        raise RuntimeError("counter unavailable")


class InvalidCounter:
    tokenizer_id = UTF8_COUNTER_ID
    tokenizer_sha256 = UTF8_COUNTER_SHA256

    def count(self, projection_bytes: bytes) -> int:
        return -1


class ContextPackageContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = load_fixture(ROOT)

    def request(self) -> dict[str, object]:
        return copy.deepcopy(self.golden["request"])

    def package(self) -> dict[str, object]:
        return copy.deepcopy(self.golden["expected_package"])

    def counter(self) -> context.Utf8ByteTokenCounter:
        return context.Utf8ByteTokenCounter()

    def resign_source(self, source: dict[str, object], content: str) -> None:
        source["content"] = content
        source["content_sha256"] = hashlib.sha256(content.encode("utf-8")).hexdigest()

    def one_optional(self) -> dict[str, object]:
        request = self.request()
        source = copy.deepcopy(request["sources"][1])
        source.update({"category": "decision", "expires_at_unix_ms": None,
                       "max_bytes": 131072, "priority": 1, "required": False})
        request["sources"] = [source]
        request["redactions"] = []
        request["budget"].update({"max_content_bytes": 524288, "max_snippets": 24,
                                  "max_tokens": 1000000})
        return request

    def test_golden_fixture_is_assembled_shadow(self):
        package = context.assemble(self.request(), self.counter())
        self.assertEqual(package, self.package())
        self.assertEqual(package["result"], RESULT)
        self.assertEqual(package["accounting"]["candidate_count"], 5)
        self.assertEqual(package["accounting"]["selected_snippet_count"], 3)
        self.assertEqual(package["accounting"]["omitted_source_count"], 2)
        snippets = [item for lane in package["lanes"].values() for item in lane]
        self.assertTrue(all(item["instruction_allowed"] is False for item in snippets))
        self.assertTrue(all(item["delimiter"] == "structured_json_lane_no_text_delimiter"
                            for item in snippets))

    def test_untrusted_instruction_lane_is_rejected(self):
        request = self.request()
        request["sources"][2]["declared_lane"] = "instruction"
        with self.assertRaisesRegex(context.ContractError, "cannot escalate"):
            context.assemble(request, self.counter())
        request["sources"][2]["declared_lane"] = "trusted_context"
        with self.assertRaisesRegex(context.ContractError, "cannot escalate"):
            context.assemble(request, self.counter())
        request = self.request()
        request["sources"][2]["declared_trust"] = "project_governance"
        with self.assertRaisesRegex(context.ContractError, "cannot escalate declared trust"):
            context.assemble(request, self.counter())

    def test_required_ineligible_sources_fail_closed(self):
        mutations = (
            ("availability", "missing", "missing"), ("disposition", "deny", "denied"),
            ("freshness", "stale", "stale"), ("freshness", "contested", "contested"),
            ("freshness", "unknown", "unknown_freshness"),
            ("expires_at_unix_ms", 1700000000000, "expired"),
            ("injection_risk", "suspected", "quarantined_prompt_injection"),
        )
        for field, value, reason in mutations:
            with self.subTest(reason=reason):
                request = self.request()
                source = request["sources"][0]
                source[field] = value
                if field == "availability":
                    source["content"] = source["content_sha256"] = None
                    request["redactions"] = []
                with self.assertRaisesRegex(context.ContractError, reason):
                    context.assemble(request, self.counter())

    def test_optional_ineligible_sources_emit_unique_reasons(self):
        mutations = (
            ("availability", "missing", "missing"), ("disposition", "deny", "denied"),
            ("freshness", "stale", "stale"), ("freshness", "contested", "contested"),
            ("freshness", "unknown", "unknown_freshness"),
            ("expires_at_unix_ms", 1700000000000, "expired"),
            ("injection_risk", "suspected", "quarantined_prompt_injection"),
        )
        for field, value, reason in mutations:
            with self.subTest(reason=reason):
                request = self.one_optional()
                source = request["sources"][0]
                source[field] = value
                if field == "availability":
                    source["content"] = source["content_sha256"] = None
                package = context.assemble(request, self.counter())
                self.assertEqual(package["omissions"][0]["reason"], reason)
                self.assertEqual(package["accounting"]["candidate_count"], 1)
                self.assertEqual(package["accounting"]["selected_snippet_count"], 0)

    def test_required_source_and_aggregate_budgets_fail(self):
        for field, value, expected in (
                ("max_bytes", 1, "source max_bytes"),
                ("max_snippets", 1, "snippet_budget_exceeded"),
                ("max_content_bytes", 1, "content_budget_exceeded"),
                ("max_tokens", 100, "token_budget_exceeded")):
            with self.subTest(field=field):
                request = self.request()
                if field == "max_bytes":
                    request["sources"][0][field] = value
                else:
                    request["budget"][field] = value
                if field == "max_snippets":
                    request["sources"][1]["required"] = True
                with self.assertRaisesRegex(context.ContractError, expected):
                    context.assemble(request, self.counter())

    def test_optional_budget_and_source_limit_omissions(self):
        cases = (
            ("max_snippets", 1, "snippet_budget_exceeded"),
            ("max_content_bytes", 1, "content_budget_exceeded"),
            ("max_tokens", 100, "token_budget_exceeded"),
        )
        for field, value, expected in cases:
            with self.subTest(field=field):
                request = self.request() if field == "max_snippets" else self.one_optional()
                request["budget"][field] = value
                package = context.assemble(request, self.counter())
                self.assertIn(expected, {item["reason"] for item in package["omissions"]})
        request = self.one_optional()
        request["sources"][0].update({"max_bytes": 1, "truncation": "forbidden"})
        package = context.assemble(request, self.counter())
        self.assertEqual(package["omissions"][0]["reason"], "source_limit_exceeded")

    def test_redaction_utf8_boundaries_truncation_and_no_preimage(self):
        package = context.assemble(self.request(), self.counter())
        raw = context.canonical_json(package)
        self.assertNotIn(b"SECRET", raw)
        self.assertEqual(package["redaction_receipts"], [{
            "ranges": [{"end_byte": 19, "rule_id": "fixture-secret", "start_byte": 13}],
            "source_id": "source-01-policy",
        }])
        truncated = package["lanes"]["untrusted_data"][0]
        self.assertEqual(truncated["content"], "Repository says: αβ")
        self.assertEqual(truncated["truncation"]["retained_bytes"], 21)
        request = self.request()
        request["redactions"] = [{"ranges": [{"end_byte": 19, "rule_id": "bad",
                                                "start_byte": 18}],
                                   "source_id": "source-03-repository"}]
        with self.assertRaisesRegex(context.ContractError, "UTF-8 boundaries"):
            context.assemble(request, self.counter())

    def test_utf8_prefix_that_cannot_retain_a_character_is_omitted(self):
        request = self.one_optional()
        source = request["sources"][0]
        self.resign_source(source, "α")
        source.update({"max_bytes": 1, "truncation": "utf8_prefix"})
        package = context.assemble(request, self.counter())
        self.assertEqual(package["omissions"][0]["reason"], "source_limit_exceeded")
        self.assertEqual(package["accounting"]["selected_snippet_count"], 0)

    def test_source_and_redaction_order_are_canonical(self):
        request = self.request()
        request["sources"][0], request["sources"][1] = request["sources"][1], request["sources"][0]
        with self.assertRaisesRegex(context.ContractError, "sources must be sorted"):
            context.assemble(request, self.counter())
        request = self.request()
        request["redactions"].append({"ranges": [{"end_byte": 1, "rule_id": "x",
                                                   "start_byte": 0}],
                                      "source_id": "source-03-repository"})
        request["redactions"].reverse()
        with self.assertRaisesRegex(context.ContractError, "redactions must be sorted"):
            context.assemble(request, self.counter())

    def test_token_identity_counter_failure_and_empty_budget_fail(self):
        request = self.request()
        request["budget"]["tokenizer_id"] = "unknown-tokenizer/v1"
        with self.assertRaisesRegex(context.ContractError, "identity"):
            context.assemble(request, self.counter())
        with self.assertRaisesRegex(context.ContractError, "counter failed"):
            context.assemble(self.request(), FailingCounter())
        with self.assertRaisesRegex(context.ContractError, "invalid count"):
            context.assemble(self.request(), InvalidCounter())
        request = self.one_optional()
        request["budget"]["max_tokens"] = 1
        with self.assertRaisesRegex(context.ContractError, "empty projection"):
            context.assemble(request, self.counter())

    def test_digest_cache_and_package_mutations_fail_reassembly(self):
        request, package = self.request(), self.package()
        context.validate_package(request, package, self.counter())
        mutated = copy.deepcopy(package)
        mutated["lanes"]["trusted_context"][0]["content"] += "x"
        with self.assertRaisesRegex(context.ContractError, "content_bytes|reassembly"):
            context.validate_package(request, mutated, self.counter())
        changed_request = self.request()
        changed_request["task_binding"]["task_id"] = "another-task"
        expected_key = digest(CACHE_DOMAIN, context.canonical_json(changed_request))
        self.assertNotEqual(expected_key, package["cache_key_sha256"])
        with self.assertRaisesRegex(context.ContractError, "cached package key"):
            context.validate_cache_hit(changed_request, package, self.counter())

    def test_package_decoder_rejects_lane_trust_escalation(self):
        package = self.package()
        snippet = package["lanes"]["untrusted_data"].pop()
        snippet["lane"] = "trusted_context"
        package["lanes"]["trusted_context"].append(snippet)
        with self.assertRaisesRegex(context.ContractError, "lane boundary"):
            context.decode_package(context.canonical_json(package))

    def test_package_decoder_rejects_accounting_drift(self):
        for field in ("candidate_count", "content_bytes", "selected_snippet_count",
                      "redacted_range_count"):
            with self.subTest(field=field):
                package = self.package()
                package["accounting"][field] += 1
                with self.assertRaisesRegex(context.ContractError, field):
                    context.decode_package(context.canonical_json(package))
        package = self.package()
        package["lanes"] = {lane: [] for lane in package["lanes"]}
        package["omissions"] = []
        package["redaction_receipts"] = []
        package["accounting"] = {
            "actual_tokens": 0,
            "candidate_count": 0,
            "content_bytes": 0,
            "omitted_source_count": 0,
            "redacted_range_count": 0,
            "selected_snippet_count": 0,
            "truncated_snippet_count": 0,
        }
        with self.assertRaisesRegex(context.ContractError, "candidate_count must be at least one"):
            context.decode_package(context.canonical_json(package))

    def test_package_decoder_rejects_required_truncation(self):
        package = self.package()
        snippet = package["lanes"]["untrusted_data"][0]
        snippet.update({"required": True, "selection_reason": "required_source"})
        with self.assertRaisesRegex(context.ContractError, "required snippet cannot be truncated"):
            context.decode_package(context.canonical_json(package))

    def test_exact_decoder_rejects_duplicate_unknown_float_and_noncanonical(self):
        for raw, expected in (
            (b'{"api_version":1,"api_version":2}', "duplicate"),
            (b'{"unknown":1.5}', "floating"),
            (b'{ "unknown":1}', "not exact compact canonical"),
        ):
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(context.ContractError, expected):
                    context.decode_request(raw)
        request = self.request()
        request["unknown"] = True
        with self.assertRaisesRegex(context.ContractError, "fields mismatch"):
            context.validate_request(request)
        with self.assertRaisesRegex(context.ContractError, "exceeds"):
            context.decode_request(b" " * (MAX_REQUEST_BYTES + 1))

    def test_control_bidi_and_content_cr_are_rejected(self):
        for field, value in (("source_ref", "bad\u202evalue"), ("content", "bad\rvalue")):
            with self.subTest(field=field):
                request = self.one_optional()
                source = request["sources"][0]
                if field == "content":
                    self.resign_source(source, value)
                else:
                    source[field] = value
                with self.assertRaisesRegex(context.ContractError, "forbidden Unicode"):
                    context.assemble(request, self.counter())

    def test_cli_golden_and_exact_instance_modes(self):
        command = [sys.executable, "-B", str(ROOT / "harness/context_package_contract_check.py")]
        golden = subprocess.run(command + ["--golden", str(ROOT)], capture_output=True, text=True)
        self.assertEqual(golden.returncode, 0, golden.stderr)
        with tempfile.TemporaryDirectory() as directory:
            request_path, package_path = Path(directory) / "request.json", Path(directory) / "package.json"
            request_path.write_bytes(context.canonical_json(self.request()))
            package_path.write_bytes(context.canonical_json(self.package()))
            result = subprocess.run(command + [str(ROOT), str(request_path), str(package_path)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
