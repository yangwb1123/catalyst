from __future__ import annotations

import copy
import sys
import unittest
from unittest import mock
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "harness"))

from bootstrap_grant_issuance_contract import grant_envelope_sha256  # noqa: E402
from bootstrap_grant_issuance_contract.documents import (  # noqa: E402
    receipt_sha256 as issuance_receipt_sha256,
    record_key_sha256 as issuance_record_key_sha256,
    request_sha256 as issuance_request_sha256,
)
from bootstrap_grant_issuance_contract.ledger import (  # noqa: E402
    ledger_sha256 as issuance_ledger_sha256,
)
from bootstrap_repo_read_execution_contract import (  # noqa: E402
    ContractError, load_fixture, terminal_replay,
)
from bootstrap_repo_read_execution_contract.documents import (  # noqa: E402
    invocation_sha256, policy_sha256,
)
from bootstrap_repo_read_execution_contract.ledger import (  # noqa: E402
    ledger_sha256, receipt_sha256, validate_ledger,
)
from bootstrap_repo_read_execution_contract.results import validate_delivery  # noqa: E402
from bootstrap_repo_read_execution_contract.shape import record_key_sha256  # noqa: E402
from capability_grant_contract.grant import grant_sha256  # noqa: E402

SHAPED_SIGNATURE = "A" * 86


class BootstrapRepoReadExecutionLedgerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.golden = load_fixture(ROOT)

    def test_two_distinct_grant_groups_share_one_global_receipt_chain(self) -> None:
        issuance, issued = _append_issued_grant(self.golden)
        usage = _append_execution_group(self.golden, issued, "fixture-execution-request-0002")
        _resign_usage_ledger(usage)
        validated = _validate(self.golden, issuance, usage)
        self.assertEqual([entry["sequence"] for entry in validated["entries"]],
                         [1, 2, 3, 4, 5, 6])
        self.assertEqual(validated["entries"][3]["receipt"]["prior_usage_receipt_sha256"],
                         validated["entries"][2]["receipt"]["receipt_sha256"])
        second = validated["entries"][3]
        replay = terminal_replay(validated,
                                 second["execution_policy"]["execution_policy_sha256"],
                                 second["invocation"]["invocation_sha256"])
        self.assertEqual(replay["receipt"]["state"], "failed_consumed")
        self.assertIsNone(replay["execution_result"])
        self.assertIsNone(replay["result_metadata"])
        delivery = {"api_version": "forgeos.bootstrap-repo-read-execution-delivery/v1",
                    "canonicalization": "forgeos.canonical-json/v1",
                    "delivery_disposition": "exact_replay", "execution_result": None,
                    "kind": "BootstrapRepoReadExecutionDelivery",
                    "receipt": replay["receipt"], "result_metadata": None}
        validate_delivery(delivery, replay["receipt"], None)

    def test_grant_envelope_cannot_be_reused_after_terminal(self) -> None:
        issuance = copy.deepcopy(self.golden["issuance_ledger"])
        issued = issuance["entries"][0]
        usage = _append_execution_group(self.golden, issued,
                                        "fixture-execution-request-0002")
        _resign_usage_ledger(usage)
        with self.assertRaisesRegex(ContractError, "reuses a Grant envelope"):
            _validate(self.golden, issuance, usage)

    def test_idempotency_record_key_cannot_be_reused_by_another_grant(self) -> None:
        issuance, issued = _append_issued_grant(self.golden)
        old_key = self.golden["execution_policy"]["idempotency_key"]
        usage = _append_execution_group(self.golden, issued, old_key)
        _resign_usage_ledger(usage)
        with self.assertRaisesRegex(ContractError, "idempotency record key"):
            _validate(self.golden, issuance, usage)

    def test_terminal_state_cannot_continue_without_a_fresh_reservation(self) -> None:
        issuance, issued = _append_issued_grant(self.golden)
        usage = _append_execution_group(self.golden, issued, "fixture-execution-request-0002")
        extra = copy.deepcopy(usage["entries"][-2])
        extra["sequence"] = 7
        extra["receipt"]["ledger_sequence"] = 7
        extra["receipt"]["prior_usage_receipt_sha256"] = usage["entries"][-1]["receipt"][
            "receipt_sha256"]
        _resign_receipt(extra["receipt"])
        usage["entries"].append(extra)
        _resign_usage_ledger(usage)
        with self.assertRaisesRegex(ContractError, "no active reservation"):
            _validate(self.golden, issuance, usage)

    def test_orphaned_active_tails_close_only_by_matching_quarantine(self) -> None:
        issuance, issued = _append_issued_grant(self.golden)
        for after_intent in (False, True):
            with self.subTest(after_intent=after_intent):
                usage = _append_quarantined_group(self.golden, issued, after_intent)
                _resign_usage_ledger(usage)
                validated = _validate(self.golden, issuance, usage)
                terminal = validated["entries"][-1]["receipt"]
                expected = ("orphaned_effect_intent" if after_intent else
                            "orphaned_reserved_no_repo_io")
                self.assertEqual(terminal["reason_code"], expected)
                self.assertEqual(terminal["state"], "quarantined")

    def test_receipt_time_is_global_nondecreasing_but_high_water_is_an_upper_bound(self) -> None:
        usage = copy.deepcopy(self.golden["usage_ledger"])
        usage["clock_high_water_unix_ms"] += 1000
        _resign_usage_ledger(usage)
        _validate(self.golden, self.golden["issuance_ledger"], usage)
        receipt = usage["entries"][1]["receipt"]
        receipt["recorded_at_unix_ms"] = usage["entries"][0]["receipt"][
            "recorded_at_unix_ms"] - 1
        _resign_receipt(receipt)
        _resign_usage_ledger(usage)
        with self.assertRaisesRegex(ContractError, "must not move backward"):
            _validate(self.golden, self.golden["issuance_ledger"], usage)

    def test_reservation_must_start_inside_invocation_freshness(self) -> None:
        usage = copy.deepcopy(self.golden["usage_ledger"])
        receipt = usage["entries"][0]["receipt"]
        receipt["recorded_at_unix_ms"] = self.golden["invocation"][
            "requested_at_unix_ms"] - 1
        _resign_receipt(receipt)
        _resign_usage_ledger(usage)
        with self.assertRaisesRegex(ContractError, "outside Invocation freshness"):
            _validate(self.golden, self.golden["issuance_ledger"], usage)

    def test_completed_ledger_persists_metadata_but_never_raw_content(self) -> None:
        usage = self.golden["usage_ledger"]
        self.assertIsNotNone(usage["entries"][2]["result_metadata"])
        self.assertTrue(all(entry["result_metadata"] is None
                            for entry in usage["entries"][:2]))
        from bootstrap_repo_read_execution_contract import canonical_json
        self.assertNotIn(b"content_base64url", canonical_json(usage))

    def test_reservation_preflights_future_byte_capacity(self) -> None:
        from bootstrap_repo_read_execution_contract import ledger as ledger_contract
        with mock.patch.object(ledger_contract, "MAX_LEDGER_BYTES", 1024):
            with self.assertRaisesRegex(ContractError, "future ledger byte capacity"):
                ledger_contract._validate_capacity(self.golden["usage_ledger"])


