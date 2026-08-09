#!/usr/bin/env python3
"""CLI and integration facade for the authority-free governance checker."""

from __future__ import annotations

import sys
from pathlib import Path

from governance_contract import (SUCCESS, ContractError, canonical_json,
                                 canonical_record_payload, check_record_set_bytes,
                                 compute_record_digest, decode_record_set,
                                 read_bounded_file, validate_golden_fixture,
                                 validate_record_set)

__all__ = [
    "SUCCESS", "canonical_json", "canonical_record_payload", "check_record_set_bytes",
    "compute_record_digest", "decode_record_set", "read_bounded_file",
    "validate_golden_fixture", "validate_record_set",
]


def _report_result(issues: list[str]) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(SUCCESS)
    return 0


def _check_golden(repo_root: Path) -> int:
    if not repo_root.is_dir():
        print(f"repository root does not exist: {repo_root}", file=sys.stderr)
        return 2
    return _report_result(validate_golden_fixture(repo_root))


def _main(arguments: list[str]) -> int:
    if len(arguments) == 2 and arguments[0] == "--golden":
        return _check_golden(Path(arguments[1]))
    if len(arguments) != 2:
        print("usage: governance_contract_check.py <repo-root> <record-set.json>\n"
              "   or: governance_contract_check.py --golden <repo-root>", file=sys.stderr)
        return 2
    repo_root, record_path = Path(arguments[0]), Path(arguments[1])
    if not repo_root.is_dir():
        print(f"repository root does not exist: {repo_root}", file=sys.stderr)
        return 2
    try:
        raw = read_bounded_file(record_path, label="record set")
    except ContractError as error:
        return _report_result([str(error)])
    except OSError as error:
        print(f"cannot read record set: {error}", file=sys.stderr)
        return 2
    return _report_result(check_record_set_bytes(raw))


def main(argv: list[str] | None = None) -> int:
    """Run either public mode without leaking a memory-exhaustion traceback."""
    try:
        return _main(sys.argv[1:] if argv is None else argv)
    except MemoryError:
        return _report_result(["governance contract processing exhausted memory"])


if __name__ == "__main__":
    raise SystemExit(main())
