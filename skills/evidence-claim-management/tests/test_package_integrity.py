#!/usr/bin/env python3
"""Closed-package integrity tests for evidence-claim-management."""

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
MINI_SKILL = (b"---\nname: evidence-claim-management\n---\n"
              b"[references/contract.md](references/contract.md)\n"
              b"[references/evals.json](references/evals.json)\n")


def _load_checker() -> object:
    path = SCRIPTS / "check_package.py"
    specification = importlib.util.spec_from_file_location(
        "_evidence_claim_package_integrity_test", path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load test subject: {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


check_package = _load_checker()


class ClosedPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.root = self.base / "evidence-claim-management"
        (self.root / "scripts").mkdir(parents=True)
        (self.root / "references").mkdir()
        self.write("SKILL.md", MINI_SKILL, 0o644)
        self.write("references/contract.md", b"contract\n", 0o644)
        self.write("references/evals.json", b"{}\n", 0o644)
        self.write("scripts/tool.py", b"#!/usr/bin/env python3\n", 0o755)
        self.seal()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write(self, relative: str, raw: bytes, mode: int) -> None:
        path = self.root / relative
        path.write_bytes(raw)
        path.chmod(mode)

    def seal(self) -> None:
        files = []
        for relative in ("SKILL.md", "references/contract.md",
                         "references/evals.json", "scripts/tool.py"):
            path = self.root / relative
            raw = path.read_bytes()
            files.append({"bytes": len(raw),
                          "mode": f"{path.stat().st_mode & 0o777:04o}",
                          "path": relative,
                          "sha256": hashlib.sha256(raw).hexdigest()})
        value = {"api_version": check_package.API_VERSION, "files": files,
                 "manifest_path": check_package.MANIFEST,
                 "package_name": "evidence-claim-management"}
        self.write(check_package.MANIFEST,
                   check_package.canonical_json(value) + b"\n", 0o644)

    def rejects(self) -> None:
        with self.assertRaises(check_package.PackageError):
            check_package.validate_package(self.root)

    def test_valid_closed_package(self) -> None:
        check_package.validate_package(self.root)

    def test_missing_descriptor_primitives_fail_closed(self) -> None:
        with mock.patch.object(check_package, "DESCRIPTOR_BOUNDARY_AVAILABLE", False):
            self.rejects()
            error = io.StringIO()
            with redirect_stderr(error):
                self.assertEqual(check_package.main([str(self.root)]), 1)
            self.assertNotIn("Traceback", error.getvalue())

    def test_hash_mode_missing_and_unknown_entries_reject(self) -> None:
        (self.root / "SKILL.md").write_bytes(b"tampered\n")
        self.rejects()
        self.write("SKILL.md", MINI_SKILL, 0o644)
        self.seal()
        (self.root / "SKILL.md").chmod(0o600)
        self.rejects()
        (self.root / "SKILL.md").chmod(0o644)
        self.seal()
        (self.root / "scripts/tool.py").unlink()
        self.rejects()
        self.write("scripts/tool.py", b"#!/usr/bin/env python3\n", 0o755)
        self.seal()
        (self.root / "unknown.txt").write_text("unknown")
        self.rejects()

    def test_unknown_directory_symlink_hardlink_and_fifo_reject(self) -> None:
        (self.root / "unknown-directory").mkdir()
        self.rejects()
        (self.root / "unknown-directory").rmdir()
        target = self.base / "outside"
        target.write_text("outside")
        (self.root / "SKILL.md").unlink()
        (self.root / "SKILL.md").symlink_to(target)
        self.rejects()
        (self.root / "SKILL.md").unlink()
        self.write("SKILL.md", MINI_SKILL, 0o644)
        self.seal()
        outside_link = self.base / "outside-link"
        os.link(self.root / "SKILL.md", outside_link)
        self.rejects()
        outside_link.unlink()
        os.mkfifo(self.root / "special")
        self.rejects()

    def test_broken_and_alternate_markdown_references_reject(self) -> None:
        path = self.root / "SKILL.md"
        original = path.read_bytes()
        variants = (
            original.replace(b"references/evals.json", b"references/missing.json"),
            original + b"\n[escape][outside]\n[outside]: references/contract.md\n",
            original + b"\n[contract][]\n",
            original + b"\n![contract](references/contract.md)\n",
            original + b"\n<https://example.invalid/evidence>\n",
        )
        for raw in variants:
            with self.subTest(raw=raw[-40:]):
                path.write_bytes(raw)
                self.seal()
                with self.assertRaisesRegex(check_package.PackageError,
                                            "direct references"):
                    check_package.validate_package(self.root)

    def test_noncanonical_unknown_and_deep_manifest_reject(self) -> None:
        path = self.root / check_package.MANIFEST
        path.write_bytes(b" " + path.read_bytes())
        self.rejects()
        self.seal()
        value = json.loads(path.read_bytes())
        value["unknown"] = True
        path.write_bytes(check_package.canonical_json(value) + b"\n")
        self.rejects()
        path.write_bytes(b"[" * 2000 + b"0" + b"]" * 2000 + b"\n")
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(SCRIPTS / "check_package.py"),
             str(self.root)], capture_output=True, env={}, check=False, timeout=10)
        self.assertEqual((result.returncode, result.stdout), (1, b""))
        self.assertEqual(result.stderr, b"evidence-claim-management package rejected: "
                         b"package manifest JSON is invalid\n")

    def test_manifest_depth_quotes_boundary_and_reencode_failure(self) -> None:
        check_package.precheck_manifest_depth(b'{"text":"[\\"{]}"}')
        depth = check_package.MAX_MANIFEST_JSON_DEPTH
        check_package.precheck_manifest_depth(b"[" * depth + b"0" + b"]" * depth)
        with self.assertRaisesRegex(check_package.PackageError, "JSON is invalid"):
            check_package.precheck_manifest_depth(
                b"[" * (depth + 1) + b"0" + b"]" * (depth + 1))
        observation = check_package.FileObservation(
            check_package.MANIFEST, 3, 0o644, "", (), b"{}\n")
        with mock.patch.object(check_package.json, "loads", return_value={}), \
                mock.patch.object(check_package, "canonical_json",
                                  side_effect=RecursionError):
            with self.assertRaisesRegex(check_package.PackageError, "manifest text"):
                check_package.read_manifest(observation)

    def test_platform_and_case_aliases_reject(self) -> None:
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
        manifest["files"].append({"bytes": len(raw), "mode": "0644",
                                  "path": "skill.md",
                                  "sha256": hashlib.sha256(raw).hexdigest()})
        manifest["files"].sort(key=lambda item: item["path"])
        manifest_path.write_bytes(check_package.canonical_json(manifest) + b"\n")
        with self.assertRaisesRegex(check_package.PackageError, "case alias"):
            check_package.validate_package(self.root)

    def test_stat_to_open_file_and_directory_replacement_reject(self) -> None:
        replacement = self.base / "replacement"
        replacement.write_bytes((self.root / "SKILL.md").read_bytes())
        replacement.chmod(0o644)
        real_open, swapped = os.open, [False]

        def racing_file(path: object, flags: int, *args: object,
                        **kwargs: object) -> int:
            if path == "SKILL.md" and kwargs.get("dir_fd") is not None and not swapped[0]:
                swapped[0] = True
                os.replace(replacement, self.root / "SKILL.md")
            return real_open(path, flags, *args, **kwargs)
        with mock.patch.object(check_package.os, "open", side_effect=racing_file):
            self.rejects()
        self.assertTrue(swapped[0])

        self.write("SKILL.md", MINI_SKILL, 0o644)
        self.seal()
        replacement_dir = self.base / "replacement-directory"
        replacement_dir.mkdir()
        displaced = self.base / "displaced-references"
        swapped[0] = False

        def racing_directory(path: object, flags: int, *args: object,
                             **kwargs: object) -> int:
            if path == "references" and kwargs.get("dir_fd") is not None and not swapped[0]:
                swapped[0] = True
                (self.root / "references").rename(displaced)
                replacement_dir.rename(self.root / "references")
            return real_open(path, flags, *args, **kwargs)
        with mock.patch.object(check_package.os, "open", side_effect=racing_directory):
            self.rejects()
        self.assertTrue(swapped[0])

    def test_deleted_cwd_relative_rejects_and_no_arg_is_anchored(self) -> None:
        wrapper = ("import os,sys;p=sys.argv[1];os.mkdir(p);os.chdir(p);os.rmdir(p);"
                   "os.execv(sys.executable,[sys.executable,'-I','-B',sys.argv[2],"
                   "*sys.argv[3:]])")
        with tempfile.TemporaryDirectory() as directory:
            command = [sys.executable, "-I", "-B", "-c", wrapper]
            relative, anchored = (subprocess.run(
                command + [str(Path(directory) / label),
                           str(SCRIPTS / "check_package.py"), *arguments],
                capture_output=True, env={}, check=False, timeout=10)
                for label, arguments in (("relative", ["relative"]), ("anchored", [])))
        self.assertEqual((relative.returncode, relative.stdout,
                          b"Traceback" in relative.stderr), (1, b"", False))
        self.assertEqual((anchored.returncode, anchored.stdout),
                         (0, b"evidence-claim-management portable package VALID\n"))


if __name__ == "__main__":
    unittest.main()
