from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from harness.project_source_snapshot_contract.codec import (
    ContractError, canonical_json, decode_canonical, domain_digest,
)
from harness.project_source_snapshot_contract.constants import (
    DOMAINS, MAX_ARRAY_ITEMS, MAX_DEPTH, MAX_ENVELOPE_BYTES, MAX_EXCLUDED,
    MAX_FIELDS, MAX_FILE_BYTES, MAX_GIT_BYTES, MAX_I64, MAX_IGNORED,
    MAX_PATH_BYTES, MAX_PATH_COMPONENTS, MAX_PATH_SCALARS, MAX_SHORT_TEXT_BYTES,
    MAX_TOTAL_BYTES, MAX_UNIVERSE, MIN_I64,
)
from harness.project_source_snapshot_contract.derive import (
    build_entry, build_production,
)
from harness.project_source_snapshot_contract.fixture import golden_value
from harness.project_source_snapshot_contract.validation import (
    decode_production, validate_production,
)
from harness.project_source_snapshot_contract.shapes import (
    exact_array, identifier, integer, validate_entry, validate_git_facts,
    validate_path, validate_total_bytes,
)

ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "harness/project_source_snapshot_contract/check.py"


class ProjectSourceSnapshotAdversarialTest(unittest.TestCase):
    def setUp(self) -> None:
        self.value = golden_value()
        self.raw = canonical_json(self.value)

    def reject_raw(self, raw: bytes) -> None:
        with self.assertRaises(ContractError):
            decode_production(raw)

    def reject_value(self, value: object) -> None:
        with self.assertRaises(ContractError):
            validate_production(value)

    def test_canonical_duplicate_unknown_trailing_and_encoding(self) -> None:
        duplicate = self.raw.replace(
            b'{"api_version":', b'{"api_version":"duplicate","api_version":', 1)
        for raw in (duplicate, self.raw + b"\n", b"\xef\xbb\xbf" + self.raw,
                    self.raw + b"x", b'\xff'):
            self.reject_raw(raw)
        unknown = copy.deepcopy(self.value)
        unknown["request"]["additional_paths"] = []
        self.reject_raw(canonical_json(unknown))

    def test_cli_malformed_input_fails_with_zero_stdout(self) -> None:
        with tempfile.NamedTemporaryFile() as stream:
            stream.write(self.raw + b"\n")
            stream.flush()
            result = subprocess.run(
                [sys.executable, "-B", str(CHECK), "--input", stream.name],
                cwd=ROOT, capture_output=True, check=False,
            )
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"ERROR", result.stderr)

    def test_numbers_bom_control_bidi_and_depth(self) -> None:
        self.reject_raw(self.raw.replace(b'"ignored_path_count":2',
                                         b'"ignored_path_count":2.0', 1))
        self.reject_raw(self.raw.replace(b'"ignored_path_count":2',
                                         b'"ignored_path_count":9223372036854775808', 1))
        project = b'"project_id":"fixture-project"'
        for forbidden in ("\u202e", "\ufeff", "\u0001"):
            self.reject_raw(self.raw.replace(
                project, f'"project_id":"x{forbidden}y"'.encode("utf-8"), 1))
        nested: object = None
        for _ in range(MAX_DEPTH + 1):
            nested = [nested]
        with self.assertRaises(ContractError):
            decode_canonical(json.dumps(nested).encode(), MAX_ENVELOPE_BYTES, "nested")

    def test_boolean_int64_and_canonical_container_boundaries(self) -> None:
        self.reject_raw(self.raw.replace(b'"ignored_path_count":2',
                                         b'"ignored_path_count":true', 1))
        numeric = f'{{"maximum":{MAX_I64},"minimum":{MIN_I64}}}'.encode()
        self.assertEqual(decode_canonical(numeric, len(numeric), "int64"),
                         {"maximum": MAX_I64, "minimum": MIN_I64})
        for outside in (MAX_I64 + 1, MIN_I64 - 1):
            raw = f'{{"value":{outside}}}'.encode()
            with self.assertRaises(ContractError):
                decode_canonical(raw, len(raw), "int64")

        depth: object = 0
        for _ in range(MAX_DEPTH - 1):
            depth = [depth]
        depth_raw = canonical_json(depth)
        self.assertEqual(decode_canonical(depth_raw, len(depth_raw), "depth"), depth)
        too_deep = json.dumps([depth], separators=(",", ":")).encode()
        with self.assertRaises(ContractError):
            decode_canonical(too_deep, len(too_deep), "depth")

        fields = {f"k{index}": index for index in range(MAX_FIELDS)}
        field_raw = canonical_json(fields)
        self.assertEqual(decode_canonical(field_raw, len(field_raw), "fields"), fields)
        too_wide = dict(fields, overflow=0)
        with self.assertRaises(ContractError):
            decode_canonical(json.dumps(too_wide, separators=(",", ":"),
                                        sort_keys=True).encode(), MAX_ENVELOPE_BYTES, "fields")

        items = [0] * MAX_ARRAY_ITEMS
        item_raw = canonical_json(items)
        self.assertEqual(len(decode_canonical(item_raw, len(item_raw), "items")),
                         MAX_ARRAY_ITEMS)
        with self.assertRaises(ContractError):
            decode_canonical(item_raw[:-1] + b",0]", len(item_raw) + 2, "items")

        bounded = b'"bounded"'
        self.assertEqual(decode_canonical(bounded, len(bounded), "bytes"), "bounded")
        with self.assertRaises(ContractError):
            decode_canonical(bounded, len(bounded) - 1, "bytes")

    def test_digest_and_binding_tampering(self) -> None:
        paths = [
            ("envelope_sha256",),
            ("request", "request_sha256"),
            ("snapshot", "snapshot_sha256"),
            ("snapshot", "snapshot_identity_sha256"),
            ("snapshot", "coverage", "coverage_sha256"),
            ("snapshot", "source_manifest", "source_manifest_sha256"),
            ("snapshot", "source_manifest", "entry_set_sha256"),
            ("snapshot", "source_manifest", "exclusion_set_sha256"),
            ("snapshot", "source_manifest", "entries", 0, "entry_sha256"),
            ("snapshot", "source_manifest", "excluded", 0, "exclusion_sha256"),
        ]
        for path in paths:
            value = copy.deepcopy(self.value)
            item = value
            for component in path[:-1]:
                item = item[component]
            item[path[-1]] = "0" * 64
            self.reject_value(value)

    def test_digest_domain_swap_is_rejected(self) -> None:
        value = copy.deepcopy(self.value)
        entry = value["snapshot"]["source_manifest"]["entries"][0]
        preimage = dict(entry, entry_sha256="")
        entry["entry_sha256"] = domain_digest(DOMAINS["exclusion"], preimage)
        self.reject_value(value)

    def test_privacy_shape_and_path_policy(self) -> None:
        manifest = self.value["snapshot"]["source_manifest"]
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["excluded"][0]["path"] = ".env"
        self.reject_value(value)
        for field, injected in (("symlink_target", "target"), ("raw_path", ".env")):
            value = copy.deepcopy(self.value)
            value["snapshot"]["source_manifest"]["excluded"][0][field] = injected
            self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["entries"][0]["path"] = ".env"
        self.reject_value(value)
        self.assertTrue(all("path" not in item for item in manifest["excluded"]))

    def test_order_uniqueness_and_conservation(self) -> None:
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["entries"].reverse()
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["excluded"].reverse()
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        manifest = value["snapshot"]["source_manifest"]
        manifest["excluded"][0]["path_sha256"] = manifest["entries"][0]["path_sha256"]
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["universe_count"] += 1
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["coverage"]["counts"]["tracked_count"] += 1
        self.reject_value(value)

    def test_fixed_surfaces_and_authority_markers(self) -> None:
        value = copy.deepcopy(self.value)
        value["snapshot"]["coverage"]["surfaces"][0]["status"] = "partial"
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["coverage"]["surfaces"].reverse()
        self.reject_value(value)
        for field in ("atomic", "authority_attested", "effect_attested",
                      "permission_attested", "persistence_attested", "truth_attested"):
            value = copy.deepcopy(self.value)
            value["snapshot"][field] = True
            self.reject_value(value)

    def test_entry_modes_kinds_content_and_revision(self) -> None:
        mutations = [
            ("kind", "symlink"), ("tracking", "ambient"),
            ("index_mode", "100600"), ("bytes", MAX_FILE_BYTES + 1),
            ("content_sha256", None), ("executable", None),
        ]
        for field, changed in mutations:
            value = copy.deepcopy(self.value)
            entries = value["snapshot"]["source_manifest"]["entries"]
            regular = next(item for item in entries if item["kind"] == "regular" and
                           item["tracking"] == "tracked")
            regular[field] = changed
            self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["source_revision"] = "HEAD"
        self.reject_value(value)

    def test_git_observer_bounds_and_fixed_facts(self) -> None:
        for field, changed in (
            ("executable_bytes", MAX_GIT_BYTES + 1),
            ("executable_sha256", "A" * 64),
            ("version", "2.50.1"),
            ("identity_attestation", "authenticated"),
            ("local_config_isolation", "isolated"),
            ("network_containment", "provided"),
        ):
            value = copy.deepcopy(self.value)
            value["snapshot"]["source_manifest"]["git_observer"][field] = changed
            self.reject_value(value)

    def test_path_resource_boundaries(self) -> None:
        base = {"bytes": 0, "content_sha256": "0" * 64, "executable": False,
                "index_mode": None, "kind": "regular", "tracking": "untracked"}
        for path in ("a" * (MAX_PATH_BYTES + 1),
                     "/".join("a" for _ in range(MAX_PATH_COMPONENTS + 1)),
                     "é" * (MAX_PATH_SCALARS + 1), "../escape", r"x\y", "C:/x"):
            value = copy.deepcopy(self.value)
            facts = dict(base, path=path)
            value["snapshot"]["source_manifest"]["entries"][0] = build_entry(facts)
            self.reject_value(value)

        byte_path = "😀" * MAX_PATH_SCALARS
        self.assertEqual(len(byte_path.encode("utf-8")), MAX_PATH_BYTES)
        self.assertEqual(validate_path(byte_path, "path"), byte_path)
        component_path = "/".join("a" for _ in range(MAX_PATH_COMPONENTS))
        self.assertEqual(validate_path(component_path, "path"), component_path)
        scalar_path = "é" * MAX_PATH_SCALARS
        self.assertEqual(validate_path(scalar_path, "path"), scalar_path)

    def test_semantic_numeric_resource_boundaries(self) -> None:
        self.assertEqual(identifier("a" * 160, "identifier"), "a" * 160)
        with self.assertRaises(ContractError):
            identifier("a" * 161, "identifier")
        self.assertEqual(integer(MAX_IGNORED, 0, MAX_IGNORED, "ignored"), MAX_IGNORED)
        with self.assertRaises(ContractError):
            integer(True, 0, MAX_IGNORED, "ignored")

        entry = build_entry({
            "bytes": MAX_FILE_BYTES, "content_sha256": "0" * 64,
            "executable": False, "index_mode": None, "kind": "regular",
            "path": "maximum.bin", "tracking": "untracked",
        })
        self.assertEqual(validate_entry(entry, "entry")["bytes"], MAX_FILE_BYTES)
        entries = [dict(entry, bytes=MAX_TOTAL_BYTES)]
        validate_total_bytes(entries)
        with self.assertRaises(ContractError):
            validate_total_bytes([dict(entry, bytes=MAX_TOTAL_BYTES + 1)])

        git = copy.deepcopy(self.value["snapshot"]["source_manifest"]["git_observer"])
        git["executable_bytes"] = MAX_GIT_BYTES
        git["version"] = "git version " + "x" * (MAX_SHORT_TEXT_BYTES - len("git version "))
        self.assertEqual(validate_git_facts(git)["executable_bytes"], MAX_GIT_BYTES)
        for field, changed in (("executable_bytes", MAX_GIT_BYTES + 1),
                               ("version", git["version"] + "x")):
            invalid = dict(git, **{field: changed})
            with self.assertRaises(ContractError):
                validate_git_facts(invalid)

        self.assertEqual(len(exact_array([None] * MAX_UNIVERSE, MAX_UNIVERSE,
                                         "universe")), MAX_UNIVERSE)
        with self.assertRaises(ContractError):
            exact_array([None] * (MAX_EXCLUDED + 1), MAX_EXCLUDED, "excluded")

    def test_collection_bounds(self) -> None:
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["entries"] = [{}] * (MAX_UNIVERSE + 1)
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["excluded"] = [{}] * (MAX_EXCLUDED + 1)
        self.reject_value(value)
        value = copy.deepcopy(self.value)
        value["snapshot"]["source_manifest"]["ignored_path_count"] = MAX_IGNORED + 1
        self.reject_value(value)

    def test_reconstruction_rejects_alternate_observation(self) -> None:
        value = copy.deepcopy(self.value)
        manifest = value["snapshot"]["source_manifest"]
        facts = [{key: item[key] for key in
                  {"bytes", "content_sha256", "executable", "index_mode", "kind", "path", "tracking"}}
                 for item in manifest["entries"]]
        regular = next(item for item in facts if item["kind"] == "regular")
        regular["content_sha256"] = "f" * 64
        altered = build_production(
            value["request"]["project_id"], value["request"]["run_id"], facts,
            [{key: item[key] for key in
              {"index_mode", "leaf_filesystem_observed", "path_sha256", "reason", "tracking"}}
             for item in manifest["excluded"]],
            {key: manifest["git_observer"][key]
             for key in {"executable_bytes", "executable_sha256", "version"}},
            manifest["ignored_path_count"], manifest["source_revision"],
        )
        self.assertEqual(validate_production(altered), altered)
        self.assertNotEqual(altered["envelope_sha256"], self.value["envelope_sha256"])


if __name__ == "__main__":
    unittest.main()
