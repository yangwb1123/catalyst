"""Closed shapes and lexical validation for CapabilityGrant v1."""

from __future__ import annotations

import base64
import ipaddress
import re
from typing import Any

from .canonical import ContractError, canonical_json
from .constants import MAX_BYTES, MAX_COST_MICROS, MAX_TIMEOUT_MS, MAX_USAGE


def require_keys(value: Any, label: str, keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    actual = set(value)
    if actual != keys:
        raise ContractError(f"{label} fields must be exactly {sorted(keys)!r}")
    return value


def text(value: Any, label: str, maximum: int = 160) -> str:
    if not isinstance(value, str) or not 1 <= len(value.encode("utf-8")) <= maximum:
        raise ContractError(f"{label} must be non-empty text of at most {maximum} bytes")
    return value


def enum(value: Any, label: str, allowed: tuple[str, ...]) -> str:
    if value not in allowed:
        raise ContractError(f"{label} must be one of {allowed!r}")
    return value


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return value


def nullable_sha256(value: Any, label: str) -> str | None:
    if value is not None:
        return sha256(value, label)
    return None


def integer(value: Any, label: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ContractError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def boolean(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise ContractError(f"{label} must be a boolean")
    return value


def array(value: Any, label: str, minimum: int, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ContractError(f"{label} item count must be {minimum}..{maximum}")
    return value


def sorted_unique_nodes(values: list[Any], label: str) -> None:
    encoded = [canonical_json(value) for value in values]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")


def sorted_unique_resources(values: list[dict[str, Any]], label: str) -> None:
    encoded = [(value["scope_kind"].encode("utf-8"), canonical_json(value)) for value in values]
    if any(left >= right for left, right in zip(encoded, encoded[1:])):
        raise ContractError(f"{label} must be scope-kind/canonical-byte sorted and unique")


def sorted_unique_strings(values: Any, label: str, allowed: tuple[str, ...]) -> list[str]:
    result = array(values, label, 1, len(allowed))
    if any(not isinstance(value, str) or value not in allowed for value in result):
        raise ContractError(f"{label} contains an unsupported value")
    if any(left >= right for left, right in zip(result, result[1:])):
        raise ContractError(f"{label} must be strictly UTF-8 sorted and unique")
    return result


def validate_principal(value: Any, label: str) -> None:
    node = require_keys(value, label, {"authority_domain", "principal_id", "principal_type"})
    text(node["authority_domain"], f"{label}.authority_domain")
    text(node["principal_id"], f"{label}.principal_id")
    enum(node["principal_type"], f"{label}.principal_type",
         ("agent", "human", "operator", "service"))


def validate_task_binding(value: Any, label: str = "task_binding") -> None:
    keys = {"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
            "project_id", "role", "run_id", "target_id", "task_id"}
    node = require_keys(value, label, keys)
    for field in ("change_id", "environment_id", "node_id", "project_id", "role", "run_id",
                  "task_id"):
        text(node[field], f"{label}.{field}")
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "local", "production", "staging", "test"))
    for field in ("attempt_id", "target_id"):
        if node[field] is not None:
            text(node[field], f"{label}.{field}")


def validate_capability(value: Any, label: str = "capability") -> None:
    node = require_keys(value, label,
                        {"capability_contract_sha256", "capability_id", "capability_version"})
    sha256(node["capability_contract_sha256"], f"{label}.capability_contract_sha256")
    text(node["capability_id"], f"{label}.capability_id")
    text(node["capability_version"], f"{label}.capability_version")


def validate_bindings(value: Any, label: str = "bindings") -> None:
    keys = {"context_sha256", "grant_request_sha256", "impact_sha256", "plan_sha256",
            "policy_sha256", "risk_sha256", "source_revision", "source_tree_sha256"}
    node = require_keys(value, label, keys)
    for field in ("context_sha256", "grant_request_sha256", "policy_sha256",
                  "source_tree_sha256"):
        sha256(node[field], f"{label}.{field}")
    for field in ("impact_sha256", "plan_sha256", "risk_sha256"):
        nullable_sha256(node[field], f"{label}.{field}")
    text(node["source_revision"], f"{label}.source_revision")


def validate_budget(value: Any, label: str = "budget") -> None:
    keys = {"max_calls", "max_cost_usd_micros", "max_input_tokens", "max_network_bytes",
            "max_output_bytes", "max_output_tokens", "timeout_ms"}
    node = require_keys(value, label, keys)
    integer(node["max_calls"], f"{label}.max_calls", 1, MAX_USAGE)
    integer(node["max_cost_usd_micros"], f"{label}.max_cost_usd_micros", 0,
            MAX_COST_MICROS)
    for field in ("max_input_tokens", "max_output_tokens"):
        integer(node[field], f"{label}.{field}", 0, MAX_USAGE)
    for field in ("max_network_bytes", "max_output_bytes"):
        integer(node[field], f"{label}.{field}", 0, MAX_BYTES)
    integer(node["timeout_ms"], f"{label}.timeout_ms", 1, MAX_TIMEOUT_MS)


def validate_usage(value: Any, label: str = "usage") -> None:
    keys = {"call_count", "cost_usd_micros", "input_tokens", "network_bytes",
            "output_bytes", "output_tokens", "timeout_ms"}
    node = require_keys(value, label, keys)
    integer(node["call_count"], f"{label}.call_count", 1, MAX_USAGE)
    integer(node["cost_usd_micros"], f"{label}.cost_usd_micros", 0, MAX_COST_MICROS)
    for field in ("input_tokens", "output_tokens"):
        integer(node[field], f"{label}.{field}", 0, MAX_USAGE)
    for field in ("network_bytes", "output_bytes"):
        integer(node[field], f"{label}.{field}", 0, MAX_BYTES)
    integer(node["timeout_ms"], f"{label}.timeout_ms", 1, MAX_TIMEOUT_MS)


def canonical_path(value: Any, label: str, allow_root: bool) -> str:
    path = text(value, label, 4096)
    bad = (path.startswith("/") or path.endswith("/") or "\\" in path or
           re.search(r"[*?\[\]{}]", path) is not None)
    if path == ".":
        if allow_root:
            return path
        raise ContractError(f"{label} cannot name repository root")
    if bad or any(part in ("", ".", "..") for part in path.split("/")):
        raise ContractError(f"{label} is not a canonical repo-relative path")
    return path


def _validate_repo_path(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"match", "path", "scope_kind"})
    enum(node["match"], f"{label}.match", ("exact", "subtree"))
    canonical_path(node["path"], f"{label}.path", node["match"] == "subtree")


def _validate_artifact(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"artifact_kind", "artifact_ref", "artifact_sha256", "scope_kind"})
    text(node["artifact_kind"], f"{label}.artifact_kind")
    text(node["artifact_ref"], f"{label}.artifact_ref", 4096)
    sha256(node["artifact_sha256"], f"{label}.artifact_sha256")


