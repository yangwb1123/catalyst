"""Adversarial semantic tests for the ADR-0053 universal checker."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

from go_package_dependency_graph_observation_producer import (  # noqa: E402
    validate_production,
)
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    CANONICALIZATION, FILE_SELECTION_PROFILE, FIXTURE_PATH, GRAPH_API,
    GRAPH_PROFILE, IMPORT_RESOLUTION_PROFILE, MAX_GO_FILE_BYTES, MODULE_PROFILE,
    PARAMETERS_API, PARSER_PROFILE, PRODUCER_ID, PRODUCER_VERSION,
    PRODUCTION_API, SOURCE_API, SOURCE_PROFILE,
)
from go_package_dependency_graph_observation_producer.profiles import (  # noqa: E402
    canonical_import_path,
)
from go_package_dependency_graph_observation_producer.semantics import (  # noqa: E402
    expected_coverage, expected_dependencies, expected_packages,
    parameters_digest, selected_source_entries, source_digest,
)

EMPTY_SHA = hashlib.sha256(b"").hexdigest()
CANONICAL_IMPORT_PATHS = (
    "fmt", "crypto/x509", "example.com/m", "a/b-c_d~e", "a/c++",
)
NONCANONICAL_IMPORT_PATHS = (
    "", "/a", "a/", "a//b", "-flag", "./relative", "../up", "a/.",
    "a/..", "a/.hidden", "a/b.", "a\\b", "a!b", 'a"b', "a:b", "a@b",
    "@response",
    "a/CON", "a/com1.txt", "a/x~1", "例子.com/x", "a..b", "a b", "a\tb",
)


def _source_entry(path: str, size: int) -> dict[str, object]:
    return {
        "bytes": size, "content_sha256": EMPTY_SHA, "executable": False,
        "index_mode": None, "kind": "regular", "path": path,
        "symlink_target": None, "tracking": "untracked",
    }


def _parameters() -> dict[str, object]:
    return {
        "api_version": PARAMETERS_API, "canonicalization": CANONICALIZATION,
        "file_selection_profile_id": FILE_SELECTION_PROFILE,
        "import_resolution_profile_id": IMPORT_RESOLUTION_PROFILE,
        "module_directory": "service", "module_profile_id": MODULE_PROFILE,
        "parser_profile_id": PARSER_PROFILE, "source_profile_id": SOURCE_PROFILE,
    }


def _rederive(production: dict[str, object]) -> None:
    graph = production["graph_observation"]
    graph["source"]["source_tree_sha256"] = source_digest(production["source_manifest"])
    graph["coverage"] = expected_coverage(production)
    graph["packages"] = expected_packages(graph)
    graph["dependencies"] = expected_dependencies(graph)


def _synthetic_graph(parameters: dict[str, object], source: dict[str, object],
                     go_paths: list[str], size: int,
                     successful: int) -> dict[str, object]:
    go_mod = b"module example.com/service\n"
    files = [{
        "bytes": size, "content_sha256": EMPTY_SHA, "imports": [],
        "package_name": "p", "path": path, "role": "compile",
    } for path in go_paths[:successful]]
    diagnostics = [{"code": "go_file_parse_error", "path": path}
                   for path in go_paths[successful:]]
    return {
        "api_version": GRAPH_API, "canonicalization": CANONICALIZATION,
        "coverage": {}, "dependencies": [], "diagnostics": diagnostics,
        "files": files,
        "module": {
            "directory": "service", "go_mod_bytes": len(go_mod),
            "go_mod_content_sha256": hashlib.sha256(go_mod).hexdigest(),
            "go_mod_path": "service/go.mod", "module_path": "example.com/service",
            "nested_modules": [],
        },
        "observed_at_unix_ms": 1, "packages": [],
        "producer": {
            "parameters_sha256": parameters_digest(parameters),
            "producer_id": PRODUCER_ID, "producer_type": "tool",
            "producer_version": PRODUCER_VERSION, "run_id": "adversarial-run",
        },
        "profile_id": GRAPH_PROFILE,
        "source": {
            "source_revision": source["source_revision"],
            "source_tree_sha256": source_digest(source),
        },
    }


def synthetic_production(count: int, size: int,
                         successful: int) -> dict[str, object]:
    go_mod = b"module example.com/service\n"
    go_paths = [f"service/f{index:05d}.go" for index in range(count)]
    entries = [_source_entry(path, size) for path in go_paths]
    entries.append({
        "bytes": len(go_mod), "content_sha256": hashlib.sha256(go_mod).hexdigest(),
        "executable": False, "index_mode": None, "kind": "regular",
        "path": "service/go.mod", "symlink_target": None, "tracking": "untracked",
    })
    source = {
        "api_version": SOURCE_API, "canonicalization": CANONICALIZATION,
        "entries": sorted(entries, key=lambda item: item["path"]),
        "profile_id": SOURCE_PROFILE, "source_revision": "git-sha1:" + "3" * 40,
    }
    parameters = _parameters()
    graph = _synthetic_graph(parameters, source, go_paths, size, successful)
    production = {
        "api_version": PRODUCTION_API, "canonicalization": CANONICALIZATION,
        "graph_observation": graph, "parameters_manifest": parameters,
        "source_manifest": source,
    }
    _rederive(production)
    return production


class GoDependencyGraphAdversarialTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        wrapper = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        cls.golden = wrapper["production"]

    def candidate(self) -> dict[str, object]:
        return copy.deepcopy(self.golden)

    def test_canonical_import_path_matches_frozen_matrix(self):
        for value in CANONICAL_IMPORT_PATHS:
            with self.subTest(value=value):
                self.assertTrue(canonical_import_path(value))
        for value in NONCANONICAL_IMPORT_PATHS:
            with self.subTest(value=value):
                self.assertFalse(canonical_import_path(value))

    def test_noncanonical_module_paths_are_rejected(self):
        for module_path in NONCANONICAL_IMPORT_PATHS:
            with self.subTest(module_path=module_path):
                value = self.candidate()
                value["graph_observation"]["module"]["module_path"] = module_path
                issues = validate_production(value)
                self.assertTrue(issues)

    def test_noncanonical_import_requires_unsupported_semantics(self):
        value = synthetic_production(1, 0, 1)
        value["graph_observation"]["files"][0]["imports"] = ["a..b"]
        _rederive(value)
        dependency = value["graph_observation"]["dependencies"][0]
        self.assertEqual((dependency["resolution"], dependency["resolution_detail"]),
                         ("unsupported", "noncanonical_import_path"))
        self.assertEqual(validate_production(value), [])
        dependency["resolution"] = "stdlib_candidate"
        dependency["resolution_detail"] = None
        issues = validate_production(value)
        self.assertTrue(any("lexical edge derivation drifted" in item
                            for item in issues), issues)

    def test_source_derived_symlink_boundary_cannot_be_omitted(self):
        value = self.candidate()
        value["graph_observation"]["module"]["nested_modules"].pop(0)
        issues = validate_production(value)
        self.assertTrue(any("source-derived boundary" in item for item in issues), issues)

    def test_test_only_package_cannot_claim_import_path(self):
        value = self.candidate()
        package = next(item for item in value["graph_observation"]["packages"]
                       if item["name"] == "p_test")
        package["import_path"] = "example.com/service/internal/p"
        issues = validate_production(value)
        self.assertTrue(any("package grouping drifted" in item for item in issues), issues)

    def test_deleted_nested_go_mod_does_not_create_boundary(self):
        value = self.candidate()
        source = value["source_manifest"]
        source["entries"].extend([
            {
                "bytes": 0, "content_sha256": None, "executable": None,
                "index_mode": "100644", "kind": "deleted",
                "path": "service/retired/go.mod", "symlink_target": None,
                "tracking": "tracked",
            },
            _source_entry("service/retired/legacy.go", 0),
        ])
        source["entries"].sort(key=lambda item: item["path"])
        value["graph_observation"]["files"].append({
            "bytes": 0, "content_sha256": EMPTY_SHA, "imports": [],
            "package_name": "retired", "path": "service/retired/legacy.go",
            "role": "compile",
        })
        value["graph_observation"]["files"].sort(key=lambda item: item["path"])
        _rederive(value)
        self.assertEqual(validate_production(value), [])
        directories = value["graph_observation"]["module"]["nested_modules"]
        self.assertNotIn("service/retired", {item["directory"] for item in directories})

    def test_one_failed_file_cannot_publish_two_diagnostics(self):
        value = self.candidate()
        graph = value["graph_observation"]
        graph["diagnostics"].append({
            "code": "go_file_invalid_utf8",
            "path": "service/internal/broken/bad.go",
        })
        graph["diagnostics"].sort(key=lambda item: (item["path"], item["code"]))
        issues = validate_production(value)
        self.assertTrue(any("each failed path" in item for item in issues), issues)

    def test_selected_regular_count_is_limited_not_only_success_files(self):
        value = synthetic_production(16_385, 0, 1)
        issues = validate_production(value)
        self.assertTrue(any("selected regular Go file count" in item for item in issues), issues)

    def test_parser_budget_counts_diagnostic_files_from_source(self):
        value = synthetic_production(17, MAX_GO_FILE_BYTES, 0)
        issues = validate_production(value)
        self.assertTrue(any("aggregate selected parser input" in item for item in issues), issues)

    def test_oversize_diagnostic_is_size_derived(self):
        value = self.candidate()
        value["graph_observation"]["diagnostics"][0]["code"] = (
            "go_file_exceeds_parser_limit")
        issues = validate_production(value)
        self.assertTrue(any("size-derived diagnostic" in item for item in issues), issues)

    def test_files_and_diagnostics_must_partition_selected_regular_files(self):
        value = self.candidate()
        value["graph_observation"]["diagnostics"][0]["path"] = (
            "service/internal/q/q.go")
        issues = validate_production(value)
        self.assertTrue(any("do not partition" in item for item in issues), issues)

    def test_derived_edges_and_coverage_cannot_drift(self):
        edge_value = self.candidate()
        edge_value["graph_observation"]["dependencies"][0]["resolution"] = "local"
        self.assertTrue(any("edge derivation drifted" in item
                            for item in validate_production(edge_value)))
        coverage_value = self.candidate()
        coverage_value["graph_observation"]["coverage"][
            "regular_go_files_selected"] += 1
        self.assertTrue(any("selection accounting drifted" in item
                            for item in validate_production(coverage_value)))

    def test_unknown_fields_and_boolean_counts_fail_closed(self):
        unknown = self.candidate()
        unknown["graph_observation"]["module"]["parser_message"] = "invented"
        self.assertTrue(validate_production(unknown))
        boolean = self.candidate()
        boolean["graph_observation"]["coverage"]["regular_go_files_parsed"] = True
        issues = validate_production(boolean)
        self.assertTrue(any("signed-int64" in item for item in issues), issues)

    def test_package_name_uses_bounded_go_identifier_profile(self):
        for package_name in ("for", "²", "π", "\U000105c0", "p" * 4_097):
            with self.subTest(package_name=package_name[:16]):
                value = self.candidate()
                value["graph_observation"]["files"][0]["package_name"] = package_name
                _rederive(value)
                self.assertTrue(validate_production(value))

    def test_derived_package_import_path_is_bounded(self):
        value = synthetic_production(1, 0, 1)
        module = value["graph_observation"]["module"]
        module["module_path"] = "m" * 4_090
        value["graph_observation"]["files"][0]["path"] = (
            "service/descendant/package/file.go")
        value["source_manifest"]["entries"][0]["path"] = (
            "service/descendant/package/file.go")
        value["source_manifest"]["entries"].sort(key=lambda item: item["path"])
        _rederive(value)
        issues = validate_production(value)
        self.assertTrue(any("4096 Unicode scalars" in item for item in issues), issues)

    def test_drive_prefixed_import_is_unsupported(self):
        value = synthetic_production(1, 0, 1)
        value["parameters_manifest"]["module_directory"] = "."
        module = value["graph_observation"]["module"]
        module["directory"] = "."
        module["go_mod_path"] = "go.mod"
        for entry in value["source_manifest"]["entries"]:
            entry["path"] = entry["path"].removeprefix("service/")
        for item in value["graph_observation"]["files"]:
            item["path"] = item["path"].removeprefix("service/")
        value["graph_observation"]["producer"]["parameters_sha256"] = (
            parameters_digest(value["parameters_manifest"]))
        value["graph_observation"]["files"][0]["imports"] = ["example.com/service/C:/x"]
        _rederive(value)
        dependency = value["graph_observation"]["dependencies"][0]
        self.assertEqual((dependency["resolution"], dependency["resolution_detail"],
                          dependency["target_directory"]),
                         ("unsupported", "noncanonical_import_path", None))
        self.assertEqual(validate_production(value), [])

    def test_high_cardinality_nested_selection_uses_exact_ancestors(self):
        nested = [{"directory": f"nested{index:04d}"}
                  for index in range(1_024)]
        entries = []
        for index in range(1_024):
            entries.extend([
                {"kind": "regular", "path": f"nested{index:04d}/x.go"},
                {"kind": "regular", "path": f"top{index:04d}.go"},
            ])
        production = {
            "graph_observation": {"diagnostics": [], "files": [],
                                  "module": {"nested_modules": nested}},
            "parameters_manifest": {"module_directory": "."},
            "source_manifest": {"entries": entries},
        }
        universe, excluded, nonregular, selected = selected_source_entries(production)
        self.assertEqual((len(universe), len(excluded), len(nonregular), len(selected)),
                         (2_048, 1_024, 0, 1_024))

    def test_nested_boundary_is_compared_to_go_file_directory(self):
        production = {
            "graph_observation": {
                "diagnostics": [], "files": [],
                "module": {
                    "nested_modules": [{"directory": "service/f.go"}],
                },
            },
            "parameters_manifest": {"module_directory": "service"},
            "source_manifest": {
                "entries": [
                    {"kind": "regular", "path": "service/f.go"},
                    {"kind": "regular", "path": "service/f.go/go.mod"},
                ],
            },
        }

        universe, excluded, nonregular, selected = selected_source_entries(production)

        self.assertEqual([entry["path"] for entry in universe], ["service/f.go"])
        self.assertEqual(excluded, [])
        self.assertEqual(nonregular, [])
        self.assertEqual([entry["path"] for entry in selected], ["service/f.go"])


if __name__ == "__main__":
    unittest.main()
