"""Public pure Platform Core Receipt v1 operations."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from .codec import (ContractError, canonical_json, decode_canonical, decode_json,
                    exact_object, observation_digest, validate_document_shape)
from .constants import (DOCUMENT_INVALID, EXECUTION_RECEIPT_DIGEST_DOMAIN,
                        EXECUTION_RECEIPT_TYPE_PROFILE,
                        MAX_RECEIPT_BYTES, RECEIPT_FIXTURE_API_VERSION,
                        RECEIPT_FIXTURE_PATH,
                        VERIFICATION_RECEIPT_DIGEST_DOMAIN,
                        VERIFICATION_RECEIPT_TYPE_PROFILE,
                        VERIFICATION_REQUEST_DIGEST_DOMAIN)
from .constants import VERIFICATION_REQUEST_TYPE_PROFILE
from .contract import read_bounded_file
from .execution_receipt import validate_execution_receipt
from .verification import (validate_verification_exchange,
                           validate_verification_receipt,
                           validate_verification_request)

_FIXTURE_FIELDS = {
    "api_version", "execution_receipt", "expected", "verification_receipt",
    "verification_request",
}
_EXPECTED_FIELDS = {
    "execution_receipt_sha256", "verification_receipt_sha256",
    "verification_request_sha256",
}

def decode_execution_receipt(raw: bytes) -> dict[str, Any]:
    """Decode one exact canonical ExecutionReceipt."""
    return validate_execution_receipt(
        _decode_document(raw, EXECUTION_RECEIPT_TYPE_PROFILE, "ExecutionReceipt"))


def canonical_execution_receipt(value: Any) -> bytes:
    """Validate and canonically encode one ExecutionReceipt."""
    return _validated_canonical(
        value, validate_execution_receipt, EXECUTION_RECEIPT_TYPE_PROFILE,
        "ExecutionReceipt")


def execution_receipt_sha256(value: Any) -> str:
    """Return the ExecutionReceipt conformance observation digest."""
    return observation_digest(EXECUTION_RECEIPT_DIGEST_DOMAIN,
                              canonical_execution_receipt(value))


def decode_verification_request(raw: bytes) -> dict[str, Any]:
    """Decode one exact canonical VerificationRequest."""
    return validate_verification_request(_decode_document(
        raw, VERIFICATION_REQUEST_TYPE_PROFILE, "VerificationRequest"))


def canonical_verification_request(value: Any) -> bytes:
    """Validate and canonically encode one VerificationRequest."""
    return _validated_canonical(
        value, validate_verification_request, VERIFICATION_REQUEST_TYPE_PROFILE,
        "VerificationRequest")


def verification_request_sha256(value: Any) -> str:
    """Return the VerificationRequest conformance observation digest."""
    return observation_digest(VERIFICATION_REQUEST_DIGEST_DOMAIN,
                              canonical_verification_request(value))


def decode_verification_receipt(raw: bytes) -> dict[str, Any]:
    """Decode one exact canonical VerificationReceipt."""
    return validate_verification_receipt(_decode_document(
        raw, VERIFICATION_RECEIPT_TYPE_PROFILE, "VerificationReceipt"))


def canonical_verification_receipt(value: Any) -> bytes:
    """Validate and canonically encode one VerificationReceipt."""
    return _validated_canonical(
        value, validate_verification_receipt, VERIFICATION_RECEIPT_TYPE_PROFILE,
        "VerificationReceipt")


def verification_receipt_sha256(value: Any) -> str:
    """Return the VerificationReceipt conformance observation digest."""
    return observation_digest(VERIFICATION_RECEIPT_DIGEST_DOMAIN,
                              canonical_verification_receipt(value))


def load_receipt_golden(repo_root: Path) -> dict[str, Any]:
    """Load and fully validate the shared receipt golden."""
    path = repo_root / RECEIPT_FIXTURE_PATH
    value = decode_json(read_bounded_file(path, MAX_RECEIPT_BYTES),
                        MAX_RECEIPT_BYTES)
    fixture = exact_object(value, _FIXTURE_FIELDS, "receipt golden fixture")
    if fixture["api_version"] != RECEIPT_FIXTURE_API_VERSION:
        raise ContractError("receipt golden fixture api_version is unsupported")
    validate_document_shape(fixture["execution_receipt"],
                            EXECUTION_RECEIPT_TYPE_PROFILE, "ExecutionReceipt")
    validate_document_shape(fixture["verification_request"],
                            VERIFICATION_REQUEST_TYPE_PROFILE,
                            "VerificationRequest")
    validate_document_shape(fixture["verification_receipt"],
                            VERIFICATION_RECEIPT_TYPE_PROFILE,
                            "VerificationReceipt")
    validate_execution_receipt(fixture["execution_receipt"])
    validate_verification_request(fixture["verification_request"])
    validate_verification_receipt(fixture["verification_receipt"])
    validate_verification_exchange(
        fixture["verification_request"], fixture["verification_receipt"])
    _validate_expected(fixture)
    return fixture


def _validated_canonical(
        value: Any, validator: Callable[[Any], dict[str, Any]],
        profile: Any, label: str) -> bytes:
    canonical = canonical_json(value, MAX_RECEIPT_BYTES)
    snapshot = _decode_document(canonical, profile, label)
    validator(snapshot)
    return canonical


def _decode_document(raw: bytes, profile: Any, label: str) -> dict[str, Any]:
    try:
        value = decode_canonical(raw, MAX_RECEIPT_BYTES)
        validate_document_shape(value, profile, label)
        return value
    except ContractError as error:
        raise error.with_code(DOCUMENT_INVALID) from error


def _validate_expected(fixture: dict[str, Any]) -> None:
    expected = exact_object(fixture["expected"], _EXPECTED_FIELDS,
                            "receipt golden expected")
    actual = {
        "execution_receipt_sha256": execution_receipt_sha256(
            fixture["execution_receipt"]),
        "verification_request_sha256": verification_request_sha256(
            fixture["verification_request"]),
        "verification_receipt_sha256": verification_receipt_sha256(
            fixture["verification_receipt"]),
    }
    for field, digest in actual.items():
        if expected[field] != digest:
            raise ContractError(f"receipt golden {field} pin drifted")
