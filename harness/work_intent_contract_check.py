#!/usr/bin/env python3
"""Focused CLI for the WorkIntent v1 Proposed candidate semantic core."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from work_intent_contract import (ContractError, SUCCESS_MARKER,
                                  decode_work_intent, load_golden)
from work_intent_contract.codec import read_bounded_file


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="validate one exact authority-neutral WorkIntent v1 declaration")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--golden", metavar="REPO_ROOT", type=Path)
    mode.add_argument("--file", metavar="WORK_INTENT_JSON", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden is not None:
        load_golden(args.golden.resolve())
    else:
        decode_work_intent(read_bounded_file(args.file, "WorkIntent instance"))
    print(SUCCESS_MARKER)


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except (ContractError, OSError) as error:
        print(f"WorkIntent v1: ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
