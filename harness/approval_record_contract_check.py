#!/usr/bin/env python3
"""CLI for ADR-0059 golden and exact declared-assessment checks."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from approval_record_contract import (ContractError, decode_assessment, decode_request,
                                      load_golden, validate_assessment)
from approval_record_contract.canonical import read_bounded_file
from approval_record_contract.constants import MAX_ASSESSMENT_BYTES, MAX_REQUEST_BYTES


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Return integration issues without minting any approval authority."""
    try:
        load_golden(repo_root)
    except ContractError as error:
        return [f"ApprovalRecord v1 golden invalid: {error}"]
    return []


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="validate ApprovalRecord v1 declarations")
    parser.add_argument("repo_root", nargs="?", type=Path)
    parser.add_argument("request", nargs="?", type=Path)
    parser.add_argument("assessment", nargs="?", type=Path)
    parser.add_argument("--golden", dest="golden_root", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden_root is not None:
        if any(value is not None for value in (args.repo_root, args.request, args.assessment)):
            raise ContractError("--golden cannot be combined with instance paths")
        load_golden(args.golden_root.resolve())
        print("ApprovalRecord v1 golden: OK (declarations only; no authority)")
        return
    if args.repo_root is None or args.request is None or args.assessment is None:
        raise ContractError("instance mode requires REPO_ROOT REQUEST ASSESSMENT")
    if not args.repo_root.resolve().is_dir():
        raise ContractError("repo root is not a directory")
    load_golden(args.repo_root.resolve())
    request = decode_request(read_bounded_file(args.request, MAX_REQUEST_BYTES, "request"))
    assessment = decode_assessment(read_bounded_file(
        args.assessment, MAX_ASSESSMENT_BYTES, "assessment"))
    validate_assessment(request, assessment)
    print("ApprovalRecord v1 instance: OK (declarations only; no authority)")


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except ContractError as error:
        print(f"ApprovalRecord v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

