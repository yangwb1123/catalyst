#!/usr/bin/env python3
"""CLI for ContextPackage v1 golden and exact-instance checks."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from context_package_contract import (ContractError, Utf8ByteTokenCounter, decode_package,
                                      decode_request, validate_package)
from context_package_contract.codec import read_bounded_file
from context_package_contract.constants import MAX_PACKAGE_BYTES, MAX_REQUEST_BYTES
from context_package_contract.fixture import check_golden


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Return integration-check issues instead of raising on golden drift."""
    try:
        check_golden(repo_root)
    except ContractError as error:
        return [f"ContextPackage v1 golden invalid: {error}"]
    return []


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="validate ContextPackage v1")
    parser.add_argument("repo_root", nargs="?", type=Path)
    parser.add_argument("request", nargs="?", type=Path)
    parser.add_argument("package", nargs="?", type=Path)
    parser.add_argument("--golden", dest="golden_root", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden_root is not None:
        if any(value is not None for value in (args.repo_root, args.request, args.package)):
            raise ContractError("--golden cannot be combined with instance paths")
        check_golden(args.golden_root.resolve())
        print("ContextPackage v1 golden: OK")
        return
    if args.repo_root is None or args.request is None or args.package is None:
        raise ContractError("instance mode requires REPO_ROOT REQUEST PACKAGE")
    if not args.repo_root.resolve().is_dir():
        raise ContractError("repo root is not a directory")
    request = decode_request(read_bounded_file(args.request, MAX_REQUEST_BYTES, "build request"))
    package = decode_package(read_bounded_file(args.package, MAX_PACKAGE_BYTES, "context package"))
    validate_package(request, package, Utf8ByteTokenCounter())
    print("ContextPackage v1 instance: OK")


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except ContractError as error:
        print(f"ContextPackage v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
