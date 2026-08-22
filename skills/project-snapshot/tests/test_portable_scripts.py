#!/usr/bin/env python3
"""Independent executable and integrity tests for the portable Skill scripts."""

from __future__ import annotations

import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

SKILL = Path(__file__).resolve().parents[1]
REPOSITORY = SKILL.parents[1]
SCRIPTS = SKILL / "scripts"


def _load_script(name: str, path: Path) -> object:
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


capture = _load_script("_project_snapshot_capture_test", SCRIPTS / "capture.py")
MAX_ENVELOPE_BYTES = capture.MAX_ENVELOPE_BYTES


class CaptureAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.root = self.base / "worktree"
        self.root.mkdir()
        self.golden = (REPOSITORY / "docs/contracts/fixtures/"
                       "project-source-snapshot-v1.json")
        self.assertTrue(self.golden.is_file())

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _runtime(self, body: str) -> Path:
        path = self.base / f"runtime-{len(list(self.base.glob('runtime-*')))}"
        path.write_text("#!/usr/bin/python3\n" + body, encoding="utf-8")
        path.chmod(0o755)
        return path

    def _golden_runtime(self, project: str = "fixture-project",
                        run: str = "fixture-run-001") -> Path:
        expected = ["project-snapshot", "capture", "--project-id", project,
                    "--run-id", run, "--root", str(self.root)]
        return self._runtime(
            "import pathlib,sys\n"
            f"expected={expected!r}\n"
            "if sys.argv[1:] != expected:\n"
            " sys.stderr.write('unexpected arguments: '+repr(sys.argv[1:]))\n"
            " raise SystemExit(91)\n"
            f"sys.stdout.buffer.write(pathlib.Path({str(self.golden)!r}).read_bytes())\n"
        )

    def _run(self, runtime: Path, project: str = "fixture-project",
             run: str = "fixture-run-001", env: dict[str, str] | None = None
             ) -> subprocess.CompletedProcess[bytes]:
        command = [sys.executable, "-I", "-B", str(SCRIPTS / "capture.py"),
                   "--run-id", run, "--root", str(self.root),
                   "--forge", str(runtime), "--project-id", project]
        return subprocess.run(command, input=b"ignored stdin", capture_output=True,
                              env=env, check=False, timeout=10)

    def test_exact_child_arguments_and_valid_golden(self) -> None:
        result = self._run(self._golden_runtime())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, self.golden.read_bytes())
        self.assertEqual(result.stderr, b"")

    def test_absolute_runtime_and_path_confusion(self) -> None:
        malicious = self.base / "forge"
        malicious.write_text("#!/bin/sh\necho selected-path-forge >&2\nexit 88\n")
        malicious.chmod(0o755)
        environment = dict(os.environ)
        environment["PATH"] = str(self.base)
        result = self._run(self._golden_runtime(), env=environment)
        self.assertEqual(result.returncode, 0, result.stderr)
        relative = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "capture.py"),
             "--forge", "forge", "--root", str(self.root),
             "--project-id", "fixture-project", "--run-id", "fixture-run-001"],
            capture_output=True, check=False,
        )
        self.assertEqual(relative.returncode, 2)
        self.assertEqual(relative.stdout, b"")

    def test_strict_digest_tamper_writes_no_stdout(self) -> None:
        value = json.loads(self.golden.read_bytes())
        value["envelope_sha256"] = "0" * 64
        payload = self.base / "tampered.json"
        payload.write_bytes(json.dumps(
            value, ensure_ascii=False, separators=(",", ":"), sort_keys=True,
        ).encode() + b"\n")
        runtime = self._runtime(
            "import pathlib,sys\n"
            f"sys.stdout.buffer.write(pathlib.Path({str(payload)!r}).read_bytes())\n"
        )
        result = self._run(runtime)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"strict contract", result.stderr)

    def test_valid_but_wrong_request_binding_is_rejected(self) -> None:
        runtime = self._golden_runtime("another-project", "another-run")
        result = self._run(runtime, "another-project", "another-run")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"does not bind", result.stderr)

    def test_missing_runtime_is_not_executed(self) -> None:
        missing = self.base / "missing-forge"
        result = self._run(missing)
        self.assertEqual(result.returncode, 3)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"not_executed", result.stderr)

    def test_vendored_contract_is_anchored_without_sys_path_insertion(self) -> None:
        loaded = sys.modules[capture._CONTRACT_MODULE]
        expected = SCRIPTS / "_vendor/project_source_snapshot_contract/__init__.py"
        self.assertEqual(Path(loaded.__file__).resolve(), expected.resolve())
        self.assertNotIn(str(SCRIPTS), sys.path)
        self.assertNotIn(str(SCRIPTS / "_vendor"), sys.path)

    def test_unsupported_host_is_not_executed_before_runtime_open(self) -> None:
        output, error = io.StringIO(), io.StringIO()
        arguments = ["--forge", r"C:\\ForgeOS\\forge.exe", "--root", r"C:\\worktree",
                     "--project-id", "fixture-project", "--run-id", "fixture-run-001"]
        with mock.patch.object(capture.sys, "platform", "win32"), \
                mock.patch.object(capture, "run_capture") as run, \
                redirect_stdout(output), redirect_stderr(error):
            self.assertEqual(capture.main(arguments), 3)
        run.assert_not_called()
        self.assertEqual(output.getvalue(), "")
        self.assertIn("not_executed", error.getvalue())

    def test_child_failure_discards_child_stdout(self) -> None:
        runtime = self._runtime(
            "import sys\n"
            "sys.stdout.buffer.write(b'untrusted partial success')\n"
            "sys.stderr.write('intentional failure')\n"
            "raise SystemExit(9)\n"
        )
        result = self._run(runtime)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"exit 9", result.stderr)
        self.assertIn(b"stderr_bytes=19", result.stderr)
        self.assertIn(b"stderr_sha256=", result.stderr)
        self.assertNotIn(b"intentional failure", result.stderr)

    def _writer(self, stream: int, count: int) -> subprocess.Popen[bytes]:
        code = f"import os; os.write({stream}, b'x'*{count})"
        return subprocess.Popen(
            [sys.executable, "-c", code], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True,
        )

    def test_independent_stdout_and_stderr_n_and_n_plus_one(self) -> None:
        self.assertEqual(capture.MAX_STDOUT, MAX_ENVELOPE_BYTES + 1)
        self.assertEqual(capture.MAX_STDERR, 32 * 1024 * 1024)
        with mock.patch.object(capture, "MAX_STDOUT", 64), \
                mock.patch.object(capture, "MAX_STDERR", 32):
            output, error, code = capture._read_process(self._writer(1, 64))
            self.assertEqual((len(output), error, code), (64, b"", 0))
            with self.assertRaises(capture.AdapterError):
                capture._read_process(self._writer(1, 65))
            output, error, code = capture._read_process(self._writer(2, 32))
            self.assertEqual((output, len(error), code), (b"", 32, 0))
            with self.assertRaises(capture.AdapterError):
                capture._read_process(self._writer(2, 33))

    def test_timeout_kills_forked_pipe_holder(self) -> None:
        marker = self.base / "descendant-survived"
        code = (
            "import os,time\n"
            "if os.fork()==0:\n"
            " time.sleep(0.6)\n"
            f" open({str(marker)!r},'w').write('survived')\n"
            " os._exit(0)\n"
            "os._exit(0)\n"
        )
        process = subprocess.Popen(
            [sys.executable, "-c", code], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True,
        )
        started = time.monotonic()
        with mock.patch.object(capture, "TIMEOUT_SECONDS", 0.1):
            with self.assertRaisesRegex(capture.AdapterError, "timed out"):
                capture._read_process(process)
        self.assertLess(time.monotonic() - started, 2)
        time.sleep(0.7)
        self.assertFalse(marker.exists())


class IsolatedStartupTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.package = self.base / "project-snapshot"
        shutil.copytree(SKILL, self.package)
        self.marker = self.base / "sentinel-executed"
        sentinel = (
            f"open({str(self.marker)!r}, 'ab').write(b'executed')\n"
            "raise RuntimeError('import shadow sentinel executed')\n"
        )
        (self.package / "scripts/hashlib.py").write_text(sentinel, encoding="utf-8")
        self.pythonpath = self.base / "pythonpath"
        self.pythonpath.mkdir()
        (self.pythonpath / "sitecustomize.py").write_text(sentinel, encoding="utf-8")
        (self.pythonpath / "hashlib.py").write_text(sentinel, encoding="utf-8")
        self.worktree = self.base / "worktree"
        self.worktree.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _run(self, relative: str, arguments: list[str], isolated: bool,
             pythonpath: bool = False) -> subprocess.CompletedProcess[bytes]:
        flags = ["-I", "-B"] if isolated else ["-B"]
        environment = dict(os.environ)
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        if pythonpath:
            environment["PYTHONPATH"] = str(self.pythonpath)
        else:
            environment.pop("PYTHONPATH", None)
        return subprocess.run(
            [sys.executable, *flags, str(self.package / relative), *arguments],
            cwd=self.package, capture_output=True, env=environment,
            check=False, timeout=10,
        )

    def test_isolated_startup_ignores_script_and_pythonpath_shadows(self) -> None:
        missing = self.base / "missing-forge"
        capture_args = ["--forge", str(missing), "--root", str(self.worktree),
                        "--project-id", "fixture-project", "--run-id", "fixture-run"]
        cases = [("scripts/check_package.py", [], 1, b"file set differs"),
                 ("scripts/capture.py", capture_args, 3, b"not_executed")]
        for relative, arguments, expected, marker in cases:
            first = self._run(relative, arguments, True, True)
            second = self._run(relative, arguments, True, True)
            self.assertEqual(first.returncode, expected, first.stderr)
            self.assertEqual(first.stdout, b"")
            self.assertEqual(first.stderr, second.stderr)
            self.assertIn(marker, first.stderr)
            self.assertNotIn(b"Traceback", first.stderr)
        self.assertFalse(self.marker.exists())

    def test_nonisolated_startup_rejects_before_script_shadow_import(self) -> None:
        expected = {
            "scripts/check_package.py": b"project-snapshot package rejected",
            "scripts/capture.py": b"project-snapshot capture rejected",
        }
        for relative, prefix in expected.items():
            first = self._run(relative, [], False)
            second = self._run(relative, [], False)
            self.assertEqual(first.returncode, 1)
            self.assertEqual(first.stdout, b"")
            self.assertEqual(first.stderr, second.stderr)
            self.assertIn(prefix, first.stderr)
            self.assertIn(b"python3 -I -B", first.stderr)
            self.assertNotIn(b"Traceback", first.stderr)
        self.assertFalse(self.marker.exists())


if __name__ == "__main__":
    unittest.main()
