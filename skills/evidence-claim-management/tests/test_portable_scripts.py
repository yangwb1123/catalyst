#!/usr/bin/env python3
"""Portable adapter tests for evidence-claim-management."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

SKILL = Path(__file__).resolve().parents[1]
SCRIPTS = SKILL / "scripts"
FIXTURE = SKILL / "references/fixtures/governance-evidence-claim-v1.json"
COMMAND = [sys.executable, "-I", "-B", str(SCRIPTS / "validate.py")]


def _load_adapter() -> object:
    path = SCRIPTS / "validate.py"
    specification = importlib.util.spec_from_file_location(
        "_evidence_claim_portable_adapter_test", path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


validate = _load_adapter()


def canonical_json(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                      sort_keys=True).encode("utf-8")


class PortableValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.golden = json.loads(FIXTURE.read_bytes())
        cls.records = [copy.deepcopy(entry["record"])
                       for entry in cls.golden["records"]]
        cls.raw = canonical_json(cls.records)
        cls.marker = (validate.SUCCESS + "\n").encode("ascii")

    def run_adapter(self, raw: bytes, *arguments: str,
                    cwd: Path | None = None,
                    environment: dict[str, str] | None = None,
                    command: list[str] | None = None
                    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            (COMMAND if command is None else command) + list(arguments),
            input=raw, capture_output=True, cwd=cwd, env=environment,
            check=False, timeout=10,
        )

    def assert_rejected(self, raw: bytes) -> None:
        result = self.run_adapter(raw)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"evidence-claim validation rejected\n")
        self.assertNotIn(b"Traceback", result.stderr)

    @staticmethod
    def reseal(records: list[dict[str, object]], index: int) -> None:
        record = records[index]
        record["integrity"]["canonical_sha256"] = (
            validate._load_contract().compute_record_digest(record))

    def test_exact_golden_record_set_emits_only_fixed_marker(self) -> None:
        result = self.run_adapter(self.raw)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, self.marker, b""))
        self.assertNotIn(self.raw[:64], result.stdout)

    def test_adapter_is_independent_of_cwd_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_adapter(self.raw, cwd=Path(directory), environment={})
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, self.marker, b""))

    def test_nonisolated_entrypoints_reject_before_other_imports(self) -> None:
        cases = (
            (SCRIPTS / "validate.py", self.raw,
             b"evidence-claim validation rejected: isolated Python (-I) is required\n"),
            (SCRIPTS / "check_package.py", b"",
             b"evidence-claim-management package rejected: isolated Python (-I) is required\n"),
        )
        for script, raw, error in cases:
            with self.subTest(script=script.name):
                result = self.run_adapter(
                    raw, command=[sys.executable, "-B", str(script)], environment={})
                self.assertEqual((result.returncode, result.stdout, result.stderr),
                                 (1, b"", error))

    def test_any_argument_is_usage_error_without_stdout(self) -> None:
        result = self.run_adapter(self.raw, "--repair")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr,
                         b"usage: validate.py < canonical-record-set.json\n")
        checker = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             "first", "second"], capture_output=True, env={}, check=False, timeout=10)
        self.assertEqual((checker.returncode, checker.stdout, checker.stderr),
                         (2, b"", b"usage: check_package.py [PACKAGE_ROOT]\n"))

    def test_empty_wrapper_noncanonical_duplicate_unknown_and_float_reject(self) -> None:
        unknown = copy.deepcopy(self.records)
        unknown[0]["unexpected"] = True
        self.reseal(unknown, 0)
        cases = (b"", b"[]", canonical_json(self.golden), self.raw + b"\n",
                 b'[{"a":1,"a":2}]', b'[{"a":1.5}]', canonical_json(unknown))
        for raw in cases:
            with self.subTest(raw=raw[:24]):
                self.assert_rejected(raw)

    def test_digest_authority_trust_and_reference_escalations_reject(self) -> None:
        mutations = []
        wrong_digest = copy.deepcopy(self.records)
        wrong_digest[0]["integrity"]["canonical_sha256"] = "0" * 64
        mutations.append(wrong_digest)
        authority = copy.deepcopy(self.records)
        authority[1]["status"]["state"] = "confirmed"
        self.reseal(authority, 1)
        mutations.append(authority)
        trusted = copy.deepcopy(self.records)
        trusted[0]["spec"]["source_trust"] = "authoritative"
        self.reseal(trusted, 0)
        mutations.append(trusted)
        broken = copy.deepcopy(self.records)
        broken[1]["spec"]["supporting_evidence_record_ids"] = ["missing-record"]
        self.reseal(broken, 1)
        mutations.append(broken)
        for records in mutations:
            with self.subTest(state=records[0].get("status")):
                self.assert_rejected(canonical_json(records))

    def test_missing_digest_is_not_authored_or_repaired(self) -> None:
        records = copy.deepcopy(self.records)
        del records[0]["integrity"]["canonical_sha256"]
        raw = canonical_json(records)
        self.assert_rejected(raw)
        self.assertNotIn(b"STRUCTURALLY_VALID", raw)

    def test_deep_and_oversized_input_reject_without_traceback(self) -> None:
        deep = b"[" * 2000 + b"0" + b"]" * 2000
        oversized = b"x" * (validate.MAX_RECORD_SET_BYTES + 1)
        self.assert_rejected(deep)
        self.assert_rejected(oversized)

    def test_record_set_read_bound_n_and_n_plus_one(self) -> None:
        class Input:
            def __init__(self, raw: bytes) -> None:
                self.buffer = io.BytesIO(raw)
        with mock.patch.object(validate, "MAX_RECORD_SET_BYTES", 4), \
                mock.patch.object(validate.sys, "stdin", Input(b"1234")):
            self.assertEqual(validate._read_record_set(), b"1234")
        with mock.patch.object(validate, "MAX_RECORD_SET_BYTES", 4), \
                mock.patch.object(validate.sys, "stdin", Input(b"12345")):
            with self.assertRaisesRegex(validate.AdapterError, "byte bound"):
                validate._read_record_set()

    def test_nonblocking_stdin_requires_explicit_eof(self) -> None:
        read_fd, write_fd = os.pipe()
        try:
            os.set_blocking(read_fd, False)
            os.write(write_fd, self.raw)
            result = subprocess.run(COMMAND, stdin=read_fd, capture_output=True,
                                    env={}, check=False, timeout=10)
        finally:
            os.close(read_fd)
            os.close(write_fd)
        self.assertEqual((result.returncode, result.stdout), (1, b""))

    def test_short_stdout_writes_complete_and_zero_progress_fails(self) -> None:
        class Output:
            buffer: "Output"

            def __init__(self, progress: int) -> None:
                self.buffer = self
                self.progress = progress
                self.raw = bytearray()
                self.flushed = False
            def write(self, raw: bytes) -> int:
                count = min(self.progress, len(raw))
                self.raw.extend(raw[:count])
                return count
            def flush(self) -> None:
                self.flushed = True
        output = Output(3)
        with mock.patch.object(validate.sys, "stdout", output):
            validate._write_all(b"0123456789")
        self.assertEqual(output.raw, b"0123456789")
        self.assertTrue(output.flushed)
        with mock.patch.object(validate.sys, "stdout", Output(0)):
            with self.assertRaisesRegex(validate.AdapterError, "forward progress"):
                validate._write_all(b"x")

    def test_loader_failure_and_memory_failure_are_stable(self) -> None:
        error = io.StringIO()
        with mock.patch.object(validate, "_read_record_set", side_effect=MemoryError), \
                redirect_stderr(error):
            self.assertEqual(validate.main([]), 1)
        self.assertEqual(error.getvalue(), "evidence-claim validation rejected\n")
        cached, validate._contract_module = validate._contract_module, None
        try:
            error = io.StringIO()
            with mock.patch.object(validate.importlib.util, "spec_from_file_location",
                                   return_value=None), \
                    mock.patch.object(validate, "_read_record_set",
                                      return_value=self.raw), redirect_stderr(error):
                self.assertEqual(validate.main([]), 1)
            self.assertEqual(error.getvalue(), "evidence-claim validation rejected\n")
        finally:
            validate._contract_module = cached

    def test_script_and_pythonpath_import_shadows_do_not_execute(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            copied = base / "package"
            shutil.copytree(SKILL, copied)
            marker = base / "shadow-executed"
            shadow = copied / "scripts/hashlib.py"
            shadow.write_text(
                f"open({str(marker)!r}, 'w').write('executed')\n"
                "raise RuntimeError('shadow executed')\n", encoding="utf-8")
            environment = {"PYTHONPATH": str(shadow.parent),
                           "PYTHONDONTWRITEBYTECODE": "1"}
            result = self.run_adapter(
                self.raw, environment=environment,
                command=[sys.executable, "-I", "-B",
                         str(copied / "scripts/validate.py")])
            checker = subprocess.run(
                [sys.executable, "-I", "-B",
                 str(copied / "scripts/check_package.py"), str(copied)],
                capture_output=True, env=environment, check=False, timeout=10)
            shadow_executed = marker.exists()
        self.assertEqual((result.returncode, result.stdout), (0, self.marker))
        self.assertEqual((checker.returncode, checker.stdout), (1, b""))
        self.assertFalse(shadow_executed)

    def test_eval_metadata_freezes_normal_and_dangerous_cases(self) -> None:
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertEqual([case["name"] for case in evals["cases"]], [
            "normal_supplied_shadow_record_set",
            "dangerous_authoring_ambient_io_and_authority_request",
        ])
        instructions = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertNotIn("TODO", instructions)
        self.assertIn("never turn prose", instructions)


if __name__ == "__main__":
    unittest.main()
