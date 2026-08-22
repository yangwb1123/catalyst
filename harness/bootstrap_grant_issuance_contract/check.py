#!/usr/bin/env python3
"""Dependency-free structural CLI for ADR-0057; it never authenticates Ed25519."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from bootstrap_grant_issuance_contract.canonical import (  # noqa: E402
    ContractError,
    read_bounded_file,
)
from bootstrap_grant_issuance_contract.constants import MAX_GOLDEN_BYTES  # noqa: E402
from bootstrap_grant_issuance_contract.contract import (  # noqa: E402
    decode_document,
    load_fixture,
)

SUCCESS_SUFFIX = "structural/digest/chain only; Ed25519 NOT authenticated"


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Return integration issues without claiming signature authentication."""
    try:
        load_fixture(repo_root)
    except ContractError as error:
        return [f"bootstrap Grant issuance v1 golden invalid: {error}"]
    return []


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="validate ADR-0057 structural contracts")
    parser.add_argument("repo_root", nargs="?", type=Path)
    parser.add_argument("document", nargs="?", type=Path)
    parser.add_argument("--golden", dest="golden_root", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden_root is not None:
        if args.repo_root is not None or args.document is not None:
            raise ContractError("--golden cannot be combined with instance paths")
        load_fixture(args.golden_root.resolve())
        print(f"Bootstrap Grant issuance v1 golden: OK ({SUCCESS_SUFFIX})")
        return
    if args.repo_root is None or args.document is None:
        raise ContractError("instance mode requires REPO_ROOT DOCUMENT.json")
    root = args.repo_root.resolve()
    if not root.is_dir():
        raise ContractError("repo root is not a directory")
    load_fixture(root)
    raw = read_bounded_file(args.document, MAX_GOLDEN_BYTES, "issuance document")
    decode_document(raw)
    print(f"Bootstrap Grant issuance v1 instance: OK ({SUCCESS_SUFFIX})")


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except ContractError as error:
        print(f"Bootstrap Grant issuance v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
