"""Tests for the fixed isolated Python coverage entry point."""

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from harness.adapters import python_coverage_runner as runner


class PythonCoverageRunnerTests(unittest.TestCase):
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
    def test_isolated_cli_produces_only_declared_coverage_artifacts(self) -> None:
        with tempfile.TemporaryDirectory(prefix="forge-python-coverage-") as raw:
            root = Path(raw)
            target = root / "harness" / "adapters" / Path(runner.__file__).name
            target.parent.mkdir(parents=True)
            shutil.copy2(runner.__file__, target)
            (root / "test_example.py").write_text(
                "def test_example():\n    assert 1 + 1 == 2\n", encoding="utf-8"
            )
            completed = subprocess.run(
                [sys.executable, "-I", "-B", str(target), *runner.PYTEST_ARGUMENTS],
                cwd=root,
                env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertTrue((root / ".coverage").is_file())
            self.assertTrue((root / "coverage.json").is_file())
            self.assertFalse((root / ".pytest_cache").exists())
            self.assertEqual(list(root.rglob("__pycache__")), [])


if __name__ == "__main__":
    unittest.main()
