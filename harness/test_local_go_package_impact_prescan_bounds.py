"""Boundary and negative CLI regressions for the ADR-0062 checker."""

from __future__ import annotations

import base64
import ast
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

from governance_contract import ContractError
from local_go_package_impact_prescan_contract import canonical_json, derive_envelope
from local_go_package_impact_prescan_contract.codec import (
    _walk, decode_base64url, decode_canonical,
)
from local_go_package_impact_prescan_contract.constants import (
    FIXTURE_PATH, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_ENVELOPE_BYTES, MAX_FIELDS,
    MAX_GRAPH_BASE64URL_BYTES, MAX_GRAPH_BYTES, MAX_I64, MAX_NODES,
    MAX_PATH_BYTES, MAX_PATH_SCALARS, MAX_REPORT_BYTES, MAX_REQUEST_BYTES,
    MAX_RUN_ID_BYTES, MIN_I64,
)
from local_go_package_impact_prescan_contract.graph import graph_nodes
from local_go_package_impact_prescan_contract.profiles import (
    validate_repo_path, validate_run_id,
)
import local_go_package_impact_prescan_contract_check as cli
from go_package_dependency_graph_observation_producer import canonical_json as graph_json
from go_package_dependency_graph_observation_producer.semantics import (
    expected_dependencies, expected_packages,
)


def _stdout_from_cli(arguments: list[str]) -> tuple[int, str, str]:
    stdout, stderr = io.StringIO(), io.StringIO()
    with redirect_stdout(stdout), redirect_stderr(stderr):
        code = cli.main(arguments)
    return code, stdout.getvalue(), stderr.getvalue()


def _package(index: int) -> dict[str, object]:
    directory = f"p{index:05d}"
    return {
        "compile_files": [f"{directory}/file.go"],
        "directory": directory,
        "import_path": f"example.com/bound/{directory}",
        "name": "p", "test_files": [],
    }


def _graph(imports: list[str], *, ambiguous: bool = False) -> dict[str, object]:
    files = [{
        "bytes": 1, "content_sha256": "1" * 64, "imports": imports,
        "package_name": "base", "path": "service/base.go", "role": "compile",
    }]
    if ambiguous:
        files += [{
            "bytes": 1, "content_sha256": "2" * 64, "imports": [],
            "package_name": name, "path": f"service/dupe/{name}.go", "role": "compile",
        } for name in ("a", "b")]
    graph = {
        "api_version": "forgeos.go-package-dependency-graph-observation/v1",
        "canonicalization": "forgeos.canonical-json/v1",
        "coverage": {
            "go_entries_excluded_by_nested_module": 0,
            "go_entries_excluded_nonregular": 0,
            "go_entries_in_selected_subtree": len(files),
            "regular_go_files_parsed": len(files),
            "regular_go_files_selected": len(files),
            "regular_go_files_with_diagnostics": 0,
        },
        "dependencies": [], "diagnostics": [], "files": files,
        "module": {"directory": "service", "go_mod_bytes": 1,
                   "go_mod_content_sha256": "3" * 64,
                   "go_mod_path": "service/go.mod",
                   "module_path": "example.com/service", "nested_modules": []},
        "observed_at_unix_ms": 0, "packages": [],
        "producer": {"parameters_sha256": "4" * 64,
                     "producer_id": "forgeos.local-go-package-dependency-graph-observer",
                     "producer_type": "tool", "producer_version": "v1",
                     "run_id": "impact-bounds-001"},
        "profile_id": "selected-go-module-lexical-dependency-graph-v1",
        "source": {"source_revision": "git-sha1:" + "5" * 40,
                   "source_tree_sha256": "6" * 64},
    }
    graph["packages"] = expected_packages(graph)
    graph["dependencies"] = expected_dependencies(graph)
    return graph


