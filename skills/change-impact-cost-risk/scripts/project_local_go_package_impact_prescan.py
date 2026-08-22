#!/usr/bin/env python3
"""Project the exact ADR-0062 local Go lexical ImpactPreScan request wire."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    sys.stderr.write(
        "local Go package ImpactPreScan projection rejected: "
        "isolated no-bytecode Python (-I -B) is required\n"
    )
    raise SystemExit(1)

import importlib.util
from pathlib import Path

ADAPTER_MODULE = "_forgeos_change_impact_cost_risk_portable_adapter"


def _load_adapter() -> object:
    sys.modules.pop(ADAPTER_MODULE, None)
    path = Path(__file__).resolve().parent / "_adapter.py"
    specification = importlib.util.spec_from_file_location(ADAPTER_MODULE, path)
    if specification is None or specification.loader is None:
        raise RuntimeError("anchored shared adapter is unavailable")
    module = importlib.util.module_from_spec(specification)
    sys.modules[ADAPTER_MODULE] = module
    specification.loader.exec_module(module)
    return module


try:
    _adapter = _load_adapter()
except BaseException:
    sys.stderr.write("local Go package ImpactPreScan projection rejected\n")
    raise SystemExit(1)

if __name__ == "__main__":
    raise SystemExit(_adapter.main(sys.argv[1:]))
