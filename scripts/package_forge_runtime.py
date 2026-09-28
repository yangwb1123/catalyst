#!/usr/bin/env python3
"""Build and verify a local Linux x86_64 developer-preview archive."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
SOURCE_PATHS = ["forge-runtime", "scripts/package_forge_runtime.py",
                "scripts/forge_runtime_release_smoke.py"]


def capture(command, cwd=ROOT):
    return subprocess.check_output(command, cwd=cwd, text=True).strip()


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def source_snapshot(root=ROOT):
    names = capture(["git", "ls-files", "-z", "--cached", "--others",
                     "--exclude-standard", "--", *SOURCE_PATHS], root)
    entries = []
    for name in sorted(set(names.split("\0")) - {""}):
        path = root / name
        if path.is_symlink():
            value = "symlink:" + os.readlink(path)
        else:
            value = digest(path) if path.is_file() else "missing"
        entries.append([name, value])
    fingerprint = hashlib.sha256(json.dumps(entries).encode()).hexdigest()
    return {
        "commit": capture(["git", "rev-parse", "HEAD"], root),
        "source_fingerprint_sha256": fingerprint,
        "source_paths": SOURCE_PATHS,
        "source_changes": capture(["git", "status", "--porcelain",
                                   "--untracked-files=all", "--", *SOURCE_PATHS], root).splitlines(),
    }


def prerequisites():
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("preview packaging currently supports Linux x86_64 only")
    for name in ("cargo", "rustc", "strip", "readelf", "ldd", "git"):
        if shutil.which(name) is None:
            raise ValueError(f"required build tool is missing: {name}")


def crate_version():
    metadata = json.loads(capture([
        "cargo", "metadata", "--manifest-path", str(ROOT / "forge-runtime/Cargo.toml"),
        "--format-version", "1", "--no-deps", "--locked", "--offline",
    ], ROOT / "forge-runtime"))
    return next(package["version"] for package in metadata["packages"]
                if package["name"] == "forge-runtime-cli")


def destinations(output, name):
    paths = [output / name, output / (name + ".tar.gz"),
             output / (name + ".tar.gz.sha256")]
    for path in paths:
        if os.path.lexists(path):
            raise ValueError(f"refusing to overwrite existing artifact: {path}")
    return paths


def build(package, target_dir):
    command = ["cargo", "build", "--manifest-path", str(ROOT / "forge-runtime/Cargo.toml"),
               "-p", "forge-runtime-cli", "--bin", "forge-runtime", "--release",
               "--locked", "--offline", "--target", TARGET, "--target-dir", str(target_dir)]
    print("Building locked release binary...", flush=True)
    with (package / "build.log").open("w") as log:
        subprocess.run(command, cwd=ROOT / "forge-runtime", stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    binary = package / "forge-runtime"
    shutil.copy2(target_dir / TARGET / "release/forge-runtime", binary)
    subprocess.run(["strip", "--strip-unneeded", str(binary)], check=True)
    return binary


def dependency_metadata(dynamic, versions, linked):
    if "not found" in linked:
        raise ValueError("release binary has unresolved shared libraries")
    glibc = set(re.findall(r"GLIBC_(\d+(?:\.\d+)+)", versions))
    if not glibc:
        raise ValueError("expected a GNU/Linux dynamically linked binary")
    return {
        "linkage": "dynamic",
        "minimum_required_glibc_symbol_version": max(
            glibc, key=lambda value: tuple(map(int, value.split(".")))),
        "direct_shared_libraries": re.findall(r"Shared library: \[(.*?)\]", dynamic),
    }


def inspect_binary(binary, package):
    dynamic = capture(["readelf", "-d", str(binary)])
    versions = capture(["readelf", "--version-info", str(binary)])
    linked = capture(["ldd", str(binary)])
    (package / "dependencies.txt").write_text(
        linked + "\n\n" + dynamic + "\n\n" + versions + "\n", encoding="utf-8")
    return dependency_metadata(dynamic, versions, linked)


def verify_version(binary, version):
    with tempfile.TemporaryDirectory(prefix="forge-version-check-") as temporary:
        process = subprocess.run([str(binary), "--json", "--version"], cwd=temporary,
                                 env={"PATH": "", "LANG": "C.UTF-8"},
                                 capture_output=True, text=True, check=True, timeout=15)
        expected = {"name": "forge-runtime", "version": version}
        if json.loads(process.stdout) != expected:
            raise ValueError("binary version does not match Cargo package metadata")
        if list(Path(temporary).iterdir()):
            raise ValueError("version query unexpectedly created local state")


def run_smoke(binary, package):
    shutil.copy2(ROOT / "scripts/forge_runtime_release_smoke.py", package / "smoke.py")
    print("Checking standalone binary with offline provider fixtures...", flush=True)
    subprocess.run([sys.executable, str(package / "smoke.py"), str(binary),
                    str(package / "smoke-report.json")], cwd=package, check=True, timeout=180)
    report = json.loads((package / "smoke-report.json").read_text(encoding="utf-8"))
    if report["status"] != "passed" or report["binary_sha256"] != digest(binary):
        raise ValueError("smoke report does not attest the packaged binary")


def write_manifest(package, binary, version, source, dependencies):
    manifest = {
        "name": "forge-runtime", "version": version,
        "release_channel": "local-developer-preview", "target": TARGET,
        "source": source, **dependencies,
        "packaged_at_utc": datetime.now(timezone.utc).isoformat(),
        "build_host": platform.platform(),
        "rustc": capture(["rustc", "-Vv"], ROOT / "forge-runtime").splitlines(),
        "build_working_directory": "forge-runtime/",
        "build_command": "cargo build -p forge-runtime-cli --bin forge-runtime --release "
                         "--locked --offline --target " + TARGET,
        "postprocessing": "strip --strip-unneeded forge-runtime",
        "binary": binary.name, "binary_size_bytes": binary.stat().st_size,
        "binary_sha256": digest(binary),
        "cargo_lock_sha256": digest(ROOT / "forge-runtime/Cargo.lock"),
        "validation_report": "smoke-report.json", "real_model_validated": False,
        "cross_device_validated": False, "cross_distribution_validated": False,
        "formal_repository_acceptance": "not established by this packaging command",
        "signed": False, "published": False,
    }
    (package / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    shutil.copy2(ROOT / "forge-runtime/PREVIEW.md", package / "README.md")


def write_checksums(package):
    files = sorted(path for path in package.iterdir() if path.name != "SHA256SUMS")
    checksums = {path.name: digest(path) for path in files}
    text = "".join(f"{value}  {name}\n" for name, value in checksums.items())
    (package / "SHA256SUMS").write_text(text, encoding="utf-8")
    return checksums


def verify_archive(archive, name, checksums):
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        expected = {f"{name}/{item}" for item in [*checksums, "SHA256SUMS"]}
        actual = {member.name for member in members if member.isfile()}
        if actual != expected or len(members) != len(expected):
            raise ValueError("archive member list does not match package files")
        for filename, value in checksums.items():
            with source.extractfile(f"{name}/{filename}") as member:
                if hashlib.sha256(member.read()).hexdigest() != value:
                    raise ValueError(f"archive checksum mismatch: {filename}")
        with source.extractfile(f"{name}/SHA256SUMS") as member:
            expected_text = "".join(f"{value}  {file}\n" for file, value in checksums.items())
            if member.read().decode("utf-8") != expected_text:
                raise ValueError("archive SHA256SUMS does not match checked files")


def archive_package(package, archive, checksums):
    with tarfile.open(archive, "w:gz") as destination:
        for path in sorted(package.iterdir()):
            destination.add(path, arcname=f"{package.name}/{path.name}", recursive=False)
    verify_archive(archive, package.name, checksums)
    return digest(archive)


def publish_locally(package, archive, checksum, output):
    final_package, final_archive, final_checksum = destinations(output, package.name)
    # Reserve the directory exclusively, including against a concurrent creator.
    final_package.mkdir()
    created = []
    archive_created = False
    checksum_created = False
    try:
        os.link(archive, final_archive)
        archive_created = True
        with final_checksum.open("x", encoding="utf-8") as target:
            checksum_created = True
            target.write(f"{checksum}  {final_archive.name}\n")
        for path in package.iterdir():
            target = final_package / path.name
            os.link(path, target)
            created.append(target)
    except BaseException:
        for path in created:
            path.unlink()
        if archive_created:
            final_archive.unlink()
        if checksum_created:
            final_checksum.unlink()
        try:
            final_package.rmdir()
        except OSError:
            pass  # Preserve any concurrently-created contents.
        raise
    print(json.dumps({"package": str(final_package), "archive": str(final_archive),
                      "sha256": checksum, "archive_bytes": final_archive.stat().st_size}, indent=2))


def package_preview(output):
    prerequisites()
    source = source_snapshot()
    version = crate_version()
    fingerprint = source["source_fingerprint_sha256"][:12]
    name = f"forge-runtime-{version}-preview-{fingerprint}-linux-x86_64"
    destinations(output, name)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".forge-preview-", dir=output) as temporary:
        package = Path(temporary) / name
        package.mkdir()
        try:
            binary = build(package, ROOT / "forge-runtime/target")
            verify_version(binary, version)
            dependencies = inspect_binary(binary, package)
            run_smoke(binary, package)
            if source_snapshot() != source:
                raise ValueError("source changed during packaging; rerun on a stable worktree")
            write_manifest(package, binary, version, source, dependencies)
            archive = Path(temporary) / (name + ".tar.gz")
            checksum = archive_package(package, archive, write_checksums(package))
            publish_locally(package, archive, checksum, output)
        except BaseException:
            if (package / "build.log").exists():
                log = package / "build.log"
                print("\nBuild log tail:\n" + "\n".join(log.read_text().splitlines()[-20:]),
                      file=sys.stderr)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist",
                        help="output directory (default: repository dist/); never overwrites")
    args = parser.parse_args()
    try:
        package_preview(args.output.resolve())
    except (ValueError, OSError, subprocess.SubprocessError, KeyError) as error:
        print(f"preview packaging failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
