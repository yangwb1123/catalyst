"""Codec, digest, ordering, limit, and golden tamper tests for ADR-0053."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

from governance_contract import ContractError  # noqa: E402
from go_package_dependency_graph_observation_producer import (  # noqa: E402
    canonical_json, decode_production, validate_golden_fixture, validate_production,
)
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    FILE_FIELDS, FIXTURE_PATH, MAX_EDGES, MAX_GO_FILE_BYTES, MAX_GO_FILES,
    MAX_IMPORTS_PER_FILE,
)
from go_package_dependency_graph_observation_producer.profiles import (  # noqa: E402
    validate_file,
)
from go_package_dependency_graph_observation_producer.semantics import (  # noqa: E402
    _validate_dependencies, _validate_files,
)
from go_package_dependency_graph_observation_producer.test_adversarial import (  # noqa: E402
    synthetic_production,
)


class GoDependencyGraphStrictnessTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.wrapper = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        cls.golden = cls.wrapper["production"]

    def candidate(self) -> dict[str, object]:
        return copy.deepcopy(self.golden)

    def assert_decode_rejected(self, raw: bytes) -> None:
        with self.assertRaises(ContractError):
            decode_production(raw)

    def test_duplicate_key_float_overflow_and_forbidden_unicode_rejected(self):
        raw = canonical_json(self.golden)
        self.assert_decode_rejected(raw.replace(
            b"{", b'{"api_version":"duplicate",', 1))
        timestamp = b'"observed_at_unix_ms":1786320000123'
        self.assert_decode_rejected(raw.replace(
            timestamp, b'"observed_at_unix_ms":1.5'))
        self.assert_decode_rejected(raw.replace(
            timestamp, b'"observed_at_unix_ms":9223372036854775808'))
        self.assert_decode_rejected(raw.replace(
            b"fixture-go-deps-001", "fixture\u2028run".encode("utf-8")))

    def test_noncanonical_and_unknown_production_are_rejected(self):
        pretty = json.dumps(self.golden, indent=2).encode("utf-8")
        self.assert_decode_rejected(pretty)
        value = self.candidate()
        value["graph_observation"]["unknown"] = None
        issues = validate_production(value)
        self.assertTrue(any("unknown fields" in item for item in issues), issues)

    def test_parameter_source_graph_and_production_digest_drift(self):
        parameter = self.candidate()
        parameter["graph_observation"]["producer"]["parameters_sha256"] = "a" * 64
        self.assertTrue(any("producer binding" in item
                            for item in validate_production(parameter)))
        source = self.candidate()
        source["graph_observation"]["source"]["source_tree_sha256"] = "a" * 64
        self.assertTrue(any("tree digest" in item
                            for item in validate_production(source)))
        for field in ("graph_sha256", "production_sha256"):
            with self.subTest(field=field):
                value = copy.deepcopy(self.wrapper)
                value["expected"][field] = "a" * 64
                issues = self.fixture_issues(value)
                self.assertTrue(any(field in item for item in issues), issues)

    def test_files_and_imports_require_sorted_unique_order(self):
        for name, mutate in (
            ("unsorted files", lambda graph: graph["files"].reverse()),
            ("duplicate files", lambda graph: graph["files"].append(
                copy.deepcopy(graph["files"][0]))),
            ("unsorted imports", lambda graph: graph["files"][0]["imports"].reverse()),
            ("duplicate imports", lambda graph: graph["files"][0]["imports"].append(
                graph["files"][0]["imports"][0])),
        ):
            with self.subTest(name=name):
                value = self.candidate()
                mutate(value["graph_observation"])
                self.assertTrue(validate_production(value))

    def test_packages_edges_and_diagnostics_require_sorted_unique_order(self):
        for field in ("packages", "dependencies"):
            for operation in ("reverse", "duplicate"):
                with self.subTest(field=field, operation=operation):
                    value = self.candidate()
                    items = value["graph_observation"][field]
                    items.reverse() if operation == "reverse" else items.append(
                        copy.deepcopy(items[0]))
                    self.assertTrue(validate_production(value))
        value = synthetic_production(2, 0, 0)
        value["graph_observation"]["diagnostics"].reverse()
        issues = validate_production(value)
        self.assertTrue(any("sorted and unique" in item for item in issues), issues)

    def test_source_go_mod_and_nested_module_binding_drift(self):
        mutations = (
            ("source revision", lambda graph: graph["source"].update(
                source_revision="git-sha1:" + "4" * 40)),
            ("go.mod bytes", lambda graph: graph["module"].update(
                go_mod_bytes=graph["module"]["go_mod_bytes"] + 1)),
            ("go.mod hash", lambda graph: graph["module"].update(
                go_mod_content_sha256="a" * 64)),
            ("nested kind", lambda graph: graph["module"]["nested_modules"][0].update(
                kind="regular")),
        )
        for name, mutate in mutations:
            with self.subTest(name=name):
                value = self.candidate()
                mutate(value["graph_observation"])
                self.assertTrue(validate_production(value))

    def test_fixed_profiles_run_id_and_timestamp_fail_closed(self):
        mutations = (
            lambda value: value["parameters_manifest"].update(parser_profile_id="other"),
            lambda value: value["source_manifest"].update(profile_id="other"),
            lambda value: value["graph_observation"].update(profile_id="other"),
            lambda value: value["graph_observation"]["producer"].update(run_id="UPPER"),
            lambda value: value["graph_observation"].update(observed_at_unix_ms=-1),
            lambda value: value["graph_observation"].update(observed_at_unix_ms=True),
        )
        for mutate in mutations:
            value = self.candidate()
            mutate(value)
            self.assertTrue(validate_production(value))

    def test_exact_file_import_and_edge_limits_are_enforced(self):
        source = {"service/x.go": {
            "bytes": MAX_GO_FILE_BYTES + 1, "content_sha256": "a" * 64,
            "kind": "regular",
        }}
        file_value = {field: None for field in FILE_FIELDS}
        file_value.update({
            "bytes": MAX_GO_FILE_BYTES + 1, "content_sha256": "a" * 64,
            "imports": [], "package_name": "p", "path": "service/x.go",
            "role": "compile",
        })
        self.assertTrue(validate_file(file_value, 0, source))
        file_value["bytes"] = 0
        source["service/x.go"]["bytes"] = 0
        file_value["imports"] = [f"x/{index:04d}" for index in
                                 range(MAX_IMPORTS_PER_FILE + 1)]
        self.assertTrue(validate_file(file_value, 0, source))
        issues: list[str] = []
        _validate_dependencies({"dependencies": [{}] * (MAX_EDGES + 1)}, issues)
        self.assertTrue(any("invalid list or limit" in item for item in issues), issues)
        file_issues = _validate_files(
            {"files": [{}] * (MAX_GO_FILES + 1)}, {"entries": []})
        self.assertTrue(any("invalid list or limit" in item for item in file_issues))

    def test_zero_file_graph_is_valid(self):
        self.assertEqual(validate_production(synthetic_production(0, 0, 0)), [])

    def test_fixture_expected_and_preimage_tamper_are_rejected(self):
        expected = copy.deepcopy(self.wrapper)
        expected["expected"]["parameters_sha256"] = "a" * 64
        self.assertTrue(self.fixture_issues(expected))
        preimage = copy.deepcopy(self.wrapper)
        preimage["preimages"]["source_regular_files"][0]["utf8"] += "tamper"
        issues = self.fixture_issues(preimage)
        self.assertTrue(any("preimages" in item for item in issues), issues)

    @staticmethod
    def fixture_issues(value: dict[str, object]) -> list[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / FIXTURE_PATH
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return validate_golden_fixture(root)


if __name__ == "__main__":
    unittest.main()
