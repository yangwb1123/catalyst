#!/usr/bin/env python3
"""ForgeOS YAML→JSON transcoder (`forge yaml2json`).

Bridges ForgeOS's declarative ``.agent/`` YAML assets into JSON so the
parallel Go runtime (``forge-core``) can read them with only the standard
library — Go's stdlib ships no YAML parser, so the boundary is crossed here,
once, in the harness rather than vendoring a YAML dependency into the runtime.

Dependencies: ``python3`` + PyYAML (``pip install pyyaml``). PyYAML is the
sole third-party requirement; if it is missing the tool exits 2 with a clear
actionable message rather than crashing (same import guard as
``harness/check.py``).

CLI contract::

    python3 harness/yaml2json.py <path.yml>

Reads the file with ``yaml.safe_load`` and writes the result to stdout as
JSON (``json.dumps`` with ``ensure_ascii=False`` and ``sort_keys=True`` for
deterministic, diff-stable output).

Exit 0 + JSON on stdout when the file parses.
Exit 1 + ``forge-yaml2json: <reason>`` on stderr when the argument is missing,
the file is absent, or YAML parsing fails.
Exit 2 + ``forge-yaml2json: PyYAML is required ...`` when PyYAML is unavailable.

Design: single responsibility (read one YAML file → emit JSON); each function
<=50 lines. Sibling to ``harness/check.py`` (polyglot harness tools are
allowed).
"""
import json
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - clear actionable error, not a crash
    sys.stderr.write(
        "forge-yaml2json: PyYAML is required (pip install pyyaml). "
        "Could not import 'yaml'.\n"
    )
    sys.exit(2)


def load_yaml(path):
    """Parse one YAML file; return (data, error_message_or_None)."""
    file_path = Path(path)
    if not file_path.is_file():
        return None, f"no such file: {file_path}"
    try:
        with file_path.open(encoding="utf-8") as fh:
            return yaml.safe_load(fh), None
    except (yaml.YAMLError, OSError) as exc:
        return None, f"invalid YAML in {file_path} ({str(exc).replace(chr(10), ' ')})"


def to_json(data):
    """Serialize parsed YAML to deterministic, unicode-preserving JSON."""
    return json.dumps(data, ensure_ascii=False, sort_keys=True)


def main(argv):
    """CLI entry: transcode argv[1] (a YAML path) to JSON on stdout."""
    if len(argv) != 2:
        sys.stderr.write(
            "forge-yaml2json: exactly one argument required.\n"
            "  usage: python3 harness/yaml2json.py <path.yml>\n"
        )
        return 1
    data, err = load_yaml(argv[1])
    if err:
        sys.stderr.write(f"forge-yaml2json: {err}\n")
        return 1
    print(to_json(data))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
