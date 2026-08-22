"""Exact member validation for Kernel operational semantic records."""

from __future__ import annotations

from .codec import ContractError, canonical_json
from .constants import *  # noqa: F403 - this module is the contract vocabulary


def exact_object(value: object, fields: set[str], label: str) -> dict[str, object]:
    """Require one object with exactly the named fields."""
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ContractError(f"{label} must be an object with string keys")
    unknown, missing = set(value) - fields, fields - set(value)
    if unknown or missing:
        raise ContractError(
            f"{label} fields differ: missing={sorted(missing)}, unknown={sorted(unknown)}")
    return value


def text(value: object, label: str, maximum: int = MAX_SHORT_BYTES) -> str:  # noqa: F405
    if not isinstance(value, str) or not value:
        raise ContractError(f"{label} must be non-empty UTF-8 text <= {maximum} bytes")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError as error:
        raise ContractError(f"{label} must be valid UTF-8 text") from error
    if len(encoded) > maximum:
        raise ContractError(f"{label} must be non-empty UTF-8 text <= {maximum} bytes")
    return value


def identifier(value: object, label: str) -> str:
    member = text(value, label)
    if IDENTIFIER_RE.fullmatch(member) is None:  # noqa: F405
        raise ContractError(f"{label} must match the frozen identifier grammar")
    return member


def bare_hash(value: object, label: str) -> str:
    if not isinstance(value, str) or HASH_RE.fullmatch(value) is None:  # noqa: F405
        raise ContractError(f"{label} must be a lowercase bare SHA-256")
    return value


def unsigned(value: object, label: str, maximum: int = MAX_I64) -> int:  # noqa: F405
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise ContractError(f"{label} must be an integer in 0..{maximum}")
    return value


def positive(value: object, label: str, maximum: int) -> int:
    number = unsigned(value, label, maximum)
    if number == 0:
        raise ContractError(f"{label} must be positive")
    return number


def enum(value: object, allowed: set[str], label: str) -> str:
    if not isinstance(value, str) or value not in allowed:
        raise ContractError(f"{label} is unsupported")
    return value


def principal(value: object, label: str) -> dict[str, object]:
    member = exact_object(value, PRINCIPAL_FIELDS, label)  # noqa: F405
    text(member["authority_domain"], f"{label}.authority_domain")
    text(member["principal_id"], f"{label}.principal_id")
    enum(member["principal_type"], PRINCIPAL_TYPES, f"{label}.principal_type")  # noqa: F405
    return member


def task_binding(value: object) -> dict[str, object]:
    member = exact_object(value, TASK_BINDING_FIELDS, "task_binding")  # noqa: F405
    for field in TASK_BINDING_FIELDS - {"attempt_id", "environment_class", "target_id"}:  # noqa: F405
        text(member[field], f"task_binding.{field}")
    enum(member["environment_class"], ENVIRONMENT_CLASSES,  # noqa: F405
         "task_binding.environment_class")
    for field in ("attempt_id", "target_id"):
        if member[field] is not None:
            text(member[field], f"task_binding.{field}")
    return member


def bindings(value: object) -> dict[str, object]:
    member = exact_object(value, BINDING_FIELDS, "bindings")  # noqa: F405
    for field in ("context_sha256", "environment_sha256", "policy_sha256",
                  "source_tree_sha256"):
        bare_hash(member[field], f"bindings.{field}")
    for field in ("environment_profile_id", "source_profile_id", "source_revision"):
        text(member[field], f"bindings.{field}")
    return member


def capability(value: object) -> dict[str, object]:
    member = exact_object(value, CAPABILITY_FIELDS, "capability")  # noqa: F405
    bare_hash(member["capability_contract_sha256"], "capability.capability_contract_sha256")
    text(member["capability_id"], "capability.capability_id")
    text(member["capability_version"], "capability.capability_version")
    return member


def grant_ref(value: object) -> dict[str, object]:
    member = exact_object(value, GRANT_REF_FIELDS, "capability_grant_ref")  # noqa: F405
    text(member["authority_domain"], "capability_grant_ref.authority_domain")
    digest = bare_hash(member["grant_sha256"], "capability_grant_ref.grant_sha256")
    if member["grant_id"] != f"capability-grant-{digest}":
        raise ContractError("capability_grant_ref.grant_id must bind grant_sha256")
    return member


