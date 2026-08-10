#!/usr/bin/env python3
"""Bridge the legacy AI-batch contract suite into the main harness gate."""

import importlib.util
import unittest
from pathlib import Path


LEGACY_ROOT = Path(__file__).resolve().parents[1] / "docs" / "ai-batch"
LEGACY_SUITE = LEGACY_ROOT / "tests" / "test_cli_smoke.py"


class LegacyBundleAbsentTest(unittest.TestCase):
    """A scaffold need not carry the optional legacy documentation bundle."""

    def test_legacy_bundle_is_optional_for_scaffolds(self):
        self.skipTest("docs/ai-batch is not installed in this scaffold")


def load_tests(loader, _tests, _pattern):
    if not LEGACY_ROOT.exists():
        return loader.loadTestsFromTestCase(LegacyBundleAbsentTest)
    if not LEGACY_SUITE.is_file():
        raise FileNotFoundError(
            f"legacy AI-batch bundle exists but its contract suite is missing: {LEGACY_SUITE}")
    spec = importlib.util.spec_from_file_location(
        "_forge_legacy_ai_batch_contract", LEGACY_SUITE)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load legacy AI-batch suite: {LEGACY_SUITE}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return loader.loadTestsFromModule(module)


if __name__ == "__main__":
    unittest.main()
