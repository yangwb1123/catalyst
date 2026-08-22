"""Focused adversarial tests for the pure ADR-0062 Python checker."""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

from go_package_dependency_graph_observation_producer import (  # noqa: E402
    canonical_json as graph_json,
)
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    GRAPH_DOMAIN,
)
from go_package_dependency_graph_observation_producer.semantics import (  # noqa: E402
    expected_dependencies, expected_packages,
)
from local_go_package_impact_prescan_contract import (  # noqa: E402
    canonical_json, derive_envelope, validate_envelope_bytes,
    validate_golden_fixture,
)
from local_go_package_impact_prescan_contract.codec import self_digest  # noqa: E402
from local_go_package_impact_prescan_contract.constants import (  # noqa: E402
    ENVELOPE_DOMAIN, FIXTURE_PATH, MAX_ENVELOPE_BYTES, MAX_FIXTURE_BYTES,
    MAX_REPORT_BYTES, MAX_REQUEST_BYTES, REPORT_DOMAIN, REQUEST_DOMAIN,
    SYSTEM_UNKNOWN_REASONS,
)
import local_go_package_impact_prescan_contract_check as cli  # noqa: E402

RUN_ID = "impact-prescan-test-001"
MODULE_PATH = "example.com/acme/service"
REPO_ROOT = HARNESS.parent


def go_file(path: str, package: str, imports: list[str] | None = None):
    return {
        "bytes": 10, "content_sha256": hashlib.sha256(path.encode()).hexdigest(),
        "imports": [] if imports is None else imports, "package_name": package,
        "path": path, "role": "test" if path.endswith("_test.go") else "compile",
    }


def synthetic_graph(*, diagnostics=None, nested=None, cycle=False):
    imports = lambda suffix: f"{MODULE_PATH}/{suffix}"
    files = [
        go_file("service/base/base.go", "base", [imports("top")] if cycle else []),
        go_file("service/left/left.go", "left", [imports("base")]),
        go_file("service/right/right.go", "right", [imports("base")]),
        go_file("service/top/top.go", "top", [imports("left"), imports("right")]),
        go_file("service/top/top_test.go", "top"),
    ]
    diagnostics = [] if diagnostics is None else diagnostics
    nested = [] if nested is None else nested
    selected = len(files) + len(diagnostics)
    graph = {
        "api_version": "forgeos.go-package-dependency-graph-observation/v1",
        "canonicalization": "forgeos.canonical-json/v1",
        "coverage": {
            "go_entries_excluded_by_nested_module": 0,
            "go_entries_excluded_nonregular": 0,
            "go_entries_in_selected_subtree": selected,
            "regular_go_files_parsed": len(files),
            "regular_go_files_selected": selected,
            "regular_go_files_with_diagnostics": len(diagnostics),
        },
        "dependencies": [], "diagnostics": diagnostics, "files": files,
        "module": {
            "directory": "service", "go_mod_bytes": 24,
            "go_mod_content_sha256": "1" * 64, "go_mod_path": "service/go.mod",
            "module_path": MODULE_PATH, "nested_modules": nested,
        },
        "observed_at_unix_ms": 1_786_320_000_123, "packages": [],
        "producer": {
            "parameters_sha256": "2" * 64,
            "producer_id": "forgeos.local-go-package-dependency-graph-observer",
            "producer_type": "tool", "producer_version": "v1", "run_id": RUN_ID,
        },
        "profile_id": "selected-go-module-lexical-dependency-graph-v1",
        "source": {
            "source_revision": "git-sha1:" + "3" * 40,
            "source_tree_sha256": "4" * 64,
        },
    }
    graph["packages"] = expected_packages(graph)
    graph["dependencies"] = expected_dependencies(graph)
    return graph


def graph_bytes(**options) -> bytes:
    return graph_json(synthetic_graph(**options))


def envelope_bytes(paths=None, **options) -> bytes:
    changed = ["service/base/base.go"] if paths is None else paths
    return canonical_json(derive_envelope(graph_bytes(**options), changed, RUN_ID))


def reseal_report_and_envelope(value: dict[str, object]) -> None:
    report = value["report"]
    report["report_sha256"] = self_digest(
        REPORT_DOMAIN, report, "report_sha256", max_bytes=MAX_REPORT_BYTES)
    value["envelope_sha256"] = self_digest(
        ENVELOPE_DOMAIN, value, "envelope_sha256", max_bytes=MAX_ENVELOPE_BYTES)


def reseal_request(value: dict[str, object]) -> None:
    request = value["request"]
    request["request_sha256"] = self_digest(
        REQUEST_DOMAIN, request, "request_sha256", max_bytes=MAX_REQUEST_BYTES)


