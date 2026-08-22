"""Typed-scope and resource-boundary regressions for ADR-0056."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

try:
    import jsonschema
except ModuleNotFoundError:  # Universal scaffold has no third-party Python dependency.
    jsonschema = None

import capability_grant_contract as contract
from capability_grant_contract.assessment import (evaluate_declared_assessment, request_sha256,
                                                   assessment_sha256, validate_request)
from capability_grant_contract.canonical import digest
from capability_grant_contract.constants import (GRANT_DOMAIN, MAX_ASSESSMENT_BYTES,
                                                   MAX_GRANT_BYTES, MAX_REQUEST_BYTES,
                                                   MAX_STRING_BYTES, MAX_VOCABULARY_BYTES)
from capability_grant_contract.grant import grant_sha256
from capability_grant_contract.vocabulary import vocabulary_sha256

ROOT = Path(__file__).resolve().parents[1]
EMPTY_SHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"


def artifact(name: str, marker: str = "a") -> dict:
    return {"artifact_kind": "migration_bundle", "artifact_ref": f"artifact:{name}",
            "artifact_sha256": marker * 64, "scope_kind": "artifact"}


def environment(name: str, environment_class: str, marker: str = "b") -> dict:
    return {"environment_class": environment_class, "environment_id": name,
            "environment_sha256": marker * 64, "scope_kind": "environment"}


def repo(path: str, match: str = "exact") -> dict:
    return {"match": match, "path": path, "scope_kind": "repo_path"}


def governance(kind: str) -> dict:
    return {"object_kind": kind, "object_ref": f"{kind}:fixture",
            "object_scope_sha256": "c" * 64, "scope_kind": "governance_object"}


def network(host: str, host_kind: str = "ipv6") -> dict:
    return {"host": host, "host_kind": host_kind, "port": 443,
            "scheme": "https", "scope_kind": "network_origin"}


def command(argv: list[str], stdin_bytes: int = 0, stdin_sha256: str = EMPTY_SHA) -> dict:
    return {"argv": argv, "cwd": ".", "environment_sha256": "d" * 64,
            "scope_kind": "command", "stdin_bytes": stdin_bytes,
            "stdin_sha256": stdin_sha256, "timeout_ms": 1000,
            "tool_snapshot_sha256": "e" * 64}


def saturated_command(index: int) -> dict:
    first = f"{index:04d}" + "x" * 4092
    return command([first] + ["x" * 4096] * 7)


def sized_command(index: int, payload_bytes: int) -> dict:
    prefix = f"{index:04d}"
    first_bytes = min(payload_bytes, 4096)
    argv = [prefix + "x" * (first_bytes - len(prefix))]
    remaining = payload_bytes - first_bytes
    while remaining:
        next_bytes = min(remaining, 4096)
        argv.append("x" * next_bytes)
        remaining -= next_bytes
    return command(argv)


def near_boundary_grant(value: dict) -> dict:
    target, base_payload = MAX_GRANT_BYTES - 32, 4097
    baseline = _grant_with_commands(value, base_payload, 0)
    remaining = target - len(_grant_preimage(baseline))
    result = _grant_with_commands(value, base_payload + remaining // 128, remaining % 128)
    preimage = _grant_preimage(result)
    assert len(preimage) == target
    result["grant_sha256"] = digest(GRANT_DOMAIN, _grant_payload(result))
    result["grant_id"] = f"capability-grant-{result['grant_sha256']}"
    assert len(contract.canonical_json(result)) == 1_048_717
    return result


def _grant_with_commands(value: dict, common_bytes: int, first_extra: int) -> dict:
    result = copy.deepcopy(value)
    allow = [{"resources": [sized_command(index, common_bytes + (first_extra if index == 0
                                                                  else 0))]}
             for index in range(64)]
    deny = [sized_command(index, common_bytes) for index in range(64, 128)]
    result["scope"] = {"allow": sorted(allow, key=contract.canonical_json),
                       "deny": sorted(deny, key=lambda item: (
                           item["scope_kind"].encode(), contract.canonical_json(item))),
                       "effect_id": "process.exec"}
    return result


def _grant_payload(value: dict) -> dict:
    payload = copy.deepcopy(value)
    payload["authority_proof"]["proof_base64url"] = ""
    payload["grant_id"] = ""
    payload["grant_sha256"] = ""
    return payload


def _grant_preimage(value: dict) -> bytes:
    return contract.canonical_json(_grant_payload(value))


def padded_self_digest_document(value: dict, digest_field: str, maximum: int) -> dict:
    result = copy.deepcopy(value)
    result["padding"] = []
    payload = copy.deepcopy(result)
    payload[digest_field] = ""
    remaining = maximum - 32 - len(contract.canonical_json(payload))
    for count in range(1, 257):
        content_bytes = remaining - (3 * count - 1)
        if 0 <= content_bytes <= count * MAX_STRING_BYTES:
            break
    else:
        raise AssertionError("cannot construct bounded digest regression")
    result["padding"] = []
    for _ in range(count):
        next_bytes = min(content_bytes, MAX_STRING_BYTES)
        result["padding"].append("x" * next_bytes)
        content_bytes -= next_bytes
    payload = copy.deepcopy(result)
    payload[digest_field] = ""
    assert len(contract.canonical_json(payload)) == maximum - 32
    assert len(contract.canonical_json(result)) > maximum
    return result


def secret(version: str) -> dict:
    return {"broker_id": "fixture-broker", "scope_kind": "secret_ref",
            "secret_ref": "secret:database", "version_ref": version}


class CapabilityGrantScopeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.golden = contract.load_golden(ROOT)
        cls.schema = json.loads((ROOT / "docs/contracts/capability-grant-v1.schema.json").read_text())

    def grant(self):
        return copy.deepcopy(self.golden["grant"])

    def request(self):
        return copy.deepcopy(self.golden["assessment_request"])

    def resign_grant(self, grant):
        value = grant_sha256(grant)
        grant["grant_sha256"] = value
        grant["grant_id"] = f"capability-grant-{value}"

    def resign_request(self, request):
        request["request_sha256"] = request_sha256(request)

    def set_scope(self, grant, effect, resources, deny=None):
        grant["scope"] = {"allow": [{"resources": resources}], "deny": deny or [],
                          "effect_id": effect}
        self.resign_grant(grant)

    def action(self, request, effect, resources):
        request["requested_action"]["effect_id"] = effect
        request["requested_action"]["resources"] = resources
        self.resign_request(request)

    def make_external(self, grant, with_approval=True):
        issuer = grant["authority_proof"]["issuer"]
        issuer.update({"authority_class": "external_operator", "authority_domain": "operator.fixture",
                       "principal_id": "fixture-operator", "principal_type": "operator"})
        grant["approval_refs"] = ([{"approval_id": "approval-fixture",
                                    "approval_sha256": "f" * 64,
                                    "authority_domain": "operator.fixture"}]
                                  if with_approval else [])

    def test_profile_cardinality_and_governance_kind_are_enforced(self):
        grant = self.grant()
        self.set_scope(grant, "migration.apply", [artifact("a")])
        with self.assertRaisesRegex(contract.ContractError, "required scope kind|artifact"):
            contract.validate_grant(grant)
        grant = self.grant()
        self.set_scope(grant, "release.plan", [repo("out/release.json")])
        with self.assertRaisesRegex(contract.ContractError, "required scope kind|one environment"):
            contract.validate_grant(grant)
        grant = self.grant()
        self.set_scope(grant, "approval.request", [governance("knowledge")])
        with self.assertRaisesRegex(contract.ContractError, "object_kind"):
            contract.validate_grant(grant)

    def test_requested_repository_paths_are_always_exact(self):
        request = self.request()
        request["requested_action"]["resources"][0]["match"] = "subtree"
        self.resign_request(request)
        with self.assertRaisesRegex(contract.ContractError, "exact"):
            evaluate_declared_assessment(request)

    def test_allow_clauses_never_cross_product_resources(self):
        grant = self.grant()
        self.make_external(grant)
        clauses = [{"resources": [artifact("a", "a"), environment("dev", "development", "b")]},
                   {"resources": [artifact("b", "c"), environment("prod", "production", "d")]}]
        grant["scope"] = {"allow": sorted(clauses, key=contract.canonical_json), "deny": [],
                          "effect_id": "migration.apply"}
        self.resign_grant(grant)
        request = self.request()
        request["grant"] = grant
        self.action(request, "migration.apply",
                    [artifact("a", "a"), environment("prod", "production", "d")])
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["scope"], "outside_declared_scope")
        self.assertEqual(assessment["authorization_decision"], "none")

    def test_migration_generate_environment_is_an_exact_clause_qualifier(self):
        cases = (("dev", "dev", "covered_by_declaration"),
                 ("dev", None, "outside_declared_scope"),
                 ("dev", "other", "outside_declared_scope"),
                 (None, "dev", "outside_declared_scope"))
        order = lambda item: (item["scope_kind"], contract.canonical_json(item))
        for declared_environment, requested_environment, expected in cases:
            with self.subTest(declared=declared_environment, requested=requested_environment):
                grant = self.grant()
                declared = [repo("out/migration.sql")]
                if declared_environment:
                    declared.append(environment(declared_environment, "development"))
                self.set_scope(grant, "migration.generate", sorted(declared, key=order))
                request = self.request()
                request["grant"] = grant
                requested = [repo("out/migration.sql")]
                if requested_environment:
                    requested.append(environment(requested_environment, "development"))
                self.action(request, "migration.generate", sorted(requested, key=order))
                assessment = evaluate_declared_assessment(request)
                self.assertEqual(assessment["relations"]["scope"], expected)
                reasons = [] if expected == "covered_by_declaration" else ["scope_not_covered"]
                self.assertEqual(assessment["reason_codes"], reasons)

    def test_production_apply_requires_external_operator_and_approval(self):
        resources = [artifact("a"), environment("prod", "production")]
        grant = self.grant()
        self.set_scope(grant, "migration.apply", resources)
        with self.assertRaisesRegex(contract.ContractError, "external operator"):
            contract.validate_grant(grant)
        grant = self.grant()
        self.make_external(grant, with_approval=False)
        self.set_scope(grant, "migration.apply", resources)
        with self.assertRaisesRegex(contract.ContractError, "approval"):
            contract.validate_grant(grant)
        grant = self.grant()
        self.set_scope(grant, "migration.apply",
                       [artifact("a"), environment("dev", "development")])
        contract.validate_grant(grant)

    def test_secret_and_command_resource_safety_shapes(self):
        request = self.request()
        self.action(request, "secrets.read", [secret("current")])
        with self.assertRaisesRegex(contract.ContractError, "immutable version"):
            evaluate_declared_assessment(request)
        request = self.request()
        self.action(request, "process.exec", [command(["tool"], stdin_sha256="0" * 64)])
        with self.assertRaisesRegex(contract.ContractError, "SHA-256\(empty\)"):
            evaluate_declared_assessment(request)
        request = self.request()
        self.action(request, "process.exec", [command(["x" * 4096] * 9)])
        with self.assertRaisesRegex(contract.ContractError, "aggregate"):
            evaluate_declared_assessment(request)

    def test_secret_version_ref_is_ascii_and_immutable(self):
        for version in ("LaTeSt", "lateſt", "version with space", "版本1"):
            with self.subTest(version=version):
                request = self.request()
                self.action(request, "secrets.read", [secret(version)])
                with self.assertRaises(contract.ContractError):
                    evaluate_declared_assessment(request)
        request = self.request()
        self.action(request, "secrets.read", [secret("v2026.08.11+build/7@sha256:abc")])
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["effect"], "effect_mismatch")

    def test_process_exec_binds_command_and_requested_usage_timeout(self):
        request = self.request()
        process = command(["tool"])
        process["timeout_ms"] = 60000
        self.action(request, "process.exec", [process])
        request["requested_action"]["usage"]["timeout_ms"] = 1000
        self.resign_request(request)
        for operation in (validate_request, evaluate_declared_assessment):
            with self.subTest(operation=operation.__name__), self.assertRaisesRegex(
                    contract.ContractError, "must equal requested_action.usage.timeout_ms"):
                operation(copy.deepcopy(request))

    def test_ipv4_mapped_ipv6_network_resources_are_rejected(self):
        for host in ("::ffff:c000:201", "::ffff:192.0.2.1"):
            with self.subTest(host=host):
                request = self.request()
                self.action(request, "network.read", [network(host)])
                with self.assertRaisesRegex(contract.ContractError, "IPv4-mapped IPv6"):
                    evaluate_declared_assessment(request)

    def test_ipv6_zone_ids_are_rejected(self):
        for host in ("fe80::1%eth0", "::1%lo"):
            with self.subTest(host=host):
                request = self.request()
                self.action(request, "network.read", [network(host)])
                with self.assertRaisesRegex(contract.ContractError, "IPv6 zone ID"):
                    evaluate_declared_assessment(request)

    def test_canonical_ipv4_literal_has_only_the_ipv4_tag(self):
        request = self.request()
        self.action(request, "network.read", [network("192.0.2.1", "dns")])
        with self.assertRaisesRegex(contract.ContractError, "cannot tag canonical IPv4 as DNS"):
            evaluate_declared_assessment(request)
        request = self.request()
        self.action(request, "network.read", [network("192.0.2.1", "ipv4")])
        assessment = evaluate_declared_assessment(request)
        self.assertEqual(assessment["relations"]["effect"], "effect_mismatch")

    def test_resource_tuple_order_hard_32_and_total_256_are_enforced(self):
        request = self.request()
        resources = [repo("out/result.json"), environment("dev", "development")]
        self.action(request, "migration.generate", resources)
        with self.assertRaisesRegex(contract.ContractError, "sorted"):
            evaluate_declared_assessment(request)
        grant = self.grant()
        too_many = [environment("dev", "development")] + [repo(f"out/{index:02d}")
                                                              for index in range(32)]
        grant["scope"] = {"allow": [{"resources": sorted(
            too_many, key=lambda item: (item["scope_kind"], contract.canonical_json(item)))}],
            "deny": [], "effect_id": "release.plan"}
        with self.assertRaisesRegex(contract.ContractError, "1..32"):
            contract.validate_grant(grant)
        grant = self.grant()
        clauses = [{"resources": [repo(f"src/{group:02d}/{index:02d}")
                                    for index in range(32)]} for group in range(9)]
        grant["scope"] = {"allow": sorted(clauses, key=contract.canonical_json), "deny": [],
                          "effect_id": "repo.read"}
        with self.assertRaisesRegex(contract.ContractError, "256"):
            contract.validate_grant(grant)

    def test_programmatic_grant_and_request_obey_canonical_byte_ceilings(self):
        allow = [{"resources": [saturated_command(index)]} for index in range(64)]
        deny = [saturated_command(index) for index in range(64, 128)]
        grant = self.grant()
        grant["scope"] = {
            "allow": sorted(allow, key=contract.canonical_json),
            "deny": sorted(deny, key=lambda item: (
                item["scope_kind"].encode("utf-8"), contract.canonical_json(item))),
            "effect_id": "process.exec",
        }
        self.assertGreater(len(contract.canonical_json(grant)), MAX_GRANT_BYTES)
        with self.assertRaisesRegex(contract.ContractError, "CapabilityGrant canonical byte"):
            contract.validate_grant(grant)
        with self.assertRaisesRegex(contract.ContractError, "CapabilityGrant canonical byte"):
            grant_sha256(grant)
        request = self.request()
        request["grant"] = grant
        self.assertGreater(len(contract.canonical_json(request)), MAX_REQUEST_BYTES)
        for operation in (validate_request, evaluate_declared_assessment, request_sha256):
            with self.subTest(operation=operation.__name__), self.assertRaisesRegex(
                    contract.ContractError, "assessment request canonical byte"):
                operation(copy.deepcopy(request))

    def test_self_digest_helpers_bound_the_full_supplied_document(self):
        grant = near_boundary_grant(self.grant())
        self.assertEqual(len(_grant_preimage(grant)), MAX_GRANT_BYTES - 32)
        with self.assertRaisesRegex(contract.ContractError, "CapabilityGrant canonical byte"):
            grant_sha256(grant)
        with self.assertRaisesRegex(contract.ContractError, "CapabilityGrant canonical byte"):
            contract.validate_grant(grant)

        vocabulary = padded_self_digest_document(
            self.golden["effect_vocabulary"], "vocabulary_sha256", MAX_VOCABULARY_BYTES)
        request = padded_self_digest_document(
            self.request(), "request_sha256", MAX_REQUEST_BYTES)
        assessment = padded_self_digest_document(
            self.golden["expected_assessment"], "assessment_sha256", MAX_ASSESSMENT_BYTES)
        for operation, value, label in (
                (vocabulary_sha256, vocabulary, "effect vocabulary canonical byte"),
                (request_sha256, request, "assessment request canonical byte"),
                (assessment_sha256, assessment, "assessment canonical byte")):
            with self.subTest(operation=operation.__name__), self.assertRaisesRegex(
                    contract.ContractError, label):
                operation(value)

    def test_programmatic_non_string_object_keys_fail_as_contract_errors(self):
        grant = self.grant()
        grant["authority_proof"][7] = "not-a-json-object-key"
        for operation in (grant_sha256, contract.validate_grant):
            with self.subTest(operation=operation.__name__), self.assertRaisesRegex(
                    contract.ContractError, "object key 7"):
                operation(copy.deepcopy(grant))

    def test_strict_direct_grant_decoder_has_no_plain_json_escape_hatch(self):
        raw = contract.canonical_json(self.grant())
        self.assertEqual(contract.decode_grant(raw), self.grant())
        unknown = self.grant()
        unknown["authorized"] = True
        with self.assertRaisesRegex(contract.ContractError, "fields"):
            contract.decode_grant(contract.canonical_json(unknown))
        duplicate = raw.replace(b'{"api_version":',
                                b'{"api_version":"duplicate","api_version":', 1)
        with self.assertRaisesRegex(contract.ContractError, "duplicate"):
            contract.decode_grant(duplicate)
        with self.assertRaisesRegex(contract.ContractError, "canonical"):
            contract.decode_grant(json.dumps(self.grant()).encode())

    @unittest.skipIf(jsonschema is None, "jsonschema is unavailable in this scaffold")
    def test_schema_rejects_trailing_lf_identity_and_major_bounds(self):
        validator = jsonschema.Draft202012Validator(self.schema)
        self.assertEqual(
            self.schema["x-forgeos-authority-semantics"][
                "process_exec_requested_timeout_relation"],
            "requested_action.resources[0].timeout_ms == requested_action.usage.timeout_ms")
        self.assertTrue(
            self.schema["x-forgeos-authority-semantics"][
                "canonical_ip_rejects_ipv4_mapped_ipv6"])
        self.assertTrue(
            self.schema["x-forgeos-authority-semantics"]["ipv6_zone_ids_rejected"])
        self.assertTrue(
            self.schema["x-forgeos-authority-semantics"][
                "dns_rejects_canonical_ipv4_literal"])
        self.assertEqual(
            self.schema["x-forgeos-authority-semantics"]["secret_version_ref_ascii_pattern"],
            "^[A-Za-z0-9][A-Za-z0-9._:/@+\\-]{0,4095}(?![\\s\\S])")
        fixture = copy.deepcopy(self.golden)
        fixture["grant"]["scope"] = {
            "allow": [{"resources": [secret("v1\n")]}], "deny": [],
            "effect_id": "secrets.read",
        }
        self.assertTrue(list(validator.iter_errors(fixture)))
        fixture = copy.deepcopy(self.golden)
        fixture["grant"]["grant_id"] += "\n"
        self.assertTrue(list(validator.iter_errors(fixture)))
        fixture = copy.deepcopy(self.golden)
        fixture["grant"]["authority_proof"]["proof_base64url"] += "\n"
        fixture["assessment_request"]["grant"]["authority_proof"][
            "proof_base64url"] += "\n"
        self.assertTrue(list(validator.iter_errors(fixture)))
        for proof in ("A" * 17, "_" * 18):
            with self.subTest(noncanonical_proof=proof):
                fixture = copy.deepcopy(self.golden)
                fixture["grant"]["authority_proof"]["proof_base64url"] = proof
                fixture["assessment_request"]["grant"]["authority_proof"][
                    "proof_base64url"] = proof
                self.assertTrue(list(validator.iter_errors(fixture)))
        fixture = copy.deepcopy(self.golden)
        fixture["assessment_request"]["grant"]["scope"]["deny"] = [
            repo(f"blocked/{index:02d}") for index in range(65)]
        self.assertTrue(list(validator.iter_errors(fixture)))


if __name__ == "__main__":
    unittest.main()
