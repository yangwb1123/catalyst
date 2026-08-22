"""Raw first-delivery result and durable content-free metadata."""

from __future__ import annotations

from typing import Any

from .canonical import (ContractError, bounded_canonical_json, plain_sha256,
                        self_digest)
from .constants import (CANONICALIZATION, DELIVERY_API, MAX_DELIVERY_BYTES,
                        MAX_METADATA_BYTES, MAX_OUTPUT_BYTES, MAX_RESULT_BYTES,
                        METADATA_API, METADATA_DOMAIN, PROFILE_ID, RESULT_API,
                        RESULT_DOMAIN)
from .manifest import manifest_content_bytes
from .shape import (decode_raw_base64url, integer, require_keys, sha256,
                    validate_observed_usage, validate_path)

RESULT_FIELDS = {
    "api_version", "canonicalization", "completed_at_unix_ms", "content_bytes",
    "execution_policy_sha256", "execution_result_id", "execution_result_sha256",
    "execution_trust_epoch", "execution_trust_root_sha256", "grant_envelope_sha256",
    "grant_id", "grant_sha256", "invocation_id", "invocation_sha256",
    "issuance_trust_epoch", "issuance_trust_root_sha256", "kind", "manifest_sha256",
    "observation_semantics", "observed_usage", "profile_id", "reads",
    "requested_action_sha256",
}
RAW_READ_FIELDS = {"content_base64url", "content_bytes", "content_sha256", "path"}
METADATA_FIELDS = {
    "api_version", "canonicalization", "content_bytes", "execution_result_id",
    "execution_result_sha256", "kind", "manifest_sha256", "metadata_sha256",
    "observed_usage", "read_count", "reads",
}
METADATA_READ_FIELDS = {"content_bytes", "content_sha256", "path"}
DELIVERY_FIELDS = {
    "api_version", "canonicalization", "delivery_disposition", "execution_result",
    "kind", "receipt", "result_metadata",
}


def result_sha256(value: dict[str, Any]) -> str:
    return self_digest(RESULT_DOMAIN, value, "execution_result_sha256", MAX_RESULT_BYTES,
                       "BootstrapRepoReadExecutionResult",
                       derived_id_field="execution_result_id")


def metadata_sha256(value: dict[str, Any]) -> str:
    return self_digest(METADATA_DOMAIN, value, "metadata_sha256", MAX_METADATA_BYTES,
                       "BootstrapRepoReadResultMetadata")


def validate_result(value: Any, manifest: dict[str, Any], policy: dict[str, Any],
                    invocation: dict[str, Any]) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadExecutionResult", RESULT_FIELDS)
    bounded_canonical_json(node, MAX_RESULT_BYTES, "BootstrapRepoReadExecutionResult")
    _validate_result_envelope(node)
    _validate_result_identity(node, manifest, policy, invocation)
    raw_total = _validate_raw_reads(node["reads"], manifest)
    observed = validate_observed_usage(node["observed_usage"], "result.observed_usage")
    if raw_total != node["content_bytes"] or observed["output_bytes"] != raw_total:
        raise ContractError("result raw content accounting does not match")
    if observed["elapsed_ms"] > invocation["requested_action"]["usage"]["timeout_ms"]:
        raise ContractError("result observed elapsed time exceeds cooperative budget")
    digest = result_sha256(node)
    expected_id = "bootstrap-repo-read-result-" + digest
    if node["execution_result_sha256"] != digest or node["execution_result_id"] != expected_id:
        raise ContractError("execution result digest-derived identity does not match")
    return node


def _validate_result_envelope(node: dict[str, Any]) -> None:
    expected = (RESULT_API, CANONICALIZATION, "BootstrapRepoReadExecutionResult", PROFILE_ID)
    actual = (node["api_version"], node["canonicalization"], node["kind"], node["profile_id"])
    if actual != expected:
        raise ContractError("execution result envelope drifted from v1")
    if node["observation_semantics"] != "manifest_bound_ordered_non_atomic_raw_file_reads":
        raise ContractError("execution result observation semantics drifted")
    integer(node["completed_at_unix_ms"], "result.completed_at_unix_ms", 0, 2**63 - 1)
    integer(node["content_bytes"], "result.content_bytes", 0, MAX_OUTPUT_BYTES)
    for field in ("execution_policy_sha256", "execution_result_sha256",
                  "execution_trust_root_sha256", "grant_envelope_sha256", "grant_sha256",
                  "invocation_sha256", "issuance_trust_root_sha256", "manifest_sha256",
                  "requested_action_sha256"):
        sha256(node[field], f"result.{field}")
    for field in ("execution_trust_epoch", "issuance_trust_epoch"):
        integer(node[field], f"result.{field}", 1, 2**63 - 1)


