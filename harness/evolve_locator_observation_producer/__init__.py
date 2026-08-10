"""Public strict fixture/checking API for ADR-0052."""

from .codec import canonical_json, decode_production
from .constants import CHECKED, RESULT
from .fixture import computed_expected, validate_golden_fixture
from .profiles import canonical_report
from .semantics import (expected_observations, parameters_digest,
                        production_digest, source_digest, validate_production)

__all__ = [
    "CHECKED", "RESULT", "canonical_json", "canonical_report", "computed_expected",
    "decode_production", "expected_observations", "parameters_digest",
    "production_digest", "source_digest", "validate_golden_fixture",
    "validate_production",
]