def _validate_environment(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"environment_class", "environment_id", "environment_sha256",
                               "scope_kind"})
    enum(node["environment_class"], f"{label}.environment_class",
         ("development", "production", "staging", "test"))
    text(node["environment_id"], f"{label}.environment_id")
    sha256(node["environment_sha256"], f"{label}.environment_sha256")


def _validate_governance(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"object_kind", "object_ref", "object_scope_sha256", "scope_kind"})
    enum(node["object_kind"], f"{label}.object_kind", ("approval", "knowledge", "policy"))
    text(node["object_ref"], f"{label}.object_ref", 4096)
    sha256(node["object_scope_sha256"], f"{label}.object_scope_sha256")


def _validate_network(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"host", "host_kind", "port", "scheme", "scope_kind"})
    kind = enum(node["host_kind"], f"{label}.host_kind", ("dns", "ipv4", "ipv6"))
    host = text(node["host"], f"{label}.host", 253)
    _validate_host(host, kind, label)
    integer(node["port"], f"{label}.port", 1, 65535)
    enum(node["scheme"], f"{label}.scheme", ("http", "https"))


def _validate_host(host: str, kind: str, label: str) -> None:
    if kind in ("ipv4", "ipv6"):
        if kind == "ipv6" and "%" in host:
            raise ContractError(f"{label}.host cannot contain an IPv6 zone ID")
        try:
            parsed = ipaddress.ip_address(host)
        except ValueError as error:
            raise ContractError(f"{label}.host is not an IP address") from error
        if kind == "ipv6" and parsed.ipv4_mapped is not None:
            raise ContractError(f"{label}.host cannot be IPv4-mapped IPv6")
        if parsed.version != (4 if kind == "ipv4" else 6) or str(parsed) != host:
            raise ContractError(f"{label}.host is not canonical {kind}")
        return
    try:
        parsed_dns = ipaddress.ip_address(host)
    except ValueError:
        parsed_dns = None
    if parsed_dns is not None and parsed_dns.version == 4 and str(parsed_dns) == host:
        raise ContractError(f"{label}.host cannot tag canonical IPv4 as DNS")
    labels = host.split(".")
    valid = host == host.lower() and not host.endswith(".") and all(
        part and len(part) <= 63 and re.fullmatch(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?", part)
        for part in labels)
    if not valid:
        raise ContractError(f"{label}.host is not canonical DNS")


def _validate_command(node: dict[str, Any], label: str) -> None:
    keys = {"argv", "cwd", "environment_sha256", "scope_kind", "stdin_bytes",
            "stdin_sha256", "timeout_ms", "tool_snapshot_sha256"}
    require_keys(node, label, keys)
    argv = array(node["argv"], f"{label}.argv", 1, 64)
    for index, argument in enumerate(argv):
        text(argument, f"{label}.argv[{index}]", 4096)
    if sum(len(argument.encode("utf-8")) for argument in argv) > 32768:
        raise ContractError(f"{label}.argv exceeds the aggregate UTF-8 byte limit")
    canonical_path(node["cwd"], f"{label}.cwd", True)
    for field in ("environment_sha256", "stdin_sha256", "tool_snapshot_sha256"):
        sha256(node[field], f"{label}.{field}")
    integer(node["stdin_bytes"], f"{label}.stdin_bytes", 0, 2**63 - 1)
    if node["stdin_bytes"] == 0 and node["stdin_sha256"] != (
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"):
        raise ContractError(f"{label} zero-byte stdin must bind SHA-256(empty)")
    integer(node["timeout_ms"], f"{label}.timeout_ms", 1, MAX_TIMEOUT_MS)


def _validate_secret(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"broker_id", "scope_kind", "secret_ref", "version_ref"})
    text(node["broker_id"], f"{label}.broker_id")
    text(node["secret_ref"], f"{label}.secret_ref", 4096)
    version = text(node["version_ref"], f"{label}.version_ref", 4096)
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._:/@+\-]{0,4095}", version) is None:
        raise ContractError(f"{label}.version_ref must be an ASCII visible identifier")
    if version.lower() in {"active", "current", "latest"}:
        raise ContractError(f"{label}.version_ref must identify an immutable version")


