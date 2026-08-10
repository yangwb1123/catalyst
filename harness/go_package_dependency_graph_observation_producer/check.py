#!/usr/bin/env python3
"""CLI for deterministic ADR-0053 fixture checking."""

from __future__ import annotations

import sys
from pathlib import Path

HARNESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HARNESS))

from governance_contract import ContractError, read_bounded_file  # noqa: E402

if __package__ in {None, ""}:
    from go_package_dependency_graph_observation_producer import (  # type: ignore
        CHECKED, decode_production, validate_golden_fixture, validate_production,
    )
else:
    from . import (CHECKED, decode_production, validate_golden_fixture,
                   validate_production)
from go_package_dependency_graph_observation_producer.constants import (  # noqa: E402
    MAX_PRODUCTION_BYTES,
)


def report(issues: list[str]) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(CHECKED)
    return 0


def _production_issues(path: Path) -> list[str]:
    try:
        raw = read_bounded_file(
            path, label="local Go dependency graph production",
            max_bytes=MAX_PRODUCTION_BYTES,
        )
        production = decode_production(raw)
    except (OSError, ContractError) as error:
        return [f"{path}: {error}"]
    return validate_production(production)


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) == 2 and arguments[0] == "--golden":
        root = Path(arguments[1])
        if not root.is_dir():
            print(f"repository root does not exist: {root}", file=sys.stderr)
            return 2
        return report(validate_golden_fixture(root))
    if len(arguments) == 1:
        return report(_production_issues(Path(arguments[0])))
    print("usage: check.py --golden <repo-root> | <production.json>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
