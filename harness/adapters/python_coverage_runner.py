#!/usr/bin/env python3
"""Run the repository Python coverage probe with a fixed isolated import plan."""

from __future__ import annotations

import sys
from collections.abc import Callable, Sequence
from pathlib import Path


PYTEST_ARGUMENTS = (
    "--import-mode=importlib",
    "-p",
    "no:cacheprovider",
    "--cov",
    "--cov-report=json:coverage.json",
    "-q",
)


def _repository_paths(script_path: Path) -> tuple[Path, Path]:
    script = script_path.resolve(strict=True)
    adapters = script.parent
    harness = adapters.parent
    root = harness.parent
    if adapters.name != "adapters" or harness.name != "harness":
        raise ValueError("coverage runner must remain at harness/adapters")
    return root, harness


def _validated_arguments(arguments: Sequence[str]) -> tuple[str, ...]:
    supplied = tuple(arguments)
    if supplied != PYTEST_ARGUMENTS:
        raise ValueError("coverage runner arguments do not match the fixed profile")
    return supplied


def run(
    arguments: Sequence[str],
    *,
    flags: object = sys.flags,
    script_path: Path = Path(__file__),
    pytest_main: Callable[[list[str]], int] | None = None,
) -> int:
    if not getattr(flags, "isolated", False) or not getattr(
        flags, "dont_write_bytecode", False
    ):
        sys.stderr.write("python coverage rejected: isolated no-bytecode Python is required\n")
        return 2
    try:
        pytest_arguments = _validated_arguments(arguments)
        root, harness = _repository_paths(script_path)
    except (OSError, ValueError) as error:
        sys.stderr.write(f"python coverage rejected: {error}\n")
        return 2
    sys.path[:0] = [str(harness), str(root)]
    if pytest_main is None:
        try:
            import pytest
        except ModuleNotFoundError as error:
            if error.name != "pytest":
                raise
            sys.stderr.write("python coverage unavailable: No module named pytest\n")
            return 3
        pytest_main = pytest.main
    return int(pytest_main(list(pytest_arguments)))


if __name__ == "__main__":
    raise SystemExit(run(sys.argv[1:]))
