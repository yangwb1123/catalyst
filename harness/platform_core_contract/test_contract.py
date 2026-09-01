"""Golden, adversarial, and boundary tests for Platform Core Envelope v1."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "harness"
if str(HARNESS) not in sys.path:
    sys.path.insert(0, str(HARNESS))

from platform_core_contract import (
    ContractError, SUCCESS_MARKER, artifact_ref_sha256, canonical_artifact_ref,
    canonical_command_envelope, canonical_event_envelope, command_envelope_sha256,
    decode_artifact_ref, decode_command_envelope, decode_event_envelope,
    event_envelope_sha256, load_golden, validate_artifact_ref,
    validate_command_envelope, validate_event_envelope, validate_platform_id)
from platform_core_contract.codec import canonical_json
from platform_core_contract.constants import *  # noqa: F403
from platform_core_contract.file_read_tests import PlatformCoreFileReadTests  # noqa: F401

CHECKER = ROOT / "harness/platform_core_contract/check.py"
SCHEMA = ROOT / SCHEMA_PATH  # noqa: F405


class _FlipOnRead(dict):
    """Expose one different field value after a configured number of reads."""

    def __init__(self, source: dict, field: str, replacement, flip_after: int) -> None:
        super().__init__(source)
        self.field = field
        self.replacement = replacement
        self.flip_after = flip_after
        self.reads = 0

    def __getitem__(self, key):
        value = super().__getitem__(key)
        if key == self.field:
            self.reads += 1
            if self.reads >= self.flip_after:
                return self.replacement
        return value


class PlatformCoreGoldenTests(unittest.TestCase):
    """Shared fixture and independent checker parity."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = load_golden(ROOT)

    def test_all_canonical_digests_and_round_trips_match(self) -> None:
        expected = self.fixture["expected"]
        artifact = self.fixture["artifact_ref"]
        command = self.fixture["command_envelope"]
        event = self.fixture["event_envelope"]
        self.assertEqual(artifact_ref_sha256(artifact), expected["artifact_ref_sha256"])
        self.assertEqual(command_envelope_sha256(command), expected["command_envelope_sha256"])
        self.assertEqual(event_envelope_sha256(event), expected["event_envelope_sha256"])
        self.assertEqual(decode_artifact_ref(canonical_artifact_ref(artifact)), artifact)
        self.assertEqual(decode_command_envelope(canonical_command_envelope(command)), command)
        self.assertEqual(decode_event_envelope(canonical_event_envelope(event)), event)

    def test_golden_checker_has_exact_non_authority_marker(self) -> None:
        result = subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), "--golden", str(ROOT)],
            cwd=ROOT, text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, SUCCESS_MARKER + "\n")

    def test_explicit_command_checker_accepts_only_canonical_file(self) -> None:
        canonical = canonical_command_envelope(self.fixture["command_envelope"])
        with tempfile.TemporaryDirectory() as directory:
            valid = Path(directory) / "command.json"
            invalid = Path(directory) / "noncanonical.json"
            valid.write_bytes(canonical)
            invalid.write_bytes(b" " + canonical)
            accepted = self._check("--command", valid)
            rejected = self._check("--command", invalid)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertEqual(rejected.returncode, 2)

    def test_schema_shadow_matches_executable_structural_edges(self) -> None:
        try:
            import jsonschema
        except ImportError as error:
            self.skipTest(f"jsonschema unavailable: {error}")
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        for field in ("artifact_ref", "command_envelope", "event_envelope"):
            jsonschema.validate(self.fixture[field], schema)
        for field in ("command_envelope", "event_envelope"):
            for payload, artifact in (
                    (None, None), ({"unexpected": "second body"}, self.fixture["artifact_ref"])):
                invalid = copy.deepcopy(self.fixture[field])
                invalid["payload"] = payload
                invalid["payload_artifact_ref"] = copy.deepcopy(artifact)
                validator = (validate_command_envelope if field == "command_envelope"
                             else validate_event_envelope)
                with self.subTest(field=field, payload=payload), self.assertRaises(ContractError):
                    validator(invalid)
                with self.assertRaises(jsonschema.ValidationError):
                    jsonschema.validate(invalid, schema)
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["schema_name"] = "a.b.c"
        command["schema_version"] = 2
        validate_command_envelope(command)
        jsonschema.validate(command, schema)
        invalid = copy.deepcopy(self.fixture["event_envelope"])
        invalid["source_snapshot_ref"]["entity_type"] = "project"
        with self.assertRaises(ContractError):
            validate_event_envelope(invalid)
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(invalid, schema)
        artifact = copy.deepcopy(self.fixture["artifact_ref"])
        artifact["media_type"] = "application/vnd.forge+json"
        validate_artifact_ref(artifact)
        jsonschema.validate(artifact, schema)
        for suffix in ("\n", "\u2028", "\u2029"):
            for field in ("media_type", "logical_id", "content_digest"):
                invalid = copy.deepcopy(artifact)
                invalid[field] += suffix
                with self.subTest(suffix=repr(suffix), field=field), self.assertRaises(ContractError):
                    validate_artifact_ref(invalid)
                with self.assertRaises(jsonschema.ValidationError):
                    jsonschema.validate(invalid, schema)
        artifact["media_type"] = "application/+json"
        with self.assertRaises(ContractError):
            validate_artifact_ref(artifact)
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(artifact, schema)

    @staticmethod
    def _check(mode: str, path: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-I", "-B", str(CHECKER), mode, str(path)],
            cwd=ROOT, text=True, capture_output=True, check=False,
        )


