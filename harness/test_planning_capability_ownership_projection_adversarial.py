"""Executable cross-runtime parser and resource adversaries for ADR-0069."""

from __future__ import annotations

import copy
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from planning_capability_ownership_projection import ContractError
from planning_capability_ownership_projection.check import _read
from planning_capability_ownership_projection.constants import (
    CATALOG_PATH, MAPPING_PATH, MAX_CATALOG_BYTES, MAX_COLLECTIONS,
    MAX_SCALAR_BYTES, MAX_TOKENS,
)
from planning_capability_ownership_projection.request import build_request, validate_request
from planning_capability_ownership_projection.yaml_resources import Resources
from planning_capability_ownership_projection.yaml_subset import BlockParser, parse_yaml

ROOT = Path(__file__).resolve().parents[1]
NODE_ARRAY_FIELDS = (
    "activities", "authority", "entry_criteria", "escalation", "exit_criteria",
    "forbidden", "handoff", "inputs", "memory_updates", "outputs",
    "quality_gates", "rules",
)


def _node(node_id: int, capabilities: list[str]) -> list[str]:
    lines = [f'  - id: "{node_id:02d}"', "    name: name", "    owner_lens: owner",
             "    purpose: purpose"]
    for field in NODE_ARRAY_FIELDS:
        lines.append(f"    {field}: [x]")
    lines.append(f"    capabilities: [{', '.join(capabilities)}]")
    return lines


def _catalog(rows: list[list[str]]) -> bytes:
    lines = [
        "api_version: forgeos.design/v1", "kind: AIEngineeringCapabilityCatalog",
        "status: planning_only", "executable: false", "decision_ref: ADR",
        "extension_decision_refs: [ADR]", "runtime_note: note",
        "canonical_vocabulary: {}", "control_plane_joins: {}",
        "authority_semantics: {}", "risk_levels: {}",
        "universal_node_contract: {}", "gates: {}", "nodes:",
    ]
    for node_id, capabilities in enumerate(rows):
        lines.extend(_node(node_id, capabilities))
    return ("\n".join(lines) + "\n").encode()


def _mapping(packages: list[list[str]]) -> bytes:
    lines = [
        "api_version: forgeos.design/v1", "kind: CapabilitySkillOwnershipMap",
        "status: planning_only", "executable: false",
        "source_catalog: capability-catalog.v1.yml",
        "skill_specification: spec", "mapping_rules: [rule]", "packages:",
    ]
    for index, capabilities in enumerate(packages):
        lines.extend((f"  - skill: skill{index}", "    implementation_wave: 1",
                      f"    includes: [{', '.join(capabilities)}]"))
    return ("\n".join(lines) + "\n").encode()


class OwnershipProjectionAdversarialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.catalog = (ROOT / CATALOG_PATH).read_bytes()
        cls.mapping = (ROOT / MAPPING_PATH).read_bytes()

    def assert_yaml_rejected(self, raw: bytes) -> None:
        with self.assertRaises(ContractError):
            parse_yaml(raw)

    def test_shared_accepted_yaml_corpus(self) -> None:
        raw = (
            b'A9_b-c.d/e: {"B2/c": [], C3: {}}\n'
            b'quoted: "value#&*!\'"\n'
            b'nested:\n  -\n    key: value\n'
        )
        self.assertEqual(parse_yaml(raw)["quoted"], "value#&*!'")

    def test_shared_forbidden_indicator_corpus(self) -> None:
        plain = (b"#", b"&", b"*", b"!", b"'", b"\\")
        cases = [b"a: value" + indicator + b"data\n" for indicator in plain]
        cases.extend(b"a: >-\n  value" + indicator + b"data\n" for indicator in plain)
        cases.append(b'a: >-\n  "value#data"\n')
        cases.extend((
            b'a: >-\n  unmatched"\n', b"a: >-\n  %YAML data\n",
            b"a: >-\n  ---\n", b"a: >-\n  ...\n", b"a: >-\n  <<: value\n",
        ))
        for raw in cases:
            with self.subTest(raw=raw):
                self.assert_yaml_rejected(raw)

    def test_every_c0_and_del_byte_is_rejected(self) -> None:
        forbidden = [value for value in range(0x20) if value != 0x0A] + [0x7F]
        for value in forbidden:
            with self.subTest(value=value):
                self.assert_yaml_rejected(b"a: x" + bytes([value]) + b"\n")

    def test_exact_block_and_flow_colon_spacing(self) -> None:
        accepted = (b"a:\n  b: x\n", b"a: {b: x}\n")
        rejected = (
            b"a:x\n", b"a:  x\n", b"a: {b:x}\n", b"a: {b:  x}\n",
            b"a:\n  - a]: b\n", b"a:\n  - a}: b\n",
        )
        for raw in accepted:
            parse_yaml(raw)
        for raw in rejected:
            self.assert_yaml_rejected(raw)

    def test_scalar_number_timestamp_and_indicator_rejections(self) -> None:
        values = (
            "Y", "n", "YES", "Off", "Null", "~", ".Inf", "NAN", "01", "-0",
            "+nan", "-.NaN", "+1", "1.0", "1e3", "0x10", "2026-8-3",
            "12:34:56", "12:34z", "12:34+05:30", "@value", "value:",
            "9223372036854775808",
        )
        for value in values:
            with self.subTest(value=value):
                self.assert_yaml_rejected(f"a: {value}\n".encode())

    def test_long_integer_like_scalars_fail_with_contract_error(self) -> None:
        for digits in (4_301, MAX_SCALAR_BYTES):
            with self.subTest(digits=digits):
                self.assert_yaml_rejected(b"a: " + b"9" * digits + b"\n")

    def test_folded_total_bound_and_single_token(self) -> None:
        first = "x" * (MAX_SCALAR_BYTES // 2)
        second = "y" * (MAX_SCALAR_BYTES - len(first) - 1)
        raw = f"a: >-\n  {first}\n  {second}\n".encode()
        parser = BlockParser(raw)
        parser.parse()
        self.assertEqual(parser.resources.tokens, 3)
        self.assert_yaml_rejected(raw[:-1] + b"y\n")
        self.assertEqual(parse_yaml(b'a: >-\n  "literal"\n'), {"a": '"literal"'})
        self.assertEqual(parse_yaml(b"a: >-\n  x\n\nb: ok\n"), {"a": "x", "b": "ok"})
        self.assert_yaml_rejected(b"a: >-\n  x\n\n  y\n")

    def test_collection_key_scalar_token_accounting_and_boundaries(self) -> None:
        parser = BlockParser(b"a: [x, {}]\n")
        parser.parse()
        self.assertEqual((parser.resources.tokens, parser.resources.collections), (5, 3))
        at = Resources(tokens=MAX_TOKENS - 1, collections=MAX_COLLECTIONS - 1)
        at.collection(1)
        with self.assertRaises(ContractError):
            at.token()
        with self.assertRaises(ContractError):
            Resources(collections=MAX_COLLECTIONS).collection(1)

    def test_direct_yaml_byte_bound_precedes_line_construction(self) -> None:
        with patch(
            "planning_capability_ownership_projection.yaml_subset._screen_line",
            side_effect=AssertionError("line construction must not run"),
        ) as screen:
            with self.assertRaises(ContractError):
                parse_yaml(b"x" * (MAX_CATALOG_BYTES + 1), MAX_CATALOG_BYTES)
        screen.assert_not_called()

    def test_base64_text_bound_precedes_decode(self) -> None:
        request = copy.deepcopy(build_request(self.catalog, self.mapping))
        maximum = 4 * ((MAX_CATALOG_BYTES + 2) // 3)
        request["catalog_source"]["content_base64"] = "A" * (maximum + 1)
        with patch(
            "planning_capability_ownership_projection.request.base64.b64decode",
            side_effect=AssertionError("decoder must not run"),
        ) as decoder, self.assertRaises(ContractError):
            validate_request(request)
        decoder.assert_not_called()

    def test_ignored_node_arrays_allow_generic_subset_values(self) -> None:
        changed = self.catalog.replace(
            b"entry_criteria: [user_or_runtime_intent_exists]",
            b"entry_criteria: [false, null, 1, [], {}]", 1,
        )
        build_request(changed, self.mapping)

    def test_unique_capability_bound_at_n_and_n_plus_one(self) -> None:
        capabilities = [f"c{index}" for index in range(512)]
        build_request(_catalog([capabilities]), _mapping([capabilities]))
        with self.assertRaises(ContractError):
            build_request(_catalog([capabilities, ["extra"]]), _mapping([capabilities]))

    def test_occurrence_bound_at_n_and_n_plus_one(self) -> None:
        capabilities = [f"c{index}" for index in range(64)]
        rows = [capabilities for _ in range(64)]
        build_request(_catalog(rows), _mapping([capabilities]))
        rows[0] = capabilities + ["extra"]
        with self.assertRaises(ContractError):
            build_request(_catalog(rows), _mapping([capabilities + ["extra"]]))

    def test_mapping_owner_bound_at_n_and_n_plus_one(self) -> None:
        capabilities = [f"c{index}" for index in range(512)]
        build_request(_catalog([capabilities]), _mapping([capabilities]))
        with self.assertRaises(ContractError):
            build_request(_catalog([capabilities]), _mapping([capabilities, ["extra"]]))

    def test_cli_read_detects_path_identity_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first, second = Path(directory) / "first", Path(directory) / "second"
            first.write_bytes(b"a: x\n")
            second.write_bytes(b"a: y\n")
            snapshots = [os.lstat(first), os.lstat(second)]
            with patch(
                "planning_capability_ownership_projection.check.os.lstat",
                side_effect=snapshots,
            ), self.assertRaises(ContractError):
                _read(str(first), 100, "source")

    def test_cli_open_is_nonblocking_against_special_file_race(self) -> None:
        nonblocking = getattr(os, "O_NONBLOCK", 0)
        if not nonblocking:
            self.skipTest("host has no O_NONBLOCK")
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source"
            source.write_bytes(b"a: x\n")
            real_open = os.open

            def guarded_open(path: str, flags: int) -> int:
                self.assertTrue(flags & nonblocking)
                return real_open(path, flags)

            with patch(
                "planning_capability_ownership_projection.check.os.open",
                side_effect=guarded_open,
            ):
                self.assertEqual(_read(str(source), 100, "source"), b"a: x\n")


if __name__ == "__main__":
    unittest.main()