def _validate_result_identity(node: dict[str, Any], manifest: dict[str, Any],
                              policy: dict[str, Any], invocation: dict[str, Any]) -> None:
    expected = {
        "execution_policy_sha256": policy["execution_policy_sha256"],
        "execution_trust_epoch": policy["execution_trust_epoch"],
        "execution_trust_root_sha256": policy["execution_trust_root_sha256"],
        "grant_envelope_sha256": policy["grant_envelope_sha256"],
        "grant_id": policy["grant_id"], "grant_sha256": policy["grant_sha256"],
        "invocation_id": invocation["invocation_id"],
        "invocation_sha256": invocation["invocation_sha256"],
        "issuance_trust_epoch": policy["issuance_trust_epoch"],
        "issuance_trust_root_sha256": policy["issuance_trust_root_sha256"],
        "manifest_sha256": manifest["manifest_sha256"],
        "requested_action_sha256": policy["requested_action_sha256"],
    }
    if any(node[field] != value for field, value in expected.items()):
        raise ContractError("execution result identity differs from Policy or Invocation")
    if node["content_bytes"] != manifest_content_bytes(manifest):
        raise ContractError("execution result byte total differs from manifest")


def _validate_raw_reads(value: Any, manifest: dict[str, Any]) -> int:
    if not isinstance(value, list) or len(value) != len(manifest["entries"]):
        raise ContractError("execution result reads must equal manifest cardinality")
    total = 0
    for index, (item, expected) in enumerate(zip(value, manifest["entries"])):
        label = f"result.reads[{index}]"
        node = require_keys(item, label, RAW_READ_FIELDS)
        validate_path(node["path"], f"{label}.path")
        integer(node["content_bytes"], f"{label}.content_bytes", 0, MAX_OUTPUT_BYTES)
        sha256(node["content_sha256"], f"{label}.content_sha256")
        raw = decode_raw_base64url(node["content_base64url"], f"{label}.content_base64url")
        if (node["path"] != expected["path"] or node["content_bytes"] != len(raw) or
                node["content_bytes"] != expected["content_bytes"] or
                node["content_sha256"] != plain_sha256(raw) or
                node["content_sha256"] != expected["content_sha256"]):
            raise ContractError(f"{label} does not reproduce expected raw manifest bytes")
        total += len(raw)
    return total


def validate_metadata(value: Any, result: dict[str, Any] | None = None) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadResultMetadata", METADATA_FIELDS)
    bounded_canonical_json(node, MAX_METADATA_BYTES, "BootstrapRepoReadResultMetadata")
    expected = (METADATA_API, CANONICALIZATION, "BootstrapRepoReadResultMetadata")
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError("result metadata envelope drifted from v1")
    _validate_metadata_shape(node)
    if result is not None:
        _validate_metadata_result_relation(node, result)
    if node["metadata_sha256"] != metadata_sha256(node):
        raise ContractError("result metadata self digest does not match")
    return node


