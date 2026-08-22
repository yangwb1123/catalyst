#!/usr/bin/env python3
"""CLI for pure local ADR-0065 envelope validation."""

from __future__ import annotations

import sys
from pathlib import Path

from governance_contract import ContractError, read_bounded_file
from graph_snapshot_contract import (
    validate_envelope_bytes, validate_golden_fixture,
    validate_test_source_envelope_bytes, validate_test_source_golden_fixture,
)
from graph_snapshot_contract.constants import CHECKED, MAX_ENVELOPE_BYTES
from graph_snapshot_contract.lexical_test_source_constants import CHECKED as TEST_CHECKED


def report(issues: list[str], checked: str = CHECKED) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(checked)
    return 0


def _envelope_issues(path: Path, validator=validate_envelope_bytes) -> list[str]:
    try:
        raw = read_bounded_file(
            path, label="ADR-0065 envelope", max_bytes=MAX_ENVELOPE_BYTES)
    except (OSError, ContractError) as error:
        return [f"{path}: {error}"]
    return validator(raw)


def _rooted_envelope_issues(root: Path, relative: Path,
                            validator=validate_envelope_bytes) -> list[str]:
    if relative.is_absolute() or ".." in relative.parts:
        return ["graph snapshot path must stay repository-relative"]
    candidate = root / relative
    try:
        if root.is_symlink() or not root.is_dir():
            return [f"{root}: expected non-symlink repository directory"]
        current = root
        for component in relative.parts:
            current = current / component
            if current.is_symlink():
                return [f"{current}: symlink path component is forbidden"]
        if not candidate.is_file():
            return [f"{candidate}: expected regular file"]
        resolved_root = root.resolve(strict=True)
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(resolved_root)
    except (OSError, RuntimeError, ValueError) as error:
        return [f"{candidate}: invalid rooted graph snapshot path: {error}"]
    return _envelope_issues(candidate, validator)


def _checked_root(arguments: list[str]) -> Path | None:
    root = Path(arguments[1])
    if not root.is_dir():
        print(f"repository root does not exist: {root}", file=sys.stderr)
        return None
    return root


def _test_source(arguments: list[str]) -> int:
    if len(arguments) == 2:
        return report(_envelope_issues(
            Path(arguments[1]), validate_test_source_envelope_bytes), TEST_CHECKED)
    if len(arguments) == 3:
        root, relative = Path(arguments[1]), Path(arguments[2])
        if not root.is_dir():
            print(f"repository root does not exist: {root}", file=sys.stderr)
            return 2
        return report(_rooted_envelope_issues(
            root, relative, validate_test_source_envelope_bytes), TEST_CHECKED)
    return 2


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) == 2 and arguments[0] == "--golden":
        root = _checked_root(arguments)
        if root is None:
            return 2
        issues = validate_golden_fixture(root)
        issues.extend(validate_test_source_golden_fixture(root))
        return report(issues)
    if len(arguments) == 2 and arguments[0] == "--test-source-golden":
        root = _checked_root(arguments)
        if root is None:
            return 2
        return report(validate_test_source_golden_fixture(root), TEST_CHECKED)
    if arguments and arguments[0] == "--test-source":
        result = _test_source(arguments)
        if result != 2:
            return result
    if len(arguments) == 1:
        return report(_envelope_issues(Path(arguments[0])))
    if len(arguments) == 2:
        root, relative = Path(arguments[0]), Path(arguments[1])
        if not root.is_dir():
            print(f"repository root does not exist: {root}", file=sys.stderr)
            return 2
        return report(_rooted_envelope_issues(root, relative))
    print("usage: graph_snapshot_contract_check.py "
          "--golden <repo-root> | --test-source-golden <repo-root> | "
          "--test-source <envelope.json> | <envelope.json> | "
          "<repo-root> <relative-envelope.json>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
