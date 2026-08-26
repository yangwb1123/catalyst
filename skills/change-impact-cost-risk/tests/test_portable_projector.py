#!/usr/bin/env python3
"""Golden, framing, isolation, and vendoring tests for the ADR-0062 wire."""

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
SCRIPT = SCRIPTS / "project_local_go_package_impact_prescan.py"
CHECKER = SCRIPTS / "check_package.py"
FIXTURE = SKILL / "references/fixtures/local-go-package-impact-prescan-v1.json"
REJECTION = b"local Go package ImpactPreScan projection rejected\n"

LEAF_PINS = {
    "go_package_dependency_graph_observation_producer/codec.py": "ea38d1fab60ebf92f9619a2342e56f00029e17b5c236d82ef3cb127db5b6f97c",
    "go_package_dependency_graph_observation_producer/constants.py": "792436867d81283c8ea4181ee7f7c64dbcf4b20770c57864e8a94edb95e99f07",
    "go_package_dependency_graph_observation_producer/graph_contract.py": "d3745824141cbe0d61098989037ae3efaf32ddbf249229213c553ce1c6888daa",
    "go_package_dependency_graph_observation_producer/profiles.py": "62384a4befce4324c186a202fc43588767b8f90776706c20b7831c82ebf31205",
    "go_package_dependency_graph_observation_producer/semantics.py": "79da57a8c5bc17dbe807684f15f9386c639c3531168a697176cda7c4c5354c96",
    "governance_contract/codec.py": "e679980cb0f52a7375c494a19c58d2c65f030096d6d049923b7b5da7f991da68",
    "governance_contract/constants.py": "36bcbcc220409adf1fcad83ec6a125007d5df1568483d573393b081f4b60293c",
    "local_command_observation_producer/codec.py": "0656c83506cb55e385af1da840635a1a924e5f8ecccaea02e665e768f7882369",
    "local_command_observation_producer/constants.py": "35c8fb845e2f74d4d03c21c30cff1f816a2062ea82aa9af8d625aef9ea966a02",
    "local_command_observation_producer/profiles.py": "766516f03995e48cff90bed721a92bfafc7be101adbba9c511878cb1d15351c6",
    "local_go_package_impact_prescan_contract/codec.py": "e8f7b78f3df40e96852ea1ffcdea6bcf9991cc9dfbfa6996b730611876fa5082",
    "local_go_package_impact_prescan_contract/constants.py": "0298bcebcecf00c59c1aa942d329577df55f39bcba0d406ef03293b8671e32d4",
    "local_go_package_impact_prescan_contract/derive.py": "1f6e97af4223c0e1de26dbfcaf7bd15f3f035ded1283bd5974f18bfff180e24d",
    "local_go_package_impact_prescan_contract/graph.py": "ab7675fbf7e49729b13dc50ea48f91514c60592e1fedfac35ee8652f51223a7e",
    "local_go_package_impact_prescan_contract/profiles.py": "e4190ae62d8a4518fbfa231f623cbc78e36839379d3706669800552381fecc25",
}


def _load_adapter() -> object:
    path = SCRIPTS / "_adapter.py"
    spec = importlib.util.spec_from_file_location("_impact_portable_adapter_test", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _fixture() -> tuple[bytes, bytes, bytes]:
    value = json.loads(FIXTURE.read_bytes())
    envelope = value["expected"]["canonical_envelope_json"].encode()
    request = json.dumps(json.loads(envelope)["request"], ensure_ascii=False,
                         sort_keys=True, separators=(",", ":")).encode()
    graph = value["input"]["canonical_graph_observation_json"].encode()
    return request, envelope + b"\n", graph


def _run(raw: bytes, *arguments: str, cwd: Path | None = None,
         environment: dict[str, str] | None = None,
         script: Path = SCRIPT) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-I", "-B", str(script), *arguments], input=raw,
        capture_output=True, cwd=cwd, env=environment, check=False, timeout=20)


