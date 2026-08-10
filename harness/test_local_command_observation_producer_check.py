#!/usr/bin/env python3
"""Golden and adversarial tests for the local producer fixture checker."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

import local_command_observation_producer_check as checker  # noqa: E402
from local_command_observation_producer.constants import FIXTURE_PATH  # noqa: E402
from local_command_observation_producer import validate_production  # noqa: E402
from local_command_observation_producer.constants import (  # noqa: E402
    ENVIRONMENT_DOMAIN, SOURCE_DOMAIN,
)
from local_command_observation_producer.profiles import (  # noqa: E402
    bounded_text, normalized_absolute, safe_repo_path,
)
from local_command_observation_producer.semantics import domain_digest  # noqa: E402


class LocalCommandObservationProducerCheckTest(unittest.TestCase):
    def test_repository_fixture_is_valid_and_non_live(self):
        self.assertEqual(checker.validate_golden_fixture(ROOT), [])
        self.assertEqual(checker.main(["--golden", str(ROOT)]), 0)

    def test_expected_digest_drift_is_rejected(self):
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            value = json.loads(path.read_text(encoding="utf-8"))
            value["expected"]["production_sha256"] = "a" * 64
            path.write_text(json.dumps(value), encoding="utf-8")
            issues = checker.validate_golden_fixture(root)
            self.assertTrue(any("production_sha256" in issue for issue in issues), issues)

    def test_coherent_fixed_profile_drift_is_rejected(self):
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            value = json.loads(path.read_text(encoding="utf-8"))
            value["production"]["tool_manifest"]["profile_id"] = "other"
            path.write_text(json.dumps(value), encoding="utf-8")
            issues = checker.validate_golden_fixture(root)
            self.assertTrue(any("tool: fixed fields" in issue for issue in issues), issues)

    def test_duplicate_json_key_is_rejected(self):
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            raw = path.read_text(encoding="utf-8")
            path.write_text(raw.replace("{", '{"api_version":"duplicate",', 1),
                            encoding="utf-8")
            issues = checker.validate_golden_fixture(root)
            self.assertTrue(any("duplicate JSON key" in issue for issue in issues), issues)

    def test_tool_and_source_preimage_drift_is_rejected(self):
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            value = json.loads(path.read_text(encoding="utf-8"))
            value["preimages"]["tool"]["utf8"] = "opaque replacement"
            path.write_text(json.dumps(value), encoding="utf-8")
            issues = checker.validate_golden_fixture(root)
            self.assertTrue(any("preimages.tool" in issue for issue in issues), issues)

    def test_profile_semantic_drift_is_rejected(self):
        value = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))["production"]
        mutations = self.profile_mutations()
        for name, mutate in mutations:
            with self.subTest(name=name):
                candidate = copy.deepcopy(value)
                mutate(candidate)
                self.assertTrue(validate_production(candidate), name)

    def test_scrubbed_path_rejects_every_malformed_component_after_rebinding(self):
        for path_value in (
                "",
                "/fixture/bin:relative",
                "/fixture/bin:",
                ":/fixture/bin",
                "/fixture/bin::/usr/bin",
                "/fixture/bin:/usr/bin/../bin"):
            with self.subTest(path_value=path_value):
                candidate = self.production()
                environment = candidate["environment_manifest"]
                path = next(variable for variable in environment["variables"]
                            if variable["name"] == "PATH")
                path["value"] = path_value
                candidate["observation"]["command"]["environment_sha256"] = domain_digest(
                    ENVIRONMENT_DOMAIN, environment)
                issues = validate_production(candidate)
                self.assertTrue(any("PATH components" in issue for issue in issues), issues)

    def test_source_bytes_reject_boolean_integer_impostors_after_rebinding(self):
        attacks = [
            ("regular false", 0, lambda entry: entry.update(bytes=False)),
            ("symlink true", 1, self.make_one_byte_symlink),
            ("deleted false", 0, self.make_deleted_with_false_bytes),
        ]
        for name, index, mutate in attacks:
            with self.subTest(name=name):
                candidate = self.production()
                source = candidate["source_manifest"]
                mutate(source["entries"][index])
                candidate["observation"]["source"]["source_tree_sha256"] = domain_digest(
                    SOURCE_DOMAIN, source)
                issues = validate_production(candidate)
                self.assertTrue(any("source.entries" in issue for issue in issues), issues)

    def test_nested_type_drift_never_raises(self):
        production = self.production()
        candidates = []
        for path, replacement in [
                (("observation", "source"), []),
                (("observation", "command"), "command"),
                (("environment_manifest",), []),
                (("source_manifest",), "source"),
                (("tool_manifest", "symlink_hops"), [None])]:
            candidate = copy.deepcopy(production)
            self.replace(candidate, path, replacement)
            candidates.append(candidate)
        for candidate in candidates:
            self.assertTrue(validate_production(candidate))
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            value = json.loads(path.read_text(encoding="utf-8"))
            value["production"]["observation"]["source"] = []
            path.write_text(json.dumps(value), encoding="utf-8")
            self.assertTrue(checker.validate_golden_fixture(root))

    def profile_mutations(self):
        return [
            ("null environment list", lambda value: value["environment_manifest"].update(variables=None)),
            ("unsafe source path", lambda value: value["source_manifest"]["entries"][0].update(path=".forge/state")),
            ("unsorted source", lambda value: value["source_manifest"]["entries"].reverse()),
            ("regular symlink mode", lambda value: value["source_manifest"]["entries"][2].update(index_mode="120000")),
            ("symlink regular mode", lambda value: value["source_manifest"]["entries"][1].update(index_mode="100644")),
            ("broken tool chain", lambda value: value["tool_manifest"].update(final_path="/fixture/bin/other")),
            ("malformed tool hop", lambda value: value["tool_manifest"].update(symlink_hops=[None])),
        ]

    def test_float_and_forbidden_unicode_are_rejected(self):
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            raw = path.read_text(encoding="utf-8").replace('"bytes": 16', '"bytes": 16.5', 1)
            path.write_text(raw, encoding="utf-8")
            self.assertTrue(checker.validate_golden_fixture(root))
        with self.fixture_root() as root:
            path = root / FIXTURE_PATH
            raw = path.read_text(encoding="utf-8").replace("gate-link.mjs", "gate\u202e-link.mjs")
            path.write_text(raw, encoding="utf-8")
            self.assertTrue(checker.validate_golden_fixture(root))

    def test_text_limits_and_path_semantics_match_go_profile(self):
        issues: list[str] = []
        self.assertTrue(bounded_text("😀" * 4096, "text", issues))
        self.assertEqual(issues, [])
        for value in ("a" * 4097, "control\x1f", "next\u0085", "bidi\u202e", "line\u2028"):
            local_issues: list[str] = []
            self.assertFalse(bounded_text(value, "text", local_issues))
            self.assertTrue(local_issues)
        self.assertTrue(safe_repo_path("vendor/.forge/x"))
        self.assertTrue(safe_repo_path("vendor/.git/x"))
        for value in (".", ".forge/x", ".Git/config", "a/../b", "a//b", "/rooted"):
            self.assertFalse(safe_repo_path(value), value)
        self.assertTrue(normalized_absolute("/fixture/bin/node"))
        for value in ("//fixture/bin/node", "/fixture/bin/../bin", "/fixture/bin/."):
            self.assertFalse(normalized_absolute(value), value)

    def test_valid_production_can_exceed_generic_256_item_limit(self):
        production = self.production()
        empty_sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        source = production["source_manifest"]
        source["entries"] = [self.regular_entry(index, empty_sha) for index in range(300)]
        production["observation"]["source"]["source_tree_sha256"] = domain_digest(
            SOURCE_DOMAIN, source)
        self.assertEqual(validate_production(production), [])

    def production(self):
        value = json.loads((ROOT / FIXTURE_PATH).read_text(encoding="utf-8"))
        return value["production"]

    @staticmethod
    def replace(value, path, replacement):
        parent = value
        for segment in path[:-1]:
            parent = parent[segment]
        parent[path[-1]] = replacement

    @staticmethod
    def regular_entry(index, digest):
        return {
            "bytes": 0, "content_sha256": digest, "executable": False,
            "index_mode": None, "kind": "regular",
            "path": f"generated/file-{index:04d}.txt", "symlink_target": None,
            "tracking": "untracked",
        }

    @staticmethod
    def make_one_byte_symlink(entry):
        entry.update(bytes=True, content_sha256=hashlib.sha256(b"x").hexdigest(),
                     executable=False, kind="symlink", symlink_target="x")

    @staticmethod
    def make_deleted_with_false_bytes(entry):
        entry.update(bytes=False, content_sha256=None, executable=None,
                     index_mode="100644", kind="deleted", symlink_target=None,
                     tracking="tracked")

    def fixture_root(self):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        target = root / Path(FIXTURE_PATH).parent
        target.mkdir(parents=True)
        shutil.copy2(ROOT / FIXTURE_PATH, target / Path(FIXTURE_PATH).name)
        return FixtureRoot(temporary, root)


class FixtureRoot:
    def __init__(self, temporary: tempfile.TemporaryDirectory, root: Path):
        self.temporary, self.root = temporary, root

    def __enter__(self) -> Path:
        return self.root

    def __exit__(self, *_args) -> None:
        self.temporary.cleanup()


if __name__ == "__main__":
    unittest.main()