class LocalGoPackageImpactPrescanContractTest(unittest.TestCase):
    def valid(self, paths=None, **options):
        return derive_envelope(graph_bytes(**options),
                               ["service/base/base.go"] if paths is None else paths,
                               RUN_ID)

    def assert_rejected(self, value):
        issues = validate_envelope_bytes(canonical_json(value))
        self.assertTrue(issues, "tampered envelope unexpectedly passed")

    def test_diamond_reverse_closure_and_full_edge_tie_break(self):
        value = self.valid()
        self.assertEqual(validate_envelope_bytes(canonical_json(value)), [])
        report = value["report"]
        self.assertEqual(report["package_lexical_closure_status"],
                         "complete_within_observation")
        self.assertEqual(report["system_unknown_reason_codes"], SYSTEM_UNKNOWN_REASONS)
        self.assertEqual(len(report["reachable_nodes"]), 4)
        self.assertEqual(len(report["reachable_edges"]), 4)
        nodes = {item["directory"]: item for item in report["reachable_nodes"]}
        edges = {(item["from_node_id"], item["to_node_id"]): item
                 for item in report["reachable_edges"]}
        base, left, right, top = (nodes[f"service/{name}"] for name in
                                  ("base", "left", "right", "top"))
        choices = [
            ([edges[(left["node_id"], base["node_id"])]][0]["edge_id"],
             edges[(top["node_id"], left["node_id"])] ["edge_id"]),
            ([edges[(right["node_id"], base["node_id"])]][0]["edge_id"],
             edges[(top["node_id"], right["node_id"])] ["edge_id"]),
        ]
        self.assertEqual(tuple(top["witness"]["edge_ids"]), min(choices))

    def test_multi_seed_prefers_lexically_smallest_seed_id(self):
        value = self.valid(["service/left/left.go", "service/right/right.go"])
        report = value["report"]
        seeds = [item["node_id"] for item in report["resolved_seeds"]]
        top = next(item for item in report["reachable_nodes"]
                   if item["directory"] == "service/top")
        self.assertEqual(top["witness"]["hop_count"], 1)
        self.assertEqual(top["witness"]["seed_node_id"], min(seeds))
        self.assertEqual(validate_envelope_bytes(canonical_json(value)), [])

    def test_cycle_is_bounded_and_witnesses_are_simple(self):
        value = self.valid(cycle=True)
        self.assertEqual(validate_envelope_bytes(canonical_json(value)), [])
        for node in value["report"]["reachable_nodes"]:
            witness = node["witness"]
            self.assertEqual(len(witness["node_ids"]), len(set(witness["node_ids"])))
            self.assertEqual(len(witness["edge_ids"]), witness["hop_count"])

    def test_unresolved_priority_partition_and_unknown_status(self):
        diagnostics = [{"code": "go_file_parse_error", "path": "service/bad.go"}]
        nested = [{"directory": "service/nested", "go_mod_path":
                   "service/nested/go.mod", "kind": "regular"}]
        paths = ["other/x.go", "service/README", "service/bad.go",
                 "service/missing.go", "service/nested/file.go"]
        value = self.valid(paths, diagnostics=diagnostics, nested=nested)
        report = value["report"]
        self.assertEqual(report["resolved_seeds"], [])
        self.assertEqual(report["reachable_nodes"], [])
        self.assertEqual([item["reason"] for item in report["unresolved_seeds"]], [
            "outside_selected_module", "not_a_go_file", "go_file_diagnostic",
            "not_in_observed_file_or_diagnostic", "inside_nested_module_boundary",
        ])
        self.assertEqual(report["package_lexical_closure_status"], "unknown")
        self.assertEqual(report["closure_reason_codes"],
                         ["changed_path_unresolved", "go_file_diagnostic_present"])

    def test_noncanonical_envelope_and_graph_bytes_fail_closed(self):
        value = self.valid()
        pretty_envelope = json.dumps(value, indent=2).encode()
        self.assertTrue(validate_envelope_bytes(pretty_envelope))
        pretty_graph = json.dumps(synthetic_graph(), sort_keys=True).encode()
        request = value["request"]
        request["graph_observation_base64url"] = base64.urlsafe_b64encode(
            pretty_graph).rstrip(b"=").decode()
        request["graph_observation_sha256"] = hashlib.sha256(
            GRAPH_DOMAIN + pretty_graph).hexdigest()
        reseal_request(value)
        self.assert_rejected(value)

    def test_base64url_padding_and_request_digest_tamper_fail(self):
        value = self.valid()
        value["request"]["graph_observation_base64url"] += "="
        reseal_request(value)
        self.assert_rejected(value)
        value = self.valid()
        value["request"]["request_sha256"] = "f" * 64
        self.assert_rejected(value)

    def test_graph_package_and_dependency_derivations_are_rechecked(self):
        for field in ("packages", "dependencies"):
            with self.subTest(field=field):
                value = self.valid()
                graph = synthetic_graph()
                graph[field].reverse()
                raw = graph_json(graph)
                request = value["request"]
                request["graph_observation_base64url"] = base64.urlsafe_b64encode(
                    raw).rstrip(b"=").decode()
                request["graph_observation_sha256"] = hashlib.sha256(
                    GRAPH_DOMAIN + raw).hexdigest()
                reseal_request(value)
                self.assert_rejected(value)

    def test_every_derived_report_surface_rejects_full_rehash_tamper(self):
        mutations = [
            lambda report: report["resolved_seeds"][0]["changed_paths"].append(
                "service/top/top.go"),
            lambda report: report["reachable_nodes"][0].update(node_sha256="f" * 64),
            lambda report: report["reachable_edges"].pop(),
            lambda report: report["reachable_nodes"][-1]["witness"].update(hop_count=0),
            lambda report: report.update(package_lexical_closure_status="unknown"),
            lambda report: report.update(system_impact_status="complete"),
        ]
        for mutate in mutations:
            value = self.valid()
            mutate(value["report"])
            reseal_report_and_envelope(value)
            self.assert_rejected(value)

    def test_boolean_cannot_masquerade_as_integer_hop_count(self):
        for directory, replacement in (("service/base", False),
                                       ("service/left", True)):
            with self.subTest(directory=directory):
                value = self.valid()
                node = next(item for item in value["report"]["reachable_nodes"]
                            if item["directory"] == directory)
                node["witness"]["hop_count"] = replacement
                value["envelope_sha256"] = self_digest(
                    ENVELOPE_DOMAIN, value, "envelope_sha256",
                    max_bytes=MAX_ENVELOPE_BYTES)
                self.assert_rejected(value)

    def test_changed_path_sort_uniqueness_and_bound_fail_before_output(self):
        for paths in (["service/right/right.go", "service/left/left.go"],
                      ["service/base/base.go", "service/base/base.go"],
                      [f"service/p{index:03d}.go" for index in range(257)]):
            with self.subTest(count=len(paths)):
                with self.assertRaises(Exception):
                    derive_envelope(graph_bytes(), paths, RUN_ID)

    def test_graph_contained_source_entry_bound_fails_closed(self):
        graph = synthetic_graph()
        graph["coverage"]["go_entries_excluded_nonregular"] = 65_532
        graph["coverage"]["go_entries_in_selected_subtree"] = 65_537
        with self.assertRaisesRegex(Exception, "subtree entry count exceeds limit"):
            derive_envelope(graph_json(graph), ["service/base/base.go"], RUN_ID)

    def test_cli_checks_only_the_supplied_bounded_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "envelope.json"
            path.write_bytes(envelope_bytes())
            self.assertEqual(cli.main([str(path)]), 0)
            self.assertEqual(cli.main([]), 2)

    def test_golden_cli_validates_repository_wrapper(self):
        self.assertEqual(validate_golden_fixture(REPO_ROOT), [])
        self.assertEqual(cli.main(["--golden", str(REPO_ROOT)]), 0)
        self.assertEqual(cli.main(["--golden", str(REPO_ROOT / "absent")]), 2)

    def test_golden_wrapper_rejects_input_output_and_digest_drift(self):
        fixture = json.loads((REPO_ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        mutations = [
            lambda value: value.update(api_version="forgeos.invalid/v1"),
            lambda value: value["input"]["changed_paths"].reverse(),
            lambda value: value["input"].update(graph_observation_sha256="f" * 64),
            lambda value: value["input"].update(
                canonical_graph_observation_json=
                value["input"]["canonical_graph_observation_json"] + " "),
            lambda value: value["expected"].update(
                canonical_envelope_json=
                value["expected"]["canonical_envelope_json"] + " "),
            lambda value: value["expected"].update(envelope_sha256="f" * 64),
            lambda value: value["expected"].update(report_sha256="f" * 64),
            lambda value: value["expected"].update(request_sha256="f" * 64),
            lambda value: value.update(unexpected=True),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutations.index(mutate)):
                tampered = copy.deepcopy(fixture)
                mutate(tampered)
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    path = root / FIXTURE_PATH
                    path.parent.mkdir(parents=True)
                    path.write_bytes(canonical_json(
                        tampered, max_bytes=MAX_FIXTURE_BYTES) + b"\n")
                    self.assertTrue(validate_golden_fixture(root))


if __name__ == "__main__":
    unittest.main()
