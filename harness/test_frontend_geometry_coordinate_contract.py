#!/usr/bin/env python3
"""Adversarial coverage for the geometry report's common coordinate space."""
import hashlib
import json
import shutil
import sys
import unittest
from pathlib import Path

HARNESS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(HARNESS_DIR))
import frontend_design_check as frontend  # noqa: E402
import frontend_design_test_support as fixtures  # noqa: E402


def _artifact(package, artifact_id):
    return next(item for item in package["evidence_artifacts"] if item["id"] == artifact_id)


class GeometryCoordinateContractTest(unittest.TestCase):
    def setUp(self):
        self.repo = fixtures.make_temp_repo()
        self.package = fixtures.valid_package(self.repo)
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)

    def issues(self):
        return frontend.validate_frontend_package(self.package, self.repo)

    def assert_issue(self, text):
        issues = self.issues()
        self.assertTrue(any(text in issue for issue in issues), issues)

    def mutate_report(self, mutate):
        record = _artifact(self.package, "geometry-report")
        path = self.repo / record["locator"]
        value = json.loads(path.read_text(encoding="utf-8"))
        mutate(value)
        payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
        path.write_bytes(payload)
        record["bytes"] = len(payload)
        record["content_sha256"] = "sha256:" + hashlib.sha256(payload).hexdigest()

    def test_fixture_uses_one_capture_bound_coordinate_space(self):
        self.assertEqual(self.issues(), [])

    def test_coordinate_space_is_required(self):
        self.mutate_report(lambda value: value.pop("coordinate_space"))
        self.assert_issue("missing fields ['coordinate_space']")

    def test_coordinate_space_vocabulary_is_closed(self):
        self.mutate_report(lambda value: value["coordinate_space"].update(unit="px"))
        self.assert_issue("coordinate_space.unit: invalid")
        self.mutate_report(lambda value: value["coordinate_space"].update(
            unit="css_px", origin="document_top_left",
        ))
        self.assert_issue("coordinate_space.origin: invalid")
        self.mutate_report(lambda value: value["coordinate_space"].update(
            origin="capture_viewport_top_left", axis_orientation="y_up",
        ))
        self.assert_issue("coordinate_space.axis_orientation: invalid")

    def test_css_and_logical_units_bind_scale_to_capture_dpr(self):
        self.mutate_report(lambda value: value["coordinate_space"].update(
            device_pixels_per_unit=2,
        ))
        self.assert_issue("device_pixels_per_unit: must equal capture environment dpr")
        self.mutate_report(lambda value: value["coordinate_space"].update(
            unit="logical_dp", device_pixels_per_unit=3,
        ))
        self.assert_issue("device_pixels_per_unit: must equal capture environment dpr")

    def test_device_pixel_scale_is_exactly_one(self):
        self.mutate_report(lambda value: value["coordinate_space"].update(
            unit="device_px", device_pixels_per_unit=2,
        ))
        self.assert_issue("device_pixels_per_unit: device_px requires 1")

    def test_observations_cannot_override_the_report_coordinate_space(self):
        self.mutate_report(lambda value: value["assertions"][0]["observations"][0].update(
            unit="device_px",
        ))
        self.assert_issue("unknown fields ['unit']")


if __name__ == "__main__":
    unittest.main()