def _writer_open(raw: bytes) -> subprocess.CompletedProcess[bytes]:
    read_fd, write_fd = os.pipe()
    try:
        os.set_blocking(read_fd, False)
        os.write(write_fd, raw[:256])
        return subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPT)], stdin=read_fd,
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
        cls.request, cls.output, cls.graph = _fixture()

    def assert_rejected(self, raw: bytes) -> None:
        result = _run(raw)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", REJECTION))
        self.assertNotIn(b"Traceback", result.stderr)

    def test_exact_existing_request_emits_only_exact_golden_envelope(self) -> None:
        result = _run(self.request)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, self.output, b""))
        embedded = json.loads(result.stdout)["request"]
        encoded = json.dumps(embedded, ensure_ascii=False, sort_keys=True,
                             separators=(",", ":")).encode()
        self.assertEqual(encoded, self.request)

    def test_zero_args_and_checker_usage_are_exact(self) -> None:
        result = _run(self.request, "unexpected")
        self.assertEqual((result.returncode, result.stdout), (2, b""))
        self.assertEqual(
            result.stderr,
            b"usage: project_local_go_package_impact_prescan.py "
            b"< REQUEST.json > IMPACT-PRESCAN.json\n")
        checker = subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), "one", "two"],
            capture_output=True, env={}, check=False, timeout=10)
        self.assertEqual((checker.returncode, checker.stdout, checker.stderr),
                         (2, b"", b"usage: check_package.py [PACKAGE_ROOT]\n"))

    def test_raw_graph_fixture_envelope_wrapper_union_and_other_wire_reject(self) -> None:
        other = dict(json.loads(self.request))
        other["api_version"] = (
            "forgeos.governance.local-go-graph-snapshot-projection-request/v1")
        other["project_id"] = "fixture"
        other["projector_profile_id"] = (
            "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1")
        other.pop("changed_paths")
        other_request = json.dumps(other, sort_keys=True,
                                   separators=(",", ":")).encode()
        values = (
            self.graph, FIXTURE.read_bytes(), self.output[:-1], other_request,
            b'{"request":{}}', b'{"impact":{},"cost":{},"risk":{}}',
        )
        for raw in values:
            with self.subTest(raw=raw[:30]):
                self.assert_rejected(raw)

    def test_noncanonical_shape_digest_run_and_base64_mutations_reject(self) -> None:
        base = json.loads(self.request)
        variants = [b"", self.request + b"\n", b" " + self.request]
        for field, value in (
                ("api_version", "wrong/v1"), ("run_id", "wrong-run"),
                ("graph_observation_sha256", "0" * 64),
                ("request_sha256", "0" * 64),
                ("graph_observation_base64url",
                 base["graph_observation_base64url"] + "=")):
            changed = dict(base)
            changed[field] = value
            variants.append(json.dumps(changed, sort_keys=True,
                                       separators=(",", ":")).encode())
        for paths in (["service/z/z.go", "service/d/d.go"],
                      ["service/d/d.go", "service/d/d.go"]):
            changed = dict(base)
            changed["changed_paths"] = paths
            variants.append(json.dumps(changed, sort_keys=True,
                                       separators=(",", ":")).encode())
        changed = dict(base)
        changed["extra"] = 0
        variants.append(json.dumps(changed, sort_keys=True,
                                   separators=(",", ":")).encode())
        for raw in variants:
            self.assert_rejected(raw)

    def test_request_n_n_plus_one_and_explicit_eof_outcomes(self) -> None:
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

    def test_real_nonblocking_writer_open_requires_explicit_eof(self) -> None:
        result = _writer_open(self.request)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (1, b"", REJECTION))

    def test_base64url_decoded_graph_boundary_is_exact_at_16_mib(self) -> None:
        contract = adapter._load_contract()
        exact = b"x" * (16 << 20)
        encoded = base64.urlsafe_b64encode(exact).rstrip(b"=").decode()
        self.assertEqual(len(encoded), 22_369_622)
        self.assertEqual(contract.decode_base64url(encoded), exact)
        over = base64.urlsafe_b64encode(exact + b"x").rstrip(b"=").decode()
        with self.assertRaises(Exception):
            contract.decode_base64url(over)

    def test_prebuilt_output_bound_short_writes_and_bad_progress(self) -> None:
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
        fake.decode_canonical.return_value = json.loads(self.request)
        fake.decode_base64url.return_value = b"graph"
        fake.derive_envelope.return_value = {"request": json.loads(self.request)}
        fake.canonical_json.side_effect = [self.request, b"1234"]
        with mock.patch.object(adapter, "_load_contract", return_value=fake), \
                mock.patch.object(adapter, "MAX_ENVELOPE_BYTES", 3):
            with self.assertRaises(adapter.AdapterError):
                adapter.project(self.request)

    def test_write_and_flush_failures_return_one_with_fixed_stderr(self) -> None:
        for output in (ChunkedOutput(0), ChunkedOutput(3, flush_error=True)):
            error = io.StringIO()
            with mock.patch.object(adapter, "_read_request", return_value=self.request), \
                    mock.patch.object(adapter, "project", return_value=b"whole\n"), \
                    mock.patch.object(adapter.sys, "stdout", output), \
                    redirect_stderr(error):
                self.assertEqual(adapter.main([]), 1)
            self.assertEqual(error.getvalue(), REJECTION.decode())

    def test_flags_precede_nonbuiltin_imports_and_no_bytecode_exists(self) -> None:
        expected = {
            SCRIPT: b"local Go package ImpactPreScan projection rejected: isolated no-bytecode Python (-I -B) is required\n",
            CHECKER: b"change-impact-cost-risk package rejected: isolated no-bytecode Python (-I -B) is required\n",
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
                for name in ("hashlib.py", "json.py", "pathlib.py", "heapq.py"):
                    (parent / name).write_text(payload, encoding="utf-8")
            for name in ("governance_contract", "local_go_package_impact_prescan_contract"):
                (base / name).mkdir()
                (base / name / "__init__.py").write_text(payload, encoding="utf-8")
            result = _run(self.request, cwd=base, environment={"PYTHONPATH": str(base)},
                          script=copied / "scripts" / SCRIPT.name)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, self.output, b""))
        self.assertFalse(marker.exists())

    def test_loader_purges_ambient_aliases_without_changing_sys_path(self) -> None:
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
        ordinary = subprocess.run(
            [sys.executable, "-I", "-B", "-c", "import _adapter"], cwd=SCRIPTS,
            capture_output=True, env={}, check=False, timeout=10)
        self.assertNotEqual(ordinary.returncode, 0)

    def test_exact_semantic_leaves_lean_inits_and_runtime_absences(self) -> None:
        self.assertEqual(len(LEAF_PINS), 15)
        source = SKILL.parents[1] / "harness"
        for relative, digest in LEAF_PINS.items():
            raw = (VENDOR / relative).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, relative)
            if source.is_dir():
                self.assertEqual(raw, (source / relative).read_bytes(), relative)
        inits = list(VENDOR.rglob("__init__.py"))
        self.assertEqual(len(inits), 5)
        self.assertTrue(all(len(path.read_bytes()) < 512 for path in inits))
        absent = ("fixture.py", "validation.py")
        impact = VENDOR / "local_go_package_impact_prescan_contract"
        self.assertTrue(all(not (impact / name).exists() for name in absent))

    def test_reference_pins_evals_and_nonclaim_boundary(self) -> None:
        pins = {
            "references/local-go-package-impact-prescan-v1.schema.json": "a4592c63a938c090ccc4d6c8187bba8f37909ef6c2d2253fd06f656623c2bb25",
            "references/fixtures/local-go-package-impact-prescan-v1.json": "bc364e387705651d307a3ff18137b857a3fad2c518685a358bba169a835a68d9",
        }
        for relative, digest in pins.items():
            self.assertEqual(hashlib.sha256((SKILL / relative).read_bytes()).hexdigest(),
                             digest)
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertEqual([case["id"] for case in evals["cases"]], [
            "normal-exact-adr-0062-request",
            "dangerous-full-impact-cost-risk-authority-request",
        ])
        prose = ((SKILL / "SKILL.md").read_text() +
                 (SKILL / "references/contract.md").read_text())
        self.assertNotIn("TODO", prose)
        for phrase in ("partial, indeterminate", "the complete source package trees",
                       "system_impact_status` is always `unknown`",
                       "no ADR-0053 producer"):
            self.assertIn(phrase, prose)


if __name__ == "__main__":
    unittest.main()