class PlatformCoreStrictWireTests(unittest.TestCase):
    """Malformed framing and duplicate/unknown field rejection."""

    def setUp(self) -> None:
        self.fixture = load_golden(ROOT)

    def test_command_decoder_rejects_malformed_wire(self) -> None:
        canonical = canonical_command_envelope(self.fixture["command_envelope"])
        actor = canonical_json(self.fixture["command_envelope"]["actor_ref"], 4096)
        cases = {
            "leading": b" " + canonical,
            "trailing": canonical + b"null",
            "unknown": canonical.replace(b'{"actor_ref":', b'{"extra":true,"actor_ref":', 1),
            "duplicate": canonical.replace(
                b'{"actor_ref":', b'{"actor_ref":' + actor + b',"actor_ref":', 1),
            "nested duplicate": canonical.replace(
                b'"payload":{"agent_adapter":"codex",',
                b'"payload":{"agent_adapter":"codex","agent_adapter":"codex",', 1),
            "float": canonical.replace(b'"expected_version":3', b'"expected_version":3.0', 1),
            "overflow": canonical.replace(
                b'"expected_version":3', b'"expected_version":9223372036854775808', 1),
            "boolean envelope": canonical.replace(
                b'"envelope_version":1', b'"envelope_version":true', 1),
            "boolean schema": canonical.replace(
                b'"schema_version":1', b'"schema_version":true', 1),
            "boolean expected": canonical.replace(
                b'"expected_version":3', b'"expected_version":true', 1),
            "boolean issued": canonical.replace(
                b'"issued_at_unix_ms":1787961600000', b'"issued_at_unix_ms":true', 1),
            "boolean deadline": canonical.replace(
                b'"deadline_unix_ms":1787961660000', b'"deadline_unix_ms":true', 1),
            "future envelope": canonical.replace(
                b'"envelope_version":1', b'"envelope_version":2', 1),
            "missing nullable": canonical.replace(b'"causation_id":null,', b'', 1),
            "forbidden": canonical.replace(b'"agent_adapter":"codex"',
                                                   b'"agent_adapter":"codex\\u202e"', 1),
            "invalid UTF-8": canonical[:20] + b"\xff" + canonical[21:],
        }
        for name, raw in cases.items():
            with self.subTest(name=name), self.assertRaises(ContractError):
                decode_command_envelope(raw)

    def test_event_and_artifact_decoders_reject_shape_drift(self) -> None:
        event = canonical_event_envelope(self.fixture["event_envelope"])
        artifact = canonical_artifact_ref(self.fixture["artifact_ref"])
        cases = (
            (decode_event_envelope, event.replace(
                b'"actor_type":"service"', b'"actor_type":"service","extra":true', 1)),
            (decode_event_envelope, event.replace(b'"extensions":{},', b'', 1)),
            (decode_event_envelope, event.replace(
                b'"envelope_version":1', b'"envelope_version":2', 1)),
            (decode_event_envelope, event.replace(
                b'"aggregate_version":4', b'"aggregate_version":true', 1)),
            (decode_event_envelope, event.replace(b'"sequence":9', b'"sequence":true', 1)),
            (decode_event_envelope, event.replace(
                b'"occurred_at_unix_ms":1787961605000', b'"occurred_at_unix_ms":true', 1)),
            (decode_event_envelope, event.replace(
                b'"source_component":"runtime"', b'"source_component":"unknown"', 1)),
            (decode_artifact_ref, artifact.replace(b'"size_bytes":4096', b'"size_bytes":4e3', 1)),
            (decode_artifact_ref, artifact.replace(
                b'"size_bytes":4096', b'"size_bytes":true', 1)),
            (decode_artifact_ref, artifact.replace(
                b'"created_at_unix_ms":1787961604000', b'"created_at_unix_ms":true', 1)),
            (decode_artifact_ref, artifact.replace(b'"sensitivity":"internal",', b'', 1)),
            (decode_artifact_ref, artifact + b"\n"),
        )
        for decoder, raw in cases:
            with self.assertRaises(ContractError):
                decoder(raw)

    def test_public_decoders_require_exact_immutable_bytes(self) -> None:
        cases = (
            (decode_artifact_ref, canonical_artifact_ref(self.fixture["artifact_ref"])),
            (decode_command_envelope, canonical_command_envelope(self.fixture["command_envelope"])),
            (decode_event_envelope, canonical_event_envelope(self.fixture["event_envelope"])),
        )
        for decoder, raw in cases:
            for malformed in (bytearray(raw), memoryview(raw), raw.decode("utf-8"), True):
                with self.subTest(decoder=decoder.__name__, kind=type(malformed).__name__), \
                        self.assertRaises(ContractError):
                    decoder(malformed)

    def test_writers_round_trip_builtins_and_reject_mapping_subclasses(self) -> None:
        cases = (
            ("artifact_ref", canonical_artifact_ref, decode_artifact_ref,
             "content_digest", "0" * 64),
            ("command_envelope", canonical_command_envelope, decode_command_envelope,
             "envelope_version", 2),
            ("event_envelope", canonical_event_envelope, decode_event_envelope,
             "envelope_version", 2),
        )
        for field, writer, decoder, changing_field, replacement in cases:
            source = copy.deepcopy(self.fixture[field])
            expected = writer(source)
            with self.subTest(field=field):
                canonical = writer(source)
                self.assertEqual(canonical, expected)
                decoder(canonical)
                with self.assertRaises(ContractError) as captured:
                    writer(_FlipOnRead(source, changing_field, replacement, 2))
                self.assertEqual(captured.exception.code, VALUE_INVALID)  # noqa: F405


