#!/usr/bin/env python3
"""stdin-only projector for the frozen ADR-0086 canonical request wire."""

from __future__ import annotations

import sys

from legacy_governance_read_import_contract import ContractError, project_request


def main() -> int:
    if len(sys.argv) != 1:
        print("usage: legacy_governance_read_import_contract_check.py < request.json", file=sys.stderr)
        return 2
    try:
        request = sys.stdin.buffer.read()
        projected = project_request(request)
    except (ContractError, OSError) as error:
        print(f"legacy governance read import rejected: {error}", file=sys.stderr)
        return 2
    sys.stdout.buffer.write(projected + b"\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
