#!/usr/bin/env python3
"""Focused regressions for shared engineering YAML parsing."""
import os
from pathlib import Path
import tempfile
import unittest

import engineering_check_support as support


class EngineeringCheckSupportTest(unittest.TestCase):
    def setUp(self):
        support._YAML_PARSE_CACHE.clear()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name) / "contract.yml"

    def test_unused_anchor_is_rejected_without_an_alias_event(self):
        self.path.write_text("value: &unused 1\n", encoding="utf-8")
        data, error = support.load_yaml(self.path)
        self.assertIsNone(data)
        self.assertIn("anchors and aliases", error)

    def test_cache_identity_is_exact_bytes_not_restorable_metadata(self):
        self.path.write_text("a: 1\n", encoding="utf-8")
        original = self.path.stat()
        self.assertEqual(support.load_yaml(self.path), ({"a": 1}, None))
        self.path.write_text("b: 2\n", encoding="utf-8")
        os.utime(self.path, ns=(original.st_atime_ns, original.st_mtime_ns))
        self.assertEqual(support.load_yaml(self.path), ({"b": 2}, None))

    def test_first_result_cannot_mutate_the_cached_parse_tree(self):
        self.path.write_text("outer:\n  values: [one, two]\n", encoding="utf-8")
        first, error = support.load_yaml(self.path)
        self.assertIsNone(error)
        first["outer"]["values"].append("poison")
        self.assertEqual(
            support.load_yaml(self.path),
            ({"outer": {"values": ["one", "two"]}}, None),
        )


if __name__ == "__main__":
    unittest.main()
