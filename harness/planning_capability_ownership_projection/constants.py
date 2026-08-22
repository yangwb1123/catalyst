"""Frozen ADR-0069 vocabulary, domains, paths, and limits."""

API_REQUEST = "forgeos.planning-capability-ownership-projection-request/v1"
API_PROJECTION = "forgeos.planning-capability-ownership-projection/v1"
API_GOLDEN = "forgeos.planning-capability-ownership-projection-golden/v1"
CANONICAL = "forgeos.canonical-json/v1"
REQUEST_DOMAIN = b"forgeos.planning-capability-ownership-projection-request.v1\0"
BINDING_DOMAIN = b"forgeos.planning-capability-ownership-binding.v1\0"
PROJECTION_DOMAIN = b"forgeos.planning-capability-ownership-projection.v1\0"

CATALOG_NAME = "capability-catalog.v1.yml"
MAPPING_NAME = "capability-skill-map.v1.yml"
CATALOG_PATH = "docs/design/ai-engineering-os/capability-catalog.v1.yml"
MAPPING_PATH = "docs/design/ai-engineering-os/capability-skill-map.v1.yml"
FIXTURE_PATH = "docs/contracts/fixtures/planning-capability-ownership-projection-v1.json"
CATALOG_SHA256 = "bc6efe535539c5f129af51486d8e81b9844b5ee6448fae2bce649fc159658d74"
MAPPING_SHA256 = "bfb2277fe66cd9f0c609b5be10ad77ad0969603edd19e5a6ccbe38b8e3409462"
CATALOG_BYTES = 33000
MAPPING_BYTES = 5924
FIXTURE_SHA256 = "3d0a877bef0939cff5752fc5d602e0d3a90e19639308801008f9d2d9ff139f36"

MAX_REQUEST_BYTES = 1_048_576
MAX_PROJECTION_BYTES = 4_194_304
MAX_BINDING_BYTES = 16_384
MAX_CATALOG_BYTES = 262_144
MAX_MAPPING_BYTES = 131_072
MAX_DEPTH = 16
MAX_FIELDS = 64
MAX_ITEMS = 512
MAX_TOKENS = 65_536
MAX_COLLECTIONS = 4_096
MAX_SCALAR_BYTES = 16_384
MAX_JSON_STRING_BYTES = 349_528
MAX_IDENTIFIER_BYTES = 160
MAX_ADAPTER_BYTES = 192
MIN_I64 = -(2 ** 63)
MAX_I64 = 2 ** 63 - 1

POSITIVE_RESULT = (
    "PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY (complete declared primary-owner "
    "coverage and logical adapter references for the supplied planning sources only; "
    "no file existence, Skill availability, Registry mutation, authentication, "
    "authorization, permission, invocation, routing, execution, persistence, "
    "transition, or effect attestation)"
)

AUTHORITY = {
    "adapter_file_existence": "not_evaluated",
    "adapter_skill_availability": "not_evaluated",
    "attestations": [],
    "authentication_attestation": False,
    "authorization_decision": "none",
    "capability_invocation": False,
    "capability_registry_mutation": False,
    "effect_attestation": False,
    "execution_attestation": False,
    "grant_or_pdp_activation": False,
    "implementation_selection": False,
    "ownership_authority_attestation": False,
    "permission_attestation": False,
    "persistence": "none",
    "runtime_routing": False,
    "source_authentication": False,
    "transition_attestation": False,
}
