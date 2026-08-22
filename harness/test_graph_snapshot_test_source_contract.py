"""ADR-0066 Python projector/checker adversarial contract matrix."""

from __future__ import annotations

import builtins
import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

HARNESS = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS))

from go_package_dependency_graph_observation_producer import (  # noqa: E402
    canonical_json as graph_json,
)
from go_package_dependency_graph_observation_producer.graph_contract import (  # noqa: E402
    observation_digest, validate_graph_bytes,
)
from go_package_dependency_graph_observation_producer.semantics import (  # noqa: E402
    expected_dependencies, expected_packages,
)
from graph_snapshot_contract import (  # noqa: E402
    canonical_json, derive_envelope, derive_profile_envelope,
    derive_test_source_envelope, validate_envelope_bytes,
    validate_golden_fixture, validate_profile_envelope_bytes,
    validate_test_source_envelope_bytes, validate_test_source_golden_fixture,
)
from graph_snapshot_contract.codec import self_digest  # noqa: E402
from graph_snapshot_contract.constants import (  # noqa: E402
    EDGE_SET_DOMAIN, ENVELOPE_DOMAIN as LEGACY_ENVELOPE_DOMAIN,
    MAX_ENVELOPE_BYTES, NODE_IDENTITY_DOMAIN, PROFILE_ID, SNAPSHOT_API,
)
from graph_snapshot_contract.records import structured_set_digest  # noqa: E402
from graph_snapshot_contract.lexical_test_source_constants import (  # noqa: E402
    FIXTURE_PATH, MAX_AGGREGATE_LOCATORS, MAX_EDGE_UNION, MAX_FIXTURE_BYTES,
    MAX_NODES, TEST_SOURCE_PROFILE_ID,
)
from graph_snapshot_contract import (  # noqa: E402
    lexical_test_source_validation as test_validation,
)
import graph_snapshot_contract_check as cli  # noqa: E402

REPO_ROOT = HARNESS.parent
UPSTREAM_FIXTURE = REPO_ROOT / (
    "docs/contracts/fixtures/"
    "local-go-package-dependency-graph-observation-producer-v1.json")
PROJECT_ID = "fixture-catalyst-go"


def upstream_graph() -> dict[str, object]:
    wrapper = json.loads(UPSTREAM_FIXTURE.read_text(encoding="utf-8"))
    return wrapper["production"]["graph_observation"]


def graph_bytes(graph: dict[str, object] | None = None) -> bytes:
    return graph_json(upstream_graph() if graph is None else graph)


def rederive_graph(graph: dict[str, object]) -> dict[str, object]:
    graph["files"].sort(key=lambda item: item["path"].encode("utf-8"))
    graph["packages"] = expected_packages(graph)
    graph["dependencies"] = expected_dependencies(graph)
    coverage = graph["coverage"]
    parsed, diagnostics = len(graph["files"]), len(graph["diagnostics"])
    coverage["regular_go_files_parsed"] = parsed
    coverage["regular_go_files_with_diagnostics"] = diagnostics
    coverage["regular_go_files_selected"] = parsed + diagnostics
    coverage["go_entries_in_selected_subtree"] = (
        parsed + diagnostics + coverage["go_entries_excluded_by_nested_module"] +
        coverage["go_entries_excluded_nonregular"])
    return graph


def envelope(graph: dict[str, object] | None = None,
             project_id: str = PROJECT_ID) -> dict[str, object]:
    raw = graph_bytes(graph)
    checked = validate_graph_bytes(raw)
    return derive_test_source_envelope(
        raw, observation_digest(checked), checked["producer"]["run_id"],
        project_id)


def envelope_bytes(graph: dict[str, object] | None = None) -> bytes:
    return canonical_json(envelope(graph), max_bytes=MAX_ENVELOPE_BYTES)


def semantic_index(snapshot, node_types: set[str] | None = None):
    nodes = snapshot["nodes"]
    if node_types is not None:
        nodes = [item for item in nodes if item["node_type"] in node_types]
    return {(item["node_type"], tuple(item["qualified_name_components"])): item
            for item in nodes}


