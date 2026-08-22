"""Focused adversarial tests for the pure ADR-0065 Python projector/checker."""

from __future__ import annotations

import base64
import contextlib
import copy
import hashlib
import io
import json
import sys
import tempfile
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
    canonical_json, derive_envelope, validate_envelope_bytes,
    validate_golden_fixture,
)
from graph_snapshot_contract.codec import (  # noqa: E402
    decode_canonical, domain_digest as real_domain_digest, self_digest,
)
from graph_snapshot_contract.constants import (  # noqa: E402
    CROSSWALK_SET_DOMAIN, EDGE_SET_DOMAIN, ENVELOPE_DOMAIN, FIXTURE_PATH,
    MAX_ARRAY_ITEMS, MAX_EDGE_UNION, MAX_ENVELOPE_BYTES, MAX_FIXTURE_BYTES,
    MAX_GRAPH_BYTES, MAX_SNAPSHOT_BYTES, NODE_SET_DOMAIN, SNAPSHOT_API,
    SNAPSHOT_DOMAIN, SNAPSHOT_IDENTITY_DOMAIN,
    SOURCE_IDENTITY_DOMAIN, EXTRACTOR_IDENTITY_DOMAIN, SYSTEM_UNKNOWN_REASONS,
    UNRESOLVED_EDGE_SET_DOMAIN, UNRESOLVED_NODE_SET_DOMAIN,
)
from graph_snapshot_contract.records import structured_set_digest  # noqa: E402
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


def envelope(graph: dict[str, object] | None = None,
             project_id: str = PROJECT_ID) -> dict[str, object]:
    raw = graph_bytes(graph)
    checked = validate_graph_bytes(raw)
    return derive_envelope(
        raw, observation_digest(checked), checked["producer"]["run_id"], project_id)


def envelope_bytes(graph: dict[str, object] | None = None) -> bytes:
    return canonical_json(envelope(graph), max_bytes=MAX_ENVELOPE_BYTES)


def reseal_snapshot(value: dict[str, object]) -> None:
    snapshot = value["snapshot"]
    snapshot["snapshot_sha256"] = self_digest(
        SNAPSHOT_DOMAIN, snapshot, "snapshot_sha256", max_bytes=MAX_SNAPSHOT_BYTES)
    value["envelope_sha256"] = self_digest(
        ENVELOPE_DOMAIN, value, "envelope_sha256", max_bytes=MAX_ENVELOPE_BYTES)


def reseal_snapshot_identity(value: dict[str, object]) -> None:
    snapshot = value["snapshot"]
    fields = (
        "coverage_sha256", "crosswalk_set_sha256", "edge_set_sha256",
        "extractor_set_sha256", "node_set_sha256", "profile_id", "project_id",
        "request_sha256", "source_set_sha256", "unresolved_edge_set_sha256",
        "unresolved_node_set_sha256",
    )
    identity = {field: snapshot[field] for field in fields}
    digest = real_domain_digest(
        SNAPSHOT_IDENTITY_DOMAIN, identity, max_bytes=MAX_SNAPSHOT_BYTES)
    snapshot["snapshot_identity_sha256"] = digest
    snapshot["snapshot_id"] = "graph-snapshot-" + digest
    reseal_snapshot(value)


