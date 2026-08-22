#!/usr/bin/env python3
"""Focused checker for Decision Capsule structural replay core v1."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from decision_capsule_contract import (
    ContractError, SUCCESS_MARKER, decode_structural_replay_closure, load_golden,
)
from decision_capsule_contract.codec import read_bounded_file
from decision_capsule_contract.constants import MAX_CLOSURE_BYTES


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="validate one authority-neutral Decision Capsule replay closure")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--golden", metavar="REPO_ROOT", type=Path)
    mode.add_argument("--file", metavar="REPLAY_CLOSURE_JSON", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden is not None:
        load_golden(args.golden.resolve())
    else:
        raw = read_bounded_file(args.file, "Decision Capsule replay closure",
                                MAX_CLOSURE_BYTES)
        decode_structural_replay_closure(raw)
    print(SUCCESS_MARKER)


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except (ContractError, OSError) as error:
        print(f"Decision Capsule structural replay core v1: ERROR: {error}",
              file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
