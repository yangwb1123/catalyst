#!/usr/bin/env python3
"""Validate the closed policy-authority portable package."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    sys.stderr.write(
        "policy-authority package rejected: isolated no-bytecode Python (-I -B) is required\n"
    )
    raise SystemExit(1)

import hashlib
import json
import os
import re
import stat
import unicodedata
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

API_VERSION = "forgeos.portable-skill-package-manifest/v1"
MANIFEST = "references/package-manifest.json"
SKILL_INSTRUCTIONS = "SKILL.md"
DIRECT_REFERENCES = ("references/contract.md", "references/evals.json")
REFERENCE_DEFINITION_RE = re.compile(r"(?m)^[ \t]{0,3}\[[^\]\r\n]+\]:")
REFERENCE_USE_RE = re.compile(r"\[[^\]\r\n]*\]\[[^\]\r\n]*\]")
IMAGE_LINK_RE = re.compile(r"!\[[^\]\r\n]*\](?:\(|\[)")
URI_AUTOLINK_RE = re.compile(
    r"<(?:[A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\s]*|[^<>\s]+@[^<>\s]+)>"
)
MAX_MANIFEST_BYTES = 64 * 1024
MAX_MANIFEST_JSON_DEPTH = 16
MAX_PACKAGE_FILES = 512
MAX_PACKAGE_DIRECTORIES = 512
MAX_PACKAGE_ENTRIES = 1024
MAX_PACKAGE_DEPTH = 64
MAX_PACKAGE_FILE_BYTES = 16 * 1024 * 1024
MAX_PACKAGE_BYTES = 64 * 1024 * 1024
PORTABLE_PATH_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-/"
)
WINDOWS_DEVICE_NAMES = frozenset(
    {"con", "prn", "aux", "nul"} |
    {f"com{index}" for index in range(1, 10)} |
    {f"lpt{index}" for index in range(1, 10)}
)
DESCRIPTOR_BOUNDARY_AVAILABLE = (
    all(hasattr(os, name) for name in ("O_CLOEXEC", "O_DIRECTORY", "O_NOFOLLOW")) and
    os.open in os.supports_dir_fd and
    os.stat in os.supports_dir_fd and
    os.stat in os.supports_follow_symlinks and
    os.scandir in os.supports_fd
)


class PackageError(ValueError):
    """A stable package-integrity rejection."""


@dataclass(frozen=True)
class FileObservation:
    path: str
    count: int
    mode: int
    digest: str
    identity: tuple[int, ...]
    retained_bytes: bytes | None


@dataclass(frozen=True)
class DirectoryObservation:
    path: str
    identity: tuple[int, ...]


def canonical_json(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode()


def pairs_no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise PackageError(f"duplicate manifest key: {key}")
        result[key] = value
    return result


def precheck_manifest_depth(raw: bytes) -> None:
    depth, quoted, escaped = 0, False, False
    for value in raw:
        if quoted:
            if escaped:
                escaped = False
            elif value == 0x5C:
                escaped = True
            elif value == 0x22:
                quoted = False
        elif value == 0x22:
            quoted = True
        elif value in (0x5B, 0x7B):
            depth += 1
            if depth > MAX_MANIFEST_JSON_DEPTH:
                raise PackageError("package manifest JSON is invalid")
        elif value in (0x5D, 0x7D):
            depth -= 1


def read_manifest(observation: FileObservation) -> dict[str, object]:
    raw = observation.retained_bytes
    if raw is None:
        raise PackageError("package manifest was not observed")
    if not raw or len(raw) > MAX_MANIFEST_BYTES or not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise PackageError("package manifest framing is invalid")
    precheck_manifest_depth(raw[:-1])
    try:
        value = json.loads(raw[:-1], object_pairs_hook=pairs_no_duplicates)
    except PackageError:
        raise
    except (MemoryError, RecursionError, UnicodeError, ValueError) as error:
        raise PackageError("package manifest JSON is invalid") from error
    try:
        canonical = canonical_json(value) + b"\n"
    except (MemoryError, RecursionError, UnicodeError, ValueError) as error:
        raise PackageError("package manifest text is invalid") from error
    if not isinstance(value, dict) or canonical != raw:
        raise PackageError("package manifest is not exact compact canonical JSON")
    return value


def validate_observed_path(value: object) -> str:
    if not isinstance(value, str) or not value or value != unicodedata.normalize("NFC", value):
        raise PackageError("manifest path is invalid")
    if not value.isascii() or any(character not in PORTABLE_PATH_CHARS for character in value):
        raise PackageError(f"manifest path is not portable: {value!r}")
    pure = PurePosixPath(value)
    if pure.is_absolute() or str(pure) != value or ".." in pure.parts:
        raise PackageError(f"manifest path escapes: {value!r}")
    for component in pure.parts:
        stem = component.split(".", 1)[0].casefold()
        if component.endswith(".") or stem in WINDOWS_DEVICE_NAMES:
            raise PackageError(f"manifest path has a platform alias: {value!r}")
    return value


def validate_relative_path(value: object) -> str:
    path = validate_observed_path(value)
    if path == MANIFEST:
        raise PackageError(f"manifest path self-includes: {value!r}")
    return path


def validate_file_record(value: object) -> tuple[str, int, int, str]:
    if not isinstance(value, dict) or set(value) != {"bytes", "mode", "path", "sha256"}:
        raise PackageError("manifest file record has the wrong shape")
    path = validate_relative_path(value["path"])
    count, mode, digest = value["bytes"], value["mode"], value["sha256"]
    if not isinstance(count, int) or isinstance(count, bool) or count < 1:
        raise PackageError(f"manifest byte count is invalid for {path}")
    if mode not in ("0644", "0755"):
        raise PackageError(f"manifest mode is invalid for {path}")
    if not isinstance(digest, str) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise PackageError(f"manifest digest is invalid for {path}")
    return path, count, int(mode, 8), digest


def _identity(value: os.stat_result) -> tuple[int, ...]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
            value.st_uid, value.st_gid, value.st_size, value.st_mtime_ns,
            value.st_ctime_ns)


def _open_root(root: Path) -> tuple[int, os.stat_result]:
    if not DESCRIPTOR_BOUNDARY_AVAILABLE:
        raise PackageError("descriptor-relative no-follow package validation is unavailable")
    before = os.lstat(root)
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
    descriptor = os.open(root, flags)
    opened = os.fstat(descriptor)
    if not stat.S_ISDIR(opened.st_mode) or _identity(before) != _identity(opened):
        os.close(descriptor)
        raise PackageError("package root is not a stable real directory")
    return descriptor, opened


def _read_open_file(descriptor: int, include_bytes: bool,
                    max_bytes: int) -> tuple[int, str, bytes | None]:
    digest = hashlib.sha256()
    count = 0
    captured = bytearray() if include_bytes else None
    while True:
        chunk = os.read(descriptor, 65536)
        if not chunk:
            break
        count += len(chunk)
        if count > max_bytes:
            raise PackageError("package file bytes exceed their bound")
        digest.update(chunk)
        if captured is not None:
            captured.extend(chunk)
    return count, digest.hexdigest(), bytes(captured) if captured is not None else None


def _observe_file(parent: int, name: str, path: str,
                  before: os.stat_result, max_bytes: int) -> FileObservation:
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
    descriptor = os.open(name, flags, dir_fd=parent)
    try:
        opened = os.fstat(descriptor)
        if (_identity(before) != _identity(opened) or
                not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1):
            raise PackageError(f"package file is redirected or hard-linked: {path}")
        if opened.st_size > max_bytes:
            raise PackageError(f"package file size exceeds its bound: {path}")
        retained = path == MANIFEST or path == SKILL_INSTRUCTIONS
        count, digest, raw = _read_open_file(descriptor, retained, max_bytes)
        after = os.fstat(descriptor)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if _identity(opened) != _identity(after) or _identity(after) != _identity(named):
            raise PackageError(f"package file changed while observed: {path}")
        return FileObservation(path, count, stat.S_IMODE(opened.st_mode),
                               digest, _identity(opened), raw)
    finally:
        os.close(descriptor)


def _open_child(parent: int, name: str, path: str,
                before: os.stat_result) -> tuple[int, os.stat_result]:
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent)
    opened = os.fstat(descriptor)
    if _identity(before) != _identity(opened) or not stat.S_ISDIR(opened.st_mode):
        os.close(descriptor)
        raise PackageError(f"package directory is redirected: {path}")
    return descriptor, opened


def _walk(directory: int, relative: str, depth: int,
          files: dict[str, FileObservation],
          directories: dict[str, DirectoryObservation], visited: list[int],
          total_bytes: list[int]) -> None:
    if depth > MAX_PACKAGE_DEPTH:
        raise PackageError("package directory depth exceeds its bound")
    before_directory = os.fstat(directory)
    with os.scandir(directory) as entries:
        for entry in entries:
            visited[0] += 1
            if visited[0] > MAX_PACKAGE_ENTRIES:
                raise PackageError("package entry count exceeds its bound")
            name = entry.name
            path = f"{relative}/{name}" if relative else name
            validate_observed_path(path)
            before = os.stat(name, dir_fd=directory, follow_symlinks=False)
            if stat.S_ISLNK(before.st_mode):
                raise PackageError(f"package contains symlink: {path}")
            if stat.S_ISDIR(before.st_mode):
                if len(directories) >= MAX_PACKAGE_DIRECTORIES:
                    raise PackageError("package directory count exceeds its bound")
                child, opened = _open_child(directory, name, path, before)
                directories[path] = DirectoryObservation(path, _identity(opened))
                try:
                    _walk(child, path, depth + 1, files, directories, visited,
                          total_bytes)
                finally:
                    os.close(child)
            elif stat.S_ISREG(before.st_mode):
                if len(files) >= MAX_PACKAGE_FILES:
                    raise PackageError("package file count exceeds its bound")
                remaining = MAX_PACKAGE_BYTES - total_bytes[0]
                limit = MAX_MANIFEST_BYTES if path == MANIFEST else MAX_PACKAGE_FILE_BYTES
                if remaining <= 0:
                    raise PackageError("package aggregate bytes exceed their bound")
                observed = _observe_file(directory, name, path, before,
                                         min(remaining, limit))
                files[path] = observed
                total_bytes[0] += observed.count
            else:
                raise PackageError(f"package contains special file: {path}")
    if _identity(before_directory) != _identity(os.fstat(directory)):
        raise PackageError(f"package directory changed while observed: {relative or '.'}")


def _open_directory_path(root: int, relative: str) -> int:
    descriptor = os.dup(root)
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
    try:
        for component in PurePosixPath(relative).parts if relative else ():
            child = os.open(component, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _reobserve_file(root: int, original: FileObservation) -> None:
    pure = PurePosixPath(original.path)
    parent_path = "" if str(pure.parent) == "." else str(pure.parent)
    parent = _open_directory_path(root, parent_path)
    try:
        before = os.stat(pure.name, dir_fd=parent, follow_symlinks=False)
        limit = MAX_MANIFEST_BYTES if original.path == MANIFEST else MAX_PACKAGE_FILE_BYTES
        current = _observe_file(parent, pure.name, original.path, before, limit)
    finally:
        os.close(parent)
    if (current.identity != original.identity or current.count != original.count or
            current.mode != original.mode or current.digest != original.digest or
            current.retained_bytes != original.retained_bytes):
        raise PackageError(f"package file changed between observations: {original.path}")


def _validate_direct_references(files: dict[str, FileObservation]) -> None:
    observation = files.get(SKILL_INSTRUCTIONS)
    raw = observation.retained_bytes if observation is not None else None
    if raw is None:
        raise PackageError("SKILL.md instructions were not observed")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise PackageError("SKILL.md instructions are not UTF-8") from error
    links = re.findall(r"(?<!!)\[([^\]\r\n]+)\]\(([^)\r\n]+)\)", text)
    expected = [(target, target) for target in DIRECT_REFERENCES]
    alternate = (text.count("](") != len(expected) or
                 REFERENCE_DEFINITION_RE.search(text) is not None or
                 REFERENCE_USE_RE.search(text) is not None or
                 IMAGE_LINK_RE.search(text) is not None or
                 URI_AUTOLINK_RE.search(text) is not None)
    if alternate or sorted(links) != sorted(expected):
        raise PackageError("SKILL.md direct references differ from the closed set")
    for unused_label, target in links:
        if validate_observed_path(target) != target or target not in files:
            raise PackageError(f"SKILL.md direct reference is broken: {target}")


def _expected_directories(paths: list[str]) -> set[str]:
    expected: set[str] = set()
    for path in paths + [MANIFEST]:
        parent = PurePosixPath(path).parent
        while str(parent) != ".":
            expected.add(str(parent))
            parent = parent.parent
    return expected


def _verify_observations(root: int, root_path: Path, root_before: os.stat_result,
                         files: dict[str, FileObservation],
                         directories: dict[str, DirectoryObservation]) -> None:
    for path in sorted(files):
        _reobserve_file(root, files[path])
    for path in sorted(directories):
        descriptor = _open_directory_path(root, path)
        try:
            if _identity(os.fstat(descriptor)) != directories[path].identity:
                raise PackageError(f"package directory changed: {path}")
        finally:
            os.close(descriptor)
    if (_identity(os.fstat(root)) != _identity(root_before) or
            _identity(os.lstat(root_path)) != _identity(root_before)):
        raise PackageError("package root changed while observed")


def validate_package(root: Path) -> None:
    descriptor, root_before = _open_root(root)
    files: dict[str, FileObservation] = {}
    directories: dict[str, DirectoryObservation] = {}
    try:
        _walk(descriptor, "", 0, files, directories, [0], [0])
        if MANIFEST not in files:
            raise PackageError("package manifest is absent")
        manifest = read_manifest(files[MANIFEST])
        records = _validate_manifest_root(manifest)
        paths = [item[0] for item in records]
        if set(files) != set(paths) | {MANIFEST}:
            raise PackageError("package file set differs from its closed manifest")
        if set(directories) != _expected_directories(paths):
            raise PackageError("package directory set differs from its closed manifest")
        for path, count, mode, digest in records:
            observed = files[path]
            if (observed.count != count or observed.mode != mode or
                    observed.digest != digest):
                raise PackageError(f"package file metadata or bytes drifted: {path}")
        _validate_direct_references(files)
        if files[MANIFEST].mode != 0o644:
            raise PackageError("package manifest metadata is invalid")
        _verify_observations(descriptor, root, root_before, files, directories)
    finally:
        os.close(descriptor)


def _validate_manifest_root(manifest: dict[str, object]) -> list[tuple[str, int, int, str]]:
    if set(manifest) != {"api_version", "files", "manifest_path", "package_name"}:
        raise PackageError("package manifest root shape is invalid")
    if (manifest["api_version"] != API_VERSION or manifest["manifest_path"] != MANIFEST or
            manifest["package_name"] != "policy-authority"):
        raise PackageError("package manifest fixed fields drifted")
    raw_files = manifest["files"]
    if not isinstance(raw_files, list) or not raw_files:
        raise PackageError("package manifest file set is empty")
    records = [validate_file_record(item) for item in raw_files]
    paths = [item[0] for item in records]
    if paths != sorted(set(paths)):
        raise PackageError("package manifest paths are not strictly sorted and unique")
    logical_paths = set(paths + [MANIFEST]) | _expected_directories(paths)
    folded = [path.casefold() for path in logical_paths]
    if len(folded) != len(set(folded)):
        raise PackageError("package manifest paths contain a case alias")
    return records


def main(argv: list[str]) -> int:
    if len(argv) > 1:
        print("usage: check_package.py [PACKAGE_ROOT]", file=sys.stderr)
        return 2
    try:
        root = (Path(os.path.abspath(argv[0])) if argv else
                Path(os.path.abspath(__file__)).parents[1])
        validate_package(root)
    except (MemoryError, RecursionError, NotImplementedError, OSError, PackageError) as error:
        print(f"policy-authority package rejected: {error}", file=sys.stderr)
        return 1
    print("policy-authority portable package VALID")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
