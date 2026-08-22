from __future__ import annotations

import copy
import hashlib
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

from project_source_snapshot_contract import check as check_cli  # noqa: E402
from harness.project_source_snapshot_contract.codec import canonical_json
from harness.project_source_snapshot_contract.constants import (
    CONSISTENCY, COVERAGE_SPECS, DOMAINS, FIXTURE_PATH, FIXTURE_SHA256,
    MAX_ENVELOPE_BYTES, POSITIVE_RESULT,
)
from harness.project_source_snapshot_contract.fixture import golden_value, load_golden
from harness.project_source_snapshot_contract.shapes import protected_reason
from harness.project_source_snapshot_contract.validation import (
    ContractError, decode_production, validate_production,
)

ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "harness/project_source_snapshot_contract/check.py"


class ProjectSourceSnapshotContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.value = golden_value()
        self.raw = canonical_json(self.value, MAX_ENVELOPE_BYTES)

    def test_exact_fixture_round_trip(self) -> None:
        self.assertEqual(decode_production(self.raw), self.value)
        self.assertEqual(canonical_json(validate_production(self.value)), self.raw)
        self.assertFalse(self.raw.endswith(b"\n"))

    def test_physical_golden_and_cli(self) -> None:
        physical = (ROOT / FIXTURE_PATH).read_bytes()
        self.assertEqual(hashlib.sha256(physical).hexdigest(), FIXTURE_SHA256)
        self.assertEqual(physical, self.raw + b"\n")
        self.assertEqual(load_golden(ROOT), self.value)
        result = subprocess.run(
            [sys.executable, "-B", str(CHECK), "--golden", str(ROOT)],
            cwd=ROOT, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout,
                         b"Project Source Snapshot v1 golden: OK (authority neutral)\n")

    def test_semantic_pins(self) -> None:
        snapshot = self.value["snapshot"]
        manifest = snapshot["source_manifest"]
        self.assertEqual(snapshot["consistency"], CONSISTENCY)
        self.assertEqual(snapshot["positive_result"], POSITIVE_RESULT)
        self.assertEqual(len(snapshot["coverage"]["surfaces"]), 12)
        self.assertEqual(
            [item["surface"] for item in snapshot["coverage"]["surfaces"]],
            [item[0] for item in COVERAGE_SPECS],
        )
        self.assertEqual(manifest["universe_count"], 6)
        self.assertEqual(manifest["ignored_path_count"], 2)
        self.assertTrue(all("path" not in item for item in manifest["excluded"]))

    def test_digest_dag_pins(self) -> None:
        snapshot = self.value["snapshot"]
        manifest = snapshot["source_manifest"]
        got = (
            self.value["request"]["request_sha256"], manifest["entry_set_sha256"],
            manifest["exclusion_set_sha256"], manifest["source_manifest_sha256"],
            snapshot["coverage_sha256"], snapshot["snapshot_identity_sha256"],
            snapshot["snapshot_sha256"], self.value["envelope_sha256"],
        )
        self.assertEqual(got, (
            "45f7dd52aabbacf32211376b96ee8b8c234dd43d13759b13f21ff373af786435",
            "d62c484bc027f4313797ed0e785dfd634c4cf3111d4d8f78097ab49f6a4dfeab",
            "9061191adb9bab12ef2816974c4a2cad124ea834d86b305b76546043d565028f",
            "6da5ec7b94d8e587cbf72ed7a9eb23c9cf5cc4819c3274dcab86042e09a0da12",
            "994f7da1466a3bc07d1ba1a19fa7585c11ca5b01eec669a0ea89a1eb2e1bde44",
            "c069e964225e72523638b69730061a0da0631e65ba78fe8914eb234aee9f2ecc",
            "8124b7b32e4815ca0d193413dcf4181f1ec7728c44da5abf5837726577224e0e",
            "4906d58a4c90a85fe9546955efe4382118d04dcca418b47edc3972e3e1655210",
        ))

    def test_authority_denials(self) -> None:
        snapshot = self.value["snapshot"]
        for field in ("atomic", "authority_attested", "effect_attested",
                      "permission_attested", "persistence_attested", "truth_attested"):
            self.assertIs(snapshot[field], False)
        for field in ("currentness", "freshness", "system_completeness"):
            self.assertEqual(snapshot[field], "unknown")

    def test_fixed_classifier_ascii_fold_and_precedence(self) -> None:
        control = [".git/config", "src/.FoRgE/state"]
        sensitive = [".ENV", "x/.env.local", "id_RSA", "cert.KEY", ".SSH/key",
                     "x/SERVICE-ACCOUNT.JSON", "x/file.KEYSTORE"]
        for path in control:
            self.assertEqual(protected_reason(path), "control_path")
        for path in sensitive:
            self.assertEqual(protected_reason(path), "sensitive_path")
        self.assertEqual(protected_reason("secrets/.git/config"), "control_path")
        self.assertIsNone(protected_reason("ŚECRETS/file.txt"))
        self.assertIsNone(protected_reason("secrets.json"))
        self.assertIsNone(protected_reason("state.tfstate"))

    def test_cli_explicit_input(self) -> None:
        with tempfile.NamedTemporaryFile() as stream:
            stream.write(self.raw)
            stream.flush()
            result = subprocess.run(
                [sys.executable, "-B", str(CHECK), "--input", stream.name],
                cwd=ROOT, capture_output=True, check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout,
                         b"Project Source Snapshot v1: OK (authority neutral)\n")

    def test_cli_stdin_and_usage_fail_closed(self) -> None:
        success = subprocess.run(
            [sys.executable, "-B", str(CHECK), "--input", "-"], cwd=ROOT,
            input=self.raw, capture_output=True, check=False,
        )
        self.assertEqual(success.returncode, 0, success.stderr)
        failure = subprocess.run(
            [sys.executable, "-B", str(CHECK), "--input"], cwd=ROOT,
            capture_output=True, check=False,
        )
        self.assertEqual(failure.returncode, 2)
        self.assertEqual(failure.stdout, b"")

    @unittest.skipUnless(sys.platform.startswith("linux"), "requires Linux O_NONBLOCK pipe")
    def test_nonblocking_stdin_requires_explicit_eof(self) -> None:
        import fcntl
        read_fd, write_fd = os.pipe()
        try:
            capacity = fcntl.fcntl(read_fd, fcntl.F_GETPIPE_SZ)
            if capacity < len(self.raw):
                fcntl.fcntl(read_fd, fcntl.F_SETPIPE_SZ, len(self.raw))
            os.set_blocking(read_fd, False)
            self.assertEqual(os.write(write_fd, self.raw), len(self.raw))
            result = subprocess.run(
                [sys.executable, "-B", str(CHECK), "--input", "-"], cwd=ROOT,
                stdin=read_fd, capture_output=True, check=False, timeout=5,
            )
        finally:
            os.close(read_fd)
            os.close(write_fd)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_stdin_reader_rejects_incomplete_nonbytes_and_n_plus_one(self) -> None:
        class Input:
            def __init__(self, result):
                self.buffer, self.result, self.reads = self, result, 0

            def read(self, _size):
                self.reads += 1
                if isinstance(self.result, BaseException):
                    raise self.result
                return self.result

        for result in (BlockingIOError(), None, bytearray(b"x"), b"12345"):
            stream = Input(result)
            with self.subTest(result=type(result).__name__), \
                    mock.patch.object(check_cli, "MAX_ENVELOPE_BYTES", 4), \
                    mock.patch.object(check_cli.sys, "stdin", stream), \
                    self.assertRaises(check_cli.ContractError):
                check_cli._stdin()
            self.assertEqual(stream.reads, 1)

    def test_all_digest_domains_are_distinct(self) -> None:
        self.assertEqual(len(DOMAINS), len(set(DOMAINS.values())))
        self.assertEqual(set(DOMAINS), {
            "path", "request", "entry", "exclusion", "entry_set", "exclusion_set",
            "manifest", "coverage", "snapshot_identity", "snapshot", "envelope",
        })

    def test_portable_vendored_validator_is_byte_exact(self) -> None:
        authoritative = ROOT / "harness/project_source_snapshot_contract"
        vendored = (ROOT / "skills/project-snapshot/scripts/_vendor/"
                    "project_source_snapshot_contract")
        for name in ("__init__.py", "codec.py", "constants.py", "derive.py",
                     "shapes.py", "validation.py"):
            self.assertEqual((vendored / name).read_bytes(),
                             (authoritative / name).read_bytes(), name)

    def test_fixture_content_facts_are_exact(self) -> None:
        manifest = self.value["snapshot"]["source_manifest"]
        by_path = {item["path"]: item for item in manifest["entries"]}
        self.assertEqual(by_path["README.md"]["content_sha256"],
                         hashlib.sha256(b"fixture\n").hexdigest())
        self.assertEqual(by_path["scratch.txt"]["content_sha256"],
                         hashlib.sha256(b"scratch\n").hexdigest())

    def test_unknown_fields_and_authority_resealing_fail(self) -> None:
        value = copy.deepcopy(self.value)
        value["ambient_root"] = "/tmp/repo"
        with self.assertRaises(ContractError):
            validate_production(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["truth_attested"] = True
        with self.assertRaises(ContractError):
            validate_production(value)


if __name__ == "__main__":
    unittest.main()
