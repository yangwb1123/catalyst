#!/usr/bin/env python3
"""Validate the Evolve repository locator Evidence shadow adapter contract."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from governance_contract import ContractError
from governance_contract.codec import read_bounded_file

from evolve_repo_locator_evidence_adapter import (SUCCESS, check_projection_bytes,
                                                   validate_golden_fixture)
from evolve_repo_locator_evidence_adapter.constants import MAX_REQUEST_BYTES


def _read(path: Path, label: str, max_bytes: int = MAX_REQUEST_BYTES) -> bytes:
    return read_bounded_file(path, label=label, max_bytes=max_bytes)


def _golden(repo_root: Path) -> int:
    issues = validate_golden_fixture(repo_root)
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(SUCCESS)
    return 0


def _projection(repo_root: Path, request_path: Path, evidence_path: Path) -> int:
    try:
        request_raw = _read(repo_root / request_path, "Evolve locator Evidence request")
        evidence_raw = _read(repo_root / evidence_path, "EvidenceRecord")
    except (OSError, ContractError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    issues = check_projection_bytes(request_raw, evidence_raw)
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(SUCCESS)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--golden", action="store_true")
    parser.add_argument("repo_root", type=Path)
    parser.add_argument("request", type=Path, nargs="?")
    parser.add_argument("evidence", type=Path, nargs="?")
    args = parser.parse_args(argv)
    root = args.repo_root.resolve()
    if args.golden:
        if args.request is not None or args.evidence is not None:
            parser.error("--golden does not accept request/evidence paths")
        return _golden(root)
    if args.request is None or args.evidence is None:
        parser.error("request and evidence paths are required without --golden")
    return _projection(root, args.request, args.evidence)


if __name__ == "__main__":
    raise SystemExit(main())
