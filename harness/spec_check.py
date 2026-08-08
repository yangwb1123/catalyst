#!/usr/bin/env python3
"""spec_check — 解析 scheduled-successor-protocol.md 为键值对。

读取权威 spec 的 Protocol versions / Digest domains / Bounds /
Invariants / Identity shapes 表格(第一列 key,第二列 value),输出 JSON。
供 Go 与 Rust 的一致性测试使用;亦可 CLI 直接查询:

  python harness/spec_check.py --table versions --key candidate.v
  python harness/spec_check.py --json
"""

import argparse
import json
import re
import sys
from pathlib import Path

SPEC_PATH = Path(__file__).resolve().parent.parent / "docs" / "contracts" / "scheduled-successor-protocol.md"

TABLES = {
    "versions": "Protocol versions",
    "digests": "Domain-separated digest domains",
    "bounds": "Bounds",
    "invariants": "Invariants",
    "identities": "Identity shapes",
}


def parse_table(lines, header: str) -> dict:
    """Extracts a markdown table by its heading: rows are `| k | v | ...`."""
    result = {}
    in_table = False
    header_cells = None
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#") and header in stripped:
            in_table = True
            continue  # the heading itself is not a table row
        if in_table:
            if not stripped.startswith("|"):
                if stripped.startswith("#"):
                    in_table = False
                    continue
                if stripped:
                    continue
                continue
            cells = [c.strip() for c in stripped.strip("|").split("|")]
            if len(cells) >= 2 and not all(re.fullmatch(r":?-+:?", c) for c in cells):
                if header_cells is None:
                    header_cells = cells
                    continue
                key, value = cells[0], cells[1]
                if key and key != "key":
                    result[key] = value
                    if len(header_cells) >= 3 and header_cells[1] == "min":
                        result[key + ".max"] = cells[2]
    return result


def load_spec() -> dict:
    if not SPEC_PATH.is_file():
        raise FileNotFoundError(f"spec not found: {SPEC_PATH}")
    lines = SPEC_PATH.read_text(encoding="utf-8").splitlines()
    return {name: parse_table(lines, header) for name, header in TABLES.items()}


def main() -> int:
    parser = argparse.ArgumentParser(description="spec md 键值解析")
    parser.add_argument("--json", action="store_true", help="输出全部表为 JSON")
    parser.add_argument("--table", default="", help="表名(versions/digests/bounds/invariants/identities)")
    parser.add_argument("--key", default="", help="键名(与 --table 配合)")
    args = parser.parse_args()
    try:
        spec = load_spec()
    except FileNotFoundError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(spec, indent=2))
        return 0
    table = spec.get(args.table)
    if table is None:
        print(f"unknown table: {args.table}; valid: {', '.join(TABLES)}", file=sys.stderr)
        return 2
    if args.key:
        if args.key not in table:
            print(f"unknown key: {args.key}", file=sys.stderr)
            return 2
        print(table[args.key])
        return 0
    for key, value in table.items():
        print(f"{key} = {value}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
