#!/usr/bin/env python3
"""Golden, framing, and isolation tests for policy-authority adapters."""

from __future__ import annotations

import io
import importlib.util
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
FIXTURES = SKILL / "references/fixtures"
GRANT_SCRIPT = SCRIPTS / "assess_declared_capability_grant.py"
APPROVAL_SCRIPT = SCRIPTS / "assess_declared_approval_record.py"


def canonical_json(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"),
                      sort_keys=True).encode("utf-8")


def load_fixture(name: str) -> dict[str, object]:
    return json.loads((FIXTURES / name).read_bytes())


def load_adapter(path: Path, name: str) -> object:
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def run_adapter(script: Path, raw: bytes, *arguments: str,
                cwd: Path | None = None,
                environment: dict[str, str] | None = None
                ) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-I", "-B", str(script), *arguments],
        input=raw, capture_output=True, cwd=cwd, env=environment,
        check=False, timeout=10,
    )


def run_writer_open_nonblocking_pipe(script: Path, raw: bytes
                                     ) -> subprocess.CompletedProcess[bytes]:
    read_fd, write_fd = os.pipe()
    try:
        os.set_blocking(read_fd, False)
        os.write(write_fd, raw[:256])
        return subprocess.run(
            [sys.executable, "-I", "-B", str(script)], stdin=read_fd,
            capture_output=True, env={}, check=False, timeout=10,
        )
    finally:
        os.close(read_fd)
        os.close(write_fd)


class BinaryInput:
    """Expose a supplied object as sys.stdin.buffer."""

    def __init__(self, buffer: object) -> None:
        self.buffer = buffer


class FixedRead:
    """Return or raise one controlled stdin read outcome."""

    def __init__(self, outcome: object) -> None:
        self.outcome = outcome

    def read(self, unused_size: int) -> object:
        if isinstance(self.outcome, BaseException):
            raise self.outcome
        return self.outcome


class ChunkedOutput:
    """Expose controlled binary stdout write behavior."""

    buffer: "ChunkedOutput"

    def __init__(self, progress: object) -> None:
        self.buffer = self
        self.progress = progress
        self.raw = bytearray()
        self.flushed = False

    def write(self, raw: bytes) -> object:
        if callable(self.progress):
            result = self.progress(raw)
            if isinstance(result, int) and not isinstance(result, bool) and result > 0:
                self.raw.extend(raw[:min(result, len(raw))])
            return result
        if self.progress is None or isinstance(self.progress, bool):
            return self.progress
        count = min(int(self.progress), len(raw))
        if count > 0:
            self.raw.extend(raw[:count])
        return count

    def flush(self) -> None:
        self.flushed = True

grant = load_adapter(GRANT_SCRIPT, "_policy_authority_grant_adapter_test")
approval = load_adapter(APPROVAL_SCRIPT, "_policy_authority_approval_adapter_test")


class AdapterCase:
    def __init__(self, name: str, script: Path, module: object,
                 fixture_name: str, rejection: bytes) -> None:
        self.name = name
        self.script = script
        self.module = module
        self.fixture = load_fixture(fixture_name)
        self.raw = canonical_json(self.fixture["assessment_request"])
        self.expected = canonical_json(self.fixture["expected_assessment"]) + b"\n"
        self.rejection = rejection


CASES = (
    AdapterCase("grant", GRANT_SCRIPT, grant, "capability-grant-v1.json",
                b"capability-grant assessment rejected\n"),
    AdapterCase("approval", APPROVAL_SCRIPT, approval, "approval-record-v1.json",
                b"approval-record assessment rejected\n"),
)


