#!/usr/bin/env python3
"""CLI for one explicit ADR v2 document or its byte-pinned golden fixture."""

from __future__ import annotations

import argparse
from pathlib import Path

from architecture_decision_record_v2 import (
    ContractError, SUCCESS, validate_document_file, validate_golden,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--golden", metavar="ROOT", type=Path)
    group.add_argument("--file", metavar="ADR_V2", type=Path)
    args = parser.parse_args()
    try:
        metadata = (validate_golden(args.golden) if args.golden is not None
                    else validate_document_file(args.file))
    except (ContractError, OSError) as error:
        print(f"INVALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2: {error}")
        return 1
    print(f"{SUCCESS}: {metadata['adr_id']} ({metadata['document_name']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