def _validate_metadata_shape(node: dict[str, Any]) -> None:
    integer(node["content_bytes"], "metadata.content_bytes", 0, MAX_OUTPUT_BYTES)
    integer(node["read_count"], "metadata.read_count", 1, 16)
    for field in ("execution_result_sha256", "manifest_sha256", "metadata_sha256"):
        sha256(node[field], f"metadata.{field}")
    expected_id = "bootstrap-repo-read-result-" + node["execution_result_sha256"]
    if (not isinstance(node["execution_result_id"], str) or
            node["execution_result_id"] != expected_id):
        raise ContractError("metadata execution result identity is not digest-derived")
    observed = validate_observed_usage(node["observed_usage"], "metadata.observed_usage")
    reads = node["reads"]
    if not isinstance(reads, list) or len(reads) != node["read_count"]:
        raise ContractError("metadata reads differ from read_count")
    total = 0
    paths: list[bytes] = []
    for index, value in enumerate(reads):
        label = f"metadata.reads[{index}]"
        read = require_keys(value, label, METADATA_READ_FIELDS)
        paths.append(validate_path(read["path"], f"{label}.path").encode("utf-8"))
        total += integer(read["content_bytes"], f"{label}.content_bytes", 0, MAX_OUTPUT_BYTES)
        sha256(read["content_sha256"], f"{label}.content_sha256")
    if any(left >= right for left, right in zip(paths, paths[1:])):
        raise ContractError("metadata reads must be path-sorted and unique")
    if total != node["content_bytes"] or observed["output_bytes"] != total:
        raise ContractError("metadata content accounting does not match")


def _validate_metadata_result_relation(node: dict[str, Any], result: dict[str, Any]) -> None:
    raw_reads = result["reads"]
    expected_reads = [{key: item[key] for key in ("content_bytes", "content_sha256", "path")}
                      for item in raw_reads]
    expected = {"content_bytes": result["content_bytes"],
                "execution_result_id": result["execution_result_id"],
                "execution_result_sha256": result["execution_result_sha256"],
                "manifest_sha256": result["manifest_sha256"],
                "observed_usage": result["observed_usage"], "read_count": len(raw_reads),
                "reads": expected_reads}
    if any(node[field] != value for field, value in expected.items()):
        raise ContractError("durable result metadata differs from raw first-delivery result")


def validate_delivery(value: Any, receipt: dict[str, Any],
                      metadata: dict[str, Any] | None) -> dict[str, Any]:
    node = require_keys(value, "BootstrapRepoReadExecutionDelivery", DELIVERY_FIELDS)
    bounded_canonical_json(node, MAX_DELIVERY_BYTES, "BootstrapRepoReadExecutionDelivery")
    expected = (DELIVERY_API, CANONICALIZATION, "BootstrapRepoReadExecutionDelivery")
    if (node["api_version"], node["canonicalization"], node["kind"]) != expected:
        raise ContractError("execution delivery envelope drifted from v1")
    if node["receipt"] != receipt or node["result_metadata"] != metadata:
        raise ContractError("delivery does not bind exact terminal receipt and metadata")
    disposition = node["delivery_disposition"]
    if disposition == "first_delivery":
        if (node["execution_result"] is None or metadata is None or
                receipt["state"] != "completed"):
            raise ContractError("first delivery requires completed result and metadata")
    elif disposition == "exact_replay":
        if node["execution_result"] is not None:
            raise ContractError("exact replay must not return raw repository content")
        _validate_replay_metadata(receipt, metadata)
    else:
        raise ContractError("execution delivery disposition is unsupported")
    _validate_delivery_hashes(receipt, metadata)
    return node


def _validate_replay_metadata(receipt: dict[str, Any],
                              metadata: dict[str, Any] | None) -> None:
    state = receipt["state"]
    if state == "completed" and metadata is None:
        raise ContractError("completed replay requires durable result metadata")
    if state in ("failed_consumed", "quarantined") and metadata is not None:
        raise ContractError("content-free terminal replay forbids result metadata")
    if state not in ("completed", "failed_consumed", "quarantined"):
        raise ContractError("exact replay requires a terminal receipt")


def _validate_delivery_hashes(receipt: dict[str, Any],
                              metadata: dict[str, Any] | None) -> None:
    if metadata is None:
        if (receipt["execution_result_sha256"] is not None or
                receipt["result_metadata_sha256"] is not None):
            raise ContractError("content-free terminal receipt unexpectedly binds metadata")
        return
    if (receipt["execution_result_sha256"] != metadata["execution_result_sha256"] or
            receipt["result_metadata_sha256"] != metadata["metadata_sha256"]):
        raise ContractError("delivery receipt differs from result metadata")


__all__ = [
    "metadata_sha256", "result_sha256", "validate_delivery", "validate_metadata",
    "validate_result",
]
