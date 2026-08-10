#!/usr/bin/env python3
"""CLI for exact, effect-free Artifact provenance Evidence adaptation checks."""

from __future__ import annotations

import sys
from pathlib import Path

from artifact_evidence_adapter import (SUCCESS, adapt_request, canonical_artifact,
                                       canonical_json, check_projection_bytes,
                                       compute_request_digest, compute_source_digest,
                                       decode_evidence_record, decode_request,
                                       timestamp_unix_ms, validate_evidence_record,
                                       validate_golden_fixture, validate_projection,
                                       validate_request)
from governance_contract import ContractError, read_bounded_file
from governance_contract.constants import MAX_RECORD_BYTES

__all__ = [
    "SUCCESS", "adapt_request", "canonical_artifact", "canonical_json",
    "check_projection_bytes", "compute_request_digest", "compute_source_digest",
    "decode_evidence_record", "decode_request", "timestamp_unix_ms",
    "validate_evidence_record", "validate_golden_fixture", "validate_projection",
    "validate_request",
]


def _report(issues: list[str]) -> int:
    if issues:
        for issue in issues:
            print(f"ERROR: {issue}", file=sys.stderr)
        return 1
    print(SUCCESS)
    return 0


def _golden(repo_root: Path) -> int:
    if not repo_root.is_dir():
        print(f"repository root does not exist: {repo_root}", file=sys.stderr)
        return 2
    return _report(validate_golden_fixture(repo_root))


def _main(arguments: list[str]) -> int:
    if len(arguments) == 2 and arguments[0] == "--golden":
        return _golden(Path(arguments[1]))
    if len(arguments) != 3:
        print("usage: artifact_evidence_adapter_check.py <repo-root> <request.json> "
              "<evidence-record.json>\n   or: artifact_evidence_adapter_check.py "
              "--golden <repo-root>", file=sys.stderr)
        return 2
    repo_root = Path(arguments[0])
    if not repo_root.is_dir():
        print(f"repository root does not exist: {repo_root}", file=sys.stderr)
        return 2
    try:
        request_raw = read_bounded_file(Path(arguments[1]), label="adapter request",
                                        max_bytes=MAX_RECORD_BYTES)
        evidence_raw = read_bounded_file(Path(arguments[2]), label="evidence record",
                                         max_bytes=MAX_RECORD_BYTES)
    except ContractError as error:
        return _report([str(error)])
    except OSError as error:
        print(f"cannot read adapter input: {error}", file=sys.stderr)
        return 2
    return _report(check_projection_bytes(request_raw, evidence_raw))


def main(argv: list[str] | None = None) -> int:
    """Run either mode without leaking a memory-exhaustion traceback."""
    try:
        return _main(sys.argv[1:] if argv is None else argv)
    except MemoryError:
        return _report(["artifact evidence adapter processing exhausted memory"])


if __name__ == "__main__":
    raise SystemExit(main())
