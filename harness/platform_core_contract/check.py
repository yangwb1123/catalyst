#!/usr/bin/env python3
"""Focused independent checker for Platform Core Envelope v1."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HARNESS = Path(__file__).resolve().parents[1]
sys.path[:] = [entry for entry in sys.path if entry != str(HARNESS)]
sys.path.insert(0, str(HARNESS))

import platform_core_contract as contract_package
from platform_core_contract import (ContractError, SUCCESS_MARKER,
                                    RECEIPT_SUCCESS_MARKER,
                                    decode_execution_receipt,
                                    decode_artifact_ref,
                                    decode_command_envelope,
                                    decode_event_envelope,
                                    decode_verification_receipt,
                                    decode_verification_request, load_golden,
                                    load_receipt_golden)
from platform_core_contract.constants import (MAX_ARTIFACT_REF_BYTES,
                                              MAX_ENVELOPE_BYTES,
                                              MAX_RECEIPT_BYTES)
from platform_core_contract.contract import read_bounded_file

if Path(contract_package.__file__).resolve() != HARNESS / "platform_core_contract/__init__.py":
    raise RuntimeError("Platform Core checker imported a non-repository implementation")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="validate exact Platform Core v1 bytes")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--golden", metavar="REPO_ROOT", type=Path)
    source.add_argument("--receipt-golden", metavar="REPO_ROOT", type=Path)
    source.add_argument("--artifact", metavar="ARTIFACT_JSON", type=Path)
    source.add_argument("--command", metavar="COMMAND_JSON", type=Path)
    source.add_argument("--event", metavar="EVENT_JSON", type=Path)
    source.add_argument("--execution-receipt", metavar="RECEIPT_JSON", type=Path)
    source.add_argument("--verification-request", metavar="REQUEST_JSON", type=Path)
    source.add_argument("--verification-receipt", metavar="RECEIPT_JSON", type=Path)
    return parser


def _run(args: argparse.Namespace) -> str:
    if args.golden is not None:
        load_golden(args.golden)
        return SUCCESS_MARKER
    if args.receipt_golden is not None:
        load_receipt_golden(args.receipt_golden)
        return RECEIPT_SUCCESS_MARKER
    elif args.artifact is not None:
        decode_artifact_ref(read_bounded_file(args.artifact, MAX_ARTIFACT_REF_BYTES))
        return SUCCESS_MARKER
    elif args.command is not None:
        decode_command_envelope(read_bounded_file(args.command, MAX_ENVELOPE_BYTES))
        return SUCCESS_MARKER
    if args.event is not None:
        decode_event_envelope(read_bounded_file(args.event, MAX_ENVELOPE_BYTES))
        return SUCCESS_MARKER
    if args.execution_receipt is not None:
        decode_execution_receipt(read_bounded_file(args.execution_receipt, MAX_RECEIPT_BYTES))
    elif args.verification_request is not None:
        decode_verification_request(read_bounded_file(args.verification_request, MAX_RECEIPT_BYTES))
    else:
        decode_verification_receipt(read_bounded_file(args.verification_receipt, MAX_RECEIPT_BYTES))
    return RECEIPT_SUCCESS_MARKER


def main(argv: list[str] | None = None) -> int:
    """Run the CLI and map contract rejection to stable exit 2."""
    try:
        marker = _run(_parser().parse_args(argv))
    except (ContractError, OSError, RuntimeError, ValueError) as error:
        print(f"Platform Core v1: ERROR: {error}", file=sys.stderr)
        return 2
    print(marker)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