def attestations(value: object) -> dict[str, object]:
    member = exact_object(value, ATTESTATION_FIELDS, "attestations")  # noqa: F405
    if any(item is not False for item in member.values()):
        raise ContractError("every operational attestation must be exactly false")
    return member


def artifact_ref(value: object, label: str = "ArtifactRef") -> dict[str, object]:
    member = exact_object(value, ARTIFACT_FIELDS, label)  # noqa: F405
    text(member["artifact_kind"], f"{label}.artifact_kind")
    text(member["artifact_ref"], f"{label}.artifact_ref", MAX_REFERENCE_BYTES)  # noqa: F405
    bare_hash(member["artifact_sha256"], f"{label}.artifact_sha256")
    return member


def _artifact_list(value: object, label: str, maximum: int, nonempty: bool) -> list:
    if not isinstance(value, list) or len(value) > maximum or (nonempty and not value):
        raise ContractError(f"{label} cardinality is outside 1..{maximum}")
    result = [artifact_ref(item, f"{label}[{index}]") for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")
    return result


def artifact_list(value: object, label: str, maximum: int = MAX_IO_ITEMS,  # noqa: F405
                  nonempty: bool = True) -> list:
    return _artifact_list(value, label, maximum, nonempty)


def optional_artifact_list(value: object, label: str, maximum: int) -> list:
    return _artifact_list(value, label, maximum, False)


def _reference(value: object, fields: set[str], prefix: str,
               id_field: str, hash_field: str, label: str) -> dict[str, object]:
    member = exact_object(value, fields, label)
    digest = bare_hash(member[hash_field], f"{label}.{hash_field}")
    if member[id_field] != f"{prefix}{digest}":
        raise ContractError(f"{label}.{id_field} must bind {hash_field}")
    return member


def artifact_receipt_ref(value: object, label: str) -> dict[str, object]:
    return _reference(value, ARTIFACT_RECEIPT_REF_FIELDS, ARTIFACT_RECEIPT_PREFIX,  # noqa: F405
                      "artifact_receipt_id", "artifact_receipt_sha256", label)


def invocation_ref(value: object, label: str) -> dict[str, object]:
    return _reference(value, INVOCATION_REF_FIELDS, INVOCATION_PREFIX,  # noqa: F405
                      "invocation_id", "invocation_sha256", label)


def event_ref(value: object, label: str) -> dict[str, object]:
    return _reference(value, EVENT_REF_FIELDS, EVENT_PREFIX,  # noqa: F405
                      "event_id", "event_sha256", label)


def execution_receipt_ref(value: object, label: str) -> dict[str, object]:
    return _reference(value, EXECUTION_RECEIPT_REF_FIELDS, EXECUTION_RECEIPT_PREFIX,  # noqa: F405
                      "execution_receipt_id", "execution_receipt_sha256", label)


def _sorted_references(value: object, label: str, maximum: int,
                       validator, nonempty: bool = True) -> list:
    if (not isinstance(value, list) or len(value) > maximum or
            (nonempty and not value)):
        raise ContractError(f"{label} cardinality is outside the frozen bound")
    result = [validator(item, f"{label}[{index}]") for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be strictly canonical-byte sorted and unique")
    return result


def string_set(value: object, label: str, maximum: int, nonempty: bool = True) -> list[str]:
    if not isinstance(value, list) or len(value) > maximum or (nonempty and not value):
        raise ContractError(f"{label} cardinality is outside the frozen bound")
    result = [identifier(item, f"{label}[{index}]") for index, item in enumerate(value)]
    encoded = [item.encode("utf-8") for item in result]
    if encoded != sorted(encoded) or len(encoded) != len(set(encoded)):
        raise ContractError(f"{label} must be strictly UTF-8 sorted and unique")
    return result


def observed_usage(value: object) -> dict[str, object]:
    member = exact_object(value, OBSERVED_USAGE_FIELDS, "observed_usage")  # noqa: F405
    unsigned(member["call_count"], "observed_usage.call_count", MAX_CALL_COUNT)  # noqa: F405
    unsigned(member["cost_usd_micros"], "observed_usage.cost_usd_micros",
             MAX_COST_USD_MICROS)  # noqa: F405
    unsigned(member["elapsed_ms"], "observed_usage.elapsed_ms", MAX_ELAPSED_MS)  # noqa: F405
    unsigned(member["input_tokens"], "observed_usage.input_tokens", MAX_TOKEN_COUNT)  # noqa: F405
    unsigned(member["network_bytes"], "observed_usage.network_bytes", MAX_NETWORK_BYTES)  # noqa: F405
    unsigned(member["output_bytes"], "observed_usage.output_bytes", MAX_OUTPUT_BYTES)  # noqa: F405
    unsigned(member["output_tokens"], "observed_usage.output_tokens", MAX_TOKEN_COUNT)  # noqa: F405
    return member


def _constants(record: dict[str, object], api: str, kind: str) -> None:
    expected = {"api_version": api, "canonicalization": CANONICALIZATION, "kind": kind}  # noqa: F405
    for field, required in expected.items():
        if record[field] != required:
            raise ContractError(f"{field} must be {required!r}")


def _identity(record: dict[str, object], id_field: str, hash_field: str,
              prefix: str, allow_blank: bool) -> None:
    if allow_blank and record[id_field] == record[hash_field] == "":
        return
    digest = bare_hash(record[hash_field], hash_field)
    if record[id_field] != f"{prefix}{digest}":
        raise ContractError(f"{id_field} must bind {hash_field}")


def validate_artifact_receipt_shape(value: object, *, allow_blank: bool = False) -> dict:
    record = exact_object(value, ARTIFACT_RECEIPT_FIELDS, "ArtifactReceipt")  # noqa: F405
    _constants(record, ARTIFACT_RECEIPT_API, ARTIFACT_RECEIPT_KIND)  # noqa: F405
    _identity(record, "artifact_receipt_id", "artifact_receipt_sha256",
              ARTIFACT_RECEIPT_PREFIX, allow_blank)  # noqa: F405
    artifact_ref(record["artifact"])
    attestations(record["attestations"])
    bindings(record["bindings"])
    unsigned(record["content_bytes"], "content_bytes")
    unsigned(record["created_at_unix_ms"], "created_at_unix_ms")
    principal(record["producer"], "producer")
    role = enum(record["receipt_role"], RECEIPT_ROLES, "receipt_role")  # noqa: F405
    producer_ref = record["producer_invocation_ref"]
    if (role == "declared_input") != (producer_ref is None):
        raise ContractError("declared_input requires null producer; output requires a producer")
    if producer_ref is not None:
        invocation_ref(producer_ref, "producer_invocation_ref")
    identifier(record["slot"], "slot")
    task_binding(record["task_binding"])
    return record


def validate_invocation_shape(value: object, *, allow_blank: bool = False) -> dict:
    record = exact_object(value, INVOCATION_FIELDS, "CapabilityInvocation")  # noqa: F405
    _constants(record, INVOCATION_API, INVOCATION_KIND)  # noqa: F405
    _identity(record, "invocation_id", "invocation_sha256", INVOCATION_PREFIX, allow_blank)  # noqa: F405
    attempt = positive(record["attempt"], "attempt", MAX_ATTEMPT)  # noqa: F405
    attestations(record["attestations"])
    bindings(record["bindings"])
    capability(record["capability"])
    grant_ref(record["capability_grant_ref"])
    identifier(record["correlation_id"], "correlation_id")
    string_set(record["declared_output_slots"], "declared_output_slots",
               MAX_IO_ITEMS, False)  # noqa: F405
    identifier(record["idempotency_key"], "idempotency_key")
    _sorted_references(record["input_artifact_receipt_refs"],
                       "input_artifact_receipt_refs", MAX_IO_ITEMS,
                       artifact_receipt_ref, False)  # noqa: F405
    prior = record["prior_execution_receipt_ref"]
    if (attempt == 1) != (prior is None):
        raise ContractError("attempt one requires null prior receipt; retry requires one")
    if prior is not None:
        execution_receipt_ref(prior, "prior_execution_receipt_ref")
    bare_hash(record["requested_action_sha256"], "requested_action_sha256")
    unsigned(record["requested_at_unix_ms"], "requested_at_unix_ms")
    principal(record["subject"], "subject")
    task_binding(record["task_binding"])
    return record


def validate_event_shape(value: object, *, allow_blank: bool = False) -> dict:
    record = exact_object(value, EVENT_FIELDS, "InteractionEvent")  # noqa: F405
    _constants(record, EVENT_API, EVENT_KIND)  # noqa: F405
    _identity(record, "event_id", "event_sha256", EVENT_PREFIX, allow_blank)  # noqa: F405
    principal(record["actor"], "actor")
    artifact_list(record["artifact_refs"], "artifact_refs", nonempty=False)
    attestations(record["attestations"])
    bindings(record["bindings"])
    sequence = positive(record["logical_sequence"], "logical_sequence", MAX_EVENTS)  # noqa: F405
    cause = record["causation_event_ref"]
    if (sequence == 1) != (cause is None):
        raise ContractError("logical sequence one requires null cause; later events require one")
    if cause is not None:
        event_ref(cause, "causation_event_ref")
    confidence = record["confidence_micros"]
    if confidence is not None:
        unsigned(confidence, "confidence_micros", MAX_CONFIDENCE_MICROS)  # noqa: F405
    identifier(record["correlation_id"], "correlation_id")
    invocation_ref(record["invocation_ref"], "invocation_ref")
    text(record["object_ref"], "object_ref", MAX_REFERENCE_BYTES)  # noqa: F405
    unsigned(record["occurred_at_unix_ms"], "occurred_at_unix_ms")
    if record["target"] is not None:
        principal(record["target"], "target")
    task_binding(record["task_binding"])
    enum(record["verb"], EVENT_VERBS, "verb")  # noqa: F405
    return record


def _event_refs(value: object, nonempty: bool) -> list:
    if (not isinstance(value, list) or len(value) > MAX_EVENTS or  # noqa: F405
            (nonempty and not value)):
        raise ContractError("event_refs cardinality is outside the frozen bound")
    result = [event_ref(item, f"event_refs[{index}]") for index, item in enumerate(value)]
    encoded = [canonical_json(item) for item in result]
    if len(encoded) != len(set(encoded)):
        raise ContractError("event_refs must be unique")
    return result


def validate_execution_receipt_shape(value: object, *, allow_blank: bool = False) -> dict:
    record = exact_object(value, EXECUTION_RECEIPT_FIELDS, "ExecutionReceipt")  # noqa: F405
    _constants(record, EXECUTION_RECEIPT_API, EXECUTION_RECEIPT_KIND)  # noqa: F405
    _identity(record, "execution_receipt_id", "execution_receipt_sha256",
              EXECUTION_RECEIPT_PREFIX, allow_blank)  # noqa: F405
    attempt = positive(record["attempt"], "attempt", MAX_ATTEMPT)  # noqa: F405
    attestations(record["attestations"])
    bindings(record["bindings"])
    identifier(record["correlation_id"], "correlation_id")
    started = unsigned(record["started_at_unix_ms"], "started_at_unix_ms")
    ended = unsigned(record["ended_at_unix_ms"], "ended_at_unix_ms")
    if started > ended:
        raise ContractError("started_at_unix_ms must not exceed ended_at_unix_ms")
    if ended - started > MAX_ELAPSED_MS:  # noqa: F405
        raise ContractError("execution wall interval exceeds MAX_ELAPSED_MS")
    _event_refs(record["event_refs"], False)
    artifact_list(record["input_artifacts"], "input_artifacts", nonempty=False)
    invocation_ref(record["invocation_ref"], "invocation_ref")
    usage = observed_usage(record["observed_usage"])
    if usage["elapsed_ms"] != ended - started:
        raise ContractError("observed_usage.elapsed_ms must equal the execution wall interval")
    outcome = enum(record["outcome"], OUTCOMES, "outcome")  # noqa: F405
    _sorted_references(record["output_artifact_receipt_refs"],
                       "output_artifact_receipt_refs", MAX_IO_ITEMS,
                       artifact_receipt_ref, False)  # noqa: F405
    prior = record["prior_execution_receipt_ref"]
    if (attempt == 1) != (prior is None):
        raise ContractError("attempt one requires null prior receipt; retry requires one")
    if prior is not None:
        execution_receipt_ref(prior, "prior_execution_receipt_ref")
    reasons = string_set(record["reason_codes"], "reason_codes", MAX_REASON_CODES, False)  # noqa: F405
    if (outcome == "succeeded") != (not reasons):
        raise ContractError("succeeded requires no reasons; every other outcome requires one")
    principal(record["executor"], "executor")
    task_binding(record["task_binding"])
    return record
