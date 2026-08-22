#!/usr/bin/env python3
"""Adversarial, bounds, golden, CLI, and physical tests for ADR-0068."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HARNESS = Path(__file__).resolve().parent
ROOT = HARNESS.parent
sys.path.insert(0, str(HARNESS))

from capability_registry_contract.builder import _recursive_set, build_fixture  # noqa: E402
from capability_registry_contract import check as check_cli  # noqa: E402
from capability_registry_contract.codec import ContractError, canonical_json, decode_canonical  # noqa: E402
from capability_registry_contract.constants import (  # noqa: E402
    FROZEN_FIXTURE_SHA256, FROZEN_REGISTRY_SHA256, LEGACY_OPAQUE_SHA256,
    MAX_REGISTRY_BYTES,
)
from capability_registry_contract.digests import seal  # noqa: E402
from capability_registry_contract.filesystem import (  # noqa: E402
    MAX_VISITED_PATHS, _visit, guard_root, read_regular, scan_regular, stable_root,
)
from capability_registry_contract.fixture import load_fixture, validate_fixture  # noqa: E402
from capability_registry_contract.physical import validate_physical_registry  # noqa: E402
from capability_registry_contract.resolver import resolve_declared, validate_assessment  # noqa: E402
from capability_registry_contract.shapes import validate_rule  # noqa: E402
from capability_registry_contract.validation import validate_registry, validate_request  # noqa: E402


def reseal_registry(registry, mutate):
    value = copy.deepcopy(registry)
    mutate(value)
    entry = value["entries"][0]
    entry["contract"] = seal("contract", entry["contract"])
    value["entries"][0] = seal("entry", entry)
    return seal("registry", value)


def reseal_content_set(value, index, mutate):
    content_set = value["entries"][0]["content_sets"][index]
    mutate(content_set)
    value["entries"][0]["content_sets"][index] = seal("content_set", content_set)


class GoldenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = load_fixture(ROOT)
        cls.registry = cls.fixture["registry"]

    def test_physical_golden_and_builder_are_exact(self):
        raw = (ROOT / "docs/contracts/fixtures/capability-registry-v1.json").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), FROZEN_FIXTURE_SHA256)
        if not (ROOT / "forge-core/internal/goimpactprescan").is_dir():
            self.skipTest("universal scaffold intentionally omits Catalyst Go implementation")
        self.assertEqual(raw, canonical_json(build_fixture(ROOT)) + b"\n")
        self.assertEqual(self.registry["registry_sha256"], FROZEN_REGISTRY_SHA256)
        self.assertEqual(validate_physical_registry(ROOT, self.registry), [])

    def test_three_frozen_resolutions_and_authority_ceiling(self):
        expected = {
            "legacy_repository_reader_not_registered": "capability_id_not_found",
            "registered_key_digest_mismatch": "capability_contract_digest_mismatch",
            "resolved_exact": "resolved_exact",
        }
        for case_id, resolution in expected.items():
            request = self.fixture["requests"][case_id]
            assessment = resolve_declared(self.registry, request)
            self.assertEqual(assessment, self.fixture["assessments"][case_id])
            self.assertEqual(assessment["resolution"], resolution)
            self.assertEqual(assessment["authorization_decision"], "none")
            for field in ("permission_attestation", "invocation_attestation",
                          "runtime_routing_attestation", "effect_attestation"):
                self.assertIs(assessment[field], False)

    def test_legacy_opaque_digest_is_unchanged_and_not_authority(self):
        request = self.fixture["requests"]["legacy_repository_reader_not_registered"]
        self.assertEqual(request["expected_reference"]["capability_contract_sha256"],
                         LEGACY_OPAQUE_SHA256)
        assessment = self.fixture["assessments"]["legacy_repository_reader_not_registered"]
        self.assertEqual(assessment["reason_codes"], ["capability_id_not_found"])
        self.assertNotIn("legacy", " ".join(assessment["reason_codes"]))


class SemanticAdversarialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = load_fixture(ROOT)
        cls.registry = cls.fixture["registry"]

    def test_resealed_owner_rule_adapter_and_proof_mutations_fail_pin(self):
        mutators = [
            lambda value: value["entries"][0]["owner"].update(team="attacker"),
            lambda value: value["entries"][0]["contract"]["rules"][0].update(
                statement="changed declaration"),
            lambda value: value["entries"][0]["implementations"][0]["adapters"][0].update(
                entrypoint="attacker.Entry"),
            lambda value: value["entries"][0]["contract"]["proof_obligations"][0].update(
                description="changed proof"),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate), self.assertRaisesRegex(
                    ContractError, "frozen singleton"):
                validate_registry(reseal_registry(self.registry, mutate))

    def test_resealed_contract_and_test_profile_mutations_fail_pin(self):
        contract = lambda value: value["entries"][0]["contract"]
        mutators = [
            lambda value: value["entries"][0]["tests"][0].update(
                entrypoint="attacker test"),
            lambda value: contract(value)["trigger"]["predicates"][0].update(
                value="forgeos.canonical-json/v2"),
            lambda value: contract(value)["quality_gates"][0].update(
                required_test_ids=["python-contract-suite"]),
            lambda value: contract(value).update(
                effects=["repo.read"], permission_requirements=[{
                    "effect_id": "repo.read", "requirement_id": "read-input",
                    "scope_profile": "repo_read"}]),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate), self.assertRaises(ContractError):
                validate_registry(reseal_registry(self.registry, mutate))

    def test_resealed_content_set_and_cycle_mutations_fail_pin(self):
        def mutate_set(index, mutate):
            return lambda value: reseal_content_set(value, index, mutate)

        mutators = [
            lambda value: value["entries"][0]["content_sets"].reverse(),
            lambda value: value["entries"][0]["content_sets"].append(
                copy.deepcopy(value["entries"][0]["content_sets"][0])),
            mutate_set(0, lambda item: item["files"].pop()),
            mutate_set(0, lambda item: item["files"].append({
                "content_bytes": 1, "content_sha256": "0" * 64,
                "media_type": "text/x-python", "path": "extra.py", "selector": None})),
            mutate_set(0, lambda item: item["files"][0].update(
                content_sha256="0" * 64)),
            mutate_set(0, lambda item: item["files"][0].update(
                path="docs/contracts/capability-registry-v1.schema.json")),
        ]
        for mutate in mutators:
            with self.subTest(mutate=mutate), self.assertRaises(ContractError):
                validate_registry(reseal_registry(self.registry, mutate))

    def test_duplicate_registry_entry_fails_even_when_resealed(self):
        value = copy.deepcopy(self.registry)
        value["entries"].append(copy.deepcopy(value["entries"][0]))
        value = seal("registry", value)
        with self.assertRaisesRegex(ContractError, "exactly one entry"):
            validate_registry(value)

    def test_duplicate_content_ref_path_selector_fails_even_if_hash_differs(self):
        value = copy.deepcopy(self.registry)
        refs = value["entries"][0]["contract"]["input_schemas"]
        duplicate = copy.deepcopy(refs[0])
        duplicate["content_sha256"] = "0" * 64
        refs.append(duplicate)
        refs.sort(key=canonical_json)
        value = reseal_registry(value, lambda _: None)
        with self.assertRaisesRegex(ContractError, "duplicate \(path, selector\)"):
            validate_registry(value)

    def test_typed_narrative_boundary_is_utf8_bytes(self):
        rule = {"enforcement_mode": "hard_gate", "rule_id": "bounded",
                "statement": "x" * 4096}
        validate_rule(rule, "rule")
        rule["statement"] += "x"
        with self.assertRaisesRegex(ContractError, "4096 UTF-8 bytes"):
            validate_rule(rule, "rule")
        rule["statement"] = "é" * 2049
        with self.assertRaisesRegex(ContractError, "4096 UTF-8 bytes"):
            validate_rule(rule, "rule")

    def test_noncanonical_duplicate_float_depth_and_unknown_fail(self):
        raw = canonical_json(self.registry)
        attacks = [
            b" " + raw,
            raw.replace(b'{"api_version":', b'{"api_version":"duplicate","api_version":', 1),
            b'{"value":1.0}',
            (b'[' * 17) + b'0' + (b']' * 17),
        ]
        for attack in attacks:
            with self.subTest(attack=attack[:30]), self.assertRaises(ContractError):
                decode_canonical(attack, max_bytes=MAX_REGISTRY_BYTES, label="attack")

    def test_null_contract_reports_id_and_version_but_nonnull_rejects(self):
        base = copy.deepcopy(self.fixture["requests"]["registered_key_digest_mismatch"])
        for field, value, expected in (
                ("capability_id", "unknown-capability", "capability_id_not_found"),
                ("capability_version", "2", "capability_version_not_found")):
            request = copy.deepcopy(base)
            request["expected_reference"][field] = value
            request = seal("request", request)
            validate_request(request)
            self.assertEqual(resolve_declared(self.registry, request)["resolution"], expected)
        request["expected_contract"] = copy.deepcopy(self.registry["entries"][0]["contract"])
        request = seal("request", request)
        with self.assertRaises(ContractError):
            validate_request(request)

    def test_assessment_reseal_cannot_claim_permission_or_pass(self):
        request = self.fixture["requests"]["resolved_exact"]
        for field in ("permission_attestation", "test_pass_attestation",
                      "implementation_availability_attestation"):
            assessment = copy.deepcopy(self.fixture["assessments"]["resolved_exact"])
            assessment[field] = True
            assessment = seal("assessment", assessment)
            with self.subTest(field=field), self.assertRaises(ContractError):
                validate_assessment(self.registry, request, assessment)

    def test_pure_resolver_performs_no_ambient_reads_or_execution(self):
        request = self.fixture["requests"]["resolved_exact"]
        with mock.patch("builtins.open", side_effect=AssertionError("ambient read")), \
                mock.patch("os.stat", side_effect=AssertionError("ambient stat")), \
                mock.patch("subprocess.run", side_effect=AssertionError("execution")):
            result = resolve_declared(self.registry, request)
        self.assertEqual(result["resolution"], "resolved_exact")


class FramingAndCliTests(unittest.TestCase):
    def _run(self, *arguments, stdin=None):
        return subprocess.run(
            [sys.executable, "-B", "harness/capability_registry_contract/check.py", *arguments],
            cwd=ROOT, input=stdin, capture_output=True, check=False)

    def test_fixture_requires_exactly_one_lf_and_physical_pin(self):
        original = (ROOT / "docs/contracts/fixtures/capability-registry-v1.json").read_bytes()
        for suffix in (original[:-1], original + b"\n", original[:-2] + b"x\n"):
            with tempfile.TemporaryDirectory() as directory:
                target = Path(directory) / "docs/contracts/fixtures"
                target.mkdir(parents=True)
                (target / "capability-registry-v1.json").write_bytes(suffix)
                with self.assertRaises(ContractError):
                    load_fixture(Path(directory))

    def test_fixture_intermediate_or_leaf_symlink_fails(self):
        source = ROOT / "docs/contracts/fixtures/capability-registry-v1.json"
        for intermediate in (False, True):
            with self.subTest(intermediate=intermediate), tempfile.TemporaryDirectory() as directory:
                root, external = Path(directory), Path(directory) / "external"
                external.mkdir()
                (external / "capability-registry-v1.json").write_bytes(source.read_bytes())
                parent = root / "docs/contracts"
                parent.mkdir(parents=True)
                if intermediate:
                    (parent / "fixtures").symlink_to(external, target_is_directory=True)
                else:
                    (parent / "fixtures").mkdir()
                    (parent / "fixtures/capability-registry-v1.json").symlink_to(
                        external / "capability-registry-v1.json")
                with self.assertRaises(ContractError):
                    load_fixture(root)

    def test_cli_validate_resolve_golden_and_usage(self):
        fixture = load_fixture(ROOT)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            registry = root / "registry.json"
            request = root / "request.json"
            registry.write_bytes(canonical_json(fixture["registry"]))
            request.write_bytes(canonical_json(fixture["requests"]["resolved_exact"]))
            result = self._run("validate", "--registry", str(registry))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, registry.read_bytes() + b"\n")
            result = self._run("resolve", "--request", str(request),
                               "--registry", "-", stdin=registry.read_bytes())
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, canonical_json(
                fixture["assessments"]["resolved_exact"]) + b"\n")
        if (ROOT / "forge-core/internal/goimpactprescan").is_dir():
            golden = self._run("--golden", str(ROOT))
            self.assertEqual(golden.returncode, 0, golden.stderr)
        for arguments in ((), ("validate",),
                          ("resolve", "--registry", "-", "--request", "-"),
                          ("resolve", "--registry", "a", "--request", "b"),
                          ("validate", "--registry", "a", "--registry", "b"),
                          ("unknown",), ("validate", "--registry", "x", "extra", "y")):
            result = self._run(*arguments)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")
        invalid = self._run("validate", "--registry", "-", stdin=b"{}")
        self.assertEqual(invalid.returncode, 1)
        self.assertEqual(invalid.stdout, b"")

    @unittest.skipUnless(sys.platform.startswith("linux"), "requires Linux O_NONBLOCK pipe")
    def test_nonblocking_stdin_requires_explicit_eof(self):
        import fcntl
        raw = canonical_json(load_fixture(ROOT)["registry"])
        read_fd, write_fd = os.pipe()
        try:
            capacity = fcntl.fcntl(read_fd, fcntl.F_GETPIPE_SZ)
            if capacity < len(raw):
                fcntl.fcntl(read_fd, fcntl.F_SETPIPE_SZ, len(raw))
            os.set_blocking(read_fd, False)
            self.assertEqual(os.write(write_fd, raw), len(raw))
            result = subprocess.run(
                [sys.executable, "-B", "harness/capability_registry_contract/check.py",
                 "validate", "--registry", "-"],
                cwd=ROOT, stdin=read_fd, capture_output=True, check=False, timeout=5,
            )
        finally:
            os.close(read_fd)
            os.close(write_fd)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_stdin_reader_rejects_incomplete_nonbytes_and_n_plus_one(self):
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
                    mock.patch.object(check_cli.sys, "stdin", stream), \
                    self.assertRaises(ContractError):
                check_cli._stdin(4, "registry")
            self.assertEqual(stream.reads, 1)


class FilesystemAdversarialTests(unittest.TestCase):
    def test_root_and_leaf_symlinks_and_special_files_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            real = root / "real"
            real.mkdir()
            (real / "pkg").mkdir()
            (real / "pkg/file.py").write_text("ok")
            (root / "root-link").symlink_to(real, target_is_directory=True)
            with self.assertRaises(ContractError):
                stable_root(root / "root-link")
            (real / "pkg/link.py").symlink_to(real / "pkg/file.py")
            with self.assertRaises(ContractError):
                scan_regular(real, "pkg", (".py",))
            (real / "pkg/link.py").unlink()
            if hasattr(os, "mkfifo"):
                os.mkfifo(real / "pkg/pipe")
                with self.assertRaises(ContractError):
                    scan_regular(real, "pkg", (".py",))

    def test_inventory_snapshot_rejects_scan_to_read_swap(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "pkg").mkdir()
            path = root / "pkg/file.py"
            path.write_text("one")
            _, snapshots = scan_regular(root, "pkg", (".py",))
            replacement = root / "replacement"
            replacement.write_text("two")
            os.replace(replacement, path)
            with self.assertRaisesRegex(ContractError, "changed during operation"):
                read_regular(root, "pkg/file.py", 8, snapshots)

    def test_operation_guard_rejects_ancestor_swap_between_reads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "pkg").mkdir()
            (root / "pkg/a.py").write_text("a")
            anchored, root_identity = stable_root(root)
            guard = guard_root(anchored, root_identity)
            self.assertEqual(read_regular(anchored, "pkg/a.py", 8, guard), b"a")
            (root / "replacement").mkdir()
            (root / "replacement/a.py").write_text("a")
            os.rename(root / "pkg", root / "old")
            os.rename(root / "replacement", root / "pkg")
            with self.assertRaisesRegex(ContractError, "changed during operation"):
                read_regular(anchored, "pkg/a.py", 8, guard)

    def test_operation_guard_rejects_root_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            root = parent / "repo"
            root.mkdir()
            (root / "file.py").write_text("a")
            anchored, root_identity = stable_root(root)
            guard = guard_root(anchored, root_identity)
            os.rename(root, parent / "old")
            root.mkdir()
            (root / "file.py").write_text("a")
            with self.assertRaisesRegex(ContractError, "changed during operation"):
                read_regular(anchored, "file.py", 8, guard)

    def test_pending_nested_directory_identity_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            child = root / "pkg/nested"
            child.mkdir(parents=True)
            (child / "file.py").write_text("a")
            original, child_calls = Path.lstat, 0

            def changed_lstat(path):
                nonlocal child_calls
                metadata = original(path)
                if path == child:
                    child_calls += 1
                    if child_calls == 2:
                        values = {name: getattr(metadata, name) for name in (
                            "st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")}
                        values["st_ino"] += 1
                        return type("ChangedStat", (), values)()
                return metadata

            with mock.patch.object(Path, "lstat", changed_lstat), \
                    self.assertRaisesRegex(ContractError, "content-set directory changed"):
                scan_regular(root, "pkg", (".py",))

    def test_physical_walk_exact_boundary(self):
        self.assertEqual(_visit(MAX_VISITED_PATHS - 1), MAX_VISITED_PATHS)
        with self.assertRaisesRegex(ContractError, "65536"):
            _visit(MAX_VISITED_PATHS)

    def test_builder_rejects_too_many_matches_before_content_reads(self):
        paths = [f"pkg/{index}.go" for index in range(257)]
        with mock.patch("capability_registry_contract.builder.scan_regular",
                        return_value=(paths, {})), \
                mock.patch("capability_registry_contract.builder.read_regular") as read:
            with self.assertRaisesRegex(ValueError, "exceeds 256"):
                _recursive_set(Path("/repo"), "forge-core/internal/goimpactprescan",
                               ".go", "text/x-go", {})
        read.assert_not_called()


if __name__ == "__main__":
    unittest.main()
