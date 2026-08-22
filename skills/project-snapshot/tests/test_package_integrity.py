#!/usr/bin/env python3
"""Closed-package integrity tests for the portable project-snapshot Skill."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

SKILL = Path(__file__).resolve().parents[1]
SCRIPTS = SKILL / "scripts"


def _load_checker() -> object:
    path = SCRIPTS / "check_package.py"
    specification = importlib.util.spec_from_file_location(
        "_project_snapshot_package_integrity_test", path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


check_package = _load_checker()
MINI_SKILL = (b"---\nname: project-snapshot\n---\n"
              b"[references/contract.md](references/contract.md)\n"
              b"[references/evals.json](references/evals.json)\n")


class ClosedPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.root = self.base / "project-snapshot"
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
                 "package_name": "project-snapshot"}
        self._write(check_package.MANIFEST,
                    check_package.canonical_json(value) + b"\n", 0o644)

    def _rejects(self) -> None:
        with self.assertRaises(check_package.PackageError):
            check_package.validate_package(self.root)

    def test_valid_closed_package(self) -> None:
        check_package.validate_package(self.root)

    def test_missing_descriptor_primitives_fail_closed_without_traceback(self) -> None:
        with mock.patch.object(check_package, "DESCRIPTOR_BOUNDARY_AVAILABLE", False):
            with self.assertRaisesRegex(check_package.PackageError,
                                        "descriptor-relative no-follow"):
                check_package.validate_package(self.root)
            error = io.StringIO()
            with redirect_stderr(error):
                self.assertEqual(check_package.main([str(self.root)]), 1)
            self.assertNotIn("Traceback", error.getvalue())

    def test_byte_tamper_is_rejected(self) -> None:
        (self.root / "SKILL.md").write_bytes(b"tampered\n")
        self._rejects()

    def test_deep_manifest_rejects_without_traceback(self) -> None:
        path = self.root / check_package.MANIFEST
        path.write_bytes(b"[" * 2000 + b"0" + b"]" * 2000 + b"\n")
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             str(self.root)], capture_output=True, env={}, check=False, timeout=10)
        self.assertEqual((result.returncode, result.stdout), (1, b""))
        self.assertEqual(result.stderr, b"project-snapshot package rejected: "
                         b"package manifest JSON is invalid\n")

    def test_relative_root_from_deleted_cwd_rejects_stably(self) -> None:
        wrapper = ("import os,sys\n"
                   "os.chdir(sys.argv[1]); os.rmdir(sys.argv[1])\n"
                   "os.execv(sys.executable,[sys.executable,'-I','-B',sys.argv[2],'.'])\n")
        errors = []
        for index in range(2):
            current = self.base / f"deleted-cwd-{index}"
            current.mkdir()
            result = subprocess.run(
                [sys.executable, "-I", "-B", "-c", wrapper, str(current),
                 str(SCRIPTS / "check_package.py")],
                capture_output=True, env={}, check=False, timeout=10)
            self.assertEqual((result.returncode, result.stdout), (1, b""))
            self.assertNotIn(b"Traceback", result.stderr)
            errors.append(result.stderr)
        self.assertEqual(errors[0], errors[1])
        self.assertEqual(errors[0], b"project-snapshot package rejected: "
                         b"[Errno 2] No such file or directory\n")

    def test_manifest_depth_precheck_quotes_and_boundary(self) -> None:
        check_package.precheck_manifest_depth(b'{"text":"[\\\"{]}"}')
        depth = check_package.MAX_MANIFEST_JSON_DEPTH
        check_package.precheck_manifest_depth(b"[" * depth + b"0" + b"]" * depth)
        with self.assertRaisesRegex(check_package.PackageError, "JSON is invalid"):
            check_package.precheck_manifest_depth(
                b"[" * (depth + 1) + b"0" + b"]" * (depth + 1))

    def test_broken_direct_reference_is_rejected_after_reseal(self) -> None:
        path = self.root / "SKILL.md"
        path.write_bytes(path.read_bytes().replace(
            b"references/evals.json", b"references/missing.json"))
        self._seal()
        with self.assertRaisesRegex(check_package.PackageError, "direct references"):
            check_package.validate_package(self.root)

    def test_alternate_markdown_reference_syntax_is_rejected(self) -> None:
        path, original = self.root / "SKILL.md", (self.root / "SKILL.md").read_bytes()
        suffixes = (b"\n[escape][outside]\n[outside]: references/contract.md\n",
                    b"\n[contract][]\n", b"\n![contract](references/contract.md)\n",
                    b"\n<https://example.invalid/project>\n")
        for suffix in suffixes:
            with self.subTest(suffix=suffix):
                path.write_bytes(original + suffix)
                self._seal()
                with self.assertRaisesRegex(check_package.PackageError,
                                            "direct references"):
                    check_package.validate_package(self.root)

    def test_symlink_and_hardlink_are_rejected(self) -> None:
        target = self.base / "outside"
        target.write_text("outside")
        (self.root / "SKILL.md").unlink()
        (self.root / "SKILL.md").symlink_to(target)
        self._rejects()
        (self.root / "SKILL.md").unlink()
        self._write("SKILL.md", MINI_SKILL, 0o644)
        self._seal()
        os.link(self.root / "SKILL.md", self.base / "outside-link")
        self._rejects()

    def test_unknown_file_and_directory_are_rejected(self) -> None:
        (self.root / "unknown.txt").write_text("unknown")
        self._rejects()
        (self.root / "unknown.txt").unlink()
        (self.root / "unknown-directory").mkdir()
        self._rejects()

    def test_wide_directory_stops_at_global_entry_bound(self) -> None:
        for index in range(4):
            (self.root / f"extra-{index}").write_text("extra")
        yielded = [0]
        real_scandir = check_package.os.scandir

        class CountingIterator:
            def __init__(self, path: object) -> None:
                self.inner = real_scandir(path)

            def __enter__(self) -> "CountingIterator":
                return self

            def __exit__(self, *unused: object) -> None:
                self.inner.close()

            def __iter__(self) -> "CountingIterator":
                return self

            def __next__(self) -> os.DirEntry[str]:
                value = next(self.inner)
                yielded[0] += 1
                return value

        with mock.patch.object(check_package, "MAX_PACKAGE_ENTRIES", 3), \
                mock.patch.object(check_package.os, "scandir", side_effect=CountingIterator):
            with self.assertRaisesRegex(check_package.PackageError, "entry count"):
                check_package.validate_package(self.root)
        self.assertEqual(yielded[0], 4)

    def test_large_unknown_file_stops_at_file_byte_bound(self) -> None:
        (self.root / "unknown.bin").write_bytes(b"12345")
        with mock.patch.object(check_package, "MAX_PACKAGE_FILE_BYTES", 4):
            with self.assertRaisesRegex(check_package.PackageError, "size exceeds"):
                check_package.validate_package(self.root)

    def test_nonportable_actual_path_is_rejected_before_open(self) -> None:
        unsafe = self.root / "unsafe name"
        unsafe.write_text("do not open")
        real_open = check_package.os.open

        def guarded_open(path: object, flags: int, *args: object,
                         **kwargs: object) -> int:
            if path == "unsafe name":
                self.fail("nonportable package path was opened")
            return real_open(path, flags, *args, **kwargs)

        with mock.patch.object(check_package.os, "open", side_effect=guarded_open):
            with self.assertRaisesRegex(check_package.PackageError, "not portable"):
                check_package.validate_package(self.root)

    def test_platform_and_case_aliases_are_rejected(self) -> None:
        (self.root / "NUL.txt").write_text("alias")
        with self.assertRaisesRegex(check_package.PackageError, "platform alias"):
            check_package.validate_package(self.root)
        (self.root / "NUL.txt").unlink()
        alias = self.root / "skill.md"
        alias.write_bytes((self.root / "SKILL.md").read_bytes())
        alias.chmod(0o644)
        manifest_path = self.root / check_package.MANIFEST
        manifest = json.loads(manifest_path.read_bytes())
        raw = alias.read_bytes()
        manifest["files"].append({"bytes": len(raw), "mode": "0644", "path": "skill.md",
                                  "sha256": hashlib.sha256(raw).hexdigest()})
        manifest["files"].sort(key=lambda item: item["path"])
        manifest_path.write_bytes(check_package.canonical_json(manifest) + b"\n")
        with self.assertRaisesRegex(check_package.PackageError, "case alias"):
            check_package.validate_package(self.root)

    def test_mode_and_root_symlink_drift_are_rejected(self) -> None:
        (self.root / "SKILL.md").chmod(0o600)
        self._rejects()
        (self.root / "SKILL.md").chmod(0o644)
        alias = self.base / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises((OSError, check_package.PackageError)):
            check_package.validate_package(alias)

    def test_stat_to_open_replacement_is_rejected(self) -> None:
        replacement = self.base / "replacement"
        replacement.write_bytes((self.root / "SKILL.md").read_bytes())
        replacement.chmod(0o644)
        real_open, swapped = os.open, False

        def racing_open(path: object, flags: int, *args: object,
                        **kwargs: object) -> int:
            nonlocal swapped
            if path == "SKILL.md" and kwargs.get("dir_fd") is not None and not swapped:
                swapped = True
                os.replace(replacement, self.root / "SKILL.md")
            return real_open(path, flags, *args, **kwargs)

        with mock.patch.object(check_package.os, "open", side_effect=racing_open):
            self._rejects()
        self.assertTrue(swapped)

    def test_metadata_and_handoff_claims_remain_narrow(self) -> None:
        metadata = (SKILL / "agents/openai.yaml").read_text(encoding="utf-8")
        instructions = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertNotIn("secret-safe", metadata.casefold())
        self.assertIn("Capture bounded local source observations", metadata)
        self.assertIn("output locator outside the captured worktree", instructions)
        self.assertIn("snapshot_identity_sha256", instructions)
        self.assertIn("all 12 coverage surfaces", instructions)


if __name__ == "__main__":
    unittest.main()
