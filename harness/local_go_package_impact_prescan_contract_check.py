#!/usr/bin/env python3
"""CLI for pure local ADR-0062 envelope validation."""

from __future__ import annotations

import sys
from pathlib import Path

from governance_contract import ContractError, read_bounded_file
from local_go_package_impact_prescan_contract import (
    validate_envelope_bytes, validate_golden_fixture,
)
from local_go_package_impact_prescan_contract.constants import (
    CHECKED, MAX_ENVELOPE_BYTES,
)


def report(issues: list[str]) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(CHECKED)
    return 0


def _envelope_issues(path: Path) -> list[str]:
    try:
        raw = read_bounded_file(
            path, label="ADR-0062 envelope", max_bytes=MAX_ENVELOPE_BYTES)
    except (OSError, ContractError) as error:
        return [f"{path}: {error}"]
    return validate_envelope_bytes(raw)


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) == 2 and arguments[0] == "--golden":
        root = Path(arguments[1])
        if not root.is_dir():
            print(f"repository root does not exist: {root}", file=sys.stderr)
            return 2
        return report(validate_golden_fixture(root))
    if len(arguments) == 1:
        return report(_envelope_issues(Path(arguments[0])))
    print("usage: local_go_package_impact_prescan_contract_check.py "
          "--golden <repo-root> | <envelope.json>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
