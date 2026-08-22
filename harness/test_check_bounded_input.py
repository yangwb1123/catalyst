#!/usr/bin/env python3
"""Formal forge-check regressions for bounded untrusted YAML input."""
import io
import shutil
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
import check  # noqa: E402
import engineering_check_support as support  # noqa: E402
from test_check import make_temp_repo, run_cli  # noqa: E402


class CheckBoundedInputTest(unittest.TestCase):
    def setUp(self):
        self.repo = make_temp_repo()
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.policy = self.repo / ".agent/engineering/governance-contracts.yml"

    def test_formal_cli_rejects_sparse_oversized_registry(self):
        with self.policy.open("wb") as stream:
            stream.truncate(524_289)
        result = run_cli(self.repo)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("file exceeds 524288 bytes", result.stdout)
        self.assertNotIn("Traceback", result.stderr)

    def test_formal_entry_controls_memory_failure(self):
        real_read = support.read_regular_file

        def fail_policy(path, *args, **kwargs):
            if Path(path) == self.policy:
                raise MemoryError
            return real_read(path, *args, **kwargs)

        output = io.StringIO()
        with patch.object(support, "read_regular_file", fail_policy), redirect_stdout(output):
            result = check.main(["check.py", str(self.repo)])
        self.assertEqual(result, 1)
        self.assertIn("bounded spec read exhausted memory", output.getvalue())


if __name__ == "__main__":
    unittest.main()
