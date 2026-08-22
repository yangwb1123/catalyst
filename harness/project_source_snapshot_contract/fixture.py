"""Deterministic v1 golden construction and physical fixture checking."""

from __future__ import annotations

import hashlib
import os
import stat
from pathlib import Path

from .codec import ContractError, canonical_json, path_digest
from .constants import (
    DOMAINS, FIXTURE_PATH, FIXTURE_SHA256, MAX_ENVELOPE_BYTES,
)
from .derive import build_production
from .validation import decode_production


def _hash(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def golden_value() -> dict[str, object]:
    entries = [
        {"bytes": 8, "content_sha256": _hash(b"fixture\n"), "executable": False,
         "index_mode": "100644", "kind": "regular", "path": "README.md",
         "tracking": "tracked"},
        {"bytes": 0, "content_sha256": None, "executable": None,
         "index_mode": "100644", "kind": "tracked_absent", "path": "deleted.txt",
         "tracking": "tracked"},
        {"bytes": 8, "content_sha256": _hash(b"scratch\n"), "executable": False,
         "index_mode": None, "kind": "regular", "path": "scratch.txt",
         "tracking": "untracked"},
    ]
    excluded = [
        _excluded(".env", None, False, "sensitive_path", "untracked"),
        _excluded(".forge/state.json", None, False, "control_path", "untracked"),
        _excluded("linked.txt", "120000", True, "symlink_leaf", "tracked"),
    ]
    git_raw = b"fixture-git-binary"
    git = {"executable_bytes": len(git_raw), "executable_sha256": _hash(git_raw),
           "version": "git version 2.50.1"}
    return build_production("fixture-project", "fixture-run-001", entries, excluded,
                            git, 2, "git-sha1:" + "1" * 40)


def _excluded(path: str, mode: str | None, observed: bool, reason: str,
              tracking: str) -> dict[str, object]:
    return {"index_mode": mode, "leaf_filesystem_observed": observed,
            "path_sha256": path_digest(path, DOMAINS["path"]), "reason": reason,
            "tracking": tracking}


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _stable_root(candidate: Path) -> tuple[Path, dict[Path, tuple[int, ...]]]:
    try:
        before = candidate.lstat()
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
            raise ContractError("golden root must be a real directory")
        root = candidate.resolve(strict=True)
        after = root.lstat()
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot inspect golden root: {error}") from error
    if _identity(before) != _identity(after):
        raise ContractError("golden root changed during resolution")
    return root, {root: _identity(after)}


def _fixture_path(root: Path, snapshots: dict[Path, tuple[int, ...]]) -> Path:
    current = root
    for index, component in enumerate(Path(FIXTURE_PATH).parts):
        current = current / component
        try:
            metadata = current.lstat()
        except OSError as error:
            raise ContractError(f"cannot inspect golden fixture: {error}") from error
        if stat.S_ISLNK(metadata.st_mode):
            raise ContractError("golden fixture path traverses a symlink")
        if index + 1 < len(Path(FIXTURE_PATH).parts) and not stat.S_ISDIR(metadata.st_mode):
            raise ContractError("golden fixture path traverses a non-directory")
        snapshots[current] = _identity(metadata)
    return current


def _assert_stable(snapshots: dict[Path, tuple[int, ...]]) -> None:
    for path, expected in snapshots.items():
        try:
            actual = _identity(path.lstat())
        except OSError as error:
            raise ContractError(f"golden fixture path changed: {error}") from error
        if actual != expected:
            raise ContractError("golden fixture path changed during read")


def read_regular(path: Path, maximum: int,
                 snapshots: dict[Path, tuple[int, ...]]) -> bytes:
    descriptor = None
    try:
        before_path = os.lstat(path)
        if stat.S_ISLNK(before_path.st_mode) or not stat.S_ISREG(before_path.st_mode):
            raise ContractError("golden fixture must be a real regular file")
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) |
                             getattr(os, "O_NOFOLLOW", 0) |
                             getattr(os, "O_NONBLOCK", 0))
        before_fd = os.fstat(descriptor)
        with os.fdopen(descriptor, "rb") as stream:
            descriptor = None
            raw = stream.read(maximum + 1)
            after_fd = os.fstat(stream.fileno())
        after_path = os.lstat(path)
    except ContractError:
        raise
    except OSError as error:
        raise ContractError(f"cannot read golden fixture: {error}") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    if (_identity(before_path) != _identity(before_fd) or
            _identity(before_fd) != _identity(after_fd) or
            _identity(after_fd) != _identity(after_path)):
        raise ContractError("golden fixture changed during read")
    if not 1 <= len(raw) <= maximum:
        raise ContractError("golden fixture outside byte bound")
    _assert_stable(snapshots)
    return raw


def load_golden(repo_root: Path) -> dict[str, object]:
    root, snapshots = _stable_root(repo_root)
    raw = read_regular(_fixture_path(root, snapshots), MAX_ENVELOPE_BYTES + 1,
                       snapshots)
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n"):
        raise ContractError("golden fixture must have exactly one terminal LF")
    if FIXTURE_SHA256 == "TO_BE_GENERATED":
        raise ContractError("golden fixture physical SHA-256 pin is not frozen")
    if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA256:
        raise ContractError("golden fixture physical SHA-256 drifted")
    value = decode_production(raw[:-1])
    if canonical_json(value) != canonical_json(golden_value()):
        raise ContractError("golden fixture differs from exact reconstruction")
    return value


if __name__ == "__main__":
    import sys
    sys.stdout.buffer.write(canonical_json(golden_value()) + b"\n")
