"""Bounded no-follow repository reads shared by builder and physical checker."""

from __future__ import annotations

import os
import stat
from pathlib import Path

from .codec import ContractError
from .shapes import repo_path

MAX_VISITED_PATHS = 65536
Identity = tuple[int, int, int, int, int, int]
Snapshots = dict[Path, Identity]


def identity(metadata: os.stat_result) -> Identity:
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_size,
            metadata.st_mtime_ns, metadata.st_ctime_ns)


def stable_root(candidate: Path) -> tuple[Path, Identity]:
    try:
        before = candidate.lstat()
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
            raise ContractError("repository root must be a real directory")
        root = candidate.resolve(strict=True)
        after = root.lstat()
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot inspect repository root: {error}") from error
    if identity(before) != identity(after):
        raise ContractError("repository root identity changed during resolution")
    return root, identity(after)


def guard_root(root: Path, root_identity: Identity) -> Snapshots:
    return {root: root_identity}


def _record(snapshots: Snapshots, path: Path, actual: Identity, label: str) -> None:
    prior = snapshots.get(path)
    if prior is not None and prior != actual:
        raise ContractError(f"{label} changed during operation: {path}")
    snapshots[path] = actual


def _visit(count: int) -> int:
    count += 1
    if count > MAX_VISITED_PATHS:
        raise ContractError(f"content-set scan exceeds {MAX_VISITED_PATHS} paths")
    return count


def component_snapshots(root: Path, relative: str,
                        expected: Snapshots | None = None) -> tuple[Path, Snapshots]:
    repo_path(relative, "content path")
    current, snapshots = root, {}
    parts = Path(relative).parts
    for index, component in enumerate(("",) + parts):
        current = root if index == 0 else current / component
        try:
            metadata = current.lstat()
        except OSError as error:
            raise ContractError(f"cannot inspect content path {relative}: {error}") from error
        actual = identity(metadata)
        if stat.S_ISLNK(metadata.st_mode):
            raise ContractError(f"content path is or traverses symlink: {relative}")
        if index < len(parts) and not stat.S_ISDIR(metadata.st_mode):
            raise ContractError(f"content path traverses non-directory: {relative}")
        if expected is not None:
            _record(expected, current, actual, "content path")
        snapshots[current] = actual
    return current, snapshots


def assert_snapshots(snapshots: Snapshots, label: str) -> None:
    for path, expected in snapshots.items():
        try:
            actual = path.lstat()
        except OSError as error:
            raise ContractError(f"{label} changed during operation: {error}") from error
        if identity(actual) != expected:
            raise ContractError(f"{label} changed during operation: {path}")


def read_regular(root: Path, relative: str, maximum: int,
                 expected: Snapshots | None = None) -> bytes:
    path, snapshots = component_snapshots(root, relative, expected)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = None
    try:
        descriptor = os.open(path, flags)
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise ContractError(f"content path is not a regular file: {relative}")
        with os.fdopen(descriptor, "rb", closefd=True) as stream:
            descriptor = None
            raw = stream.read(maximum + 1)
            after = os.fstat(stream.fileno())
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot safely read content path {relative}: {error}") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    if identity(before) != identity(after) or identity(before) != snapshots[path]:
        raise ContractError(f"content path identity changed during read: {relative}")
    assert_snapshots(snapshots, relative)
    if len(raw) > maximum:
        raise ContractError(f"content path exceeds {maximum} bytes: {relative}")
    return raw


def scan_regular(root: Path, relative_root: str, suffixes: tuple[str, ...],
                 expected: Snapshots | None = None) -> tuple[list[str], Snapshots]:
    start, snapshots = component_snapshots(root, relative_root, expected)
    pending, found, visited = [(start, relative_root)], [], 1
    while pending:
        directory, relative = pending.pop()
        try:
            before = identity(directory.lstat())
            stream = os.scandir(directory)
        except OSError as error:
            raise ContractError(f"cannot enumerate content set: {error}") from error
        with stream:
            _record(snapshots, directory, before, "content-set directory")
            if expected is not None:
                _record(expected, directory, before, "content-set directory")
            for entry in stream:
                visited = _visit(visited)
                path, child = Path(entry.path), f"{relative}/{entry.name}"
                try:
                    metadata = path.lstat()
                except OSError as error:
                    raise ContractError(f"cannot inspect content-set path {child}: {error}") from error
                actual = identity(metadata)
                _record(snapshots, path, actual, "content-set path")
                if expected is not None:
                    _record(expected, path, actual, "content-set path")
                if stat.S_ISLNK(metadata.st_mode):
                    raise ContractError(f"content-set selection encountered symlink: {child}")
                if stat.S_ISDIR(metadata.st_mode):
                    pending.append((path, child))
                elif not stat.S_ISREG(metadata.st_mode):
                    raise ContractError(f"content-set selection encountered special path: {child}")
                elif entry.name.endswith(suffixes):
                    found.append(child)
        if identity(directory.lstat()) != before:
            raise ContractError(f"content-set directory changed during scan: {relative}")
    assert_snapshots(snapshots, relative_root)
    return sorted(found, key=lambda item: item.encode("utf-8")), snapshots