def edge_index(snapshot, include_test_contains: bool = True):
    result = {}
    node_types = {item["node_id"]: item["node_type"] for item in snapshot["nodes"]}
    for item in snapshot["edges"]:
        if (not include_test_contains and item["relation"] == "contains" and
                node_types[item["to_node_id"]] == "test"):
            continue
        key = (item["relation"], item["from_node_id"], item["to_node_id"],
               item["parallel_discriminator"])
        result[key] = item
    return result


class GraphSnapshotTestSourceContractTest(unittest.TestCase):
    def assert_rejected(self, value: dict[str, object]) -> None:
        self.assertTrue(validate_test_source_envelope_bytes(canonical_json(value)))

    def test_05_06_07_08_09_exact_test_bijection_identity_and_locators(self):
        snapshot = envelope()["snapshot"]
        tests = [item for item in snapshot["nodes"] if item["node_type"] == "test"]
        self.assertEqual(len(tests), 2)
        self.assertEqual(len(tests), sum(bool(item["test_files"])
                                        for item in upstream_graph()["packages"]))
        components = {tuple(item["qualified_name_components"]) for item in tests}
        self.assertIn(("example.com/service", "internal/p", "p"), components)
        self.assertIn(("example.com/service", "internal/p", "p_test"), components)
        self.assertEqual(len({item["node_id"] for item in tests}), 2)
        for item in tests:
            self.assertTrue(item["source_locators"])
            self.assertTrue(all(locator["role"] == "test"
                                for locator in item["source_locators"]))
            self.assertNotIn("service/internal/p/p.go",
                             {x["path"] for x in item["source_locators"]})

    def test_08_literal_root_identity_and_05_empty_test_set_behavior(self):
        graph = copy.deepcopy(upstream_graph())
        item = next(value for value in graph["files"]
                    if value["path"] == "service/internal/p/p_test.go")
        item["path"] = "service/root_test.go"
        root = envelope(rederive_graph(graph))["snapshot"]
        test = next(value for value in root["nodes"]
                    if value["node_type"] == "test" and
                    value["qualified_name_components"][-1] == "p")
        self.assertEqual(test["qualified_name_components"][1], ".")
        no_tests = copy.deepcopy(upstream_graph())
        no_tests["files"] = [x for x in no_tests["files"] if x["role"] != "test"]
        snapshot = envelope(rederive_graph(no_tests))["snapshot"]
        self.assertFalse(any(x["node_type"] == "test" for x in snapshot["nodes"]))
        test_surface = snapshot["coverage"]["surfaces"][-1]
        self.assertEqual((test_surface["status"], test_surface["node_count"]),
                         ("partial", 0))

    def test_10_diagnostic_test_suffix_never_guesses_a_package(self):
        graph = copy.deepcopy(upstream_graph())
        graph["diagnostics"][0]["path"] = "service/internal/broken/bad_test.go"
        snapshot = envelope(graph)["snapshot"]
        tests = [x for x in snapshot["nodes"] if x["node_type"] == "test"]
        self.assertEqual(len(tests), 2)
        unresolved = next(x for x in snapshot["unresolved_nodes"]
                          if x["kind"] == "go_file_diagnostic")
        self.assertEqual(unresolved["source_locators"][0]["role"], "test")
        go_surface, test_surface = snapshot["coverage"]["surfaces"][5::5]
        self.assertNotIn("go_file_diagnostic_present", go_surface["reason_codes"])
        self.assertIn("go_file_diagnostic_present", test_surface["reason_codes"])

    def test_11_12_13_exact_contains_and_no_verification_or_outcome_claim(self):
        snapshot = envelope()["snapshot"]
        types = {item["node_id"]: item["node_type"] for item in snapshot["nodes"]}
        test_contains = [item for item in snapshot["edges"]
                         if item["relation"] == "contains" and
                         types[item["to_node_id"]] == "test"]
        self.assertEqual(len(test_contains), 2)
        for edge in test_contains:
            self.assertEqual((types[edge["from_node_id"]], edge["category_axes"],
                              edge["source_role"], edge["import_discriminator"],
                              edge["parallel_discriminator"]),
                             ("module", ["structural"], None, None, "contains"))
        self.assertEqual({x["relation"] for x in snapshot["edges"]},
                         {"contains", "depends_on"})
        encoded = canonical_json(snapshot)
        for forbidden in (b"verified_by", b"observed_by", b"PASS", b"FAIL"):
            self.assertNotIn(forbidden, encoded)

    def test_14_15_16_legacy_topology_stable_but_records_profile_bound(self):
        graph = upstream_graph()
        raw = graph_bytes(graph)
        checked = validate_graph_bytes(raw)
        legacy = derive_envelope(
            raw, observation_digest(checked), checked["producer"]["run_id"],
            PROJECT_ID)["snapshot"]
        current = envelope(graph)["snapshot"]
        old_nodes = semantic_index(legacy)
        new_nodes = semantic_index(current, {"module", "package"})
        self.assertEqual(set(old_nodes), set(new_nodes))
        for key in old_nodes:
            self.assertEqual(old_nodes[key]["node_id"], new_nodes[key]["node_id"])
            self.assertNotEqual(old_nodes[key]["node_sha256"], new_nodes[key]["node_sha256"])
        old_edges, new_edges = edge_index(legacy), edge_index(current, False)
        self.assertEqual(set(old_edges), set(new_edges))
        for key in old_edges:
            self.assertEqual(old_edges[key]["edge_id"], new_edges[key]["edge_id"])
            self.assertNotEqual(old_edges[key]["edge_sha256"], new_edges[key]["edge_sha256"])
        self.assertEqual(legacy["sources"], current["sources"])
        self.assertEqual(legacy["source_set_sha256"], current["source_set_sha256"])
        self.assertEqual(legacy["adr_0062_node_crosswalk"],
                         current["adr_0062_node_crosswalk"])
        self.assertEqual(legacy["crosswalk_set_sha256"],
                         current["crosswalk_set_sha256"])
        self.assertNotEqual(legacy["extractors"], current["extractors"])
        for collection, id_field, sha_field in (
                ("unresolved_nodes", "unresolved_node_id", "unresolved_node_sha256"),
                ("unresolved_edges", "unresolved_edge_id", "unresolved_edge_sha256")):
            old = {item[id_field]: item[sha_field] for item in legacy[collection]}
            new = {item[id_field]: item[sha_field] for item in current[collection]}
            self.assertEqual(set(old), set(new))
            self.assertTrue(all(old[key] != new[key] for key in old))

    def test_18_19_unresolved_bijections_and_package_only_crosswalk(self):
        snapshot = envelope()["snapshot"]
        self.assertEqual((len(snapshot["unresolved_nodes"]),
                          len(snapshot["unresolved_edges"])), (3, 11))
        self.assertEqual({x["resolution"] for x in snapshot["unresolved_edges"]}, {
            "ambiguous_local", "cgo_pseudo", "external_candidate",
            "nested_module_boundary", "stdlib_candidate", "unresolved_local",
            "unsupported",
        })
        package_ids = {x["node_id"] for x in snapshot["nodes"]
                       if x["node_type"] == "package"}
        crosswalk_ids = {x["graph_node_id"]
                         for x in snapshot["adr_0062_node_crosswalk"]}
        self.assertEqual(crosswalk_ids, package_ids)
        altered = envelope()
        altered["snapshot"]["unresolved_edges"].pop()
        self.assert_rejected(altered)

    def test_20_disjoint_coverage_partition_and_role_specific_reasons(self):
        snapshot = envelope()["snapshot"]
        partial = {x["surface"]: x for x in snapshot["coverage"]["surfaces"]
                   if x["status"] == "partial"}
        go, test = partial["go_module_package_lexical"], partial["test_verification"]
        self.assertEqual((go["node_count"], test["node_count"]), (9, 2))
        self.assertEqual((go["edge_count"], test["edge_count"]), (10, 4))
        self.assertEqual(go["node_count"] + test["node_count"], len(snapshot["nodes"]))
        self.assertEqual(go["edge_count"] + test["edge_count"], len(snapshot["edges"]))
        self.assertIn("go_file_diagnostic_present", go["reason_codes"])
        self.assertNotIn("go_file_diagnostic_present", test["reason_codes"])
        self.assertIn("stdlib_candidate_dependency_present", test["reason_codes"])

    def test_21_freshness_and_system_knowledge_remain_unknown(self):
        snapshot = envelope()["snapshot"]
        self.assertEqual(snapshot["freshness"]["status"], "unknown")
        self.assertEqual(snapshot["system_knowledge_status"], "unknown")
        self.assertIn("test_execution_and_verification_outcomes_not_observed",
                      snapshot["system_unknown_reason_codes"])
        self.assertEqual(snapshot["result"].count("no selected-build"), 1)

    def test_01_explicit_transport_dispatch_and_no_profile_fallback(self):
        raw = graph_bytes()
        graph = validate_graph_bytes(raw)
        arguments = (raw, observation_digest(graph), graph["producer"]["run_id"],
                     PROJECT_ID)
        legacy = derive_profile_envelope(*arguments, PROFILE_ID)
        current = derive_profile_envelope(*arguments, TEST_SOURCE_PROFILE_ID)
        self.assertEqual(validate_envelope_bytes(canonical_json(legacy)), [])
        self.assertEqual(validate_test_source_envelope_bytes(canonical_json(current)), [])
        self.assertTrue(validate_test_source_envelope_bytes(canonical_json(legacy))[0]
                        .startswith("unsupported_profile:"))
        self.assertTrue(validate_envelope_bytes(canonical_json(current))[0]
                        .startswith("unsupported_profile:"))
        with self.assertRaisesRegex(Exception, "^unsupported_profile:"):
            derive_profile_envelope(*arguments, "nearby-profile-v1")
        self.assertTrue(validate_profile_envelope_bytes(b"{}", "nearby-profile-v1")[0]
                        .startswith("unsupported_profile:"))

    def test_01_future_discriminator_classifies_before_supported_shape(self):
        mutations = (
            lambda item: item.update(api_version="future-envelope/v2"),
            lambda item: item["request"].update(api_version="future-request/v2"),
            lambda item: item["request"].update(projector_profile_id="future-profile-v2"),
            lambda item: item["snapshot"].update(api_version="future-snapshot/v2"),
            lambda item: item["snapshot"].update(profile_id="future-profile-v2"),
        )
        for mutation in mutations:
            value = envelope()
            mutation(value)
            value["future_field"] = "future"
            value["snapshot"]["nodes"] = [0] * (MAX_NODES + 1)
            raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode()
            issues = validate_test_source_envelope_bytes(raw)
            self.assertTrue(issues and issues[0].startswith("unsupported_profile:"))
        current = envelope()
        current["snapshot"]["edges"] = [0] * 81_921
        issues = validate_envelope_bytes(canonical_json(current))
        self.assertTrue(issues and issues[0].startswith("unsupported_profile:"))

    def test_03_noncanonical_number_precedes_future_profile_classification(self):
        value = envelope()
        value["api_version"] = "future-envelope/v2"
        raw = canonical_json(value).replace(b'"edge_count":0', b'"edge_count":-0', 1)
        issues = validate_test_source_envelope_bytes(raw)
        self.assertTrue(issues)
        self.assertIn("not exact compact canonical JSON", issues[0])
        self.assertFalse(issues[0].startswith("unsupported_profile:"))

    def test_02_exact_base64_digest_run_and_graph_profile_binding(self):
        raw = graph_bytes()
        checked = validate_graph_bytes(raw)
        digest, run_id = observation_digest(checked), checked["producer"]["run_id"]
        for graph_sha, run in (("f" * 64, run_id), (digest, "wrong-run")):
            with self.assertRaises(Exception):
                derive_test_source_envelope(raw, graph_sha, run, PROJECT_ID)
        graph = upstream_graph()
        graph["api_version"] = "future-graph/v2"
        graph["future_field"] = "future"
        with self.assertRaisesRegex(Exception, "^unsupported_profile:"):
            derive_test_source_envelope(
                graph_json(graph), "f" * 64, run_id, "INVALID PROJECT")
        value = envelope()
        value["request"]["graph_observation_base64url"] += "="
        self.assert_rejected(value)

    def test_03_canonical_closed_json_unicode_number_and_shape_rejection(self):
        raw = envelope_bytes()
        duplicate = raw.replace(
            b'{"api_version":', b'{"api_version":"bad","api_version":', 1)
        self.assertTrue(validate_test_source_envelope_bytes(duplicate))
        self.assertTrue(validate_test_source_envelope_bytes(
            json.dumps(envelope(), indent=2).encode()))
        escaped = raw.replace(b"fixture-catalyst-go", b"fixture-catalyst-g\\u006f", 1)
        forbidden = raw.replace(
            b"fixture-catalyst-go", b"fixture-catalyst-\\u202e", 1)
        self.assertTrue(validate_test_source_envelope_bytes(escaped))
        self.assertTrue(validate_test_source_envelope_bytes(forbidden))
        self.assertTrue(validate_test_source_envelope_bytes(raw + b"\xff"))
        value = envelope()
        value["snapshot"]["future"] = True
        self.assert_rejected(value)
        for invalid_number in (True, 1.5):
            graph = upstream_graph()
            graph["observed_at_unix_ms"] = invalid_number
            with self.assertRaises(Exception):
                envelope(graph)

    def test_04_projection_has_no_ambient_observation(self):
        raw = graph_bytes()
        graph = validate_graph_bytes(raw)
        arguments = (raw, observation_digest(graph), graph["producer"]["run_id"],
                     PROJECT_ID)
        with mock.patch.object(builtins, "open", side_effect=AssertionError), \
                mock.patch.object(os, "getenv", side_effect=AssertionError), \
                mock.patch.object(subprocess, "run", side_effect=AssertionError), \
                mock.patch.object(time, "time", side_effect=AssertionError):
            first = derive_test_source_envelope(*arguments)
            second = derive_test_source_envelope(*arguments)
        self.assertEqual(canonical_json(first), canonical_json(second))

    def test_17_collision_and_dangling_endpoint_fail_closed(self):
        fixed = "a" * 64

        def collide_nodes(domain, value, **kwargs):
            from graph_snapshot_contract.codec import domain_digest as real_digest
            if domain == NODE_IDENTITY_DOMAIN:
                return fixed
            return real_digest(domain, value, **kwargs)

        with mock.patch("graph_snapshot_contract.records.domain_digest",
                        side_effect=collide_nodes):
            with self.assertRaisesRegex(Exception, "identity collision"):
                envelope()
        value = envelope()
        value["snapshot"]["edges"][0]["from_node_id"] = "graph-node-" + "f" * 64
        self.assert_rejected(value)

    def test_17_global_cross_kind_identity_collision_fails_closed(self):
        fixed = "b" * 64

        def collide_kinds(domain, value, **kwargs):
            from graph_snapshot_contract.codec import domain_digest as real_digest
            from graph_snapshot_contract.constants import (
                EXTRACTOR_IDENTITY_DOMAIN, SOURCE_IDENTITY_DOMAIN,
            )
            if domain in {SOURCE_IDENTITY_DOMAIN, EXTRACTOR_IDENTITY_DOMAIN}:
                return fixed
            return real_digest(domain, value, **kwargs)

        with mock.patch("graph_snapshot_contract.records.domain_digest",
                        side_effect=collide_kinds):
            with self.assertRaisesRegex(Exception, "global identity digest collision"):
                envelope()

    def test_22_dedicated_node_edge_and_aggregate_locator_bounds(self):
        self.assertEqual((MAX_NODES, MAX_EDGE_UNION, MAX_AGGREGATE_LOCATORS),
                         (32_769, 98_304, 132_097))
        probe = {"api_version": SNAPSHOT_API, "profile_id": TEST_SOURCE_PROFILE_ID,
                 "edges": [0] * MAX_EDGE_UNION, "nodes": [0] * MAX_NODES}
        canonical_json(probe)
        with self.assertRaisesRegex(Exception, "exceeds"):
            canonical_json({**probe, "edges": [0] * (MAX_EDGE_UNION + 1)})
        with self.assertRaisesRegex(Exception, "exceeds"):
            canonical_json({**probe, "nodes": [0] * (MAX_NODES + 1)})
        structured_set_digest(
            EDGE_SET_DOMAIN, [{}] * MAX_EDGE_UNION, max_items=MAX_EDGE_UNION)
        with self.assertRaisesRegex(Exception, "exceeds"):
            structured_set_digest(
                EDGE_SET_DOMAIN, [{}] * MAX_EDGE_UNION,
                max_items=MAX_EDGE_UNION - 1)
        with mock.patch(
                "graph_snapshot_contract.lexical_test_source_topology.MAX_TEST_NODES",
                1):
            with self.assertRaisesRegex(Exception, "test node limit"):
                envelope()
        current = envelope()["snapshot"]
        locator_count = sum(len(record["source_locators"])
                            for name in ("nodes", "edges", "unresolved_nodes",
                                         "unresolved_edges")
                            for record in current[name])
        with mock.patch(
                "graph_snapshot_contract.lexical_test_source_topology."
                "MAX_AGGREGATE_LOCATORS",
                locator_count - 1):
            with self.assertRaisesRegex(Exception, "aggregate source locator"):
                envelope()

    def test_22_hostile_envelope_aggregate_locator_precheck_boundary(self):
        def snapshot_with_locators(total: int):
            records = []
            while total:
                count = min(total, 16_384)
                records.append({"source_locators": [None] * count})
                total -= count
            return {"nodes": records, "edges": [], "unresolved_nodes": [],
                    "unresolved_edges": []}

        test_validation._locator_limits(
            snapshot_with_locators(MAX_AGGREGATE_LOCATORS))
        with self.assertRaisesRegex(Exception, "aggregate source locator"):
            test_validation._locator_limits(
                snapshot_with_locators(MAX_AGGREGATE_LOCATORS + 1))
        hostile = {"nodes": [{"source_locators": [None] * 16_385}],
                   "edges": [], "unresolved_nodes": [], "unresolved_edges": []}
        with self.assertRaisesRegex(Exception, "exceeds 16384"):
            test_validation._locator_limits(hostile)

    def test_fixture_checker_cli_and_legacy_golden_bytes(self):
        self.assertEqual(validate_golden_fixture(REPO_ROOT), [])
        self.assertEqual(validate_test_source_golden_fixture(REPO_ROOT), [])
        self.assertEqual(cli.main(["--golden", str(REPO_ROOT)]), 0)
        self.assertEqual(cli.main(["--test-source-golden", str(REPO_ROOT)]), 0)
        fixture = json.loads((REPO_ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "snapshot.json"
            target.write_text(fixture["expected"]["canonical_envelope_json"],
                              encoding="utf-8")
            self.assertEqual(cli.main(["--test-source", str(target)]), 0)
        legacy = json.loads((REPO_ROOT / "docs/contracts/fixtures/graph-snapshot-v1.json")
                            .read_text(encoding="utf-8"))
        self.assertEqual(hashlib.sha256(
            (REPO_ROOT / "docs/contracts/fixtures/graph-snapshot-v1.json")
            .read_bytes()).hexdigest(),
            "8ce8418e840c97ef28ed77dfd5112c4c4b7d7ae8d843b714674e102d6322b03e")
        stored = legacy["expected"]["canonical_envelope_json"].encode()
        self.assertEqual(validate_envelope_bytes(stored), [])
        parsed = json.loads(stored)
        self.assertEqual(parsed["envelope_sha256"],
                         self_digest(LEGACY_ENVELOPE_DOMAIN, parsed,
                                     "envelope_sha256", max_bytes=MAX_ENVELOPE_BYTES))

    def test_python_package_has_no_zero_test_runtime_module_names(self):
        package = HARNESS / "graph_snapshot_contract"
        self.assertEqual(sorted(path.name for path in package.glob("test_*.py")), [])

    def test_fixture_tamper_fails_closed(self):
        fixture = json.loads((REPO_ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        fixture["expected"]["snapshot_sha256"] = hashlib.sha256(b"tamper").hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / FIXTURE_PATH
            path.parent.mkdir(parents=True)
            path.write_bytes(canonical_json(fixture, max_bytes=MAX_FIXTURE_BYTES) + b"\n")
            self.assertTrue(validate_test_source_golden_fixture(root))


if __name__ == "__main__":
    unittest.main()
