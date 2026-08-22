#!/usr/bin/env python3
"""Golden, framing, and isolation tests for the portable ADR adapter."""

from __future__ import annotations

import hashlib
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
SCRIPT = SCRIPTS / "validate_declared_proposed_adr.py"
GOLDEN = SKILL / "references/fixtures/ADR-9001-proposed-boundary.md"
SCHEMA = SKILL / "references/architecture-decision-record-v2.schema.json"
NAME = "ADR-9001-proposed-boundary.md"
MARKER = (
    b"STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document "
    b"bytes only; no identity, ownership, approver, evidence, claim, graph, "
    b"acceptance, compliance, persistence, transition, execution, or effect "
    b"attestation)\n"
)
REJECTION = b"proposed ADR v2 validation rejected\n"


def load_adapter() -> object:
    specification = importlib.util.spec_from_file_location(
        "_adr_governance_portable_adapter_test", SCRIPT)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {SCRIPT}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


def run_adapter(raw: bytes, *arguments: str, cwd: Path | None = None,
                environment: dict[str, str] | None = None,
                script: Path = SCRIPT) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-I", "-B", str(script), *arguments], input=raw,
        capture_output=True, cwd=cwd, env=environment, check=False, timeout=10)


def run_writer_open_pipe(raw: bytes) -> subprocess.CompletedProcess[bytes]:
    read_fd, write_fd = os.pipe()
    try:
        os.set_blocking(read_fd, False)
        os.write(write_fd, raw[:256])
        return subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPT), NAME], stdin=read_fd,
            capture_output=True, env={}, check=False, timeout=10)
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
    """Expose controlled binary stdout write and flush behavior."""

    buffer: "ChunkedOutput"

    def __init__(self, progress: object, flush_error: bool = False) -> None:
        self.buffer = self
        self.progress = progress
        self.flush_error = flush_error
        self.raw = bytearray()
        self.flushed = False

    def write(self, raw: bytes) -> object:
        result = self.progress(raw) if callable(self.progress) else self.progress
        if result is None or isinstance(result, bool):
            return result
        count = min(int(result), len(raw))
        if count > 0:
            self.raw.extend(raw[:count])
        return result if callable(self.progress) else count

    def flush(self) -> None:
        self.flushed = True
        if self.flush_error:
            raise OSError("flush")


adapter = load_adapter()


class PortableAdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.golden = GOLDEN.read_bytes()

    def assert_rejected(self, raw: bytes, name: str = NAME) -> None:
        result = run_adapter(raw, name)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", REJECTION))
        self.assertNotIn(b"Traceback", result.stderr)

    def test_exact_golden_emits_only_exact_marker(self) -> None:
        result = run_adapter(self.golden, NAME)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, MARKER, b""))

    def test_adapter_is_independent_of_cwd_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = run_adapter(self.golden, NAME, cwd=Path(directory),
                                 environment={})
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, MARKER, b""))

    def test_startup_flags_are_required_before_other_imports(self) -> None:
        expected = {
            SCRIPT: b"proposed ADR v2 validation rejected: isolated no-bytecode Python (-I -B) is required\n",
            SCRIPTS / "check_package.py": b"adr-governance package rejected: isolated no-bytecode Python (-I -B) is required\n",
        }
        for script, error in expected.items():
            for flags in (("-B",), ("-I",)):
                with self.subTest(script=script.name, flags=flags):
                    result = subprocess.run(
                        [sys.executable, *flags, str(script)], input=b"",
                        capture_output=True, env={}, check=False, timeout=10)
                    self.assertEqual((result.returncode, result.stdout,
                                      result.stderr), (1, b"", error))
        self.assertEqual(list(SKILL.rglob("__pycache__")), [])
        self.assertEqual(list(SKILL.rglob("*.pyc")), [])

    def test_exactly_one_basename_argument_is_required(self) -> None:
        for arguments in ((), (NAME, "second")):
            result = run_adapter(self.golden, *arguments)
            self.assertEqual((result.returncode, result.stdout), (2, b""))
            self.assertEqual(
                result.stderr,
                b"usage: validate_declared_proposed_adr.py ADR-NNNN-slug.md\n")
        checker = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             "first", "second"], capture_output=True, env={}, check=False,
            timeout=10)
        self.assertEqual((checker.returncode, checker.stdout, checker.stderr),
                         (2, b"", b"usage: check_package.py [PACKAGE_ROOT]\n"))

    def test_wrong_path_alias_and_noncanonical_basenames_reject(self) -> None:
        invalid = (
            "./" + NAME, "/tmp/" + NAME, "subdir/" + NAME,
            "ADR-0000-proposed-boundary.md", "ADR-9001-Proposed-boundary.md",
            "ADR-9001-proposed_boundary.md", "ADR-9001-proposed-boundary.md/",
            "ADR-9002-proposed-boundary.md", "ADR-9001-x" + "x" * 250 + ".md",
        )
        for name in invalid:
            with self.subTest(name=name):
                self.assert_rejected(self.golden, name)

    def test_noncanonical_malformed_and_digest_mutations_reject(self) -> None:
        variants = (
            b"", self.golden + b"\n", self.golden.replace(b"\n", b"\r\n", 1),
            self.golden.replace(b'"status":"proposed"', b'"status":"accepted"'),
            self.golden.replace(b'"api_version":', b'"unknown":0,"api_version":', 1),
            self.golden.replace(b"## Context", b"## Context changed", 1),
            self.golden[:-2] + bytes([self.golden[-2] ^ 1]) + b"\n",
            b"[" * 2000 + b"0" + b"]" * 2000,
        )
        for raw in variants:
            with self.subTest(raw=raw[:24]):
                self.assert_rejected(raw)

    def test_bound_n_and_n_plus_one_are_exact(self) -> None:
        with mock.patch.object(adapter, "MAX_DOCUMENT_BYTES", 4), \
                mock.patch.object(adapter.sys, "stdin",
                                  BinaryInput(io.BytesIO(b"1234"))):
            self.assertEqual(adapter._read_document(), b"1234")
        with mock.patch.object(adapter, "MAX_DOCUMENT_BYTES", 4), \
                mock.patch.object(adapter.sys, "stdin",
                                  BinaryInput(io.BytesIO(b"12345"))):
            with self.assertRaisesRegex(adapter.AdapterError, "byte bound"):
                adapter._read_document()

    def test_real_nonblocking_writer_open_pipe_requires_explicit_eof(self) -> None:
        result = run_writer_open_pipe(self.golden)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", REJECTION))

    def test_none_nonbytes_and_blocking_read_fail_closed(self) -> None:
        for outcome in (None, "not-bytes", bytearray(b"bytes"), BlockingIOError()):
            with self.subTest(outcome=type(outcome).__name__), \
                    mock.patch.object(adapter.sys, "stdin",
                                      BinaryInput(FixedRead(outcome))):
                with self.assertRaises(adapter.AdapterError):
                    adapter._read_document()

    def test_short_stdout_writes_complete_and_bad_progress_fails(self) -> None:
        output = ChunkedOutput(3)
        with mock.patch.object(adapter.sys, "stdout", output):
            adapter._write_all(b"0123456789")
        self.assertEqual(output.raw, b"0123456789")
        self.assertTrue(output.flushed)
        for progress in (0, None, True, lambda raw: len(raw) + 1):
            with self.subTest(progress=progress), \
                    mock.patch.object(adapter.sys, "stdout",
                                      ChunkedOutput(progress)):
                with self.assertRaises(adapter.AdapterError):
                    adapter._write_all(b"x")

    def test_read_loader_write_and_flush_failures_return_one(self) -> None:
        for target in ("_read_document", "_load_contract"):
            error = io.StringIO()
            with mock.patch.object(adapter, target, side_effect=MemoryError()), \
                    redirect_stderr(error):
                self.assertEqual(adapter.main([NAME]), 1)
            self.assertEqual(error.getvalue(), REJECTION.decode())
        for output in (ChunkedOutput(0), ChunkedOutput(3, flush_error=True)):
            error = io.StringIO()
            with mock.patch.object(adapter, "_read_document",
                                   return_value=self.golden), \
                    mock.patch.object(adapter.sys, "stdout", output), \
                    redirect_stderr(error):
                self.assertEqual(adapter.main([NAME]), 1)
            self.assertEqual(error.getvalue(), REJECTION.decode())

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
            result = run_adapter(
                self.golden, NAME, script=copied / "scripts" / SCRIPT.name,
                environment={"PYTHONPATH": str(shadow.parent)})
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, MARKER, b""))
        self.assertFalse(marker.exists())

    def test_frozen_schema_golden_evals_and_no_todos(self) -> None:
        self.assertEqual(hashlib.sha256(SCHEMA.read_bytes()).hexdigest(),
                         "ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b")
        self.assertEqual(hashlib.sha256(self.golden).hexdigest(),
                         "b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194")
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertEqual([case["name"] for case in evals["cases"]], [
            "normal_exact_proposed_adr_v2_validation",
            "dangerous_repair_authority_lifecycle_and_ambient_io_request",
        ])
        instructions = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertNotIn("TODO", instructions)
        self.assertIn("caller-provided lexical label", instructions)


if __name__ == "__main__":
    unittest.main()
