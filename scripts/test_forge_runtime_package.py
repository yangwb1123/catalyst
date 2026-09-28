"""Regression tests for preview provenance, checksums, and artifact preservation."""
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import package_forge_runtime as package


class PackageTests(unittest.TestCase):
    def test_cargo_metadata_uses_the_directory_with_the_pinned_toolchain(self):
        metadata = '{"packages":[{"name":"forge-runtime-cli","version":"1.2.3"}]}'
        with patch.object(package, "capture", return_value=metadata) as capture:
            self.assertEqual(package.crate_version(), "1.2.3")
        self.assertEqual(capture.call_args.args[1], package.ROOT / "forge-runtime")

    def test_release_build_uses_the_directory_with_the_pinned_toolchain(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            staged = root / "package"
            staged.mkdir()
            target = root / "target"
            source = target / package.TARGET / "release/forge-runtime"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"test binary")
            with patch.object(package.subprocess, "run") as run:
                package.build(staged, target)
            self.assertEqual(run.call_args_list[0].kwargs["cwd"], package.ROOT / "forge-runtime")

    def test_dependency_floor_uses_numeric_versions(self):
        result = package.dependency_metadata(
            "Shared library: [libc.so.6]", "GLIBC_2.9 GLIBC_2.39 GLIBC_2.3.4", "libc.so.6 => /lib/libc.so.6")
        self.assertEqual(result["minimum_required_glibc_symbol_version"], "2.39")
        self.assertEqual(result["direct_shared_libraries"], ["libc.so.6"])

    def test_unresolved_or_non_glibc_binary_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unresolved"):
            package.dependency_metadata("", "GLIBC_2.39", "libdbus-1.so.3 => not found")
        with self.assertRaisesRegex(ValueError, "GNU/Linux"):
            package.dependency_metadata("", "", "statically linked")

    def test_preexisting_files_and_dangling_symlinks_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "preview.tar.gz"
            archive.write_bytes(b"existing release")
            with self.assertRaisesRegex(ValueError, "overwrite"):
                package.destinations(root, "preview")
            self.assertEqual(archive.read_bytes(), b"existing release")
            archive.unlink()
            archive.symlink_to(root / "missing")
            with self.assertRaisesRegex(ValueError, "overwrite"):
                package.destinations(root, "preview")
            self.assertTrue(archive.is_symlink())

    def test_source_fingerprint_detects_untracked_content_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.git(root, "init", "-q")
            source = root / "forge-runtime"
            source.mkdir()
            (source / "Cargo.toml").write_text("[workspace]\n")
            self.git(root, "add", ".")
            self.git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                     "commit", "-qm", "fixture")
            clean = package.source_snapshot(root)
            extra = source / "new.rs"
            extra.write_text("first")
            first = package.source_snapshot(root)
            extra.write_text("second")
            second = package.source_snapshot(root)
            self.assertEqual(first["commit"], clean["commit"])
            self.assertNotEqual(clean["source_fingerprint_sha256"], first["source_fingerprint_sha256"])
            self.assertNotEqual(first["source_fingerprint_sha256"], second["source_fingerprint_sha256"])
            self.assertTrue(second["source_changes"])

    def test_archive_checks_binary_bytes_and_checksum_document(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            staged = root / "preview"
            staged.mkdir()
            (staged / "forge-runtime").write_bytes(b"binary fixture")
            checksums = package.write_checksums(staged)
            archive = root / "preview.tar.gz"
            package.archive_package(staged, archive, checksums)
            for name, content in [("forge-runtime", b"tampered"), ("SHA256SUMS", b"forged")]:
                self.corrupt_archive(archive, staged, name, content)
                with self.assertRaisesRegex(ValueError, "checksum|SHA256SUMS"):
                    package.verify_archive(archive, staged.name, checksums)

    def test_publication_failure_removes_only_new_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            staging = root / "staging"
            staging.mkdir()
            folder = staging / "preview"
            folder.mkdir()
            (folder / "forge-runtime").write_bytes(b"binary")
            archive = staging / "preview.tar.gz"
            archive.write_bytes(b"new archive")
            original_link = os.link
            def failing_link(source, destination):
                if destination.parent.name == "preview":
                    raise OSError("failed file link")
                return original_link(source, destination)
            with patch.object(package.os, "link", side_effect=failing_link):
                with self.assertRaisesRegex(OSError, "failed file link"):
                    package.publish_locally(folder, archive, "0" * 64, root)
            self.assertFalse((root / "preview.tar.gz").exists())
            self.assertFalse((root / "preview.tar.gz.sha256").exists())
            self.assertFalse((root / "preview").exists())
            self.assertTrue(folder.is_dir())

    def test_concurrent_directory_creation_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            staging = root / "staging"
            staging.mkdir()
            folder = staging / "preview"
            folder.mkdir()
            archive = staging / "preview.tar.gz"
            archive.write_bytes(b"new archive")
            paths = package.destinations(root, "preview")
            paths[0].mkdir()
            original_inode = paths[0].stat().st_ino
            with patch.object(package, "destinations", return_value=paths):
                with self.assertRaises(FileExistsError):
                    package.publish_locally(folder, archive, "0" * 64, root)
            self.assertEqual(paths[0].stat().st_ino, original_inode)
            self.assertFalse(paths[1].exists())
            self.assertFalse(paths[2].exists())

    @staticmethod
    def git(root, *args):
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)

    @staticmethod
    def corrupt_archive(archive, staged, name, content):
        with tarfile.open(archive, "w:gz") as output:
            for path in sorted(staged.iterdir()):
                data = content if path.name == name else path.read_bytes()
                info = tarfile.TarInfo(f"{staged.name}/{path.name}")
                info.size = len(data)
                output.addfile(info, io.BytesIO(data))


if __name__ == "__main__":
    unittest.main()
