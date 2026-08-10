#!/usr/bin/env python3
"""CLI for deterministic local command observation producer fixture checks."""

from __future__ import annotations

import sys
from pathlib import Path

from local_command_observation_producer import CHECKED, validate_golden_fixture

__all__ = ["CHECKED", "validate_golden_fixture"]


def report(issues: list[str]) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(CHECKED)
    return 0


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) != 2 or arguments[0] != "--golden":
        print("usage: local_command_observation_producer_check.py --golden <repo-root>",
              file=sys.stderr)
        return 2
    root = Path(arguments[1])
    if not root.is_dir():
        print(f"repository root does not exist: {root}", file=sys.stderr)
        return 2
    try:
        return report(validate_golden_fixture(root))
    except MemoryError:
        return report(["local producer fixture checking exhausted memory"])


if __name__ == "__main__":
    raise SystemExit(main())
