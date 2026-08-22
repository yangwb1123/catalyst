"""Frozen wire constants and resource bounds for ADR-0068."""

from pathlib import Path

CANONICALIZATION = "forgeos.canonical-json/v1"
REGISTRY_API = "forgeos.capability-registry/v1"
ENTRY_API = "forgeos.capability-registry-entry/v1"
CONTRACT_API = "forgeos.capability-contract/v1"
REQUEST_API = "forgeos.capability-registry-declared-resolution-request/v1"
ASSESSMENT_API = "forgeos.capability-registry-declared-resolution/v1"
FIXTURE_API = "forgeos.capability-registry-golden/v1"

CONTENT_SET_DOMAIN = b"forgeos.capability-registry.content-set.v1\0"
CONTRACT_DOMAIN = b"forgeos.capability-contract.v1\0"
ENTRY_DOMAIN = b"forgeos.capability-registry-entry.v1\0"
REGISTRY_DOMAIN = b"forgeos.capability-registry.v1\0"
REQUEST_DOMAIN = b"forgeos.capability-registry-declared-resolution-request.v1\0"
ASSESSMENT_DOMAIN = b"forgeos.capability-registry-declared-resolution.v1\0"

EFFECT_VOCABULARY_SHA256 = (
    "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
)
LEGACY_OPAQUE_SHA256 = "8" * 64
FROZEN_REGISTRY_SHA256 = "23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4"
FROZEN_FIXTURE_SHA256 = "0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5"
FROZEN_FIXTURE_BYTES = 28758
FIXTURE_PATH = Path("docs/contracts/fixtures/capability-registry-v1.json")
SCHEMA_PATH = Path("docs/contracts/capability-registry-v1.schema.json")

MAX_REGISTRY_BYTES = 16 * 1024 * 1024
MAX_ENTRY_BYTES = 1024 * 1024
MAX_CONTRACT_BYTES = 512 * 1024
MAX_CONTENT_SET_BYTES = 4 * 1024 * 1024
MAX_REQUEST_BYTES = 256 * 1024
MAX_ASSESSMENT_BYTES = 256 * 1024
MAX_GOLDEN_BYTES = 32 * 1024 * 1024
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ITEMS = 256
MAX_STRING_BYTES = 16 * 1024
MAX_IDENTIFIER_BYTES = 160
MAX_PATH_BYTES = 4096
MAX_NARRATIVE_BYTES = 4096
MIN_I64 = -(2**63)
MAX_I64 = 2**63 - 1

REGISTRY_FIELDS = {
    "api_version", "canonicalization", "coverage_mode",
    "effect_vocabulary_sha256", "entries", "kind", "registry_id",
    "registry_mode", "registry_sha256", "status",
}
ENTRY_FIELDS = {
    "api_version", "canonicalization", "catalog_binding", "content_sets",
    "contract", "entry_id", "entry_sha256", "implementations", "kind",
    "owner", "tests",
}
CONTRACT_FIELDS = {
    "api_version", "canonicalization", "capability_contract_id",
    "capability_contract_sha256", "capability_id", "capability_version",
    "domain", "effects", "failure_modes", "input_schemas", "kind",
    "not_applicable", "observability", "output_schemas",
    "permission_requirements", "postconditions", "preconditions",
    "proof_obligations", "quality_gates", "risk_floor",
    "rollback_or_compensation", "rules", "trigger",
}
REQUEST_FIELDS = {
    "api_version", "canonicalization", "expected_contract",
    "expected_reference", "kind", "registry_sha256", "request_sha256",
}
ASSESSMENT_FIELDS = {
    "api_version", "assessment_mode", "assessment_sha256",
    "authorization_decision", "canonicalization", "effect_attestation",
    "gate_applicability_state", "implementation_availability_attestation",
    "invocation_attestation", "kind", "matched_key_entry_id",
    "matched_key_entry_sha256", "owner_authentication_state",
    "permission_attestation", "persistence_attestation",
    "proof_satisfaction_state", "reason_codes",
    "registry_authentication_state", "registry_sha256", "relations",
    "request_sha256", "resolution", "result", "rule_applicability_state",
    "runtime_routing_attestation", "test_pass_attestation",
    "transition_attestation",
}

RELATION_FIELDS = (
    "domain", "effects", "failure_modes", "identity", "input_schemas",
    "not_applicable", "observability", "output_schemas",
    "permission_requirements", "postconditions", "preconditions",
    "proof_obligations", "quality_gates", "risk_floor",
    "rollback_or_compensation", "rules", "trigger",
)
RESULT = (
    "RESOLVED_DECLARED_CAPABILITY_REFERENCE_ONLY (no registry or owner "
    "authentication, rule or gate applicability, proof satisfaction, test "
    "pass, implementation availability, Grant activation, authorization, "
    "permission, invocation, runtime routing, persistence, transition, "
    "execution, or effect attestation)"
)

FROZEN_SET_PINS = {
    "forge-core/internal/goimpactprescan": (
        18, 83167, "549818abbb33737c9198607e2d43b56efef50b476aac507446d5501f86b4de22"),
    "harness/local_go_package_impact_prescan_contract": (
        8, 35348, "effade443429146470a13b55341c73228fa8d718e88be47532260663fb534bd4"),
    "explicit": (
        3, 27574, "3d7a072ffcaa6a222ae42ef6ac1b6135029ad5158fc38c70ce35f5afb3a28100"),
}
FORBIDDEN_ENTRY_PATHS = {
    str(FIXTURE_PATH), str(SCHEMA_PATH),
    "docs/adr/ADR-0068-authority-neutral-capability-registry-v1.md",
}
