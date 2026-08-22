"""Focused golden, strict YAML, semantic, CLI, and no-resolution ADR-0069 tests."""

from __future__ import annotations

import copy
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from planning_capability_ownership_projection import ContractError, canonical_json
from planning_capability_ownership_projection import check as check_cli
from planning_capability_ownership_projection.constants import (
    CATALOG_PATH, FIXTURE_PATH, MAPPING_PATH, MAX_CATALOG_BYTES, MAX_DEPTH,
    MAX_MAPPING_BYTES,
)
from planning_capability_ownership_projection.fixture import load_golden
from planning_capability_ownership_projection.projection import project, validate_projection
from planning_capability_ownership_projection.request import build_request, validate_request
from planning_capability_ownership_projection.sources import parse_sources
from planning_capability_ownership_projection.yaml_subset import parse_yaml

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "harness/planning_capability_ownership_projection/check.py"


class OwnershipProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.catalog = (ROOT / CATALOG_PATH).read_bytes()
        cls.mapping = (ROOT / MAPPING_PATH).read_bytes()
        cls.request = build_request(cls.catalog, cls.mapping)
        cls.projection = project(cls.request)

    def assert_rejected(self, catalog: bytes | None = None,
                        mapping: bytes | None = None) -> None:
        with self.assertRaises(ContractError):
            build_request(self.catalog if catalog is None else catalog,
                          self.mapping if mapping is None else mapping)

    def test_current_exact_golden_and_counts(self) -> None:
        golden = load_golden(ROOT)
        self.assertEqual(golden["request"], self.request)
        self.assertEqual(golden["projection"], self.projection)
        self.assertEqual(self.request["request_sha256"],
                         "3639c4d3ad21db93db254b7da2643d492ca39c4dda5438de426379cd70718cfa")
        self.assertEqual(self.projection["projection_sha256"],
                         "53754ded32379d6520f3bd2b9d2956238731ad40c11124be457b724b4c150fa2")
        self.assertEqual(self.projection["coverage"], {
            "binding_count": 140, "capability_occurrence_count": 145,
            "catalog_node_count": 17, "mapped_capability_count": 140,
            "mapping_package_count": 38, "unique_capability_count": 140,
            "unmapped_capability_ids": [], "unreferenced_mapping_capability_ids": [],
        })

    def test_projection_does_not_resolve_logical_adapter(self) -> None:
        with patch("os.stat", side_effect=AssertionError("ambient stat")), \
                patch("os.open", side_effect=AssertionError("ambient open")):
            output = project(copy.deepcopy(self.request))
        self.assertTrue(all(binding["physical_resolution"] == "not_performed"
                            for binding in output["bindings"]))
        self.assertTrue(all(binding["skill_availability"] == "not_evaluated"
                            for binding in output["bindings"]))

    def test_yaml_framing_and_dangerous_syntax_rejected(self) -> None:
        variants = [
            self.mapping[:-1], self.mapping + b"\n", self.mapping.replace(b"\n", b"\r\n"),
            self.mapping.replace(b"  - skill:", b"\t- skill:", 1),
            b"\xef\xbb\xbf" + self.mapping, self.mapping.replace(b"status:", b"status: # x", 1),
            self.mapping.replace(b"planning_only", b"&a planning_only", 1),
            self.mapping.replace(b"planning_only", b"*a", 1),
            self.mapping.replace(b"planning_only", b"!tag planning_only", 1),
            b"---\n" + self.mapping, self.mapping.replace(b"planning_only", b"'planning_only'", 1),
            self.mapping.replace(b"planning_only", b"planning_only  ", 1),
        ]
        for variant in variants:
            with self.subTest(variant=variant[:40]):
                self.assert_rejected(mapping=variant)

    def test_yaml_scalar_coercion_and_duplicate_keys_rejected(self) -> None:
        replacements = [b"True", b"yes", b"01", b"1.0", b".inf", b"2026-08-13"]
        for replacement in replacements:
            with self.subTest(replacement=replacement):
                self.assert_rejected(mapping=self.mapping.replace(b"false", replacement, 1))
        duplicate = self.mapping.replace(b"kind: CapabilitySkillOwnershipMap\n",
                                         b"kind: CapabilitySkillOwnershipMap\nkind: duplicate\n")
        self.assert_rejected(mapping=duplicate)
        duplicate_nested = self.mapping.replace(b"    implementation_wave: 1\n",
                b"    implementation_wave: 1\n    implementation_wave: 1\n", 1)
        self.assert_rejected(mapping=duplicate_nested)

    def test_flow_and_folded_boundaries(self) -> None:
        self.assertEqual(parse_yaml(b"a: [x, {b: true}]\nc: >-\n  one\n  two\n"),
                         {"a": ["x", {"b": True}], "c": "one two"})
        invalid = [
            b"a: [x,]\n", b"a: {b: x,}\n", b"a: |\n  x\n", b"a: >-\n    x\n",
            b"a: >-\n  x\n\n", b"a: \"x\\ny\"\n",
        ]
        for raw in invalid:
            with self.subTest(raw=raw):
                with self.assertRaises(ContractError):
                    parse_yaml(raw)
        nested = b"a: " + b"[" * MAX_DEPTH + b"x" + b"]" * MAX_DEPTH + b"\n"
        with self.assertRaises(ContractError):
            parse_yaml(nested)

    def test_source_semantic_mutations_fail_closed(self) -> None:
        cases = [
            self.mapping.replace(b"includes: [work-intake, convergence]",
                                 b"includes: [work-intake, work-intake]", 1),
            self.mapping.replace(b"includes: [evidence-scan]", b"includes: [work-intake]", 1),
            self.mapping.replace(b"includes: [evidence-scan]", b"includes: [dangling]", 1),
            self.mapping.replace(b"implementation_wave: 1", b"implementation_wave: 7", 1),
            self.mapping.replace(b"skill: evidence-claim-management",
                                 b"skill: change-intake-orchestration", 1),
            self.mapping.replace(b"skill: evidence-claim-management",
                                 b"skill: evidence/claim", 1),
            self.mapping.replace(b"skill: evidence-claim-management",
                                 b"skill: evidence:claim", 1),
        ]
        for mapping in cases:
            with self.subTest(mapping=mapping[:80]):
                self.assert_rejected(mapping=mapping)
        duplicate_node = self.catalog.replace(b'  - id: "01"', b'  - id: "00"', 1)
        self.assert_rejected(catalog=duplicate_node)
        duplicate_capability = self.catalog.replace(b"capabilities: [work-intake,",
                                                     b"capabilities: [work-intake, work-intake,", 1)
        self.assert_rejected(catalog=duplicate_capability)
        self.assert_rejected(catalog=self.catalog.replace(
            b"extension_decision_refs: [ADR-0038, ADR-0039]",
            b"extension_decision_refs: []"))
        self.assert_rejected(mapping=self.mapping.replace(
            b'mapping_rules:\n  - "Every unique', b"mapping_rules:\n  - 1\n  - \"Every unique", 1))

    def test_direct_api_source_bounds_precede_parsing(self) -> None:
        with patch(
            "planning_capability_ownership_projection.request.parse_sources",
            side_effect=AssertionError("parser must not run"),
        ) as parser:
            with self.assertRaises(ContractError):
                build_request(b"x" * (MAX_CATALOG_BYTES + 1), self.mapping)
            with self.assertRaises(ContractError):
                build_request(self.catalog, b"x" * (MAX_MAPPING_BYTES + 1))
        parser.assert_not_called()

    def test_request_and_projection_tamper_fail_closed(self) -> None:
        request = copy.deepcopy(self.request)
        request["catalog_source"]["content_bytes"] += 1
        with self.assertRaises(ContractError):
            validate_request(request)
        for mutate in (
            lambda value: value["bindings"].reverse(),
            lambda value: value["bindings"][0].update(owner_skill="changed"),
            lambda value: value["coverage"].update(binding_count=139),
            lambda value: value["authority_semantics"].update(runtime_routing=True),
        ):
            value = copy.deepcopy(self.projection)
            mutate(value)
            with self.subTest(mutate=mutate):
                with self.assertRaises(ContractError):
                    validate_projection(value)

    def test_cli_exact_success_and_empty_failure_stdout(self) -> None:
        result = subprocess.run(
            ["python3", "-B", str(CHECKER), "project", "--catalog", "-",
             "--mapping", str(ROOT / MAPPING_PATH)], input=self.catalog,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, canonical_json(self.projection) + b"\n")
        invalid_args = [
            [], ["--help"], ["project", "--help"],
            ["project", "--catalog=-", "--mapping", str(ROOT / MAPPING_PATH)],
            ["project", "-c", "-", "--mapping", str(ROOT / MAPPING_PATH)],
            ["project", "--catalog", str(ROOT / CATALOG_PATH), "--mapping",
             str(ROOT / MAPPING_PATH)],
            ["project", "--catalog", "-", "--mapping", "-"],
            ["project", "--catalog", "-", "--catalog", str(ROOT / CATALOG_PATH)],
            ["project", "--catalog", "-", "--mapping", str(ROOT / MAPPING_PATH), "extra"],
        ]
        for arguments in invalid_args:
            failure = subprocess.run(
                ["python3", "-B", str(CHECKER), *arguments], input=self.catalog,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            with self.subTest(arguments=arguments):
                self.assertEqual((failure.returncode, failure.stdout), (2, b""))

    @unittest.skipUnless(sys.platform.startswith("linux"), "requires Linux O_NONBLOCK pipe")
    def test_nonblocking_stdin_requires_explicit_eof(self) -> None:
        import fcntl
        read_fd, write_fd = os.pipe()
        try:
            capacity = fcntl.fcntl(read_fd, fcntl.F_GETPIPE_SZ)
            if capacity < len(self.mapping):
                fcntl.fcntl(read_fd, fcntl.F_SETPIPE_SZ, len(self.mapping))
            os.set_blocking(read_fd, False)
            self.assertEqual(os.write(write_fd, self.mapping), len(self.mapping))
            result = subprocess.run(
                ["python3", "-B", str(CHECKER), "project", "--catalog",
                 str(ROOT / CATALOG_PATH), "--mapping", "-"],
                cwd=ROOT, stdin=read_fd, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, check=False, timeout=5,
            )
        finally:
            os.close(read_fd)
            os.close(write_fd)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_stdin_reader_rejects_incomplete_nonbytes_and_n_plus_one(self) -> None:
        class Input:
            def __init__(self, result):
                self.buffer, self.result, self.reads = self, result, 0

            def read(self, _size):
                self.reads += 1
                if isinstance(self.result, BaseException):
                    raise self.result
                return self.result

        for result in (BlockingIOError(), None, bytearray(b"x"), b"12345"):
            stream = Input(result)
            with self.subTest(result=type(result).__name__), \
                    patch.object(check_cli.sys, "stdin", stream), \
                    self.assertRaises(ContractError):
                check_cli._stdin(4, "mapping")
            self.assertEqual(stream.reads, 1)

    def test_physical_checker_rejects_source_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            (target / Path(CATALOG_PATH).parent).mkdir(parents=True)
            (target / Path(FIXTURE_PATH).parent).mkdir(parents=True)
            os.symlink(ROOT / CATALOG_PATH, target / CATALOG_PATH)
            (target / MAPPING_PATH).write_bytes(self.mapping)
            (target / FIXTURE_PATH).write_bytes((ROOT / FIXTURE_PATH).read_bytes())
            with self.assertRaises(ContractError):
                load_golden(target)


if __name__ == "__main__":
    unittest.main()