class LocalGoPackageImpactPrescanBoundsTest(unittest.TestCase):
    def test_python_evaluator_has_no_live_access_imports(self):
        forbidden = {"asyncio", "datetime", "git", "http", "os", "requests",
                     "socket", "sqlite3", "subprocess", "time", "urllib"}
        package = HARNESS / "local_go_package_impact_prescan_contract"
        for source in package.glob("*.py"):
            tree = ast.parse(source.read_text(encoding="utf-8"), filename=str(source))
            imported = set()
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    imported.update(alias.name.split(".")[0] for alias in node.names)
                elif isinstance(node, ast.ImportFrom) and node.module:
                    imported.add(node.module.split(".")[0])
            self.assertFalse(imported & forbidden,
                             f"{source.name} gained live-access imports")

    def test_ambiguous_and_non_gap_resolution_statuses(self):
        ambiguous = _graph(["example.com/service/dupe"], ambiguous=True)
        value = derive_envelope(
            graph_json(ambiguous), ["service/base.go"], "impact-bounds-001")
        self.assertEqual(value["report"]["closure_reason_codes"],
                         ["ambiguous_local_dependency_present"])
        for imports in (["example.org/external"], ["fmt"], ["C"],
                        ["C", "example.org/external", "fmt"]):
            with self.subTest(imports=imports):
                graph = _graph(imports)
                value = derive_envelope(
                    graph_json(graph), ["service/base.go"], "impact-bounds-001")
                self.assertEqual(value["report"]["package_lexical_closure_status"],
                                 "complete_within_observation")
                self.assertEqual(value["report"]["closure_reason_codes"], [])

    def test_base64url_encoded_and_decoded_byte_boundaries(self):
        # 16 MiB is divisible by three with remainder one, yielding the frozen
        # maximum unpadded encoding length exactly.
        raw = b"x" * MAX_GRAPH_BYTES
        encoded = base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")
        self.assertEqual(len(encoded), MAX_GRAPH_BASE64URL_BYTES)
        self.assertEqual(decode_base64url(encoded), raw)
        with self.assertRaises(ContractError):
            decode_base64url(encoded + "A")
        del raw, encoded
        over = b"x" * (MAX_GRAPH_BYTES + 1)
        encoded_over = base64.urlsafe_b64encode(over).rstrip(b"=").decode("ascii")
        with self.assertRaises(ContractError):
            decode_base64url(encoded_over)

    def test_json_depth_and_object_field_boundaries(self):
        at_depth: object = 0
        for _ in range(MAX_DEPTH - 1):
            at_depth = [at_depth]
        _walk(at_depth)
        with self.assertRaises(ContractError):
            _walk([at_depth])
        _walk({f"field_{index}": None for index in range(MAX_FIELDS)})
        with self.assertRaises(ContractError):
            _walk({f"field_{index}": None for index in range(MAX_FIELDS + 1)})

    def test_signed_int_and_array_item_boundaries(self):
        for value in (MIN_I64, MAX_I64):
            self.assertEqual(decode_canonical(
                str(value).encode(), max_bytes=32, label="integer"), value)
        for value in (MIN_I64 - 1, MAX_I64 + 1):
            with self.assertRaises(ContractError):
                decode_canonical(str(value).encode(), max_bytes=32,
                                 label="integer")
        _walk([None] * MAX_ARRAY_ITEMS)
        with self.assertRaises(ContractError):
            _walk([None] * (MAX_ARRAY_ITEMS + 1))

    def test_path_and_run_id_exact_boundaries(self):
        scalar_path = "a" * MAX_PATH_SCALARS
        byte_path = "😀" * MAX_PATH_SCALARS
        self.assertEqual(len(byte_path.encode("utf-8")), MAX_PATH_BYTES)
        self.assertEqual(validate_repo_path(scalar_path, "path"), scalar_path)
        self.assertEqual(validate_repo_path(byte_path, "path"), byte_path)
        for value in (scalar_path + "a", byte_path + "😀"):
            with self.assertRaises(ContractError):
                validate_repo_path(value, "path")
        run_id = "r" * MAX_RUN_ID_BYTES
        self.assertEqual(validate_run_id(run_id), run_id)
        with self.assertRaises(ContractError):
            validate_run_id(run_id + "r")

    def test_node_count_boundary_without_large_wire_allocation(self):
        graph = {"module": {"module_path": "example.com/bound"},
                 "packages": [_package(index) for index in range(MAX_NODES)]}
        nodes, by_package = graph_nodes(graph)
        self.assertEqual((len(nodes), len(by_package)), (MAX_NODES, MAX_NODES))
        graph["packages"].append(_package(MAX_NODES))
        with self.assertRaisesRegex(ContractError, "package node set"):
            graph_nodes(graph)

    def test_invalid_envelope_and_golden_cli_emit_zero_stdout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            envelope = root / "invalid.json"
            envelope.write_bytes(b'{"api_version":"invalid"}')
            code, stdout, stderr = _stdout_from_cli([str(envelope)])
            self.assertEqual((code, stdout), (1, ""))
            self.assertIn("ERROR:", stderr)
            golden = root / FIXTURE_PATH
            golden.parent.mkdir(parents=True)
            golden.write_bytes(b'{"api_version":"invalid"}\n')
            code, stdout, stderr = _stdout_from_cli(["--golden", str(root)])
            self.assertEqual((code, stdout), (1, ""))
            self.assertIn("ERROR:", stderr)

    def test_request_report_caps_are_exercised_without_giant_fixtures(self):
        # Exercise the shared final seal at each exact outer byte boundary;
        # synthesizing three huge semantic documents would only duplicate the
        # independently tested graph/count validators. The 48 MiB envelope
        # ceiling is also dominated by its 24 MiB request + 16 MiB report caps.
        for maximum in (MAX_REQUEST_BYTES, MAX_REPORT_BYTES,
                        MAX_ENVELOPE_BYTES):
            with self.subTest(maximum=maximum):
                with patch("local_go_package_impact_prescan_contract.codec.json.dumps",
                           return_value="x" * maximum):
                    encoded = canonical_json({}, max_bytes=maximum)
                self.assertEqual(len(encoded), maximum)
                with patch("local_go_package_impact_prescan_contract.codec.json.dumps",
                           return_value="x" * (maximum + 1)):
                    with self.assertRaises(ContractError):
                        canonical_json({}, max_bytes=maximum)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "envelope.json"
            path.touch()
            with patch("governance_contract.codec.os.fstat") as fstat, patch.object(
                    cli, "validate_envelope_bytes") as validator:
                fstat.return_value.st_size = MAX_ENVELOPE_BYTES + 1
                code, stdout, _ = _stdout_from_cli([str(path)])
            self.assertEqual((code, stdout), (1, ""))
            validator.assert_not_called()


if __name__ == "__main__":
    unittest.main()
