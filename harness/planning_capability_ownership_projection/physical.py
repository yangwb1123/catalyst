"""No-follow exact current-source reads for the ADR-0069 physical checker."""

from __future__ import annotations

import hashlib
import os
import stat
from pathlib import Path

from .codec import ContractError
from .constants import (
    CATALOG_BYTES, CATALOG_PATH, CATALOG_SHA256, MAPPING_BYTES, MAPPING_PATH,
    MAPPING_SHA256, MAX_CATALOG_BYTES, MAX_MAPPING_BYTES,
)
from .projection import project
from .request import build_request

Identity = tuple[int, int, int, int, int, int]


def _identity(value: os.stat_result) -> Identity:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


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
    if _identity(before) != _identity(after):
        raise ContractError("repository root identity changed during resolution")
    return root, _identity(after)


def _components(root: Path, relative: str,
                snapshots: dict[Path, Identity]) -> Path:
    current = root
    for index, component in enumerate(Path(relative).parts):
        current = current / component
        try:
            metadata = current.lstat()
        except OSError as error:
            raise ContractError(f"cannot inspect {relative}: {error}") from error
        actual = _identity(metadata)
        if snapshots.get(current, actual) != actual:
            raise ContractError(f"path identity changed during operation: {relative}")
        snapshots[current] = actual
        if stat.S_ISLNK(metadata.st_mode):
            raise ContractError(f"path is or traverses symlink: {relative}")
        if index + 1 < len(Path(relative).parts) and not stat.S_ISDIR(metadata.st_mode):
            raise ContractError(f"path traverses non-directory: {relative}")
    return current


def _assert_snapshots(snapshots: dict[Path, Identity]) -> None:
    for path, expected in snapshots.items():
        try:
            actual = _identity(path.lstat())
        except OSError as error:
            raise ContractError(f"path changed during operation: {error}") from error
        if actual != expected:
            raise ContractError(f"path changed during operation: {path}")


def read_regular(root: Path, relative: str, maximum: int,
                 snapshots: dict[Path, Identity]) -> bytes:
    path = _components(root, relative, snapshots)
    descriptor = None
    try:
        descriptor = os.open(
            path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) |
            getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0),
        )
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise ContractError(f"path is not a regular file: {relative}")
        with os.fdopen(descriptor, "rb", closefd=True) as stream:
            descriptor = None
            raw = stream.read(maximum + 1)
            after = os.fstat(stream.fileno())
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot safely read {relative}: {error}") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    if _identity(before) != _identity(after) or _identity(before) != snapshots[path]:
        raise ContractError(f"file identity changed during read: {relative}")
    if len(raw) > maximum:
        raise ContractError(f"file exceeds byte bound: {relative}")
    _assert_snapshots(snapshots)
    return raw


def build_current(candidate: Path) -> tuple[dict[str, object], dict[str, object]]:
    root, root_identity = stable_root(candidate)
    snapshots = {root: root_identity}
    catalog = read_regular(root, CATALOG_PATH, MAX_CATALOG_BYTES, snapshots)
    mapping = read_regular(root, MAPPING_PATH, MAX_MAPPING_BYTES, snapshots)
    if (len(catalog), hashlib.sha256(catalog).hexdigest()) != (CATALOG_BYTES, CATALOG_SHA256):
        raise ContractError("current catalog bytes differ from frozen observation")
    if (len(mapping), hashlib.sha256(mapping).hexdigest()) != (MAPPING_BYTES, MAPPING_SHA256):
        raise ContractError("current mapping bytes differ from frozen observation")
    request = build_request(catalog, mapping)
    projection = project(request)
    _assert_snapshots(snapshots)
    return request, projection
