#!/usr/bin/env python3
"""Golden-shape and adversarial tests for command observation adapter v1."""

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

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import command_observation_evidence_adapter_check as adapter  # noqa: E402
from governance_contract import ContractError, canonical_json as governance_json  # noqa: E402
from governance_contract import compute_record_digest  # noqa: E402


def sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def command_binding() -> dict[str, object]:
    """Return a fresh deterministic Governance binding for each test."""
    return {
        "aggregate_id": "gate-run-command-0049",
        "context_sha256": sha(b"context"),
        "policy_sha256": sha(b"policy"),
        "project_id": "project-catalyst",
        "scope": "project",
        "sensitivity": "internal",
        "sequence": 1,
        "subjects": ["run:command-0049", "test:harness"],
        "supersedes_record_ids": [],
    }


class CommandObservationEvidenceAdapterCheckTest(unittest.TestCase):
    def request(self):
        stdout = b"gate ok\n"
        empty = sha(b"")
        return {
            "api_version": "forgeos.governance.command-observation-evidence-adapter/v1",
            "binding": command_binding(),
            "canonicalization": "forgeos.canonical-json/v1",
            "observation": {
                "api_version": "forgeos.command-observation/v1",
                "canonicalization": "forgeos.canonical-json/v1",
                "command": {
                    "argv": ["python3", "-m", "unittest", ""],
                    "cwd": "harness",
                    "environment_sha256": sha(b"environment"),
                    "stdin_bytes": 0,
                    "stdin_sha256": empty,
                    "timeout_ms": 60000,
                    "tool_snapshot_sha256": sha(b"tool-snapshot"),
                },
                "ended_at_unix_ms": 1_786_354_860_123,
                "evidence_type": "gate_result",
                "producer": {
                    "producer_id": "forge-gate",
                    "producer_type": "tool",
                    "producer_version": "v1.2.3",
                    "run_id": "run-command-0049",
                },
                "source": {
                    "source_revision": "680babd",
                    "source_tree_sha256": sha(b"source-tree"),
                },
                "started_at_unix_ms": 1_786_354_800_000,
                "streams": {
                    "combined": {
                        "bytes": len(stdout), "retained_bytes": len(stdout),
                        "retained_sha256": sha(stdout), "sha256": sha(stdout),
                    },
                    "stderr": {
                        "bytes": 0, "retained_bytes": 0,
                        "retained_sha256": empty, "sha256": empty,
                    },
                    "stdout": {
                        "bytes": len(stdout), "retained_bytes": len(stdout),
                        "retained_sha256": sha(stdout), "sha256": sha(stdout),
                    },
                },
                "termination": {"exit_code": 0, "kind": "exited"},
            },
        }

    def record(self, request=None):
        return adapter.adapt_request(self.request() if request is None else request)

    def test_golden_fixture_is_adapted_shadow(self):
        self.assertEqual(adapter.validate_golden_fixture(ROOT), [])
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = adapter.main(["--golden", str(ROOT)])
        self.assertEqual((result, stdout.getvalue(), stderr.getvalue()),
                         (0, adapter.SUCCESS + "\n", ""))

    def request_bytes(self, request=None):
        return adapter.canonical_json(self.request() if request is None else request)

    def evidence_bytes(self, request=None):
        return governance_json(self.record(request))

    def test_fixed_mapping_and_existing_governance_validator(self):
        request = self.request()
        record = adapter.adapt_request(request)
        observation, binding = request["observation"], request["binding"]
        source_digest = adapter.compute_source_digest(request)
        command_digest = adapter.compute_command_digest(request)
        self.assertEqual(adapter.validate_request(request), [])
        self.assertEqual(adapter.validate_evidence_record(record), [])
        self.assertEqual(record["metadata"]["record_id"],
                         "command-evidence-" + adapter.compute_request_digest(request))
        self.assertEqual(record["metadata"]["source_revision"],
                         observation["source"]["source_revision"])
        self.assertEqual(record["spec"]["artifact_sha256"], source_digest)
        self.assertEqual(record["spec"]["source_snapshot"], {
            "snapshot_id": "command-observation-" + source_digest,
            "snapshot_sha256": source_digest,
            "snapshot_type": "runtime",
        })
        self.assertEqual(record["spec"]["collector"], {
            "collector_id": "forge-gate", "collector_type": "tool",
            "collector_version": "v1.2.3", "parameters_sha256": command_digest,
            "run_id": "run-command-0049",
        })
        self.assertEqual(record["metadata"]["created_by"], {
            "authority_domain": "shadow",
            "principal_id": "forgeos.command-observation-evidence-adapter",
            "principal_type": "tool", "role": "evidence-adapter",
            "run_id": "command-adaptation-" + adapter.compute_request_digest(request),
        })
        self.assertEqual(record["spec"]["locator"], {
            "content_sha256": observation["streams"]["combined"]["sha256"],
            "exit_code": 0, "line_end": None, "line_start": None,
            "locator_ref": "command-observation:" + source_digest,
            "locator_type": "command",
        })
        self.assertEqual(record["spec"]["subjects"], binding["subjects"])
        self.assertEqual(record["status"], {
            "reason_codes": [], "state": "valid",
            "valid_from_unix_ms": observation["ended_at_unix_ms"],
            "valid_until_unix_ms": None,
        })

    def test_test_run_and_nonzero_exit_are_observations_not_verdicts(self):
        request = self.request()
        request["observation"]["evidence_type"] = "test_run"
        request["observation"]["termination"]["exit_code"] = 7
        record = adapter.adapt_request(request)
        self.assertEqual(record["spec"]["evidence_type"], "test_run")
        self.assertEqual(record["spec"]["locator"]["exit_code"], 7)
        self.assertEqual(record["status"]["state"], "valid")
        for denied in ("execution", "pass", "completion", "truth", "authority",
                       "claim", "atom", "persistence", "effect"):
            self.assertIn(denied, adapter.SUCCESS)

    def test_identity_domains_are_separate(self):
        original = self.request()
        binding_changed = self.request()
        binding_changed["binding"]["scope"] = "module:harness"
        self.assertEqual(adapter.compute_command_digest(original),
                         adapter.compute_command_digest(binding_changed))
        self.assertEqual(adapter.compute_source_digest(original),
                         adapter.compute_source_digest(binding_changed))
        self.assertNotEqual(adapter.compute_request_digest(original),
                            adapter.compute_request_digest(binding_changed))

        observation_changed = self.request()
        observation_changed["observation"]["producer"]["run_id"] = "run-command-0049-b"
        self.assertEqual(adapter.compute_command_digest(original),
                         adapter.compute_command_digest(observation_changed))
        self.assertNotEqual(adapter.compute_source_digest(original),
                            adapter.compute_source_digest(observation_changed))

        command_changed = self.request()
        command_changed["observation"]["command"]["timeout_ms"] = None
        self.assertNotEqual(adapter.compute_command_digest(original),
                            adapter.compute_command_digest(command_changed))
        self.assertNotEqual(adapter.compute_source_digest(original),
                            adapter.compute_source_digest(command_changed))

    def test_exported_digest_helpers_require_a_strict_projectable_request(self):
        mutators = [
            lambda r: r.update(extra="field"),
            lambda r: r["binding"].update(subjects=["z", "a"]),
            lambda r: r["observation"]["command"].update(cwd="../outside"),
            lambda r: r["observation"].update(
                termination={"exit_code": None, "kind": "timed_out"}),
        ]
        for mutate in mutators:
            request = self.request()
            mutate(request)
            for helper in (adapter.compute_command_digest, adapter.compute_source_digest,
                           adapter.compute_request_digest):
                with self.subTest(mutate=mutate, helper=helper):
                    with self.assertRaises(ContractError):
                        helper(request)

    def test_timeout_and_cancel_are_valid_observations_but_not_projectable(self):
        for kind in ("timed_out", "cancelled"):
            request = self.request()
            request["observation"]["termination"] = {"exit_code": None, "kind": kind}
            self.assertEqual(adapter.validate_observation(request["observation"]), [])
            issues = adapter.validate_request(request)
            self.assertTrue(any("only exited" in issue for issue in issues), issues)
            with self.assertRaisesRegex(ContractError, "only exited"):
                adapter.adapt_request(request)
        request = self.request()
        request["observation"]["termination"] = {"exit_code": -1, "kind": "exited"}
        self.assertTrue(adapter.validate_observation(request["observation"]))

    def test_stream_count_empty_hash_and_truncation_invariants(self):
        mutators = [
            lambda r: r["observation"]["streams"]["combined"].update(bytes=999),
            lambda r: r["observation"]["streams"]["stdout"].update(retained_bytes=999),
            lambda r: r["observation"]["streams"]["stderr"].update(sha256="1" * 64),
            lambda r: r["observation"]["streams"]["stdout"].update(
                retained_sha256="2" * 64),
        ]
        for mutate in mutators:
            request = self.request()
            mutate(request)
            self.assertTrue(adapter.validate_request(request), mutate)

        truncated = self.request()
        stream = truncated["observation"]["streams"]["stdout"]
        stream.update(retained_bytes=4, retained_sha256=sha(b"gate"))
        combined = truncated["observation"]["streams"]["combined"]
        combined.update(retained_bytes=4, retained_sha256=sha(b"gate"))
        self.assertEqual(adapter.validate_request(truncated), [])

    def test_stdin_time_path_argv_hash_enum_and_type_fail_closed(self):
        mutators = [
            lambda r: r["observation"]["command"].update(argv=[]),
            lambda r: r["observation"]["command"].update(argv=[""]),
            lambda r: r["observation"]["command"].update(argv=["x"] * 65),
            lambda r: r["observation"]["command"].update(cwd="a//b"),
            lambda r: r["observation"]["command"].update(stdin_sha256="1" * 64),
            lambda r: r["observation"]["command"].update(timeout_ms=0),
            lambda r: r["observation"].update(ended_at_unix_ms=0),
            lambda r: r["observation"].update(evidence_type=[]),
            lambda r: r["observation"]["producer"].update(producer_type=[]),
            lambda r: r["binding"].update(sensitivity=[]),
            lambda r: r["observation"]["source"].update(source_tree_sha256="A" * 64),
        ]
        for mutate in mutators:
            request = self.request()
            mutate(request)
            self.assertTrue(adapter.validate_request(request), mutate)

    def test_strict_json_duplicate_unknown_float_overflow_unicode_and_bytes(self):
        canonical = self.request_bytes()
        duplicate = canonical.replace(b'{"api_version":',
                                      b'{"api_version":"duplicate","api_version":', 1)
        cases = {
            "duplicate JSON key": duplicate,
            "floating JSON number": canonical.replace(
                b'"sequence":1', b'"sequence":1.5'),
            "non-finite JSON number": canonical.replace(
                b'"sequence":1', b'"sequence":NaN'),
            "outside signed int64": canonical.replace(
                b'"sequence":1', b'"sequence":9223372036854775808'),
            "invalid UTF-8 JSON": canonical[:-1] + b"\xff}",
        }
        evidence = self.evidence_bytes()
        for expected, raw in cases.items():
            issues = adapter.check_projection_bytes(raw, evidence)
            self.assertTrue(any(expected in issue for issue in issues), (expected, issues))
        request = self.request()
        request["observation"]["command"]["argv"][1] = "bad\u2028arg"
        self.assertTrue(adapter.validate_request(request))
        request = self.request()
        request["observation"]["command"]["argv"][1] = "x" * 4097
        self.assertTrue(any("4096" in issue for issue in adapter.validate_request(request)))

    def test_projection_drift_is_rejected(self):
        request, record = self.request(), self.record()
        self.assertEqual(adapter.check_projection_bytes(adapter.canonical_json(request),
                                                        governance_json(record)), [])
        pretty = json.dumps(request, indent=2).encode()
        self.assertTrue(any("not exact compact canonical" in issue
                            for issue in adapter.check_projection_bytes(
                                pretty, governance_json(record))))
        drift = copy.deepcopy(record)
        drift["spec"]["collector"]["parameters_sha256"] = "0" * 64
        drift["integrity"]["canonical_sha256"] = compute_record_digest(drift)
        self.assertTrue(adapter.validate_projection(request, drift))

    def test_output_does_not_alias_request_arrays(self):
        request = self.request()
        record = adapter.adapt_request(request)
        subjects = list(record["spec"]["subjects"])
        supersedes = list(record["metadata"]["supersedes_record_ids"])
        request["binding"]["subjects"].append("z:mutated")
        request["binding"]["supersedes_record_ids"].append("record:mutated")
        self.assertEqual(record["spec"]["subjects"], subjects)
        self.assertEqual(record["metadata"]["supersedes_record_ids"], supersedes)
        self.assertEqual(adapter.validate_evidence_record(record), [])

    def test_cli_projection_is_read_only_and_usage_is_distinct(self):
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
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(adapter.main([]), 2)
        self.assertIn("usage:", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
