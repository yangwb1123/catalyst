"""Behavioral reuse locks for ADR-0088 cross-contract value objects."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from kernel_operational_contract import (ContractError, golden_closure,
                                         seal_capability_invocation)

ROOT = Path(__file__).resolve().parents[1]
OPERATIONAL = ROOT / "docs/contracts/kernel-operational-reference-core-v1.schema.json"
CAPABILITY_GRANT = ROOT / "docs/contracts/capability-grant-v1.schema.json"
APPROVAL = ROOT / "docs/contracts/approval-record-v1.schema.json"
TRANSITION = ROOT / "docs/contracts/transition-receipt-v1.schema.json"
KNOWLEDGE = ROOT / "docs/contracts/knowledge-update-proposal-v1.schema.json"
BOOTSTRAP_EXECUTION = ROOT / "docs/contracts/bootstrap-repo-read-execution-v1.schema.json"


def _schema(path: Path, definition: str) -> dict:
    source = json.loads(path.read_text(encoding="utf-8"))
    return {"$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": source["$defs"], "$ref": f"#/$defs/{definition}"}


class KernelOperationalCrossContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        try:
            import jsonschema
        except ImportError:
            raise unittest.SkipTest("jsonschema is unavailable")
        cls.jsonschema = jsonschema
        cls.closure = golden_closure()

    def _accepts(self, value: object, path: Path, definition: str) -> None:
        self.jsonschema.validate(value, _schema(path, definition))

    def _rejects(self, value: object, path: Path, definition: str) -> None:
        with self.assertRaises(self.jsonschema.ValidationError):
            self._accepts(value, path, definition)

    def test_principal_reuses_adr0056_shape_and_behavior(self) -> None:
        principal = self.closure["capability_invocations"][0]["subject"]
        for path, definition in ((OPERATIONAL, "principal"),
                                 (CAPABILITY_GRANT, "principal")):
            self._accepts(principal, path, definition)
            self._rejects({**principal, "authenticated": True}, path, definition)
            self._rejects({**principal, "principal_type": "tool"}, path, definition)
            missing = copy.deepcopy(principal)
            missing.pop("authority_domain")
            self._rejects(missing, path, definition)

    def test_task_binding_reuses_all_adr0056_nullable_and_enum_semantics(self) -> None:
        binding = self.closure["capability_invocations"][0]["task_binding"]
        schemas = ((OPERATIONAL, "task_binding"), (CAPABILITY_GRANT, "task_binding"))
        for path, definition in schemas:
            self._accepts(binding, path, definition)
            alternate = {**binding, "attempt_id": "attempt-1", "target_id": "target-1",
                         "environment_class": "production"}
            self._accepts(alternate, path, definition)
            self._rejects({**binding, "environment_class": "prod"}, path, definition)
            self._rejects({**binding, "attempt_id": 1}, path, definition)

    def test_capability_identity_reuses_adr0056_exact_triple(self) -> None:
        capability = self.closure["capability_invocations"][0]["capability"]
        for path, definition in ((OPERATIONAL, "capability_identity"),
                                 (CAPABILITY_GRANT, "capability")):
            self._accepts(capability, path, definition)
            self._rejects({**capability, "version": capability["capability_version"]},
                          path, definition)
            self._rejects({**capability, "capability_contract_sha256": "A" * 64},
                          path, definition)

    def test_grant_ref_reuses_adr0060_triple_and_runtime_id_binding(self) -> None:
        reference = self.closure["capability_invocations"][0]["capability_grant_ref"]
        for path, definition in ((OPERATIONAL, "capability_grant_ref"),
                                 (TRANSITION, "grant_ref")):
            self._accepts(reference, path, definition)
            self._rejects({**reference, "issuer": "fixture"}, path, definition)
        invocation = copy.deepcopy(self.closure["capability_invocations"][0])
        invocation["invocation_id"], invocation["invocation_sha256"] = "", ""
        invocation["capability_grant_ref"]["grant_id"] = f"capability-grant-{'0' * 64}"
        with self.assertRaisesRegex(ContractError, "must bind"):
            seal_capability_invocation(invocation)

    def test_artifact_ref_reuses_adr0059_0060_0061_value_semantics(self) -> None:
        artifact = self.closure["artifacts"][0]
        schemas = ((OPERATIONAL, "artifact_ref"), (APPROVAL, "artifact"),
                   (TRANSITION, "artifact"), (KNOWLEDGE, "artifact"))
        for path, definition in schemas:
            self._accepts(artifact, path, definition)
            self._rejects({**artifact, "content_bytes": 1}, path, definition)
            self._rejects({**artifact, "artifact_sha256": "A" * 64}, path, definition)
            missing = copy.deepcopy(artifact)
            missing.pop("artifact_ref")
            self._rejects(missing, path, definition)

    def test_observed_usage_reuses_fields_not_bootstrap_zero_profile(self) -> None:
        operational_schema = _schema(OPERATIONAL, "observed_usage")
        bootstrap_schema = _schema(BOOTSTRAP_EXECUTION, "observed_usage")
        required = set(operational_schema["$defs"]["observed_usage"]["required"])
        self.assertEqual(required,
                         set(bootstrap_schema["$defs"]["observed_usage"]["required"]))
        bootstrap_value = {
            "call_count": 1, "cost_usd_micros": 0, "elapsed_ms": 17,
            "input_tokens": 0, "network_bytes": 0, "output_bytes": 55,
            "output_tokens": 0,
        }
        self.jsonschema.validate(bootstrap_value, operational_schema)
        self.jsonschema.validate(bootstrap_value, bootstrap_schema)
        nonzero = self.closure["execution_receipts"][0]["observed_usage"]
        self.jsonschema.validate(nonzero, operational_schema)
        with self.assertRaises(self.jsonschema.ValidationError):
            self.jsonschema.validate(nonzero, bootstrap_schema)
        for schema in (operational_schema, bootstrap_schema):
            self._assert_usage_shape_failures(schema, bootstrap_value)

    def test_schema_wire_text_rejects_surrogates_standalone_and_nested(self) -> None:
        source = json.loads(OPERATIONAL.read_text(encoding="utf-8"))
        wire_text = {"$schema": "https://json-schema.org/draft/2020-12/schema",
                     "$defs": source["$defs"], "$ref": "#/$defs/wire_text"}
        for value in ("\ud800", "ok\udfff"):
            with self.assertRaises(self.jsonschema.ValidationError):
                self.jsonschema.validate(value, wire_text)
            closure = copy.deepcopy(self.closure)
            closure["capability_invocations"][0]["subject"]["authority_domain"] = value
            with self.assertRaises(self.jsonschema.ValidationError):
                self.jsonschema.validate(closure, source)

    def _assert_usage_shape_failures(self, schema: dict, value: dict) -> None:
        missing = copy.deepcopy(value)
        missing.pop("call_count")
        extra = {**value, "measured": True}
        for candidate in (missing, extra):
            with self.assertRaises(self.jsonschema.ValidationError):
                self.jsonschema.validate(candidate, schema)


if __name__ == "__main__":
    unittest.main()
