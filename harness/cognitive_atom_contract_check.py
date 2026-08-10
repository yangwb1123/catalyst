#!/usr/bin/env python3
"""CLI for exact, non-authoritative CognitiveAtom shadow reprojection."""

from __future__ import annotations

import sys
from pathlib import Path

from cognitive_atom_contract import (SUCCESS, canonical_atom_payload,
                                     check_projection_bytes, compute_atom_digest,
                                     compute_atom_id, compute_atom_set_digest,
                                     decode_atom_set, project_atom_set,
                                     project_atom_set_bytes, project_record_set,
                                     source_closure, validate_atom_set,
                                     validate_golden_fixture, validate_projection)
from governance_contract import ContractError, canonical_json, read_bounded_file

__all__ = [
    "SUCCESS", "canonical_atom_payload", "canonical_json", "check_projection_bytes",
    "compute_atom_digest", "compute_atom_id", "compute_atom_set_digest",
    "decode_atom_set", "project_atom_set", "project_atom_set_bytes", "source_closure",
    "project_record_set", "validate_atom_set", "validate_golden_fixture",
    "validate_projection",
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
    if len(arguments) != 4:
        print("usage: cognitive_atom_contract_check.py <repo-root> <task-id> "
              "<governance-record-set.json> <cognitive-atom-set.json>\n"
              "   or: cognitive_atom_contract_check.py --golden <repo-root>",
              file=sys.stderr)
        return 2
    repo_root, task_id = Path(arguments[0]), arguments[1]
    if not repo_root.is_dir():
        print(f"repository root does not exist: {repo_root}", file=sys.stderr)
        return 2
    try:
        source_raw = read_bounded_file(Path(arguments[2]), label="governance record set")
        atom_raw = read_bounded_file(Path(arguments[3]), label="cognitive atom set")
    except ContractError as error:
        return _report([str(error)])
    except OSError as error:
        print(f"cannot read projection input: {error}", file=sys.stderr)
        return 2
    return _report(check_projection_bytes(task_id, source_raw, atom_raw))


def main(argv: list[str] | None = None) -> int:
    """Run either mode without leaking a memory-exhaustion traceback."""
    try:
        return _main(sys.argv[1:] if argv is None else argv)
    except MemoryError:
        return _report(["cognitive atom contract processing exhausted memory"])


if __name__ == "__main__":
    raise SystemExit(main())
