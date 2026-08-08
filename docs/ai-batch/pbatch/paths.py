"""Canonical public-path references for repo and standalone layouts."""

from __future__ import annotations

from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parent.parent


def _resolve_path_base() -> Path:
    """Nearest ForgeOS root, or the copied tool directory when standalone."""
    for candidate in (SCRIPT_ROOT, *SCRIPT_ROOT.parents):
        try:
            if (candidate / ".agent").is_dir():
                return candidate.resolve()
        except OSError:
            continue
    return SCRIPT_ROOT


PATH_BASE_PATH = _resolve_path_base()
PATH_BASE = str(PATH_BASE_PATH)
IS_FORGE_PROJECT = (PATH_BASE_PATH / ".agent").is_dir()


def bundled_reference(relative: str) -> str:
    """Path-base-relative reference to a file shipped beside pi-batch.py."""
    target = (SCRIPT_ROOT / relative).resolve()
    if not target.is_file():
        raise RuntimeError(f"missing bundled ai-batch reference: {relative}")
    return target.relative_to(PATH_BASE_PATH).as_posix()


def project_or_bundled_reference(candidate: str, bundled: str) -> str:
    """Use a real project reference, otherwise its bundled standalone route."""
    path = Path(candidate)
    if not path.is_absolute():
        target = (PATH_BASE_PATH / path).resolve()
        try:
            target.relative_to(PATH_BASE_PATH)
            if target.is_file():
                return path.as_posix()
        except ValueError:
            pass
    return bundled_reference(bundled)
