"""Pure semantic validation for one ADR-0052 production package."""

from __future__ import annotations

import hashlib

from evolve_repo_locator_evidence_adapter import validate_observation
from governance_contract import ContractError

from .codec import canonical_json
from .constants import (CANONICALIZATION, DIMENSION_RANK, MAX_OBSERVATIONS,
                        PARAMETERS_DOMAIN, PRODUCER_ID, PRODUCER_VERSION,
                        PRODUCTION_API, PRODUCTION_DOMAIN, PRODUCTION_FIELDS,
                        SOURCE_DOMAIN)
from .profiles import (exact_fields, parse_report_manifest, source_index,
                       validate_locator_sources, validate_parameters)


def domain_digest(domain: bytes, value: object) -> str:
    return hashlib.sha256(domain + canonical_json(value)).hexdigest()


def parameters_digest(value: object) -> str:
    return domain_digest(PARAMETERS_DOMAIN, value)


def source_digest(value: object) -> str:
    return domain_digest(SOURCE_DOMAIN, value)


def production_digest(value: object) -> str:
    return domain_digest(PRODUCTION_DOMAIN, value)


def _locator_specs(report: dict[str, object]) -> list[tuple[dict[str, object], str, str, object]]:
    specs = []
    dimensions = sorted(report["dimensions"],
                        key=lambda item: DIMENSION_RANK[item["name"]])
    for dimension in dimensions:
        if dimension["status"] not in {"finding", "clear"}:
            continue
        for evidence in dimension["evidence"]:
            specs.append((evidence, dimension["name"], dimension["status"], None))
    for opportunity in sorted(report["opportunities"], key=lambda item: item["id"]):
        for evidence in opportunity["evidence"]:
            specs.append((evidence, opportunity["dimension"], "opportunity",
                          opportunity["id"]))
    return specs


def _build_observation(spec: tuple[dict[str, object], str, str, object],
                       *, run_id: str, observed_at: int,
                       parameters_sha: str, report_sha: str,
                       source_revision: str, source_sha: str,
                       entries: dict[str, dict[str, object]]) -> dict[str, object]:
    evidence, dimension, relation, opportunity_id = spec
    entry = entries[evidence["path"]]
    return {
        "api_version": "forgeos.evolve-repo-locator/v1",
        "canonicalization": CANONICALIZATION,
        "content": {"bytes": entry["bytes"], "sha256": entry["content_sha256"]},
        "locator": {"detail": evidence["detail"],
                    "line": evidence.get("line", 0), "path": evidence["path"]},
        "observed_at_unix_ms": observed_at,
        "producer": {"parameters_sha256": parameters_sha,
                     "producer_id": PRODUCER_ID, "producer_type": "tool",
                     "producer_version": PRODUCER_VERSION, "run_id": run_id},
        "scan_context": {"contract": "evolve_scan_v1",
                         "depth": None, "dimension": dimension,
                         "opportunity_id": opportunity_id, "relation": relation,
                         "report_sha256": report_sha},
        "source": {"source_revision": source_revision,
                   "source_tree_sha256": source_sha},
    }


def expected_observations(production: dict[str, object]) -> list[dict[str, object]]:
    parameters = production["parameters_manifest"]
    report, issues = parse_report_manifest(
        production["report_manifest"], parameters["expected_depth"])
    entries, source_issues = source_index(production["source_manifest"])
    if issues or source_issues or report is None:
        raise ContractError("cannot derive observations from invalid profiles")
    specs = _locator_specs(report)
    actual = production["observations"]
    if not actual:
        if specs:
            raise ContractError("locator-bearing report requires observations")
        return []
    first = actual[0]
    run_id = first["producer"]["run_id"]
    observed_at = first["observed_at_unix_ms"]
    parameters_sha = parameters_digest(parameters)
    report_sha = production["report_manifest"]["sha256"]
    source_sha = source_digest(production["source_manifest"])
    revision = production["source_manifest"]["source_revision"]
    result = []
    for spec in specs:
        observation = _build_observation(
            spec, run_id=run_id, observed_at=observed_at,
            parameters_sha=parameters_sha, report_sha=report_sha,
            source_revision=revision, source_sha=source_sha, entries=entries)
        observation["scan_context"]["depth"] = parameters["expected_depth"]
        result.append(observation)
    return result


def _validate_observation_list(value: object, issues: list[str]) -> bool:
    if not isinstance(value, list) or len(value) > MAX_OBSERVATIONS:
        issues.append(
            f"production.observations: expected array with at most {MAX_OBSERVATIONS} items"
        )
        return False
    for index, observation in enumerate(value):
        nested = validate_observation(observation)
        issues.extend(
            f"production.observations[{index}]: {issue}" for issue in nested
        )
    return True


def _validate_bindings(production: dict[str, object], report: dict[str, object],
                       issues: list[str]) -> None:
    parameters = production["parameters_manifest"]
    source = production["source_manifest"]
    if report["depth"] != parameters["expected_depth"]:
        issues.append("production: report depth does not match parameters")
    issues.extend(validate_locator_sources(report, source))
    if issues:
        return
    try:
        expected = expected_observations(production)
    except (ContractError, KeyError, TypeError, IndexError) as error:
        issues.append(f"production observation mapping: {error}")
        return
    if production["observations"] != expected:
        issues.append(
            "production observations: order, multiplicity, or report/source mapping drifted"
        )


def _validate_production(value: object) -> list[str]:
    issues: list[str] = []
    canonical_json(value)
    if not exact_fields(value, PRODUCTION_FIELDS, "production", issues):
        return issues
    if value["api_version"] != PRODUCTION_API or value["canonicalization"] != CANONICALIZATION:
        issues.append("production: API or canonicalization drifted")
    parameters_issues = validate_parameters(value["parameters_manifest"])
    issues.extend(parameters_issues)
    expected_depth = (value["parameters_manifest"].get("expected_depth")
                      if isinstance(value["parameters_manifest"], dict) else None)
    report, report_issues = parse_report_manifest(value["report_manifest"], expected_depth)
    issues.extend(report_issues)
    _, source_issues = source_index(value["source_manifest"])
    issues.extend(source_issues)
    observations_well_shaped = _validate_observation_list(value["observations"], issues)
    if (not parameters_issues and not report_issues and not source_issues and
            observations_well_shaped and report is not None):
        _validate_bindings(value, report, issues)
    return issues


def validate_production(value: object) -> list[str]:
    """Validate a decoded value without reading the repository or executing Git."""
    try:
        return _validate_production(value)
    except (ContractError, KeyError, TypeError, AttributeError,
            UnicodeError, IndexError) as error:
        return [f"production: invalid nested value: {error}"]
    except MemoryError:
        return ["production: validation exhausted memory"]
