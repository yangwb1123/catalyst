"""Tests for the fixed isolated Python coverage entry point."""

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from harness.adapters import python_coverage_runner as runner


PARALLEL_PROBE = """\
import os
import time
from pathlib import Path


def test_parallel_worker():
    root = Path(os.environ["FORGE_COVERAGE_TEST_ROOT"])
    prefixes = (".coverage.", ".forge-coverage-workers-")
    assert not [path for path in root.iterdir()
                if path.name.startswith(prefixes)]
    sync = Path(os.environ["FORGE_COVERAGE_TEST_SYNC"])
    (sync / f"worker-{os.getpid()}").write_text("started", encoding="utf-8")
    deadline = time.monotonic() + 10
    while len(list(sync.glob("worker-*"))) < 2 and time.monotonic() < deadline:
        time.sleep(0.01)
    assert len(list(sync.glob("worker-*"))) == 2
"""


class PythonCoverageRunnerTests(unittest.TestCase):
    def test_subprocess_stream_limits_are_independent(self) -> None:
        for overflowing, preserved in (("stdout", "stderr"), ("stderr", "stdout")):
            with self.subTest(stream=overflowing):
                script = (
                    "import sys, time; "
                    f"sys.{preserved}.buffer.write(b'preserved'); "
                    f"sys.{preserved}.flush(); "
                    f"sys.{overflowing}.buffer.write(b'x' * 8192); "
                    f"sys.{overflowing}.flush(); time.sleep(30)"
                )
                completed = runner._bounded_subprocess(
                    [sys.executable, "-I", "-B", "-c", script],
                    cwd=Path.cwd(), timeout=2, stream_limit=256,
                )
                self.assertEqual(completed.returncode, 1)
                self.assertEqual(len(getattr(completed, overflowing).encode()), 256)
                self.assertIn("preserved", getattr(completed, preserved))
                self.assertIn(
                    f"subprocess {overflowing} exceeded 256 bytes",
                    completed.diagnostic,
                )

    @unittest.skipUnless(os.name in {"posix", "nt"}, "process-tree kill is unsupported")
    def test_subprocess_timeout_kills_and_reaps_descendant_tree(self) -> None:
        with tempfile.TemporaryDirectory(prefix="forge-python-timeout-") as raw:
            root = Path(raw)
            ready, release, escaped = (
                root / "ready", root / "release", root / "escaped"
            )
            descendant = (
                "import time; from pathlib import Path; "
                f"ready=Path({str(ready)!r}); release=Path({str(release)!r}); "
                f"escaped=Path({str(escaped)!r}); ready.write_text('ready'); "
                "\nwhile not release.exists(): time.sleep(0.01)\n"
                "escaped.write_text('escaped')"
            )
            parent = (
                "import subprocess, sys, time; from pathlib import Path; "
                f"subprocess.Popen([sys.executable, '-I', '-B', '-c', {descendant!r}]); "
                f"ready=Path({str(ready)!r}); deadline=time.monotonic()+2; "
                "\nwhile not ready.exists() and time.monotonic()<deadline: time.sleep(0.01)\n"
                "time.sleep(30)"
            )
            completed = runner._bounded_subprocess(
                [sys.executable, "-I", "-B", "-c", parent],
                cwd=root, timeout=3, stream_limit=4096,
            )
            self.assertTrue(ready.is_file())
            self.assertEqual(completed.returncode, 1)
            self.assertIn("subprocess timed out after 3s", completed.diagnostic)
            release.write_text("release", encoding="utf-8")
            time.sleep(0.2)
            self.assertFalse(escaped.exists())

    def test_fixed_profile_injects_only_repository_import_roots(self) -> None:
        calls: list[list[str]] = []
        original = list(sys.path)
        flags = SimpleNamespace(isolated=1, dont_write_bytecode=1)
        try:
            result = runner.run(
                runner.PYTEST_ARGUMENTS,
                flags=flags,
                pytest_main=lambda arguments: calls.append(arguments) or 0,
            )
            self.assertEqual(result, 0)
            self.assertEqual(calls, [list(runner.PYTEST_ARGUMENTS)])
            self.assertIn("no:cacheprovider", calls[0])
            root = Path(runner.__file__).resolve().parents[2]
            self.assertEqual(sys.path[:2], [str(root / "harness"), str(root)])
        finally:
            sys.path[:] = original

    def test_wrong_flags_or_arguments_fail_before_pytest(self) -> None:
        called = False

        def unexpected(_arguments: list[str]) -> int:
            nonlocal called
            called = True
            return 0

        with patch.object(sys, "stderr"):
            unsafe_flags = SimpleNamespace(isolated=0, dont_write_bytecode=0)
            self.assertEqual(
                runner.run(
                    runner.PYTEST_ARGUMENTS,
                    flags=unsafe_flags,
                    pytest_main=unexpected,
                ),
                2,
            )
            flags = SimpleNamespace(isolated=1, dont_write_bytecode=1)
            self.assertEqual(runner.run(("--cov",), flags=flags, pytest_main=unexpected), 2)
        self.assertFalse(called)

    def test_measured_long_suites_shard_without_splitting_other_files(self) -> None:
        long_file = "harness/test_agent_engineering_check.py"
        long_nodes = [
            f"{long_file}::SpecValidationTest::test_case_{index}"
            for index in range(33)
        ]
        other = [f"harness/test_small.py::test_{index}" for index in range(3)]
        shards = runner._shards(long_nodes + other)
        flattened = [node for shard in shards for node in shard]
        self.assertEqual(flattened, long_nodes + other)
        self.assertTrue(all(len(shard) <= runner.MAX_ITEMS_PER_SHARD for shard in shards[:-1]))
        self.assertIn(other, shards)

    def test_cli_rejects_nonisolated_execution(self) -> None:
        completed = subprocess.run(
            [sys.executable, str(Path(runner.__file__).resolve()), *runner.PYTEST_ARGUMENTS],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 2)
        self.assertIn("isolated no-bytecode", completed.stderr)

    @unittest.skipUnless(
        importlib.util.find_spec("pytest") and importlib.util.find_spec("pytest_cov"),
        "pytest coverage tooling is unavailable",
    )
    def test_parallel_cli_keeps_worker_data_outside_repository(self) -> None:
        with tempfile.TemporaryDirectory(prefix="forge-python-coverage-") as raw, \
                tempfile.TemporaryDirectory(prefix="forge-python-sync-") as sync_raw:
            root = Path(raw)
            sync = Path(sync_raw)
            target = root / "harness" / "adapters" / Path(runner.__file__).name
            target.parent.mkdir(parents=True)
            shutil.copy2(runner.__file__, target)
            for index in range(2):
                (root / f"test_worker_{index}.py").write_text(
                    PARALLEL_PROBE, encoding="utf-8")
            environment = {
                **os.environ,
                "FORGE_COVERAGE_TEST_ROOT": str(root),
                "FORGE_COVERAGE_TEST_SYNC": str(sync),
                "FORGE_COVERAGE_WORKERS": "2",
                "PYTHONDONTWRITEBYTECODE": "1",
                "TMPDIR": str(root),
            }
            completed = subprocess.run(
                [sys.executable, "-I", "-B", str(target), *runner.PYTEST_ARGUMENTS],
                cwd=root,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertEqual(len(list(sync.glob("worker-*"))), 2)
            self.assertTrue((root / ".coverage").is_file())
            self.assertTrue((root / "coverage.json").is_file())
            self.assertEqual(list(root.glob(".coverage.*")), [])
            self.assertEqual(list(root.glob(".forge-coverage-workers-*")), [])
            self.assertFalse((root / ".pytest_cache").exists())
            self.assertEqual(list(root.rglob("__pycache__")), [])


if __name__ == "__main__":
    unittest.main()