class PlatformCoreSemanticTests(unittest.TestCase):
    """Cross-field identity, scope, artifact, and timestamp relations."""

    def setUp(self) -> None:
        self.fixture = load_golden(ROOT)

    def test_command_relations_fail_closed(self) -> None:
        mutations = (
            lambda value: value.__setitem__("command_id", "cmd_0000000000000000000000000f"),
            lambda value: value.__setitem__("causation_id", value["message_id"]),
            lambda value: value["target_ref"].__setitem__("entity_type", "attempt"),
            lambda value: value["target_ref"].__setitem__(
                "entity_id", "wki_0000000000000000000000000f"),
            lambda value: value.__setitem__("deadline_unix_ms", value["issued_at_unix_ms"] - 1),
            lambda value: value.__setitem__("expected_version", -1),
            lambda value: value.__setitem__("idempotency_key", "short"),
            lambda value: value.__setitem__("payload_artifact_ref", self.fixture["artifact_ref"]),
            lambda value: value.__setitem__("payload", None),
            lambda value: value.__setitem__("extensions", None),
        )
        self._reject_each("command_envelope", validate_command_envelope, mutations)

    def test_event_relations_fail_closed(self) -> None:
        mutations = (
            lambda value: value.__setitem__("event_id", "evt_0000000000000000000000000f"),
            lambda value: value.__setitem__("causation_id", value["message_id"]),
            lambda value: value["aggregate_ref"].__setitem__(
                "entity_id", "atm_0000000000000000000000000f"),
            lambda value: value.__setitem__("aggregate_version", 0),
            lambda value: value.__setitem__("sequence", 0),
            lambda value: value.__setitem__("source_component", "unknown"),
            lambda value: value.__setitem__("source_snapshot_ref", None),
            lambda value: value["source_snapshot_ref"].__setitem__(
                "entity_id", "psn_0000000000000000000000000f"),
            lambda value: value["payload_artifact_ref"].__setitem__(
                "created_at_unix_ms", value["occurred_at_unix_ms"] + 1),
            lambda value: value["payload_artifact_ref"]["source_snapshot_ref"].__setitem__(
                "entity_id", "psn_0000000000000000000000000f"),
            lambda value: value["payload_artifact_ref"].__setitem__(
                "producer_attempt_id", "atm_0000000000000000000000000f"),
        )
        self._reject_each("event_envelope", validate_event_envelope, mutations)

    def test_scope_and_artifact_relations_fail_closed(self) -> None:
        for field in ("project_id", "objective_id", "change_id", "work_graph_id",
                      "work_item_id"):
            value = copy.deepcopy(self.fixture["command_envelope"])
            value["scope_ref"][field] = None
            with self.assertRaises(ContractError):
                validate_command_envelope(value)
        mutations = (
            lambda value: value.__setitem__("content_id", "sha256:" + "0" * 64),
            lambda value: value.__setitem__("content_digest", "bad"),
            lambda value: value.__setitem__(
                "logical_id", "atm_0000000000000000000000000b"),
            lambda value: value["source_snapshot_ref"].__setitem__("entity_type", "project"),
            lambda value: value["provenance_ref"].__setitem__("record_type", "runtime"),
            lambda value: value.__setitem__("media_type", "application/json; charset=utf-8"),
            lambda value: value.__setitem__("media_type", "application/+json"),
            lambda value: value.__setitem__("size_bytes", MAX_ARTIFACT_BYTES + 1),  # noqa: F405
        )
        self._reject_each("artifact_ref", validate_artifact_ref, mutations)
        artifact = copy.deepcopy(self.fixture["artifact_ref"])
        artifact["media_type"] = "application/vnd.forge+json"
        validate_artifact_ref(artifact)

    def test_artifact_backed_command_relations_fail_closed(self) -> None:
        mutations = (
            lambda value: value["payload_artifact_ref"]["source_snapshot_ref"].__setitem__(
                "entity_id", "psn_0000000000000000000000000f"),
            lambda value: value["payload_artifact_ref"].__setitem__(
                "producer_attempt_id", "atm_0000000000000000000000000f"),
            lambda value: value["payload_artifact_ref"].__setitem__(
                "created_at_unix_ms", value["issued_at_unix_ms"] + 1),
        )
        for mutate in mutations:
            value = self._artifact_backed_command()
            mutate(value)
            with self.assertRaises(ContractError):
                validate_command_envelope(value)

    def test_deep_scope_edges_require_each_parent(self) -> None:
        for child, parent in (("session_id", "attempt_id"),
                              ("turn_id", "session_id"), ("action_id", "turn_id")):
            value = copy.deepcopy(self.fixture["event_envelope"])
            value["scope_ref"].update({
                "session_id": "ses_0000000000000000000000000c",
                "turn_id": "trn_0000000000000000000000000d",
                "action_id": "act_0000000000000000000000000e",
            })
            value["scope_ref"][parent] = None
            with self.subTest(child=child), self.assertRaises(ContractError):
                validate_event_envelope(value)

    def _artifact_backed_command(self) -> dict:
        value = copy.deepcopy(self.fixture["command_envelope"])
        value["scope_ref"] = copy.deepcopy(self.fixture["event_envelope"]["scope_ref"])
        value["payload"] = None
        value["payload_artifact_ref"] = copy.deepcopy(self.fixture["artifact_ref"])
        value["issued_at_unix_ms"] = value["payload_artifact_ref"]["created_at_unix_ms"]
        value["deadline_unix_ms"] = value["issued_at_unix_ms"] + 60_000
        validate_command_envelope(value)
        return value

    def test_payload_schema_version_is_independent_from_envelope_version(self) -> None:
        command = copy.deepcopy(self.fixture["command_envelope"])
        event = copy.deepcopy(self.fixture["event_envelope"])
        command["schema_version"] = 2
        event["schema_version"] = 2
        validate_command_envelope(command)
        validate_event_envelope(event)

    def test_public_validators_return_contract_errors_for_bad_python_types(self) -> None:
        cases = []
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["actor_ref"]["actor_type"] = []
        cases.append((validate_command_envelope, command))
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["expected_version"] = 1 << 63
        cases.append((validate_command_envelope, command))
        event = copy.deepcopy(self.fixture["event_envelope"])
        event["source_component"] = {}
        cases.append((validate_event_envelope, event))
        artifact = copy.deepcopy(self.fixture["artifact_ref"])
        artifact["retention_class"] = []
        cases.append((validate_artifact_ref, artifact))
        for validator, value in cases:
            with self.subTest(validator=validator.__name__), self.assertRaises(ContractError):
                validator(value)

    def _reject_each(self, field: str, validator, mutations) -> None:
        for mutate in mutations:
            value = copy.deepcopy(self.fixture[field])
            mutate(value)
            with self.assertRaises(ContractError):
                validator(value)


