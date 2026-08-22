#!/usr/bin/env python3
"""Focused structural checker for Kernel decision reference core v1."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from kernel_decision_contract import (ContractError, SUCCESS_MARKER,
                                      decode_closure, load_golden)
from kernel_decision_contract.codec import read_bounded_file
from kernel_decision_contract.constants import MAX_CLOSURE_BYTES


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="validate one authority-neutral Kernel decision reference closure")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--golden", metavar="REPO_ROOT", type=Path)
    mode.add_argument("--file", metavar="CLOSURE_JSON", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden is not None:
        load_golden(args.golden.resolve())
    else:
        raw = read_bounded_file(args.file, "Kernel decision closure", MAX_CLOSURE_BYTES)
        decode_closure(raw)
    print(SUCCESS_MARKER)


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except (ContractError, OSError) as error:
        print(f"Kernel decision reference core v1: ERROR: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
