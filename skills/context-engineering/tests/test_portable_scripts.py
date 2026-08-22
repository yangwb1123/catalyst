#!/usr/bin/env python3
"""Executable and integrity tests for the context-engineering Skill."""
from __future__ import annotations
import copy, hashlib, io, json, os
import shutil, subprocess, sys, tempfile, unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

SKILL = Path(__file__).resolve().parents[1]
SCRIPTS = SKILL / "scripts"
MINI_SKILL = (b"---\nname: context-engineering\n---\n"
              b"[references/contract.md](references/contract.md)\n"
              b"[references/evals.json](references/evals.json)\n")
sys.path.insert(0, str(SCRIPTS))

import assemble  # noqa: E402
import check_package  # noqa: E402

class AssemblyAdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        fixture = SKILL / "references/fixtures/context-package-v1.json"
        cls.golden = json.loads(fixture.read_bytes())
        cls.request = assemble.canonical_json(cls.golden["request"])
        cls.package = assemble.canonical_json(cls.golden["expected_package"])
        cls.command = [sys.executable, "-I", "-B", str(SCRIPTS / "assemble.py")]

    def _run(self, raw: bytes, *arguments: str,
             cwd: Path | None = None, environment: dict[str, str] | None = None
             ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            self.command + list(arguments), input=raw, capture_output=True,
            cwd=cwd, env=environment, check=False, timeout=10,
        )

    def test_golden_request_emits_exact_revalidated_package(self) -> None:
        result = self._run(self.request)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, self.package + b"\n")
        self.assertEqual(result.stderr, b"")
        self.assertNotIn(b"SECRET", result.stdout)

    def test_adapter_is_independent_of_cwd_and_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self._run(self.request, cwd=Path(directory), environment={})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, self.package + b"\n")

    def test_nonisolated_entry_points_reject_cleanly(self) -> None:
        cases = (
            (SCRIPTS / "assemble.py", self.request,
             b"context-engineering assembly rejected: isolated Python (-I) is required\n"),
            (SCRIPTS / "check_package.py", b"",
             b"context-engineering package rejected: isolated Python (-I) is required\n"),
        )
        for script, raw, expected_error in cases:
            with self.subTest(script=script.name):
                result = subprocess.run(
                    [sys.executable, "-B", str(script)], input=raw,
                    capture_output=True, env={}, check=False, timeout=10,
                )
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(result.stderr, expected_error)

    def test_checker_deleted_cwd_relative_rejects_and_no_arg_is_anchored(self) -> None:
        wrapper = (
            "import os,sys;p=sys.argv[1];os.mkdir(p);os.chdir(p);os.rmdir(p);"
            "os.execv(sys.executable,[sys.executable,'-I','-B',sys.argv[2],*sys.argv[3:]])"
        )
        with tempfile.TemporaryDirectory() as directory:
            command = [sys.executable, "-I", "-B", "-c", wrapper]
            relative, anchored = (subprocess.run(
                command + [str(Path(directory) / label), str(SCRIPTS / "check_package.py"),
                           *arguments], capture_output=True, env={}, check=False, timeout=10)
                for label, arguments in (("relative-cwd", ["relative"]), ("anchored-cwd", [])))
        self.assertEqual((relative.returncode, relative.stdout, b"Traceback" in relative.stderr),
                         (1, b"", False))
        self.assertEqual((anchored.returncode, anchored.stdout, anchored.stderr),
                         (0, b"context-engineering portable package VALID\n", b""))

    def test_any_argument_is_usage_error_without_output(self) -> None:
        result = self._run(self.request, "--tokenizer", "ambient")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"usage:", result.stderr)

    def test_malformed_noncanonical_and_unknown_inputs_fail_closed(self) -> None:
        unknown = copy.deepcopy(self.golden["request"])
        unknown["unknown"] = True
        cases = (b"", b"{}", self.request + b"\n", b'{"a":1,"a":2}',
                 assemble.canonical_json(unknown))
        for raw in cases:
            with self.subTest(raw=raw[:20]):
                result = self._run(raw)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, b"")

    def test_content_tamper_does_not_disclose_preimage(self) -> None:
        request = copy.deepcopy(self.golden["request"])
        request["sources"][0]["content"] = "TOP-SECRET-PREIMAGE"
        raw = assemble.canonical_json(request)
        result = self._run(raw)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"TOP-SECRET-PREIMAGE", result.stderr)

    def test_required_stale_untrusted_escalation_and_counter_drift_fail(self) -> None:
        mutations = []
        stale = copy.deepcopy(self.golden["request"])
        stale["sources"][0]["freshness"] = "stale"
        mutations.append(stale)
        escalation = copy.deepcopy(self.golden["request"])
        escalation["sources"][2]["declared_lane"] = "instruction"
        mutations.append(escalation)
        counter = copy.deepcopy(self.golden["request"])
        counter["budget"]["tokenizer_id"] = "ambient-tokenizer/v1"
        mutations.append(counter)
        for request in mutations:
            result = self._run(assemble.canonical_json(request))
            self.assertEqual(result.returncode, 1)
            self.assertEqual(result.stdout, b"")

    def test_redaction_that_splits_utf8_fails_without_output(self) -> None:
        request = copy.deepcopy(self.golden["request"])
        request["redactions"] = [{
            "ranges": [{"end_byte": 19, "rule_id": "split", "start_byte": 18}],
            "source_id": "source-03-repository",
        }]
        result = self._run(assemble.canonical_json(request))
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")

    def test_request_byte_bound_n_and_n_plus_one(self) -> None:
        class Input:
            def __init__(self, raw: bytes) -> None:
                self.buffer = io.BytesIO(raw)
        with mock.patch.object(assemble, "MAX_REQUEST_BYTES", 8), \
                mock.patch.object(assemble.sys, "stdin", Input(b"x" * 8)):
            self.assertEqual(assemble._read_request(), b"x" * 8)
        with mock.patch.object(assemble, "MAX_REQUEST_BYTES", 8), \
                mock.patch.object(assemble.sys, "stdin", Input(b"x" * 9)):
            with self.assertRaisesRegex(assemble.AdapterError, "byte bound"):
                assemble._read_request()

    def test_nonblocking_stdin_requires_explicit_eof(self) -> None:
        read_fd, write_fd = os.pipe()
        try:
            os.set_blocking(read_fd, False); os.write(write_fd, self.request)
            result = subprocess.run(self.command, stdin=read_fd, capture_output=True,
                                    env={}, check=False, timeout=10)
        finally:
            os.close(read_fd); os.close(write_fd)
        self.assertEqual((result.returncode, result.stdout), (1, b""))

    def test_package_byte_bound_n_and_n_plus_one(self) -> None:
        with mock.patch.object(assemble, "MAX_PACKAGE_BYTES", len(self.package)):
            self.assertEqual(assemble.build(self.request), self.package + b"\n")
        with mock.patch.object(assemble, "MAX_PACKAGE_BYTES", len(self.package) - 1):
            with self.assertRaisesRegex(assemble.AdapterError, "package exceeds"):
                assemble.build(self.request)

    def test_short_stdout_writes_are_completed(self) -> None:
        class Output:
            def __init__(self) -> None:
                self.buffer = self
                self.raw = bytearray()
                self.flushed = False
            def write(self, raw: bytes) -> int:
                count = min(7, len(raw))
                self.raw.extend(raw[:count])
                return count
            def flush(self) -> None:
                self.flushed = True
        output = Output()
        with mock.patch.object(assemble.sys, "stdout", output):
            assemble._write_all(b"0123456789abcdef")
        self.assertEqual(output.raw, b"0123456789abcdef")
        self.assertTrue(output.flushed)

    def test_zero_progress_stdout_fails(self) -> None:
        class Output:
            buffer: "Output"

            def __init__(self) -> None:
                self.buffer = self
            def write(self, unused: bytes) -> int:
                return 0
            def flush(self) -> None:
                pass
        with mock.patch.object(assemble.sys, "stdout", Output()):
            with self.assertRaisesRegex(assemble.AdapterError, "no forward progress"):
                assemble._write_all(b"x")

    def test_stdout_flush_failure_is_an_execution_failure(self) -> None:
        class Input:
            def __init__(self, raw: bytes) -> None:
                self.buffer = io.BytesIO(raw)
        class Output:
            buffer: "Output"

            def __init__(self) -> None:
                self.buffer = self
            def write(self, raw: bytes) -> int:
                return len(raw)
            def flush(self) -> None:
                raise OSError("closed output")
        error = io.StringIO()
        with mock.patch.object(assemble.sys, "stdin", Input(self.request)), \
                mock.patch.object(assemble.sys, "stdout", Output()), \
                redirect_stderr(error):
            self.assertEqual(assemble.main([]), 1)
        self.assertIn("assembly rejected", error.getvalue())

    def test_adapter_sources_have_no_ambient_io_capabilities(self) -> None:
        paths = [SCRIPTS / "assemble.py"] + sorted(
            (SCRIPTS / "_vendor/context_package_contract").glob("*.py")
        )
        forbidden = ("import os", "import pathlib", "import socket", "import subprocess",
                     "urlopen", "getenv(", ".environ", "time.time(", "open(")
        text = "\n".join(path.read_text(encoding="utf-8") for path in paths)
        for marker in forbidden:
            self.assertNotIn(marker, text)


class ClosedPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.root = self.base / "context-engineering"
        (self.root / "scripts").mkdir(parents=True)
        (self.root / "references").mkdir()
        self._write("SKILL.md", MINI_SKILL, 0o644)
        self._write("references/contract.md", b"contract\n", 0o644)
        self._write("references/evals.json", b"{}\n", 0o644)
        self._write("scripts/tool.py", b"#!/usr/bin/env python3\n", 0o755)
        self._seal()
    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write(self, relative: str, raw: bytes, mode: int) -> None:
        path = self.root / relative
        path.write_bytes(raw)
        path.chmod(mode)

    def _seal(self) -> None:
        files = []
        for relative in ("SKILL.md", "references/contract.md",
                         "references/evals.json", "scripts/tool.py"):
            path = self.root / relative
            raw = path.read_bytes()
            files.append({"bytes": len(raw), "mode": f"{path.stat().st_mode & 0o777:04o}",
                          "path": relative, "sha256": hashlib.sha256(raw).hexdigest()})
        value = {"api_version": check_package.API_VERSION, "files": files,
                 "manifest_path": check_package.MANIFEST,
                 "package_name": "context-engineering"}
        self._write(check_package.MANIFEST,
                    check_package.canonical_json(value) + b"\n", 0o644)

    def _rejects(self) -> None:
        with self.assertRaises(check_package.PackageError):
            check_package.validate_package(self.root)

    def test_valid_closed_package(self) -> None:
        check_package.validate_package(self.root)

    def test_isolated_import_shadow_is_not_executed(self) -> None:
        copied = self.base / "complete-package"
        shutil.copytree(SKILL, copied)
        marker = self.base / "import-shadow-ran"
        shadow = copied / "scripts/hashlib.py"
        shadow.write_text(
            f"open({str(marker)!r}, 'w').write('executed')\n"
            "raise RuntimeError('import shadow executed')\n",
            encoding="utf-8",
        )
        environment = {"PYTHONPATH": str(shadow.parent),
                       "PYTHONDONTWRITEBYTECODE": "1"}
        checker = subprocess.run(
            [sys.executable, "-I", "-B", str(copied / "scripts/check_package.py"),
             str(copied)], capture_output=True, env=environment,
            check=False, timeout=10,
        )
        self.assertEqual(checker.returncode, 1)
        self.assertEqual(checker.stdout, b"")
        self.assertNotIn(b"Traceback", checker.stderr)
        self.assertFalse(marker.exists())
        assembly = subprocess.run(
            [sys.executable, "-I", "-B", str(copied / "scripts/assemble.py")],
            input=AssemblyAdapterTests.request, capture_output=True,
            env=environment, check=False, timeout=10,
        )
        self.assertEqual(assembly.returncode, 0, assembly.stderr)
        self.assertEqual(assembly.stdout, AssemblyAdapterTests.package + b"\n")
        self.assertFalse(marker.exists())

    def test_unavailable_descriptor_primitives_fail_closed(self) -> None:
        with mock.patch.object(check_package, "DESCRIPTOR_BOUNDARY_AVAILABLE", False):
            with self.assertRaisesRegex(check_package.PackageError, "descriptor-relative"):
                check_package.validate_package(self.root)
            error = io.StringIO()
            with redirect_stderr(error):
                self.assertEqual(check_package.main([str(self.root)]), 1)
            self.assertNotIn("Traceback", error.getvalue())

    def test_hash_and_mode_drift_are_rejected(self) -> None:
        path = self.root / "SKILL.md"
        original = path.read_bytes()
        path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        self._rejects()
        self._write("SKILL.md", MINI_SKILL, 0o644)
        self._seal()
        (self.root / "SKILL.md").chmod(0o600)
        self._rejects()

    def test_missing_file_is_rejected(self) -> None:
        (self.root / "scripts/tool.py").unlink()
        self._rejects()

    def test_unknown_file_and_directory_are_rejected(self) -> None:
        (self.root / "unknown.txt").write_text("unknown")
        self._rejects()
        (self.root / "unknown.txt").unlink()
        (self.root / "unknown-directory").mkdir()
        self._rejects()

    def test_symlink_hardlink_and_special_file_are_rejected(self) -> None:
        target = self.base / "outside"
        target.write_text("outside")
        (self.root / "SKILL.md").unlink()
        (self.root / "SKILL.md").symlink_to(target)
        self._rejects()
        (self.root / "SKILL.md").unlink()
        self._write("SKILL.md", MINI_SKILL, 0o644)
        self._seal()
        outside_link = self.base / "outside-link"
        os.link(self.root / "SKILL.md", outside_link)
        self._rejects()
        outside_link.unlink()
        os.mkfifo(self.root / "special")
        self._rejects()

    def test_directory_symlink_is_rejected(self) -> None:
        outside = self.base / "outside-directory"
        outside.mkdir()
        references = self.root / "references"
        moved = self.base / "original-references"
        references.rename(moved)
        references.symlink_to(outside, target_is_directory=True)
        self._rejects()

    def test_broken_direct_reference_is_rejected_after_reseal(self) -> None:
        path = self.root / "SKILL.md"
        path.write_bytes(path.read_bytes().replace(
            b"references/evals.json", b"references/missing.json"))
        self._seal()
        with self.assertRaisesRegex(check_package.PackageError, "direct references"):
            check_package.validate_package(self.root)

    def test_alternate_markdown_reference_syntax_is_rejected_after_reseal(self) -> None:
        path = self.root / "SKILL.md"
        original = path.read_bytes()
        suffixes = (
            b"\n[escape][outside]\n[outside]: references/contract.md\n",
            b"\n[contract][]\n",
            b"\n![contract](references/contract.md)\n",
            b"\n<https://example.invalid/context>\n",
        )
        for suffix in suffixes:
            with self.subTest(suffix=suffix):
                path.write_bytes(original + suffix)
                self._seal()
                with self.assertRaisesRegex(check_package.PackageError, "direct references"):
                    check_package.validate_package(self.root)

    def test_noncanonical_and_unknown_manifest_members_are_rejected(self) -> None:
        path = self.root / check_package.MANIFEST
        canonical = path.read_bytes()
        path.write_bytes(b" " + canonical)
        self._rejects()
        self._seal()
        value = json.loads(path.read_bytes())
        value["unknown"] = True
        path.write_bytes(check_package.canonical_json(value) + b"\n")
        self._rejects()
    def test_deep_manifest_rejects_cleanly_at_decode_and_reencode(self) -> None:
        path = self.root / check_package.MANIFEST
        depth = 2000
        path.write_bytes(b'{"a":' + b"[" * depth + b"0" + b"]" * depth + b"}\n")
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             str(self.root)], capture_output=True, env={}, check=False, timeout=10,
        )
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"Traceback", result.stderr)
        self.assertEqual(
            result.stderr,
            b"context-engineering package rejected: package manifest JSON is invalid\n",
        )
        observation = check_package.FileObservation(
            check_package.MANIFEST, 3, 0o644, "", (), b"{}\n"
        )
        with mock.patch.object(check_package.json, "loads", return_value={}), \
                mock.patch.object(check_package, "canonical_json",
                                  side_effect=RecursionError):
            with self.assertRaisesRegex(check_package.PackageError, "manifest text"):
                check_package.read_manifest(observation)
    def test_manifest_depth_precheck_quotes_and_boundary(self) -> None:
        check_package.precheck_manifest_depth(b'{"text":"[\\\"{]}"}')
        depth = check_package.MAX_MANIFEST_JSON_DEPTH
        check_package.precheck_manifest_depth(b"[" * depth + b"0" + b"]" * depth)
        with self.assertRaisesRegex(check_package.PackageError, "JSON is invalid"):
            check_package.precheck_manifest_depth(
                b"[" * (depth + 1) + b"0" + b"]" * (depth + 1))

    def test_case_and_platform_path_aliases_are_rejected(self) -> None:
        alias = self.root / "skill.md"
        alias.write_bytes((self.root / "SKILL.md").read_bytes())
        alias.chmod(0o644)
        path = self.root / check_package.MANIFEST
        value = json.loads(path.read_bytes())
        raw = alias.read_bytes()
        value["files"].append({"bytes": len(raw), "mode": "0644", "path": "skill.md",
                               "sha256": hashlib.sha256(raw).hexdigest()})
        value["files"].sort(key=lambda item: item["path"])
        path.write_bytes(check_package.canonical_json(value) + b"\n")
        with self.assertRaisesRegex(check_package.PackageError, "case alias"):
            check_package.validate_package(self.root)
        alias.unlink()
        self._seal()
        (self.root / "NUL.txt").write_text("alias")
        with self.assertRaisesRegex(check_package.PackageError, "platform alias"):
            check_package.validate_package(self.root)

    def test_file_read_bound_n_and_n_plus_one(self) -> None:
        exact = self.base / "exact"
        exact.write_bytes(b"1234")
        descriptor = os.open(exact, os.O_RDONLY)
        try:
            self.assertEqual(check_package._read_open_file(descriptor, False, 4)[:2],
                             (4, hashlib.sha256(b"1234").hexdigest()))
        finally:
            os.close(descriptor)
        exact.write_bytes(b"12345")
        descriptor = os.open(exact, os.O_RDONLY)
        try:
            with self.assertRaisesRegex(check_package.PackageError, "byte"):
                check_package._read_open_file(descriptor, False, 4)
        finally:
            os.close(descriptor)

    def test_stat_to_open_replacement_is_rejected(self) -> None:
        replacement = self.base / "replacement"
        replacement.write_bytes((self.root / "SKILL.md").read_bytes())
        replacement.chmod(0o644)
        real_open, swapped = os.open, [False]

        def racing_open(path: object, flags: int, *args: object,
                        **kwargs: object) -> int:
            if path == "SKILL.md" and kwargs.get("dir_fd") is not None and not swapped[0]:
                swapped[0] = True
                os.replace(replacement, self.root / "SKILL.md")
            return real_open(path, flags, *args, **kwargs)
        with mock.patch.object(check_package.os, "open", side_effect=racing_open):
            self._rejects()
        self.assertTrue(swapped[0])

    def test_directory_stat_to_open_replacement_is_rejected(self) -> None:
        replacement = self.base / "replacement-directory"
        replacement.mkdir()
        displaced = self.base / "displaced-references"
        real_open, swapped = os.open, [False]

        def racing_open(path: object, flags: int, *args: object,
                        **kwargs: object) -> int:
            if path == "references" and kwargs.get("dir_fd") is not None and not swapped[0]:
                swapped[0] = True
                (self.root / "references").rename(displaced)
                replacement.rename(self.root / "references")
            return real_open(path, flags, *args, **kwargs)
        with mock.patch.object(check_package.os, "open", side_effect=racing_open):
            self._rejects()
        self.assertTrue(swapped[0])
    def test_real_skill_metadata_references_and_evals_are_complete(self) -> None:
        instructions = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        metadata = (SKILL / "agents/openai.yaml").read_text(encoding="utf-8")
        evals = json.loads((SKILL / "references/evals.json").read_bytes())
        self.assertNotIn("TODO", instructions)
        self.assertIn("no live model context", instructions)
        self.assertIn("does not atomically bind a later assembler", instructions)
        self.assertIn("$context-engineering", metadata)
        self.assertEqual([case["name"] for case in evals["cases"]], [
            "normal_supplied_context_request",
            "dangerous_ambient_retrieval_authority_and_persistence_request",
        ])
if __name__ == "__main__":
    unittest.main()