def _validate_target(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"scope_kind", "target_attestation_sha256", "target_id"})
    sha256(node["target_attestation_sha256"], f"{label}.target_attestation_sha256")
    text(node["target_id"], f"{label}.target_id")


def _validate_target_query(node: dict[str, Any], label: str) -> None:
    require_keys(node, label, {"query_ref", "query_sha256", "scope_kind"})
    text(node["query_ref"], f"{label}.query_ref", 4096)
    sha256(node["query_sha256"], f"{label}.query_sha256")


RESOURCE_VALIDATORS = {
    "artifact": _validate_artifact, "command": _validate_command,
    "environment": _validate_environment, "governance_object": _validate_governance,
    "network_origin": _validate_network, "repo_path": _validate_repo_path,
    "secret_ref": _validate_secret, "target": _validate_target,
    "target_query": _validate_target_query,
}


def validate_resource(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not isinstance(value.get("scope_kind"), str):
        raise ContractError(f"{label} must be a tagged scope resource")
    validator = RESOURCE_VALIDATORS.get(value["scope_kind"])
    if validator is None:
        raise ContractError(f"{label}.scope_kind is unsupported")
    validator(value, label)
    return value


def validate_resources(value: Any, label: str, minimum: int = 1,
                       maximum: int = 256) -> list[dict[str, Any]]:
    nodes = array(value, label, minimum, maximum)
    result = [validate_resource(node, f"{label}[{index}]")
              for index, node in enumerate(nodes)]
    sorted_unique_resources(result, label)
    return result


def validate_base64url(value: Any, label: str) -> str:
    encoded = text(value, label, 16_384)
    if len(encoded) < 16 or re.fullmatch(r"[A-Za-z0-9_-]+", encoded) is None:
        raise ContractError(f"{label} must be unpadded base64url text")
    try:
        decoded = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4))
    except ValueError as error:
        raise ContractError(f"{label} is not base64url") from error
    canonical = base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=")
    if canonical != encoded:
        raise ContractError(f"{label} is not canonical unpadded base64url")
    return encoded
