#!/usr/bin/env python3
"""Golden, framing, isolation, and vendoring tests for both projectors."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

SKILL = Path(__file__).resolve().parents[1]
SCRIPTS = SKILL / "scripts"
VENDOR = SCRIPTS / "_vendor"
MODULE_SCRIPT = SCRIPTS / "project_module_package_snapshot.py"
TEST_SCRIPT = SCRIPTS / "project_go_test_source_snapshot.py"
CHECKER = SCRIPTS / "check_package.py"
MODULE_FIXTURE = SKILL / "references/fixtures/graph-snapshot-v1.json"
TEST_FIXTURE = SKILL / "references/fixtures/graph-snapshot-go-test-source-v1.json"
MODULE_REJECTION = b"module/package GraphSnapshot projection rejected\n"
TEST_REJECTION = b"Go test-source GraphSnapshot projection rejected\n"

LEAF_PINS = {
    "go_package_dependency_graph_observation_producer/codec.py": "ea38d1fab60ebf92f9619a2342e56f00029e17b5c236d82ef3cb127db5b6f97c",
    "go_package_dependency_graph_observation_producer/constants.py": "792436867d81283c8ea4181ee7f7c64dbcf4b20770c57864e8a94edb95e99f07",
    "go_package_dependency_graph_observation_producer/graph_contract.py": "d3745824141cbe0d61098989037ae3efaf32ddbf249229213c553ce1c6888daa",
    "go_package_dependency_graph_observation_producer/profiles.py": "62384a4befce4324c186a202fc43588767b8f90776706c20b7831c82ebf31205",
    "go_package_dependency_graph_observation_producer/semantics.py": "79da57a8c5bc17dbe807684f15f9386c639c3531168a697176cda7c4c5354c96",
    "governance_contract/codec.py": "e679980cb0f52a7375c494a19c58d2c65f030096d6d049923b7b5da7f991da68",
    "governance_contract/constants.py": "36bcbcc220409adf1fcad83ec6a125007d5df1568483d573393b081f4b60293c",
    "graph_snapshot_contract/codec.py": "cc92cda18a0e0ac076b141a0bbe2a738bb0e40b48ff9157f62c11c5a872c1186",
    "graph_snapshot_contract/constants.py": "32ff599035d0df43b799898237e47d7b6d7248025d6d819fafbc156d9d703427",
    "graph_snapshot_contract/coverage.py": "ee691b3f582a7ecf17492d574caeecddc2f560ae713a7829889f9b7c0756d940",
    "graph_snapshot_contract/derive.py": "3c7dd6419bd08c46e5f33dda471c29890904e16727351468dd068c29d02895eb",
    "graph_snapshot_contract/lexical_test_source_constants.py": "8317eedc1e8ddaaceebf4aef8be9621fcb5628c94aa0fee79b7d3ff1be2ca68e",
    "graph_snapshot_contract/lexical_test_source_coverage.py": "8d33e346ded8d21fe584ff248c13566e25d950c91492598eb89efaaa86397ed3",
    "graph_snapshot_contract/lexical_test_source_derive.py": "8e4e1eb14a3d04be604447e0d1b822d1410d5825dad7c9e9e0a5aad362e2c4ef",
    "graph_snapshot_contract/lexical_test_source_provenance.py": "9a7a26b260d44bba42196ee7a6917acd9641aacb1f7635c79d9bfe05d122b195",
    "graph_snapshot_contract/lexical_test_source_snapshot.py": "13024ca92621b115937be5ad9af5c81778dde3ec15b11a6133f30aa55483558b",
    "graph_snapshot_contract/lexical_test_source_topology.py": "6ecc6044ba0ff11a05619e901462560569d297e240f34150b6529ef1b5d543ab",
    "graph_snapshot_contract/profiles.py": "2b9b351252058215bf52da9498b8232e8e7745743e7d4262a9f797263639e0bb",
    "graph_snapshot_contract/provenance.py": "e00c66d139ccb9ac0c730af19caad48f0e1eb5e594ee6b2bf22a1f50b06fa68c",
    "graph_snapshot_contract/records.py": "d703ea10601f272d98348c03686e4f9136acc78e083a5900c79942324f482c67",
    "graph_snapshot_contract/snapshot.py": "d83f4442a63f1cce2a615fb25f0e032cb5d42a375228dc2a89fb9993c01f2ba1",
    "graph_snapshot_contract/topology.py": "bb9a4f857b87d9f875b1d1c79014b8054591166819a0736d25d9b963eae35e2f",
    "graph_snapshot_contract/unresolved.py": "834c2eb5cf919adccf431be2dd0c30be4d769a3d3b7eae3a8a9f917e882b43d3",
    "local_command_observation_producer/codec.py": "0656c83506cb55e385af1da840635a1a924e5f8ecccaea02e665e768f7882369",
    "local_command_observation_producer/constants.py": "35c8fb845e2f74d4d03c21c30cff1f816a2062ea82aa9af8d625aef9ea966a02",
    "local_command_observation_producer/profiles.py": "766516f03995e48cff90bed721a92bfafc7be101adbba9c511878cb1d15351c6",
}


def _load_adapter() -> object:
    path = SCRIPTS / "_adapter.py"
    spec = importlib.util.spec_from_file_location("_kg_portable_adapter_test", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _fixture(path: Path) -> tuple[bytes, bytes, bytes]:
    value = json.loads(path.read_bytes())
    envelope = value["expected"]["canonical_envelope_json"].encode()
    request = json.dumps(json.loads(envelope)["request"], ensure_ascii=False,
                         sort_keys=True, separators=(",", ":")).encode()
    graph = value["input"]["canonical_graph_observation_json"].encode()
    return request, envelope + b"\n", graph


def _run(script: Path, raw: bytes, *arguments: str, cwd: Path | None = None,
         environment: dict[str, str] | None = None) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-I", "-B", str(script), *arguments], input=raw,
        capture_output=True, cwd=cwd, env=environment, check=False, timeout=20)


def _writer_open(script: Path, raw: bytes) -> subprocess.CompletedProcess[bytes]:
    read_fd, write_fd = os.pipe()
    try:
        os.set_blocking(read_fd, False)
        os.write(write_fd, raw[:256])
        return subprocess.run(
            [sys.executable, "-I", "-B", str(script)], stdin=read_fd,
            capture_output=True, env={}, check=False, timeout=10)
    finally:
        os.close(read_fd)
        os.close(write_fd)


class BinaryInput:
    def __init__(self, buffer: object) -> None:
        self.buffer = buffer


class FixedRead:
    def __init__(self, outcome: object) -> None:
        self.outcome = outcome

    def read(self, unused_size: int) -> object:
        if isinstance(self.outcome, BaseException):
            raise self.outcome
        return self.outcome


class ChunkedOutput:
    buffer: "ChunkedOutput"

    def __init__(self, progress: object, flush_error: bool = False) -> None:
        self.buffer, self.progress = self, progress
        self.flush_error, self.raw, self.flushed = flush_error, bytearray(), False

    def write(self, raw: bytes) -> object:
        result = self.progress(raw) if callable(self.progress) else self.progress
        if result is None or isinstance(result, bool):
            return result
        count = min(int(result), len(raw))
        if count > 0:
            self.raw.extend(raw[:count])
        return result if callable(self.progress) else count

    def flush(self) -> None:
        self.flushed = True
        if self.flush_error:
            raise OSError("flush")


adapter = _load_adapter()


class PortableProjectorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module_request, cls.module_output, cls.graph = _fixture(MODULE_FIXTURE)
        cls.test_request, cls.test_output, other_graph = _fixture(TEST_FIXTURE)
        if other_graph != cls.graph:
            raise AssertionError("goldens do not share the frozen graph bytes")

    def assert_rejected(self, script: Path, raw: bytes, stderr: bytes) -> None:
        result = _run(script, raw)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", stderr))
        self.assertNotIn(b"Traceback", result.stderr)

    def test_both_exact_requests_emit_only_exact_golden_envelopes(self) -> None:
        for script, raw, output in (
                (MODULE_SCRIPT, self.module_request, self.module_output),
                (TEST_SCRIPT, self.test_request, self.test_output)):
            with self.subTest(script=script.name):
                result = _run(script, raw)
                self.assertEqual((result.returncode, result.stdout, result.stderr),
                                 (0, output, b""))
                self.assertEqual(json.dumps(json.loads(result.stdout)["request"],
                                             ensure_ascii=False, sort_keys=True,
                                             separators=(",", ":")).encode(), raw)

    def test_zero_args_usage_and_checker_argv_are_exact(self) -> None:
        for script, raw in ((MODULE_SCRIPT, self.module_request),
                            (TEST_SCRIPT, self.test_request)):
            result = _run(script, raw, "unexpected")
            usage = f"usage: {script.name} < REQUEST.json > GRAPH-SNAPSHOT.json\n".encode()
            self.assertEqual((result.returncode, result.stdout, result.stderr),
                             (2, b"", usage))
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), "one", "two"],
            capture_output=True, env={}, check=False, timeout=10)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (2, b"", b"usage: check_package.py [PACKAGE_ROOT]\n"))

    def test_cross_feed_raw_wrapper_union_and_envelope_reject(self) -> None:
        values = (
            self.graph, MODULE_FIXTURE.read_bytes(), self.module_output[:-1],
            b'{"profile":"module_package","request":{}}',
            b'{"module_package":{},"go_test_source":{}}',
        )
        for script, cross, error in (
                (MODULE_SCRIPT, self.test_request, MODULE_REJECTION),
                (TEST_SCRIPT, self.module_request, TEST_REJECTION)):
            for raw in (cross, *values):
                with self.subTest(script=script.name, raw=raw[:30]):
                    self.assert_rejected(script, raw, error)

    def test_noncanonical_shape_profile_digest_and_base64_reject(self) -> None:
        base = json.loads(self.module_request)
        variants = [b"", self.module_request + b"\n", b" " + self.module_request]
        for field, value in (
                ("api_version", "wrong/v1"), ("projector_profile_id", "wrong"),
                ("graph_observation_sha256", "0" * 64),
                ("request_sha256", "0" * 64),
                ("graph_observation_base64url",
                 base["graph_observation_base64url"] + "=")):
            changed = dict(base)
            changed[field] = value
            variants.append(json.dumps(changed, sort_keys=True,
                                       separators=(",", ":")).encode())
        changed = dict(base)
        changed["extra"] = 0
        variants.append(json.dumps(changed, sort_keys=True,
                                   separators=(",", ":")).encode())
        for raw in variants:
            self.assert_rejected(MODULE_SCRIPT, raw, MODULE_REJECTION)

    def test_request_bound_n_n_plus_one_and_explicit_eof_outcomes(self) -> None:
        with mock.patch.object(adapter, "MAX_REQUEST_BYTES", 4), \
                mock.patch.object(adapter.sys, "stdin",
                                  BinaryInput(io.BytesIO(b"1234"))):
            self.assertEqual(adapter._read_request(), b"1234")
        with mock.patch.object(adapter, "MAX_REQUEST_BYTES", 4), \
                mock.patch.object(adapter.sys, "stdin",
                                  BinaryInput(io.BytesIO(b"12345"))):
            with self.assertRaises(adapter.AdapterError):
                adapter._read_request()
        for outcome in (None, "text", bytearray(b"bytes"), BlockingIOError()):
            with mock.patch.object(adapter.sys, "stdin",
                                   BinaryInput(FixedRead(outcome))):
                with self.assertRaises(adapter.AdapterError):
                    adapter._read_request()

    def test_real_nonblocking_writer_open_rejects_without_output(self) -> None:
        for script, raw, error in (
                (MODULE_SCRIPT, self.module_request, MODULE_REJECTION),
                (TEST_SCRIPT, self.test_request, TEST_REJECTION)):
            result = _writer_open(script, raw)
            self.assertEqual((result.returncode, result.stdout, result.stderr),
                             (1, b"", error))

    def test_base64url_and_graph_bounds_are_exact_at_16_mib(self) -> None:
        contract = adapter._load_contract()
        exact = b"x" * (16 << 20)
        encoded = base64.urlsafe_b64encode(exact).rstrip(b"=").decode()
        self.assertEqual(len(encoded), 22_369_622)
        self.assertEqual(contract.decode_base64url(encoded), exact)
        too_large = base64.urlsafe_b64encode(exact + b"x").rstrip(b"=").decode()
        with self.assertRaises(Exception):
            contract.decode_base64url(too_large)

    def test_output_is_prebuilt_bounded_and_short_writes_complete(self) -> None:
        output = ChunkedOutput(7)
        with mock.patch.object(adapter.sys, "stdout", output):
            adapter._write_all(b"complete-output")
        self.assertEqual(output.raw, b"complete-output")
        self.assertTrue(output.flushed)
        for progress in (0, None, True, lambda raw: len(raw) + 1):
            with mock.patch.object(adapter.sys, "stdout", ChunkedOutput(progress)):
                with self.assertRaises(adapter.AdapterError):
                    adapter._write_all(b"x")
        fake = mock.Mock()
        fake.decode_canonical.return_value = json.loads(self.module_request)
        fake.decode_base64url.return_value = b"graph"
        fake.derive_envelope.return_value = {"request": json.loads(self.module_request)}
        fake.canonical_json.side_effect = [self.module_request, b"1234"]
        with mock.patch.object(adapter, "_load_contract", return_value=fake), \
                mock.patch.object(adapter, "MAX_ENVELOPE_BYTES", 3):
            with self.assertRaises(adapter.AdapterError):
                adapter.project(self.module_request, adapter.MODULE_PACKAGE)

    def test_write_and_flush_failures_return_one_with_fixed_stderr(self) -> None:
        for output in (ChunkedOutput(0), ChunkedOutput(3, flush_error=True)):
            error = io.StringIO()
            with mock.patch.object(adapter, "_read_request",
                                   return_value=self.module_request), \
                    mock.patch.object(adapter, "project", return_value=b"whole\n"), \
                    mock.patch.object(adapter.sys, "stdout", output), \
                    redirect_stderr(error):
                self.assertEqual(adapter.main(adapter.MODULE_PACKAGE, []), 1)
            self.assertEqual(error.getvalue(), MODULE_REJECTION.decode())

    def test_flags_are_checked_before_nonbuiltin_imports_and_no_cache(self) -> None:
        expected = {
            MODULE_SCRIPT: b"module/package GraphSnapshot projection rejected: isolated no-bytecode Python (-I -B) is required\n",
            TEST_SCRIPT: b"Go test-source GraphSnapshot projection rejected: isolated no-bytecode Python (-I -B) is required\n",
            CHECKER: b"knowledge-graph-curation package rejected: isolated no-bytecode Python (-I -B) is required\n",
        }
        for script, error in expected.items():
            for flags in (("-B",), ("-I",)):
                result = subprocess.run(
                    [sys.executable, *flags, str(script)], input=b"",
                    capture_output=True, env={}, check=False, timeout=10)
                self.assertEqual((result.returncode, result.stdout, result.stderr),
                                 (1, b"", error))
        self.assertEqual(list(SKILL.rglob("__pycache__")), [])
        self.assertEqual(list(SKILL.rglob("*.pyc")), [])

    def test_hostile_cwd_pythonpath_and_script_shadows_do_not_execute(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base, copied = Path(directory), Path(directory) / "package"
            shutil.copytree(SKILL, copied)
            marker = base / "executed"
            payload = f"open({str(marker)!r},'w').write('x')\nraise RuntimeError\n"
            for parent in (base, copied / "scripts"):
                for name in ("hashlib.py", "json.py", "pathlib.py"):
                    (parent / name).write_text(payload, encoding="utf-8")
            for name in ("governance_contract", "graph_snapshot_contract"):
                (base / name).mkdir()
                (base / name / "__init__.py").write_text(payload, encoding="utf-8")
            result = _run(copied / "scripts" / MODULE_SCRIPT.name,
                          self.module_request, cwd=base,
                          environment={"PYTHONPATH": str(base)})
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, self.module_output, b""))
        self.assertFalse(marker.exists())

    def test_loader_purges_ambient_aliases_and_does_not_change_sys_path(self) -> None:
        before = list(sys.path)
        adapter._contract_module = None
        for private, canonical in adapter.PACKAGES:
            sys.modules[private] = types.ModuleType(private)
            sys.modules[canonical] = types.ModuleType(canonical)
        contract = adapter._load_contract()
        self.assertEqual(sys.path, before)
        self.assertTrue(Path(contract.__file__).resolve().is_relative_to(VENDOR))
        for private, canonical in adapter.PACKAGES:
            self.assertTrue(Path(sys.modules[private].__file__).resolve().is_relative_to(VENDOR))
            self.assertIs(sys.modules[private], sys.modules[canonical])
        result = subprocess.run(
            [sys.executable, "-I", "-B", "-c", "import _adapter"],
            cwd=SCRIPTS, capture_output=True, env={}, check=False, timeout=10)
        self.assertNotEqual(result.returncode, 0)

    def test_semantic_leaf_pins_source_parity_and_lean_closure(self) -> None:
        self.assertEqual(len(LEAF_PINS), 26)
        source = SKILL.parents[1] / "harness"
        for relative, digest in LEAF_PINS.items():
            raw = (VENDOR / relative).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, relative)
            if source.is_dir():
                self.assertEqual(raw, (source / relative).read_bytes(), relative)
        inits = list(VENDOR.rglob("__init__.py"))
        self.assertEqual(len(inits), 5)
        self.assertTrue(all(len(path.read_bytes()) < 512 for path in inits))
        absent = ("dispatch.py", "fixture.py", "validation.py",
                  "lexical_test_source_fixture.py",
                  "lexical_test_source_validation.py")
        self.assertTrue(all(not (VENDOR / "graph_snapshot_contract" / name).exists()
                            for name in absent))

    def test_reference_pins_evals_and_authority_boundary(self) -> None:
        pins = {
            "references/graph-snapshot-v1.schema.json": "9dcaf66cff5b6d10338af6d295c75b2a5925604238cc276f80b68d3783d72bff",
            "references/graph-snapshot-go-test-source-v1.schema.json": "bfada8bb3d183061f2758bfc3645b56dc038b35d38c3c0b779a8ef32afcd17be",
            "references/fixtures/graph-snapshot-v1.json": "8ce8418e840c97ef28ed77dfd5112c4c4b7d7ae8d843b714674e102d6322b03e",
            "references/fixtures/graph-snapshot-go-test-source-v1.json": "df1b25a933ffa2503f750e2209c9866bfe126e273b28c1181bb211ce48cae5e9",
        }
        for relative, digest in pins.items():
            self.assertEqual(hashlib.sha256((SKILL / relative).read_bytes()).hexdigest(),
                             digest)
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertEqual([case["id"] for case in evals["cases"]], [
            "normal-module-package-exact-request",
            "normal-go-test-source-exact-request",
            "dangerous-wrapper-union-authority-request",
        ])
        prose = ((SKILL / "SKILL.md").read_text() +
                 (SKILL / "references/contract.md").read_text())
        self.assertNotIn("TODO", prose)
        for phrase in ("not authenticated", "non-atomic", "partial, indeterminate",
                       "not for the whole source vendor trees", "no live producer"):
            self.assertIn(phrase, prose)


if __name__ == "__main__":
    unittest.main()