class GraphSnapshotContractTest(unittest.TestCase):
    def assert_rejected(self, value: dict[str, object]) -> None:
        self.assertTrue(validate_envelope_bytes(canonical_json(value)))

    def test_rich_fixture_projects_all_eight_resolutions_and_exact_sets(self):
        graph = upstream_graph()
        value = envelope(graph)
        self.assertEqual(validate_envelope_bytes(canonical_json(value)), [])
        snapshot = value["snapshot"]
        self.assertEqual({item["resolution"] for item in graph["dependencies"]}, {
            "ambiguous_local", "cgo_pseudo", "external_candidate", "local",
            "nested_module_boundary", "stdlib_candidate", "unresolved_local",
            "unsupported",
        })
        self.assertEqual((len(snapshot["nodes"]), len(snapshot["edges"]),
                          len(snapshot["unresolved_nodes"]),
                          len(snapshot["unresolved_edges"]),
                          len(snapshot["adr_0062_node_crosswalk"])),
                         (9, 12, 3, 11, 8))
        self.assertEqual(
            {item["resolution"] for item in snapshot["unresolved_edges"]},
            set({item["resolution"] for item in graph["dependencies"]}) - {"local"})
        self.assertEqual(snapshot["system_unknown_reason_codes"],
                         SYSTEM_UNKNOWN_REASONS)

    def test_taxonomy_locators_crosswalk_coverage_and_provenance_are_exact(self):
        snapshot = envelope()["snapshot"]
        self.assertNotEqual(snapshot["sources"][0]["source_id"],
                            snapshot["extractors"][0]["extractor_id"])
        relations = [item["relation"] for item in snapshot["edges"]]
        self.assertEqual(relations.count("contains"), 8)
        self.assertEqual(relations.count("depends_on"), 4)
        self.assertTrue(all(item["epistemic_status"] == "derived"
                            for item in snapshot["edges"] + snapshot["nodes"]))
        self.assertTrue(all(item["source_ids"] == [snapshot["sources"][0]["source_id"]]
                            for item in snapshot["nodes"] + snapshot["edges"]))
        self.assertEqual(len(snapshot["adr_0062_node_crosswalk"]),
                         len([item for item in snapshot["nodes"]
                              if item["node_type"] == "package"]))
        surfaces = snapshot["coverage"]["surfaces"]
        self.assertEqual([item["surface"] for item in surfaces],
                         sorted(item["surface"] for item in surfaces))
        self.assertEqual(sum(item["status"] == "partial" for item in surfaces), 1)

    def test_file_content_edit_and_in_package_rename_preserve_semantic_ids(self):
        before = envelope()
        graph = copy.deepcopy(upstream_graph())
        target = next(item for item in graph["files"]
                      if item["path"] == "service/internal/p/p_linux.go")
        target["content_sha256"] = hashlib.sha256(b"edited-content").hexdigest()
        target["bytes"] += 1
        graph["source"]["source_tree_sha256"] = "9" * 64
        graph["source"]["source_revision"] = "git-sha1:" + "8" * 40
        after_edit = envelope(graph)
        self.assertEqual(self._semantic_ids(before), self._semantic_ids(after_edit))
        self.assertNotEqual(before["snapshot"]["snapshot_sha256"],
                            after_edit["snapshot"]["snapshot_sha256"])
        target["path"] = "service/internal/p/p_portable.go"
        graph["files"].sort(key=lambda item: item["path"].encode("utf-8"))
        graph["packages"] = expected_packages(graph)
        graph["dependencies"] = expected_dependencies(graph)
        after_rename = envelope(graph)
        self.assertEqual(self._semantic_ids(before), self._semantic_ids(after_rename))

    @staticmethod
    def _semantic_ids(value: dict[str, object]):
        snapshot = value["snapshot"]
        nodes = {(item["node_type"], tuple(item["qualified_name_components"])):
                 item["node_id"] for item in snapshot["nodes"]}
        edges = {(item["relation"], item["from_node_id"], item["to_node_id"],
                  item["parallel_discriminator"]): item["edge_id"]
                 for item in snapshot["edges"]}
        return nodes, edges

    def test_project_namespace_changes_node_and_endpoint_identities(self):
        first, second = envelope(), envelope(project_id="fixture-catalyst-fork")
        self.assertNotEqual(
            {item["node_id"] for item in first["snapshot"]["nodes"]},
            {item["node_id"] for item in second["snapshot"]["nodes"]})
        self.assertNotEqual(first["snapshot"]["snapshot_id"],
                            second["snapshot"]["snapshot_id"])

    def test_semantic_tamper_fails_after_all_affected_digests_are_resealed(self):
        value = envelope()
        value["snapshot"]["system_knowledge_status"] = "complete"
        reseal_snapshot(value)
        self.assert_rejected(value)
        value = envelope()
        node = value["snapshot"]["nodes"][0]
        node["freshness_status"] = "fresh"
        node["node_sha256"] = self_digest(
            b"forgeos.governance.graph-snapshot-node.v1\0", node,
            "node_sha256", max_bytes=MAX_SNAPSHOT_BYTES)
        snapshot = value["snapshot"]
        snapshot["node_set_sha256"] = structured_set_digest(
            NODE_SET_DOMAIN, snapshot["nodes"])
        reseal_snapshot_identity(value)
        self.assert_rejected(value)

    def test_crosswalk_and_unresolved_bijection_tamper_fail_when_resealed(self):
        mutations = [
            ("adr_0062_node_crosswalk", CROSSWALK_SET_DOMAIN,
             "crosswalk_set_sha256"),
            ("unresolved_nodes", UNRESOLVED_NODE_SET_DOMAIN,
             "unresolved_node_set_sha256"),
            ("unresolved_edges", UNRESOLVED_EDGE_SET_DOMAIN,
             "unresolved_edge_set_sha256"),
            ("edges", EDGE_SET_DOMAIN, "edge_set_sha256"),
        ]
        for collection, domain, digest_field in mutations:
            with self.subTest(collection=collection):
                value = envelope()
                snapshot = value["snapshot"]
                snapshot[collection].pop()
                snapshot[digest_field] = structured_set_digest(
                    domain, snapshot[collection])
                reseal_snapshot_identity(value)
                self.assert_rejected(value)

    def test_global_cross_kind_and_snapshot_identity_digest_collisions_fail(self):
        fixed = "a" * 64

        def collide_kinds(domain, value, *, max_bytes):
            if domain in {SOURCE_IDENTITY_DOMAIN, EXTRACTOR_IDENTITY_DOMAIN}:
                return fixed
            return real_domain_digest(domain, value, max_bytes=max_bytes)

        with mock.patch("graph_snapshot_contract.records.domain_digest",
                        side_effect=collide_kinds):
            with self.assertRaisesRegex(Exception, "global identity digest collision"):
                envelope()
        valid = envelope()
        node_identity = valid["snapshot"]["nodes"][0]["node_identity_sha256"]

        def collide_snapshot(domain, value, *, max_bytes):
            if domain == SNAPSHOT_IDENTITY_DOMAIN:
                return node_identity
            return real_domain_digest(domain, value, max_bytes=max_bytes)

        with mock.patch("graph_snapshot_contract.snapshot.domain_digest",
                        side_effect=collide_snapshot):
            with self.assertRaisesRegex(Exception, "global identity digest collision"):
                envelope()
        node_identity = valid["snapshot"]["nodes"][0]["node_identity_sha256"]

        collided = [False]

        def collide_crosswalk(domain, value, *, max_bytes):
            if (domain == b"forgeos.governance.local-go-package-impact-prescan-node.v1\0" and
                    not collided[0]):
                collided[0] = True
                return node_identity
            return real_domain_digest(domain, value, max_bytes=max_bytes)

        with mock.patch("graph_snapshot_contract.topology.domain_digest",
                        side_effect=collide_crosswalk):
            with self.assertRaisesRegex(Exception, "global identity digest collision"):
                envelope()

    def test_canonical_duplicate_unicode_utf8_and_base64_fail_closed(self):
        raw = envelope_bytes()
        self.assertTrue(validate_envelope_bytes(json.dumps(envelope(), indent=2).encode()))
        duplicate = raw.replace(
            b'{"api_version":', b'{"api_version":"invalid","api_version":', 1)
        self.assertTrue(validate_envelope_bytes(duplicate))
        escaped = raw.replace(b"fixture-catalyst-go", b"fixture-catalyst-g\\u006f", 1)
        self.assertTrue(validate_envelope_bytes(escaped))
        forbidden = raw.replace(b"fixture-catalyst-go", b"fixture-catalyst-\\u202e", 1)
        self.assertTrue(validate_envelope_bytes(forbidden))
        self.assertTrue(validate_envelope_bytes(raw + b"\xff"))
        value = envelope()
        value["request"]["graph_observation_base64url"] += "="
        self.assert_rejected(value)

    def test_profile_run_digest_bool_int_and_resource_bounds_fail_before_output(self):
        raw = graph_bytes()
        checked = validate_graph_bytes(raw)
        with self.assertRaises(Exception):
            derive_envelope(raw, "f" * 64, checked["producer"]["run_id"], PROJECT_ID)
        with self.assertRaises(Exception):
            derive_envelope(raw, observation_digest(checked), "wrong-run", PROJECT_ID)
        with self.assertRaises(Exception):
            derive_envelope(raw, observation_digest(checked),
                            checked["producer"]["run_id"], "a" * 161)
        with self.assertRaises(Exception):
            derive_envelope(b"{}" + b" " * MAX_GRAPH_BYTES, "f" * 64,
                            "fixture-run", PROJECT_ID)
        graph = upstream_graph()
        graph["observed_at_unix_ms"] = True
        with self.assertRaises(Exception):
            envelope(graph)
        with mock.patch("graph_snapshot_contract.topology.MAX_PACKAGE_NODES", 7):
            with self.assertRaisesRegex(Exception, "package node limit"):
                envelope()

    def test_generic_and_edge_array_and_string_bounds_are_path_aware(self):
        generic = canonical_json({"edges": [0] * MAX_ARRAY_ITEMS,
                                  "nodes": [0] * MAX_ARRAY_ITEMS})
        self.assertEqual(len(decode_canonical(
            generic, max_bytes=MAX_ENVELOPE_BYTES, label="probe")["edges"]),
                         MAX_ARRAY_ITEMS)
        with self.assertRaisesRegex(Exception, "exceeds"):
            canonical_json({"edges": [0] * (MAX_ARRAY_ITEMS + 1)})
        snapshot = {"api_version": SNAPSHOT_API,
                    "edges": [0] * MAX_EDGE_UNION}
        self.assertEqual(len(decode_canonical(
            canonical_json(snapshot), max_bytes=MAX_ENVELOPE_BYTES,
            label="probe")["edges"]), MAX_EDGE_UNION)
        with self.assertRaisesRegex(Exception, "exceeds"):
            canonical_json({"api_version": SNAPSHOT_API,
                            "edges": [0] * (MAX_EDGE_UNION + 1)})
        with self.assertRaisesRegex(Exception, "exceeds"):
            canonical_json({"snapshot": {"nodes": [0] * 16_386}})
        canonical_json({"graph_observation_base64url": "a" * 16_385})
        canonical_json({"expected": {"canonical_envelope_json": "a" * 16_385}})
        canonical_json({"input": {"canonical_graph_observation_json": "a" * 16_385}})
        with self.assertRaisesRegex(Exception, "string.*exceeds"):
            canonical_json({"other": "a" * 16_385})
        with self.assertRaisesRegex(Exception, "string.*exceeds"):
            canonical_json({"canonical_envelope_json": "a" * 16_385})
        with self.assertRaisesRegex(Exception, "string.*exceeds"):
            canonical_json({"evil": {
                "canonical_graph_observation_json": "a" * 16_385}})
        with self.assertRaisesRegex(Exception, "string.*exceeds"):
            canonical_json({"project_id": "a" * 161})

    def test_unknown_versions_and_profiles_classify_before_future_shape_decode(self):
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
            value["snapshot"]["nodes"] = [0] * 16_386
            raw = json.dumps(value, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode("utf-8")
            issues = validate_envelope_bytes(raw)
            self.assertTrue(issues and issues[0].startswith("unsupported_profile:"))
        graph = upstream_graph()
        graph["api_version"] = "future-graph/v2"
        graph["future_field"] = "future"
        raw = graph_json(graph)
        with self.assertRaisesRegex(Exception, "^unsupported_profile:"):
            derive_envelope(raw, "f" * 64, graph["producer"]["run_id"], PROJECT_ID)

    def test_cli_rooted_file_rejects_escape_symlink_and_writes_zero_stdout_on_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "safe").mkdir()
            target = root / "safe" / "snapshot.json"
            target.write_bytes(envelope_bytes())
            self.assertEqual(cli.main([str(root), "safe/snapshot.json"]), 0)
            stdout, stderr = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(cli.main([str(root), "../escape.json"]), 1)
            self.assertEqual(stdout.getvalue(), "")
            link = root / "linked"
            link.symlink_to(root / "safe", target_is_directory=True)
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(cli.main([str(root), "linked/snapshot.json"]), 1)
            self.assertEqual(stdout.getvalue(), "")

    def test_golden_wrapper_and_cli_pin_exact_input_output_and_digests(self):
        self.assertEqual(validate_golden_fixture(REPO_ROOT), [])
        self.assertEqual(cli.main(["--golden", str(REPO_ROOT)]), 0)
        fixture = json.loads((REPO_ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        mutations = [
            lambda item: item.update(api_version="invalid"),
            lambda item: item.update(fixture_semantics="claims authority"),
            lambda item: item["input"].update(project_id="fixture-other"),
            lambda item: item["input"].update(graph_observation_sha256="f" * 64),
            lambda item: item["expected"].update(snapshot_sha256="f" * 64),
            lambda item: item["expected"].update(canonical_envelope_json=
                                                 item["expected"]["canonical_envelope_json"] + " "),
            lambda item: item.update(unexpected=True),
        ]
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as directory:
                altered = copy.deepcopy(fixture)
                mutation(altered)
                root = Path(directory)
                path = root / FIXTURE_PATH
                path.parent.mkdir(parents=True)
                path.write_bytes(canonical_json(
                    altered, max_bytes=MAX_FIXTURE_BYTES) + b"\n")
                self.assertTrue(validate_golden_fixture(root))


if __name__ == "__main__":
    unittest.main()