def _append_issued_grant(golden: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    ledger = copy.deepcopy(golden["issuance_ledger"])
    prior = ledger["entries"][-1]
    entry = copy.deepcopy(prior)
    request = entry["request"]
    request["idempotency_key"] = "fixture-grant-request-0002"
    request["request_sha256"] = ""
    request["signature"]["signature_base64url"] = ""
    request["request_sha256"] = issuance_request_sha256(request)
    request["signature"]["signature_base64url"] = SHAPED_SIGNATURE
    _rebind_grant(entry["grant"], request["request_sha256"])
    _rebind_issuance_receipt(entry, prior["receipt"], sequence=2)
    entry["sequence"] = 2
    ledger["entries"].append(entry)
    ledger["ledger_sha256"] = ""
    ledger["signature"]["signature_base64url"] = ""
    ledger["ledger_sha256"] = issuance_ledger_sha256(ledger)
    ledger["signature"]["signature_base64url"] = SHAPED_SIGNATURE
    return ledger, entry


def _rebind_grant(grant: dict[str, Any], request_sha: str) -> None:
    grant["bindings"]["grant_request_sha256"] = request_sha
    grant["grant_id"] = ""
    grant["grant_sha256"] = ""
    grant["grant_sha256"] = grant_sha256(grant)
    grant["grant_id"] = "capability-grant-" + grant["grant_sha256"]
    grant["authority_proof"]["proof_base64url"] = SHAPED_SIGNATURE


def _rebind_issuance_receipt(entry: dict[str, Any], prior: dict[str, Any],
                             sequence: int) -> None:
    grant, request, receipt = entry["grant"], entry["request"], entry["receipt"]
    receipt.update({
        "grant_envelope_sha256": grant_envelope_sha256(grant),
        "grant_id": grant["grant_id"], "grant_sha256": grant["grant_sha256"],
        "ledger_sequence": sequence, "prior_receipt_sha256": prior["receipt_sha256"],
        "record_key_sha256": issuance_record_key_sha256(request["idempotency_key"]),
        "request_sha256": request["request_sha256"], "receipt_sha256": "",
    })
    receipt["signature"]["signature_base64url"] = ""
    receipt["receipt_sha256"] = issuance_receipt_sha256(receipt)
    receipt["signature"]["signature_base64url"] = SHAPED_SIGNATURE


def _append_execution_group(golden: dict[str, Any], issued: dict[str, Any],
                            idempotency_key: str) -> dict[str, Any]:
    usage = copy.deepcopy(golden["usage_ledger"])
    policy = _second_policy(golden, issued, idempotency_key)
    invocation = _second_invocation(golden, policy)
    manifest = copy.deepcopy(golden["expected_manifest"])
    prior = usage["entries"][-1]["receipt"]["receipt_sha256"]
    reserved = _new_receipt(golden, policy, invocation, "reserved_no_repo_io", 4, prior)
    intent = _new_receipt(golden, policy, invocation, "effect_intent", 5,
                          reserved["receipt_sha256"], reserved=reserved["receipt_sha256"])
    failed = _new_receipt(golden, policy, invocation, "failed_consumed", 6,
                          intent["receipt_sha256"], reserved=reserved["receipt_sha256"],
                          intent=intent["receipt_sha256"], reason="repository_read_failed")
    usage["entries"].extend(_group_entries(policy, invocation, manifest,
                                           reserved, intent, failed))
    usage["clock_high_water_unix_ms"] = max(usage["clock_high_water_unix_ms"],
                                             failed["recorded_at_unix_ms"])
    return usage


def _append_quarantined_group(golden: dict[str, Any], issued: dict[str, Any],
                              after_intent: bool) -> dict[str, Any]:
    usage = copy.deepcopy(golden["usage_ledger"])
    policy = _second_policy(golden, issued, "fixture-execution-request-0002")
    invocation = _second_invocation(golden, policy)
    manifest = copy.deepcopy(golden["expected_manifest"])
    prior = usage["entries"][-1]["receipt"]["receipt_sha256"]
    reserved = _new_receipt(golden, policy, invocation, "reserved_no_repo_io", 4, prior)
    entries = [_usage_entry(4, reserved, policy, invocation, manifest)]
    anchor, sequence, intent = reserved["receipt_sha256"], 5, None
    if after_intent:
        intent = _new_receipt(golden, policy, invocation, "effect_intent", 5, anchor,
                              reserved=anchor)
        entries.append(_usage_entry(5, intent))
        sequence = 6
    reason = "orphaned_effect_intent" if after_intent else "orphaned_reserved_no_repo_io"
    prior = intent["receipt_sha256"] if intent is not None else anchor
    quarantine = _new_receipt(golden, policy, invocation, "quarantined", sequence, prior,
                              reserved=anchor,
                              intent=intent["receipt_sha256"] if intent is not None else None,
                              reason=reason)
    entries.append(_usage_entry(sequence, quarantine))
    usage["entries"].extend(entries)
    usage["clock_high_water_unix_ms"] = quarantine["recorded_at_unix_ms"]
    return usage


def _usage_entry(sequence: int, receipt: dict[str, Any],
                 policy: dict[str, Any] | None = None,
                 invocation: dict[str, Any] | None = None,
                 manifest: dict[str, Any] | None = None) -> dict[str, Any]:
    return {"execution_policy": policy, "invocation": invocation, "manifest": manifest,
            "receipt": receipt, "result_metadata": None, "sequence": sequence}


def _second_policy(golden: dict[str, Any], issued: dict[str, Any],
                   idempotency_key: str) -> dict[str, Any]:
    value = copy.deepcopy(golden["execution_policy"])
    grant, receipt = issued["grant"], issued["receipt"]
    value.update({
        "execution_policy_id": "fixture-bootstrap-repo-read-execution-policy-v2",
        "grant_envelope_sha256": grant_envelope_sha256(grant),
        "grant_id": grant["grant_id"], "grant_issuance_ledger_sequence": receipt["ledger_sequence"],
        "grant_issuance_receipt_sha256": receipt["receipt_sha256"],
        "grant_request_sha256": grant["bindings"]["grant_request_sha256"],
        "grant_sha256": grant["grant_sha256"], "idempotency_key": idempotency_key,
        "execution_policy_sha256": "",
    })
    value["signature"]["signature_base64url"] = ""
    value["execution_policy_sha256"] = policy_sha256(value)
    value["signature"]["signature_base64url"] = SHAPED_SIGNATURE
    return value


def _second_invocation(golden: dict[str, Any], policy: dict[str, Any]) -> dict[str, Any]:
    value = copy.deepcopy(golden["invocation"])
    mirrored = ("grant_envelope_sha256", "grant_id", "grant_issuance_ledger_sequence",
                "grant_issuance_receipt_sha256", "grant_request_sha256", "grant_sha256",
                "idempotency_key")
    for field in mirrored:
        value[field] = copy.deepcopy(policy[field])
    value.update({"execution_policy_sha256": policy["execution_policy_sha256"],
                  "invocation_id": "", "invocation_sha256": ""})
    value["signature"]["signature_base64url"] = ""
    value["invocation_sha256"] = invocation_sha256(value)
    value["invocation_id"] = "bootstrap-repo-read-invocation-" + value["invocation_sha256"]
    value["signature"]["signature_base64url"] = SHAPED_SIGNATURE
    return value


def _new_receipt(golden: dict[str, Any], policy: dict[str, Any],
                 invocation: dict[str, Any], state: str, sequence: int, prior: str,
                 *, reserved: str | None = None, intent: str | None = None,
                 reason: str | None = None) -> dict[str, Any]:
    value = copy.deepcopy(golden["reserved_receipt"])
    relations = ("execution_policy_sha256", "execution_trust_epoch",
                 "execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
                 "grant_issuance_receipt_sha256", "grant_sha256", "issuance_trust_epoch",
                 "issuance_trust_root_sha256", "manifest_sha256", "requested_action_sha256")
    for field in relations:
        value[field] = policy[field]
    value.update({"effect_intent_receipt_sha256": intent,
                  "idempotency_record_key_sha256": record_key_sha256(policy["idempotency_key"]),
                  "invocation_id": invocation["invocation_id"],
                  "invocation_sha256": invocation["invocation_sha256"],
                  "ledger_sequence": sequence, "prior_usage_receipt_sha256": prior,
                  "reason_code": reason, "receipt_sha256": "",
                  "recorded_at_unix_ms": 1700000007000 + sequence,
                  "reservation_receipt_sha256": reserved, "state": state})
    _resign_receipt(value)
    return value


def _resign_receipt(value: dict[str, Any]) -> None:
    value["receipt_sha256"] = ""
    value["signature"]["signature_base64url"] = ""
    value["receipt_sha256"] = receipt_sha256(value)
    value["signature"]["signature_base64url"] = SHAPED_SIGNATURE


def _group_entries(policy: dict[str, Any], invocation: dict[str, Any],
                   manifest: dict[str, Any], reserved: dict[str, Any],
                   intent: dict[str, Any], terminal: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {"execution_policy": policy, "invocation": invocation, "manifest": manifest,
         "receipt": reserved, "result_metadata": None, "sequence": 4},
        {"execution_policy": None, "invocation": None, "manifest": None,
         "receipt": intent, "result_metadata": None, "sequence": 5},
        {"execution_policy": None, "invocation": None, "manifest": None,
         "receipt": terminal, "result_metadata": None, "sequence": 6},
    ]


def _resign_usage_ledger(value: dict[str, Any]) -> None:
    value["ledger_sha256"] = ""
    value["signature"]["signature_base64url"] = ""
    value["ledger_sha256"] = ledger_sha256(value)
    value["signature"]["signature_base64url"] = SHAPED_SIGNATURE


def _validate(golden: dict[str, Any], issuance: dict[str, Any],
              usage: dict[str, Any]) -> dict[str, Any]:
    return validate_ledger(usage, golden["signature_profile"]["profile_sha256"],
                           golden["execution_trust_root"], golden["issuance_trust_root"],
                           issuance)


if __name__ == "__main__":
    unittest.main()
