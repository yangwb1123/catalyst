"""Pure semantic validation for one producer contract package."""

from __future__ import annotations

import hashlib

from command_observation_evidence_adapter import validate_observation
from governance_contract import ContractError

from .codec import canonical_json
from .constants import (CANONICALIZATION, COMMANDS, ENVIRONMENT_DOMAIN,
                        PRODUCTION_API, PRODUCTION_DOMAIN, PRODUCTION_FIELDS,
                        PRODUCER_ID, PRODUCER_VERSION, SOURCE_DOMAIN, TOOL_DOMAIN)
from .profiles import exact_fields, validate_environment, validate_source, validate_tool


def domain_digest(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + canonical_json(value)).hexdigest()


def validate_production(value: object) -> list[str]:
    """Validate any bounded in-memory JSON value without propagating shape errors."""
    try:
        return _validate_production(value)
    except (ContractError, KeyError, TypeError, AttributeError, UnicodeError) as error:
        return [f"production: invalid nested value: {error}"]
    except MemoryError:
        return ["production: validation exhausted memory"]


def _validate_production(value: object) -> list[str]:
    issues: list[str] = []
    try:
        canonical_json(value)
    except ContractError as error:
        return [str(error)]
    if not exact_fields(value, PRODUCTION_FIELDS, "production", issues):
        return issues
    if value["api_version"] != PRODUCTION_API or value["canonicalization"] != CANONICALIZATION:
        issues.append("production: API or canonicalization drifted")
    observation = value["observation"]
    observation_issues = validate_observation(observation)
    issues.extend(f"production observation: {issue}" for issue in observation_issues)
    requested = validate_production_observation(observation, issues)
    environment_issues = validate_environment(value["environment_manifest"])
    source_issues = validate_source(value["source_manifest"])
    tool_issues = validate_tool(value["tool_manifest"], requested or "")
    issues.extend(environment_issues + tool_issues + source_issues)
    if not observation_issues and not environment_issues and not tool_issues and not source_issues:
        issues.extend(validate_profile_bindings(value))
    return issues


def validate_production_observation(observation: object, issues: list[str]) -> str | None:
    if not isinstance(observation, dict):
        return None
    command = observation.get("command")
    producer = observation.get("producer")
    if not isinstance(command, dict):
        issues.append("production command: expected object")
        return None
    argv = command.get("argv")
    if not isinstance(argv, list) or not all(isinstance(item, str) for item in argv):
        issues.append("production command: argv is not a string list")
        return None
    if tuple(argv) not in COMMANDS:
        issues.append("production command: argv is not a closed command class")
        return None
    if (command.get("cwd") != "." or command.get("stdin_bytes") != 0 or
            observation.get("evidence_type") != "gate_result"):
        issues.append("production command: fixed fields drifted")
    if (not isinstance(producer, dict) or producer.get("producer_id") != PRODUCER_ID or
            producer.get("producer_type") != "tool" or producer.get("producer_version") != PRODUCER_VERSION):
        issues.append("production producer: fixed identity drifted")
    return argv[0]


def validate_profile_bindings(value: dict[str, object]) -> list[str]:
    try:
        observation, issues = value["observation"], []
        command, source = observation["command"], observation["source"]
        expected_environment = domain_digest(ENVIRONMENT_DOMAIN, value["environment_manifest"])
        expected_tool = domain_digest(TOOL_DOMAIN, value["tool_manifest"])
        expected_source = domain_digest(SOURCE_DOMAIN, value["source_manifest"])
        if command["environment_sha256"] != expected_environment:
            issues.append("production environment digest binding mismatch")
        if command["tool_snapshot_sha256"] != expected_tool:
            issues.append("production tool digest binding mismatch")
        if source["source_tree_sha256"] != expected_source:
            issues.append("production source digest binding mismatch")
        if source["source_revision"] != value["source_manifest"]["source_revision"]:
            issues.append("production source revision binding mismatch")
        return issues
    except (ContractError, KeyError, TypeError, AttributeError) as error:
        return [f"production profile binding shape: {error}"]


def production_digest(value: object) -> str:
    return domain_digest(PRODUCTION_DOMAIN, value)
