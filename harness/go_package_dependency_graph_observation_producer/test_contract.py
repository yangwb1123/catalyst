"""Golden, derivation, and CLI tests for ADR-0053."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

from go_package_dependency_graph_observation_producer import (  # noqa: E402
    canonical_json, validate_golden_fixture, validate_production,
)
from go_package_dependency_graph_observation_producer import check  # noqa: E402
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    FIXTURE_PATH, RESULT,
)
from go_package_dependency_graph_observation_producer.fixture import (  # noqa: E402
    computed_expected,
)


class GoDependencyGraphContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.wrapper = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        cls.production = cls.wrapper["production"]
        cls.graph = cls.production["graph_observation"]

    def test_repository_fixture_is_valid_and_non_authoritative(self):
        self.assertEqual(validate_golden_fixture(ROOT), [])
        self.assertEqual(self.wrapper["expected"]["result"], RESULT)
        self.assertIn("PURE_CONTRACT_FIXTURE", self.wrapper["fixture_semantics"])
        self.assertIn("no live repository capture", self.wrapper["fixture_semantics"])

    def test_expected_canonical_values_and_digests_recompute(self):
        self.assertEqual(self.wrapper["expected"], computed_expected(self.production))
        self.assertEqual(validate_production(self.production), [])

    def test_fixture_covers_all_lexical_resolution_classes(self):
        resolutions = {item["resolution"] for item in self.graph["dependencies"]}
        self.assertEqual(resolutions, {
            "ambiguous_local", "cgo_pseudo", "external_candidate", "local",
            "nested_module_boundary", "stdlib_candidate", "unresolved_local",
            "unsupported",
        })

    def test_test_only_package_has_no_import_path(self):
        package = next(item for item in self.graph["packages"]
                       if item["name"] == "p_test")
        self.assertEqual(package["compile_files"], [])
        self.assertIsNone(package["import_path"])

    def test_build_tagged_files_join_all_regular_file_union(self):
        package = next(item for item in self.graph["packages"]
                       if item["name"] == "p")
        self.assertIn("service/internal/p/p_linux.go", package["compile_files"])
        edge = next(item for item in self.graph["dependencies"]
                    if item["from_package_name"] == "p" and
                    item["role"] == "compile" and
                    item["import_path"].endswith("/q"))
        self.assertEqual(edge["source_paths"], [
            "service/internal/p/p.go", "service/internal/p/p_linux.go",
        ])

    def test_nested_regular_and_symlink_go_mod_are_boundaries(self):
        self.assertEqual(self.graph["module"]["nested_modules"], [
            {
                "directory": "service/generated",
                "go_mod_path": "service/generated/go.mod",
                "kind": "symlink",
            },
            {
                "directory": "service/tools",
                "go_mod_path": "service/tools/go.mod",
                "kind": "regular",
            },
        ])
        coverage = self.graph["coverage"]
        self.assertEqual(coverage["go_entries_excluded_by_nested_module"], 2)
        self.assertEqual(coverage["go_entries_excluded_nonregular"], 2)
        self.assertEqual(coverage["regular_go_files_selected"], 11)

    def test_cli_supports_golden_module_and_direct_script_modes(self):
        self.assertEqual(check.main(["--golden", str(ROOT)]), 0)
        result = subprocess.run(
            [sys.executable, "-B", str(Path(__file__).with_name("check.py")),
             "--golden", str(ROOT)],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("VALID_LOCAL_GO_PACKAGE", result.stdout)

    def test_cli_strictly_accepts_only_canonical_production_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            canonical = Path(directory) / "production.json"
            canonical.write_bytes(canonical_json(self.production))
            self.assertEqual(check.main([str(canonical)]), 0)
            direct = subprocess.run(
                [sys.executable, "-B", str(Path(__file__).with_name("check.py")),
                 str(canonical)],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(direct.returncode, 0, direct.stderr)
            noncanonical = Path(directory) / "pretty.json"
            noncanonical.write_text(json.dumps(self.production, indent=2), encoding="utf-8")
            self.assertEqual(check.main([str(noncanonical)]), 1)


if __name__ == "__main__":
    unittest.main()
