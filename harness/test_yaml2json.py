#!/usr/bin/env python3
"""Tests for the ForgeOS YAML→JSON transcoder (`harness/yaml2json.py`).

Uses only Python's built-in ``unittest`` — NO external test dependencies.
Tests drive the real CLI via ``subprocess`` (decoupled from internal APIs),
asserting on the contract the Go runtime depends on: real JSON on stdout,
exit codes on the error paths. Covered faces: happy path (a real asset),
missing-file error, and missing-argument error.

Run::

    python3 -m unittest harness.test_yaml2json
    python3 harness/test_yaml2json.py
"""
import json
import subprocess
import sys
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
REPO_ROOT = HARNESS_DIR.parent
YAML2JSON_PY = HARNESS_DIR / "yaml2json.py"
REAL_ASSET = REPO_ROOT / ".agent" / "workflows" / "build.yml"


def run_cli(*args):
    """Invoke `python3 harness/yaml2json.py <args>`; return CompletedProcess."""
    return subprocess.run(
        [sys.executable, str(YAML2JSON_PY), *args],
        capture_output=True,
        text=True,
        check=False,
    )


class RealAssetTest(unittest.TestCase):
    """A real .agent asset must transcode to valid JSON and exit 0."""

    def test_build_workflow_transcodes_to_valid_json(self):
        result = run_cli(str(REAL_ASSET))
        self.assertEqual(
            result.returncode, 0,
            msg=f"expected exit 0, got rc={result.returncode}\n{result.stderr}",
        )
        # stdout must be parseable JSON (json.loads does not raise).
        data = json.loads(result.stdout)
        # ...and carry the workflow's expected top-level keys.
        self.assertIsInstance(data, dict)
        for key in ("id", "stage", "phases", "stop_condition"):
            self.assertIn(key, data, msg=f"missing top-level key '{key}'")
        self.assertEqual(data["id"], "build")

    def test_output_is_deterministic(self):
        first = run_cli(str(REAL_ASSET))
        second = run_cli(str(REAL_ASSET))
        self.assertEqual(first.stdout, second.stdout)


class MissingFileTest(unittest.TestCase):
    """A non-existent path must exit 1 with a stderr message."""

    def test_missing_file_exits_1(self):
        result = run_cli(str(REPO_ROOT / "does-not-exist.yml"))
        self.assertEqual(result.returncode, 1)
        self.assertIn("forge-yaml2json:", result.stderr)
        self.assertEqual(result.stdout, "")


class MissingArgTest(unittest.TestCase):
    """No path argument must exit 1 with usage on stderr."""

    def test_missing_arg_exits_1(self):
        result = run_cli()
        self.assertEqual(result.returncode, 1)
        self.assertIn("usage:", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