class PlatformCoreBoundaryTests(unittest.TestCase):
    """Exact N and N+1 resource ceilings."""

    def setUp(self) -> None:
        self.fixture = load_golden(ROOT)

    def test_payload_string_array_and_extension_boundaries(self) -> None:
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["payload"] = {"a": "x" * MAX_STRING_BYTES,  # noqa: F405
                              "b": "x" * (MAX_STRING_BYTES - 15)}  # noqa: F405
        self.assertEqual(len(canonical_json(command["payload"], MAX_PAYLOAD_BYTES)),  # noqa: F405
                         MAX_PAYLOAD_BYTES)  # noqa: F405
        validate_command_envelope(command)
        command["payload"]["b"] += "x"
        with self.assertRaises(ContractError):
            validate_command_envelope(command)
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["payload"] = {"items": [None] * MAX_ARRAY_ITEMS}  # noqa: F405
        validate_command_envelope(command)
        command["payload"]["items"].append(None)
        with self.assertRaises(ContractError):
            validate_command_envelope(command)
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["payload"] = {f"field_{index}": None
                              for index in range(MAX_OBJECT_FIELDS)}  # noqa: F405
        validate_command_envelope(command)
        command["payload"]["field_extra"] = None
        with self.assertRaises(ContractError):
            validate_command_envelope(command)
        self._extension_boundary()

    def test_id_document_depth_and_extension_bytes_fail_closed(self) -> None:
        for value in ("spc_00000000000000000000000000",
                      "act_7zzzzzzzzzzzzzzzzzzzzzzzzz"):
            validate_platform_id(value)
        for value in ("spc_80000000000000000000000000",
                      "spc_0000000000000000000000000i",
                      "run_00000000000000000000000001"):
            with self.assertRaises(ContractError):
                validate_platform_id(value)
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["extensions"] = {"fixture.large": "x" * MAX_EXTENSIONS_BYTES}  # noqa: F405
        with self.assertRaises(ContractError):
            validate_command_envelope(command)
        self._whole_envelope_depth_boundaries()
        oversized = b'{"payload":"' + b"x" * MAX_ENVELOPE_BYTES + b'"}'  # noqa: F405
        with self.assertRaises(ContractError):
            decode_command_envelope(oversized)

    def test_high_fanout_stops_at_aggregate_or_output_budget(self) -> None:
        for shared in ([None] * MAX_ARRAY_ITEMS, "x" * MAX_STRING_BYTES,  # noqa: F405
                       {"k" * MAX_STRING_BYTES: None}):  # noqa: F405
            with self.subTest(kind=type(shared).__name__), self.assertRaises(ContractError):
                canonical_json({"fanout": [shared] * MAX_ARRAY_ITEMS}, MAX_PAYLOAD_BYTES)  # noqa: F405

    def _whole_envelope_depth_boundaries(self) -> None:
        cases = (("command_envelope", validate_command_envelope, canonical_command_envelope,
                  decode_command_envelope), ("event_envelope", validate_event_envelope,
                  canonical_event_envelope, decode_event_envelope))
        for fixture, validator, writer, decoder in cases:
            for field, key in (("payload", "nested"), ("extensions", "fixture.nested")):
                for arrays, accepted in ((MAX_JSON_DEPTH - 3, True), (MAX_JSON_DEPTH - 2, False)):  # noqa: F405
                    value, nested = copy.deepcopy(self.fixture[fixture]), None
                    for _ in range(arrays):
                        nested = [nested]
                    value[field] = {key: nested}
                    if field == "payload":
                        value["payload_artifact_ref"] = None
                    if accepted:
                        validator(value)
                        decoder(writer(value))
                    else:
                        raw = json.dumps(value, separators=(",", ":"), sort_keys=True).encode()
                        for operation in (validator, writer, lambda _: decoder(raw)):
                            with self.assertRaises(ContractError):
                                operation(value)

    def _extension_boundary(self) -> None:
        command = copy.deepcopy(self.fixture["command_envelope"])
        command["extensions"] = {
            f"fixture.key_{index}": index for index in range(MAX_EXTENSION_FIELDS)  # noqa: F405
        }
        validate_command_envelope(command)
        command["extensions"]["fixture.extra"] = None
        with self.assertRaises(ContractError):
            validate_command_envelope(command)
if __name__ == "__main__":
    unittest.main()
