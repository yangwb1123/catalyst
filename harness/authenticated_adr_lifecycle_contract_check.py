#!/usr/bin/env python3
"""Offline structural checker; it never performs or authorizes an ADR transition."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from authenticated_adr_lifecycle_contract import (ContractError, SUCCESS_MARKER,
                                                   decode_document, load_golden)
from authenticated_adr_lifecycle_contract.canonical import read_bounded_file
from authenticated_adr_lifecycle_contract.constants import MAX_GOLDEN_BYTES


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="validate one authenticated ADR lifecycle v1 structural candidate")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--golden", metavar="REPO_ROOT", type=Path)
    mode.add_argument("--file", metavar="BUNDLE_JSON", type=Path)
    return parser


def _run(args: argparse.Namespace) -> None:
    if args.golden is not None:
        load_golden(args.golden.resolve())
    else:
        raw = read_bounded_file(args.file, MAX_GOLDEN_BYTES,
                                "ADR lifecycle candidate bundle")
        decode_document(raw)
    print(SUCCESS_MARKER)


def main(argv: list[str] | None = None) -> int:
    try:
        _run(_parser().parse_args(argv))
    except (ContractError, OSError) as error:
        print(f"Authenticated ADR lifecycle v1 candidate: ERROR: {error}",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