class PortableAdapterTests(unittest.TestCase):
    def assert_rejected(self, case: AdapterCase, raw: bytes) -> None:
        result = run_adapter(case.script, raw)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", case.rejection))
        self.assertNotIn(b"Traceback", result.stderr)

    def test_exact_golden_requests_emit_exact_assessments(self) -> None:
        for case in CASES:
            with self.subTest(case=case.name):
                result = run_adapter(case.script, case.raw)
                self.assertEqual((result.returncode, result.stdout, result.stderr),
                                 (0, case.expected, b""))
                parsed = json.loads(result.stdout)
                self.assertEqual(parsed["authorization_decision"], "none")
                self.assertFalse(parsed["permission_attestation"])

    def test_adapters_are_independent_of_cwd_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            for case in CASES:
                with self.subTest(case=case.name):
                    result = run_adapter(case.script, case.raw,
                                         cwd=Path(directory), environment={})
                    self.assertEqual((result.returncode, result.stdout, result.stderr),
                                     (0, case.expected, b""))

    def test_startup_flags_are_required_before_other_imports(self) -> None:
        expected = {
            GRANT_SCRIPT: b"capability-grant assessment rejected: isolated no-bytecode Python (-I -B) is required\n",
            APPROVAL_SCRIPT: b"approval-record assessment rejected: isolated no-bytecode Python (-I -B) is required\n",
            SCRIPTS / "check_package.py": b"policy-authority package rejected: isolated no-bytecode Python (-I -B) is required\n",
        }
        for script, error in expected.items():
            for flags in (("-B",), ("-I",)):
                with self.subTest(script=script.name, flags=flags):
                    result = subprocess.run(
                        [sys.executable, *flags, str(script)], input=b"",
                        capture_output=True, env={}, check=False, timeout=10)
                    self.assertEqual(
                        (result.returncode, result.stdout, result.stderr),
                        (1, b"", error))
        self.assertEqual(list(SKILL.rglob("__pycache__")), [])
        self.assertEqual(list(SKILL.rglob("*.pyc")), [])

    def test_any_argument_is_usage_error_without_stdout(self) -> None:
        for case in CASES:
            with self.subTest(case=case.name):
                result = run_adapter(case.script, case.raw, "--authorize")
                self.assertEqual((result.returncode, result.stdout), (2, b""))
                self.assertEqual(
                    result.stderr,
                    f"usage: {case.module.USAGE}\n".encode("ascii"))
        checker = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             "first", "second"], capture_output=True, env={}, check=False,
            timeout=10)
        self.assertEqual((checker.returncode, checker.stdout, checker.stderr),
                         (2, b"", b"usage: check_package.py [PACKAGE_ROOT]\n"))

    def test_wrong_contract_wrapper_noncanonical_and_unknown_reject(self) -> None:
        for index, case in enumerate(CASES):
            other = CASES[1 - index]
            unknown = dict(case.fixture["assessment_request"])
            unknown["unexpected"] = True
            inputs = (b"", canonical_json(case.fixture), other.raw, case.raw + b"\n",
                      b'{"a":1,"a":2}', b'{"a":1.5}', canonical_json(unknown))
            for raw in inputs:
                with self.subTest(case=case.name, raw=raw[:24]):
                    self.assert_rejected(case, raw)

    def test_wrong_and_missing_self_digests_reject_without_repair(self) -> None:
        for case in CASES:
            request = dict(case.fixture["assessment_request"])
            request["request_sha256"] = "0" * 64
            self.assert_rejected(case, canonical_json(request))
            request = dict(case.fixture["assessment_request"])
            del request["request_sha256"]
            self.assert_rejected(case, canonical_json(request))

    def test_deep_and_oversized_input_reject_without_stdout(self) -> None:
        deep = b"[" * 2000 + b"0" + b"]" * 2000
        for case in CASES:
            self.assert_rejected(case, deep)
            self.assert_rejected(case, b"x" * (case.module.MAX_REQUEST_BYTES + 1))

    def test_bound_n_and_n_plus_one_are_exact(self) -> None:
        for case in CASES:
            with self.subTest(case=case.name, size="n"):
                with mock.patch.object(case.module, "MAX_REQUEST_BYTES", 4), \
                        mock.patch.object(case.module.sys, "stdin",
                                          BinaryInput(io.BytesIO(b"1234"))):
                    self.assertEqual(case.module._read_request(), b"1234")
            with self.subTest(case=case.name, size="n_plus_one"):
                with mock.patch.object(case.module, "MAX_REQUEST_BYTES", 4), \
                        mock.patch.object(case.module.sys, "stdin",
                                          BinaryInput(io.BytesIO(b"12345"))):
                    with self.assertRaisesRegex(case.module.AdapterError, "byte bound"):
                        case.module._read_request()

    def test_real_nonblocking_writer_open_pipe_requires_explicit_eof(self) -> None:
        for case in CASES:
            with self.subTest(case=case.name):
                result = run_writer_open_nonblocking_pipe(case.script, case.raw)
                self.assertEqual((result.returncode, result.stdout, result.stderr),
                                 (1, b"", case.rejection))

    def test_none_nonbytes_and_blocking_read_fail_closed(self) -> None:
        outcomes = (None, "not-bytes", bytearray(b"bytes"), BlockingIOError())
        for case in CASES:
            for outcome in outcomes:
                with self.subTest(case=case.name, outcome=type(outcome).__name__):
                    with mock.patch.object(case.module.sys, "stdin",
                                          BinaryInput(FixedRead(outcome))):
                        with self.assertRaises(case.module.AdapterError):
                            case.module._read_request()

    def test_short_stdout_writes_complete_and_invalid_progress_fails(self) -> None:
        invalid = (0, None, True, lambda raw: len(raw) + 1)
        for case in CASES:
            with self.subTest(case=case.name, progress="short"):
                output = ChunkedOutput(3)
                with mock.patch.object(case.module.sys, "stdout", output):
                    case.module._write_all(b"0123456789")
                self.assertEqual(output.raw, b"0123456789")
                self.assertTrue(output.flushed)
            for progress in invalid:
                with self.subTest(case=case.name, progress=progress):
                    with mock.patch.object(case.module.sys, "stdout",
                                          ChunkedOutput(progress)):
                        with self.assertRaises(case.module.AdapterError):
                            case.module._write_all(b"x")

    def test_read_loader_memory_write_and_flush_failures_are_stable(self) -> None:
        for case in CASES:
            for target, failure in (("_read_request", MemoryError()),
                                    ("_write_all", OSError("write"))):
                error = io.StringIO()
                with mock.patch.object(case.module, target, side_effect=failure), \
                        redirect_stderr(error):
                    self.assertEqual(case.module.main([]), 1)
                self.assertEqual(error.getvalue(), case.rejection.decode())
            cached, case.module._contract_module = case.module._contract_module, None
            try:
                error = io.StringIO()
                with mock.patch.object(case.module.importlib.util,
                                       "spec_from_file_location", return_value=None), \
                        mock.patch.object(case.module, "_read_request",
                                          return_value=case.raw), redirect_stderr(error):
                    self.assertEqual(case.module.main([]), 1)
                self.assertEqual(error.getvalue(), case.rejection.decode())
            finally:
                case.module._contract_module = cached

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
            for case in CASES:
                script = copied / "scripts" / case.script.name
                result = run_adapter(script, case.raw, environment=environment)
                self.assertEqual((result.returncode, result.stdout),
                                 (0, case.expected))
            checker = subprocess.run(
                [sys.executable, "-I", "-B",
                 str(copied / "scripts/check_package.py"), str(copied)],
                capture_output=True, env=environment, check=False, timeout=10)
            self.assertEqual((checker.returncode, checker.stdout), (1, b""))
            self.assertFalse(marker.exists())

    def test_metadata_freezes_three_evaluation_cases_and_no_todos(self) -> None:
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertEqual([case["name"] for case in evals["cases"]], [
            "normal_declared_capability_grant_assessment",
            "normal_declared_approval_record_assessment",
            "dangerous_authority_repair_ambient_io_and_execution_request",
        ])
        instructions = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertNotIn("TODO", instructions)
        self.assertIn("Never describe a matching relation", instructions)


if __name__ == "__main__":
    unittest.main()
