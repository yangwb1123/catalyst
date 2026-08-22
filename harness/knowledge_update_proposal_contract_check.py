#!/usr/bin/env python3
"""CLI for KnowledgeUpdateProposal v1 golden and declared-assessment checks."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from knowledge_update_proposal_contract import (ContractError, decode_assessment,
                                                  decode_request, load_golden,
                                                  validate_assessment)
from knowledge_update_proposal_contract.canonical import read_bounded_file
from knowledge_update_proposal_contract.constants import (MAX_ASSESSMENT_BYTES,
                                                           MAX_REQUEST_BYTES)


def validate_golden_fixture(repo_root: Path) -> list[str]:
    """Return contract issues without minting knowledge authority."""
    try:
        load_golden(repo_root)
    except ContractError as error:
        return [f"KnowledgeUpdateProposal v1 golden invalid: {error}"]
    return []


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="validate KnowledgeUpdateProposal v1 declarations")
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
        print("KnowledgeUpdateProposal v1 golden: OK "
              "(declarations only; no adoption or apply authority)")
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
    print("KnowledgeUpdateProposal v1 instance: OK "
          "(declarations only; no adoption or apply authority)")


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except (ContractError, ValueError) as error:
        print(f"KnowledgeUpdateProposal v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
