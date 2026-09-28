#!/usr/bin/env python3
"""Offline evidence regressions that do not require a compiled Runtime binary."""

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import forge_runtime_release_smoke as smoke

class ToolEvidenceTests(unittest.TestCase):
    def test_failed_truncated_or_wrong_content_is_rejected(self):
        good = {"type": "tool_finished", "name": "read_file", "is_error": False,
                "truncated": False, "output": smoke.README_CONTENT}
        smoke.verify_tool_events([good], {"read_file": smoke.README_CONTENT})
        for changed in ({"is_error": True}, {"truncated": True}, {"output": "wrong"}):
            with self.subTest(changed=changed), self.assertRaises(smoke.SmokeFailure):
                smoke.verify_tool_events([{**good, **changed}], {"read_file": smoke.README_CONTENT})

    def test_provider_must_receive_the_expected_read_bytes(self):
        item = {"type": "function_call_output", "call_id": "call-1", "output": smoke.NOTE_BEFORE}
        request = {"input": [item]}
        smoke.verify_provider_output([request], "call-1", smoke.NOTE_BEFORE)
        wrong = copy.deepcopy(request)
        wrong["input"][0]["output"] = "read_file: file does not exist"
        with self.assertRaises(smoke.SmokeFailure):
            smoke.verify_provider_output([wrong], "call-1", smoke.NOTE_BEFORE)

    def test_final_answer_does_not_substitute_for_tool_evidence(self):
        final = {"type": "run_finished", "outcome": {"status": "completed", "answer": "done"}}
        with self.assertRaises(smoke.SmokeFailure):
            smoke.verify_tool_events([final], {"read_file": smoke.README_CONTENT})
        with self.assertRaises(smoke.SmokeFailure):
            smoke.verify_provider_output([{"input": []}], "call-1", smoke.NOTE_BEFORE)

    def test_failed_dev_read_is_rejected_despite_successful_edit_and_exec(self):
        events = [{"type": "tool_finished", "name": name, "is_error": name == "read_file",
                   "truncated": False, "output": "missing file" if name == "read_file" else "ok"}
                  for name in ("read_file", "edit_file", "exec_command")]
        with self.assertRaisesRegex(smoke.SmokeFailure, "read_file returned a tool error"):
            smoke.verify_tool_events(events, {"read_file": smoke.NOTE_BEFORE,
                                             "edit_file": None, "exec_command": None})

    def test_check_failure_replaces_an_old_success_report(self):
        with tempfile.TemporaryDirectory(prefix="forge-smoke-report-test-") as directory:
            report = Path(directory) / "report.json"
            report.write_text('{"status":"passed","stale":true}')
            with patch.object(smoke.Smoke, "execute", side_effect=smoke.SmokeFailure("bad read")):
                with patch("sys.stderr"):
                    status = smoke.main([str(Path(directory) / "unused-binary"), str(report)])
            outcome = json.loads(report.read_text())
            self.assertEqual(status, 1)
            self.assertEqual(outcome["status"], "failed")
            self.assertNotIn("stale", outcome)
            self.assertIn("bad read", outcome["error"])


if __name__ == "__main__":
    unittest.main()
